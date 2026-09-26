# Windows packaging

## What ships

- `sleeper-zone-desktop.exe` — the release build (console subsystem, see below).
- `sleeper-zone-desktop-setup-<version>.exe` — Inno Setup installer:
  per-user install under `Program Files\Sleeper Zone`, Start Menu entry,
  optional desktop icon, uninstaller. Compiled by CI from
  [`installer.iss`](installer.iss) (Inno Setup 6).

## Local build (on Windows)

```powershell
cargo build --release
.\target\release\sleeper-zone-desktop.exe --check-league 289646328504385536
# installer (needs Inno Setup 6):
iscc /DMyAppVersion=0.1.0 packaging\windows\installer.iss
```

Cross-compiling from Linux is possible with a zig toolchain
(`cargo-zigbuild --target x86_64-pc-windows-gnu`) and is how the Windows
build was first verified — but the shippable artifacts come from the
`windows` CI job on `windows-latest`, which also runs the headless
smoke against the real APIs.

## Notes

- **Console window.** The exe keeps the console subsystem on purpose:
  the headless `--check` / `--check-league` modes report over stdout,
  and a `windows_subsystem = "windows"` binary launched from a terminal
  cannot print there. Side effect: launching the GUI opens a console
  window next to it. Revisit if the headless modes grow a log-file flag.
- **Config location.** `%APPDATA%\sleeper-zone\` (`config.json`,
  `players.json`), created on first run. Uninstall leaves it behind.
- **SmartScreen.** The exe and installer are unsigned, so Windows
  SmartScreen will warn on first run. Code signing (cert + CI secret)
  is the follow-up if this ever ships beyond sideloading.
- **No icon yet.** The exe carries version/FileDescription metadata
  (via `build.rs` + `winres`) but no custom icon; the installer pages
  use Inno's defaults.
