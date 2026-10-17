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
| `test_portal.py` | Write permission for renaming, on an installed Devel build run with `flatpak run` (the dev sandbox has no document portal): nothing renamed before access is allowed, Allow dialog, wrong folder refused, a real portal grant renames, survives a restart and undoes, a library the sandbox cannot see works through its portal path, a folder the user's account cannot change is reported. Install the Devel build first (below) |
| `test_sort.py` | Sort Folders (name, newest, most songs, recently played, manual) and the Folders view |
| `screenshots.py` | README and metainfo screenshots (playing, Folders, Settings, Song Info, phone) from the demo library |
| `test_perf.py` | Time to window and to a fully queued library, for the real library and 2000 synthetic songs |

The Python tests read `RYUON_TEST_LIBRARY` (default `~/Music`). They were written against the
developer's library (about 460 songs in 139 folders; `test_rename.py` copies a few named
folders from it), so other libraries may need the folder names adjusted.

**Devel build for `test_portal.py`.** It installs next to the real app with its own ID and data:

```sh
python3 -c "import json; m = json.load(open('io.github.k_s_harish_balagi.Ryuon.json'))
m['app-id'] += '.Devel'; m['modules'][0]['config-opts'] = ['-Dprofile=development']
m['modules'][0]['sources'][0]['skip'] += ['.test-devel.json']
json.dump(m, open('.test-devel.json', 'w'), indent=2)"
flatpak-builder --user --install --force-clean --jobs=2 /tmp/ryuon-devel-build .test-devel.json
python3 tools/test_portal.py
flatpak uninstall --user -y io.github.k_s_harish_balagi.Ryuon.Devel && rm .test-devel.json
```
