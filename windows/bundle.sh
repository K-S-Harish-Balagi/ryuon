#!/bin/bash
# Makes a self-contained Ryuon folder for Windows from a meson install, inside MSYS2 UCRT64:
# ryuon.exe with every DLL it needs, GStreamer plugins, gdk-pixbuf loaders, icons, settings
# schemas and translations. The folder runs on any Windows 10/11 PC without MSYS2.
# Usage: windows/bundle.sh <meson install prefix> <output folder>
set -euo pipefail
SRC=$(cd "$1" && pwd)
OUT=$2
P=${MINGW_PREFIX:-/ucrt64}
rm -rf "$OUT"
mkdir -p "$OUT"
cp -r "$SRC"/. "$OUT"/

# GStreamer: all installed plugins for now (trimmed once playback is confirmed), and the
# scanner that indexes them on first start
mkdir -p "$OUT/lib" "$OUT/libexec"
cp -r "$P/lib/gstreamer-1.0" "$OUT/lib/"
cp -r "$P/libexec/gstreamer-1.0" "$OUT/libexec/"
# Images (cover art) through gdk-pixbuf
cp -r "$P/lib/gdk-pixbuf-2.0" "$OUT/lib/"
# gdbus.exe lets GLib start a private session bus, which keeps Ryuon to one window
cp "$P/bin/gdbus.exe" "$OUT/bin/"

# Every DLL that the programs and plugins load, found recursively, from MSYS2 only
dlls() {
  find "$OUT" \( -name '*.exe' -o -name '*.dll' \) -print0 |
    xargs -0 ldd 2>/dev/null | awk -v p="$P/bin/" 'index($3, p) == 1 { print $3 }' | sort -u
}
while :; do
  added=0
  for dll in $(dlls); do
    if [ ! -e "$OUT/bin/$(basename "$dll")" ]; then
      cp "$dll" "$OUT/bin/"
      added=1
    fi
  done
  [ $added = 1 ] || break
done

# GTK and libadwaita use symbolic icons from the Adwaita theme; hicolor holds ours
mkdir -p "$OUT/share/icons"
cp -r "$P/share/icons/Adwaita" "$OUT/share/icons/"
cp -rn "$P/share/icons/hicolor" "$OUT/share/icons/" 2>/dev/null || true
gtk4-update-icon-cache -q -t -f "$OUT/share/icons/Adwaita" || true
gtk4-update-icon-cache -q -t -f "$OUT/share/icons/hicolor" || true

# Settings schemas: GTK's own (file chooser, colour chooser) next to Ryuon's
cp "$P"/share/glib-2.0/schemas/org.gtk.*.xml "$OUT/share/glib-2.0/schemas/"
glib-compile-schemas "$OUT/share/glib-2.0/schemas"

# Translations of GTK and libadwaita for dialogs and built-in labels
for mo in "$P"/share/locale/*/LC_MESSAGES/{gtk40,gtk40-properties,libadwaita}.mo; do
  [ -e "$mo" ] || continue
  rel=${mo#"$P/share/locale/"}
  mkdir -p "$OUT/share/locale/$(dirname "$rel")"
  cp "$mo" "$OUT/share/locale/$rel"
done

# Licences: GPL text for Ryuon; the libraries' licences come with their own notices
mkdir -p "$OUT/share/licenses/ryuon"
cp LICENSES/* "$OUT/share/licenses/ryuon/" 2>/dev/null || true

echo "Bundled into $OUT: $(du -sh "$OUT" | cut -f1), $(ls "$OUT/bin" | wc -l) files in bin"
