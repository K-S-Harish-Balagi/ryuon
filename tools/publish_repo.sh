#!/bin/bash
# Builds a signed release into the Flatpak repository and publishes it on GitHub Pages
# (https://k-s-harish-balagi.github.io/ryuon/), where installed copies pick it up as an update.
# Usage: tools/publish_repo.sh [--no-push]
#
# Needs: the signing key in $RYUON_GNUPG (default ~/.local/share/ryuon-signing/gnupg, never
# in git), the local repository in $RYUON_REPO and a checkout of the gh-pages branch in
# $RYUON_PAGES (git worktree add ../../pages gh-pages).
set -euo pipefail
R=$(cd "$(dirname "$0")/.." && pwd)
GNUPG=${RYUON_GNUPG:-$HOME/.local/share/ryuon-signing/gnupg}
KEY=${RYUON_KEY:-53D76A5662638D29AD85E2317146AA5C9D74575C}
REPO=${RYUON_REPO:-$R/../../flatpak-repo}
PAGES=${RYUON_PAGES:-$R/../../pages}
VERSION=$(sed -n "s/^  version: '\(.*\)',/\1/p" "$R/meson.build")

[ -d "$GNUPG" ] || { echo "No signing key in $GNUPG"; exit 1; }
[ -d "$PAGES/.git" ] || [ -f "$PAGES/.git" ] || { echo "No gh-pages checkout in $PAGES"; exit 1; }
[ -z "$(git -C "$R" status --porcelain)" ] || { echo "Commit your changes first"; exit 1; }

cd "$R"
nice -n 15 flatpak-builder --force-clean --jobs=2 --default-branch=stable --repo="$REPO" \
  --gpg-sign="$KEY" --gpg-homedir="$GNUPG" _flatpak-build io.github.k_s_harish_balagi.Ryuon.json
flatpak build-update-repo --title=Ryuon --comment="Ryuon music player" \
  --homepage=https://github.com/K-S-Harish-Balagi/ryuon --default-branch=stable \
  --generate-static-deltas --prune --prune-depth=3 --gpg-sign="$KEY" --gpg-homedir="$GNUPG" "$REPO"

# Pages serves the repository as plain files; --delete drops objects pruned above
rsync -a --delete --exclude=tmp/ --exclude=.lock "$REPO/" "$PAGES/repo/"
cp data/screenshots/playing.png "$PAGES/screenshot.png"
cp data/icons/hicolor/128x128/apps/io.github.k_s_harish_balagi.Ryuon.png "$PAGES/icon-128.png"
cp data/icons/hicolor/scalable/apps/io.github.k_s_harish_balagi.Ryuon.svg "$PAGES/icon.svg"
git -C "$PAGES" add -A
git -C "$PAGES" commit -q -m "Ryuon $VERSION ($(git -C "$R" rev-parse --short HEAD))" || echo "Nothing new to publish"
[ "${1:-}" = "--no-push" ] || git -C "$PAGES" push -q "${RYUON_REMOTE:-github}" gh-pages  # origin is upstream Amberol on GNOME GitLab
echo "Published Ryuon $VERSION"
