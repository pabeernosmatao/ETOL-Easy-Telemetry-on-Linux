# ETOL 0.3.0

First public release. ETOL detects your simulators on Steam, prepares SimHub
(and CrewChief) inside a Bottles bottle and launches the whole telemetry stack
with one click.

## Highlights

- **Two new simulators**: Euro Truck Simulator 2 and BeamNG.drive, both on their
  **native Linux builds**, SimHub only.
  - ETS2: installs the SCS plugin in `bin/linux_x64/plugins` and mirrors
    `/dev/shm/SCSTelemetry` into the Bottles prefix.
  - BeamNG.drive: configures OutGauge on `127.0.0.1:63392` and sets SimHub to
    the *Outgauge + motionsim* mode automatically.
- **Portable AppImage** (~6 MB) that uses the system WebKit, so it runs on
  Ubuntu 26.04+, Fedora, Arch and CachyOS without a glibc 2.44 requirement.
- **Red gear** on a game card until that game has been configured, or when ETOL
  finds something wrong (missing plugin, wrong SimHub mode).
- **Licensing**: MIT for ETOL, with `THIRD-PARTY.md` covering the bundled SCS
  plugin (MIT) and rF2 SMMP plugin (GPLv3).

## Improvements

- Single-instance guard: a second launch no longer starts a second watchdog.
- The watchdog reads `config.toml` only when it changes and scans `/proc` once
  per tick instead of once per game.
- Child processes no longer inherit the AppImage environment
  (`LD_LIBRARY_PATH`, `GI_TYPELIB_PATH`, …), which could break `python3` and
  `xdg-open`.
- Content Security Policy enabled; Tauri permissions stay at `core:default`.
- Frontend polling pauses while the window is hidden.
- `cargo clippy` clean on both crates; 11/11 unit tests passing.

## Assets

| Asset | Size | SHA-256 |
|---|---|---|
| `ETOL-0.3.0-linux-x86_64-portable.AppImage` | 5.6 MB | `00de4ce2fcb8019e8ed261f36bb43d37cf02364ffe12a340be653c1ad82634cd` |
| `ETOL-0.3.0-linux-x86_64-full.AppImage` | 111 MB | `d4cfaf10d644f940e32c5737cd063d34f21494eec4c9bd48bef97bc1b89a2d01` |

Both assets are x86_64 (`amd64` is the Debian name for the same architecture).
Use the **portable** build on Ubuntu 26.04 (glibc 2.43) or when you want a 6 MB
download; use the **full** build to avoid depending on system libraries.

## Requirements

- Linux x86_64, glibc **2.39+**.
- Portable build: `libwebkit2gtk-4.1` + `libgtk-3`.
- Full build: glibc **2.44+** (current CachyOS/Arch/Fedora).
- FUSE2 for AppImage (or `--appimage-extract-and-run`).
- Steam with the games; Bottles with SimHub (ETOL can install and prepare both).
- KDE Plasma for dashboards above the game.

## Install

```bash
chmod +x ETOL-0.3.0-linux-x86_64-portable.AppImage
./ETOL-0.3.0-linux-x86_64-portable.AppImage
```

## Known limitations

- SimHub dashboards over the game use a KWin rule: KDE Plasma only.
- The full AppImage bundles WebKit built against glibc 2.44; Ubuntu 26.04 ships
  2.43, so use the portable build there.
- Screenshots in the README are pending; the interface is described in
  `README.md`.

## Supported simulators

Le Mans Ultimate · Automobilista 2 · Assetto Corsa EVO · Assetto Corsa ·
rFactor 2 · DiRT Rally 2.0 · Euro Truck Simulator 2 · BeamNG.drive
