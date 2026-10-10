#!/bin/bash
# Smoke test of the installed Flatpak and its data files. Usage: tools/smoke.sh
# Checks: the app is installed and starts (--help), and the desktop file, metainfo and
# GSettings schema of the dev build validate.
R=$(cd "$(dirname "$0")/.." && pwd)
APP=io.github.k_s_harish_balagi.Ryuon
fail=0
check() { if "$@" >/dev/null 2>&1; then echo "PASS $label"; else echo "FAIL $label"; "$@" 2>&1 | tail -5; fail=1; fi; }
label="installed"; check flatpak info $APP
label="starts (--help)"; check flatpak run $APP --help
D=$R/_dev-build/files/share
label="desktop file valid"; check flatpak build $R/_dev-build desktop-file-validate /app/share/applications/$APP.desktop
label="metainfo valid"; check flatpak build $R/_dev-build appstreamcli validate --no-net --explain /app/share/metainfo/$APP.metainfo.xml
label="schema compiles"; check flatpak build $R/_dev-build glib-compile-schemas --strict --dry-run /app/share/glib-2.0/schemas
exit $fail
