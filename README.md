<!--
SPDX-FileCopyrightText: 2026  Harish
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Ryuon

Ryuon is a local music player for Linux. Point it at your music folder once and it plays one
folder after another. It is a fork of [Amberol](https://gitlab.gnome.org/World/amberol) by
Emmanuele Bassi and keeps its licence, GPL-3.0-or-later.

## Status

Early development (0.1.0). This release is Amberol 2026.1 with a new identity: name, app ID,
icon and about dialog. The planned work is a library and folder-by-folder mode, queue
reordering, name tidying and view and sort options. See `docs/ROADMAP.md` in the project
notes for the plan.

## Credit

Ryuon is built on Amberol. All of Amberol's code remains under the original copyright of
Emmanuele Bassi and contributors; changes made in this fork are under the same licence.
Upstream's history is kept in `CHANGES.md`, and the original project page is
https://gitlab.gnome.org/World/amberol.

## Building

Ryuon builds as a Flatpak against the GNOME nightly SDK, because it needs GTK 4.19.5 or newer
and libadwaita 1.8 or newer.

```sh
flatpak-builder --user --install --force-clean build-dir io.github.k_s_harish_balagi.Ryuon.json
flatpak run io.github.k_s_harish_balagi.Ryuon
```

To let the Flatpak read a music folder outside your standard Music directory:

```sh
flatpak override --user --filesystem=/path/to/Music:ro io.github.k_s_harish_balagi.Ryuon
```

## Licence

GPL-3.0-or-later. See `LICENSES/`. The Ryuon logo and icons are original artwork made for this project.
