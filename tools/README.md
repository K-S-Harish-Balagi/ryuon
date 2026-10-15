# Development tools

All of these run the app inside the Flatpak SDK; none of them sends mouse or keyboard input.
The end-to-end tests drive the app over D-Bus with throwaway settings, cache and data folders,
and only ever read the music library (renaming runs on copies).

| Tool | What it does |
|---|---|
| `devbuild.sh build` | Incremental build into `_dev-build` (first run: `flatpak-builder --stop-at=ryuon _dev-build io.github.k_s_harish_balagi.Ryuon.json`) |
| `devbuild.sh test` | Unit tests (`cargo test`): folder grouping and sorting, tidy names, rename planning, journal and undo |
| `devbuild.sh run` | Runs the dev build |
| `smoke.sh` | Installed app starts; desktop file, metainfo and schema validate |
| `test_reorder.py` | Play next, move songs and folders, undo, remove, order kept across restarts |
| `test_rename.py` | Tidy Names › rename on disk, on copied songs: wrong count refused, rename, queue and resume follow, undo, read-only library |
| `test_sort.py` | Sort Folders (name, newest, most songs, recently played, manual) and the Folders view |
| `screenshots.py` | README and metainfo screenshots (playing, Folders, Settings, Song Info, phone) from the demo library |
| `test_perf.py` | Time to window and to a fully queued library, for the real library and 2000 synthetic songs |

The Python tests read `RYUON_TEST_LIBRARY` (default `~/Music`). They were written against the
developer's library (about 460 songs in 139 folders; `test_rename.py` copies a few named
folders from it), so other libraries may need the folder names adjusted.
