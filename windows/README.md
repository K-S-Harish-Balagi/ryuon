# Windows

Ryuon runs on Windows 10 and 11 (64-bit) as a beta: an installer and a portable zip, both on
the [releases page](https://github.com/K-S-Harish-Balagi/ryuon/releases). Plan and status:
[ROADMAP.md](ROADMAP.md).

| File | What it does |
|---|---|
| `../.github/workflows/windows.yml` | Builds on GitHub Actions with MSYS2 (UCRT64), runs the unit tests, makes the portable folder and the installer, checks them, and on a `v*` tag attaches them to the release |
| `bundle.sh` | Collects `ryuon.exe` with its DLLs, the GStreamer plugins Ryuon uses, icons, schemas and translations into a folder that runs without MSYS2 |
| `ryuon.iss` | Inno Setup installer: per-user (all users optional), Start menu, upgrade over a running copy, uninstaller |
| `ci-check.ps1` | Install, exe version and icon, first start, a test library played folder by folder, upgrade, uninstall; screenshots |
| `ryuon.ico` | The icon put into `ryuon.exe` by `../build.rs` |
| `test-media/` | 3-second generated tones in MP3, FLAC, Ogg, Opus and M4A |

Windows-only code lives in `src/`, behind `#[cfg(windows)]`.
