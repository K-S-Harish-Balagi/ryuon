#!/bin/bash
# Fast incremental build of Ryuon inside the Flatpak SDK. Usage: tools/devbuild.sh [build|run|test|shell]
# First run compiles all dependencies (slow); later runs only recompile what changed.
set -e
R=$(cd "$(dirname "$0")/.." && pwd)
FB="flatpak build --share=network --filesystem=$R --env=PATH=/app/rust-toolchain/bin:/app/bin:/usr/bin --env=CARGO_PROFILE_RELEASE_OPT_LEVEL=1 --env=CARGO_PROFILE_RELEASE_LTO=off --env=CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16 --env=CARGO_PROFILE_RELEASE_INCREMENTAL=true --env=CARGO_BUILD_JOBS=2 $R/_dev-build"
case "${1:-build}" in
  build)
    [ -d $R/.devbuild ] || $FB meson setup $R/.devbuild $R --prefix=/app -Dprofile=default
    # The gresource target does not depend on the Blueprint output, so a .blp edit would
    # land one build late (stale template, "Unable to retrieve child object"): always rebuild it
    rm -f $R/.devbuild/src/ryuon.gresource
    nice -n 15 $FB ninja -C $R/.devbuild install ;;
  run)
    flatpak build --socket=session-bus --socket=wayland --socket=fallback-x11 --socket=pulseaudio --device=dri --share=ipc --filesystem=${RYUON_TEST_LIBRARY:-$HOME/Music}:ro \
      --env=RUST_LOG=ryuon=debug $R/_dev-build ryuon "${@:2}" ;;
  test)
    # Unit tests (library, tidy, ...), reusing the build's dependencies; extra args go to the test filter
    nice -n 15 $FB env CARGO_HOME=$R/.devbuild/cargo-home cargo test --release --manifest-path $R/Cargo.toml \
      --target-dir $R/.devbuild/src -- "${@:2}" ;;
  shell) $FB bash ;;
esac
