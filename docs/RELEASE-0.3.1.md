# ETOL 0.3.1

Patch release with fixes found during the first clean-install tests on CachyOS.

## Fixes

- **The window now opens even when the system tray is unavailable.** On a clean
  CachyOS install the portable AppImage could exit before showing the interface:
  the tray library (`libappindicator`) was missing and the crate Tauri uses
  aborts with a panic instead of returning an error. The tray is now built
  inside `catch_unwind`, so the app logs a warning and keeps working without the
  icon. To get the icon on systems without it, install `libayatana-appindicator`
  (Ubuntu/Fedora) or `libappindicator-gtk3` (Arch/CachyOS).
- **Setup scripts no longer depend on `python3`.** The runner and SimHub version
  lookup parse the GitHub JSON with `sed`. A clean system without `python3` used
  to fail at the prefix step within a second.
- **Missing tools are reported clearly.** The prefix script checks for `curl`,
  `tar`, `unzip` and `file` before starting and prints the install command for
  Arch/CachyOS, Ubuntu and Fedora.
- **The full output of the setup and per-game scripts is saved** to
  `~/.config/etol/logs/setup-<key>.log` and `config-<game>.log`, and the last
  lines are written to the daily log when a script fails.
- **Panics are written to the daily log** (module `panic`), so a crash can be
  diagnosed without launching the AppImage from a terminal.
- **Cleanup on every exit path.** Alt+F4 and closing the last window now stop
  the bridges, capture the logs and disable the KWin overlay rule, not only the
  "Quit" entry in the tray.
- The doctor reports a missing `python3` (needed by the telemetry bridges) and a
  missing `curl`.

## Assets

| Asset | Size | SHA-256 |
|---|---|---|
| `ETOL-0.3.1-linux-x86_64-portable.AppImage` | 5.6 MB | `8d32a8308947c4b038762d60685179ccdb92bc20a5bb4f540c561d1b335b1c7f` |
| `ETOL-0.3.1-linux-x86_64-full.AppImage` | 111 MB | `21b9fe3ae7c6d999e890aa3e35a5f34633dcf8524471b5aac6e1b71102a9db8b` |

## Requirements

- Linux x86_64, glibc **2.39+** (portable) or **2.44+** (full).
- `python3` and `curl` on the system.
- Portable build: `libwebkit2gtk-4.1` + `libgtk-3`.
- Steam with the games; Bottles with SimHub (ETOL can install and prepare both).
- KDE Plasma for dashboards above the game.

## Install

```bash
chmod +x ETOL-0.3.1-linux-x86_64-portable.AppImage
./ETOL-0.3.1-linux-x86_64-portable.AppImage
```

## If upgrade fails

Keep `0.3.0` installed while testing `0.3.1`; `~/.config/etol` is compatible in
both directions.
