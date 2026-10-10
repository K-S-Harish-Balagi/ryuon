# Ryuon for Windows: roadmap to a beta

Status (2026-10-11): **planned, not started.** Ryuon runs on Linux only today. This folder will
hold the Windows packaging (build script, installer, notes); code changes go in `src/` behind
`#[cfg(windows)]`, so Linux and Windows build from the same source.

**Goal of the beta:** a Windows 10/11 (64-bit) installer that plays a music folder folder by
folder, with the same features as Linux 0.2.0. Media keys and the Windows media overlay are
in the beta if time allows (W4).

## What already works on Windows (no change needed)
GTK 4, libadwaita and GStreamer all support Windows, so most of Ryuon carries over as is:
the window and layout, playlist, reordering, Folders view, sorting, Preferences, tidy names,
cover art and recolouring, file and folder dialogs (native Windows dialogs through GTK), drag and
drop from Explorer, settings (GSettings), translations, and the rename engine (names are already
made safe for NTFS, clashes are checked ignoring case, nothing is overwritten).

## What has to change

| # | Area | Today (Linux) | Change for Windows | Size |
|---|---|---|---|---|
| 1 | Media controls | MPRIS over D-Bus (`mpris-server`, `audio/mpris_controller.rs`) | Make MPRIS Linux-only; add a Windows controller using the System Media Transport Controls (media keys, volume overlay, lock screen), e.g. with the `souvlaki` crate, through the existing `Controller` trait | M |
| 2 | Install paths | `config.rs` hard-codes `/app/share/locale` and `/app/share/ryuon` | Find translations, resources and schemas relative to `ryuon.exe` on Windows | S |
| 3 | Flatpak portal path | `xattr::document-portal.host-path` lookups in `window.rs` | Skip on Windows (paths are already real paths) | S |
| 4 | Background playback | Flatpak background portal (`ashpd`), already Linux-only | Keep playing while the window is closed by holding the application; no tray icon in GTK 4, so document "Quit" from the menu | S |
| 5 | Single instance | GApplication uses the D-Bus session bus | Check what GLib does on Windows without a session bus; if a second window appears, run non-unique or add a simple lock | S-M (risk) |
| 6 | Audio decoding | GStreamer plugins from the GNOME runtime | Ship the needed plugins (playback, mpg123 for MP3, FLAC, Ogg/Opus, AAC) and their DLLs | S |
| 7 | Library defaults | `~/Music`; Flatpak permissions | `%USERPROFILE%\Music` default; no permissions step; drive letters and `\\server\share` paths in the folder label and rename | S |
| 8 | Build | Flatpak (GNOME 51 SDK) | MSYS2 (UCRT64) toolchain: `gtk4`, `libadwaita`, `gstreamer` packages, Rust `x86_64-pc-windows-gnu`, meson; automated on GitHub Actions (`windows-latest`) | M |
| 9 | Packaging | `.flatpak` bundle | Collect `ryuon.exe` + DLLs + GStreamer plugins + gdk-pixbuf loaders + icons + compiled schemas into a folder; Inno Setup installer and a portable `.zip` (about 150-250 MB) | M |
| 10 | Trust | Flathub runtime, checksum | Installer is unsigned at first: Windows SmartScreen will warn ("More info" > "Run anyway"); document it, consider code signing later | S (signing: cost) |
| 11 | Testing | Linux end-to-end tests over D-Bus | Manual test list on a real Windows PC (below); the D-Bus tests do not run there | M |

## Phases and estimate

| Phase | Work | Estimate |
|---|---|---|
| W0 | CI build on GitHub Actions with MSYS2; get it compiling (items 2, 3, 8; MPRIS switched off on Windows) | 3-4 h |
| W1 | Runs on Windows: relocatable paths, library defaults, background play, single instance (items 4, 5, 7) | 3-4 h |
| W2 | Plays music: GStreamer plugins bundled, MP3/FLAC/Ogg/AAC checked (item 6) | 2-3 h |
| W3 | Installer and portable zip from CI (items 9, 10) | 3-5 h |
| W4 | Windows media controls (item 1) | 4-6 h |
| W5 | Test on a real Windows PC, fix what turns up, publish `v0.3.0-beta` for Windows (item 11) | 3-6 h, in rounds |

**Total: about 18-28 hours of work** (13-22 hours without W4). Realistically **3-5 days** end to end,
because the Windows build only runs in CI (each round trip takes 20-40 minutes) and testing needs
someone at a Windows PC.

## What is needed from Harish
1. **A Windows PC for testing**, with a music folder on it (the homelab Lenovo could do, over
   Remote Desktop), or someone who can install the test builds and report back.
2. Whether the beta should wait for media keys (W4) or ship without them first.
3. Later: whether to pay for code signing (removes the SmartScreen warning; roughly USD 100-300
   per year, or free options for open source such as SignPath).

## Test list for W5
- Install, first launch, uninstall; portable zip runs from a USB drive
- Choose a library on `C:\Users\…\Music`, on another drive (`D:\`), and on a network share
- Plays MP3 (including damaged tags), FLAC, Ogg, M4A; seek, next and previous, repeat, shuffle
- Folder by folder, resume after closing and reopening, Folders view, all sorts
- Reorder by menu, drag and drop, Alt+arrows; undo
- Tidy names on and off; Rename Files on Disk and Undo Rename on an NTFS drive
- Media keys and the Windows media overlay (W4)
- Light and dark mode, 100 %, 150 % and 200 % scaling, a narrow window
- Closing the window with Background Playback on, then quitting from the menu

## Risks
- **GLib application uniqueness and GStreamer device selection on Windows** are the least known
  parts; W0-W1 surface them early.
- **Installer size:** GTK and GStreamer make it large; trimming unused plugins helps.
- **No Windows machine here:** every Windows fix is verified in CI and by a tester, not locally.
