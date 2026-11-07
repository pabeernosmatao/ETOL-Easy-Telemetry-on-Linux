# ETOL 0.3.18

Big update after 0.3.4: **RaceRoom Racing Experience** joins the supported
simulators, the bridges and the Steam detection are more robust, and the whole
stack was audited and tested against the eight games already supported.

## New simulator: RaceRoom Racing Experience

- Detects the Steam install (AppID `211500`, `Game/x64/RRRE64.exe`) and its
  Proton prefix, with no manual paths.
- Bridge: a **raw mirror** of the `$R3E` shared memory (45,056 bytes) over UDP
  port 8777. One sender runs inside the RaceRoom prefix and one receiver
  recreates the mapping inside the Bottles bottle. It copies bytes, it does not
  interpret the structure, so a game version bump cannot break it.
- **SimHub** uses its `RRRE` entry and native reader; **CrewChief** is set to
  `RACE_ROOM`. An inert `RRRE64.exe` stub makes both apps see the game inside
  the bottle.
- Verified in a live session: SimHub logs `Detected R3E memory version 3.6` and
  `RaceRoom Racing Experience game process detected`, and CrewChief reads
  drivers, classes and tyres.

## Group icon for SimHub + CrewChief

- ETOL now installs the half CrewChief / half SimHub window icon and its
  `.desktop` entry (`StartupWMClass=steam_proton`) on startup, so the panel
  shows one merged icon instead of only the CrewChief logo.
- The PNGs are embedded in the binary: no Python, PIL or pefile are needed on
  the target machine, and the files are only rewritten when they change.

## Detection and launching

- Steam **metadata**: install paths are resolved from `libraryfolders.vdf` and
  the app manifest (`appmanifest_*.acf`), including libraries on other disks.
- **Proton detection**: reads `config_info` and the `CompatToolMapping` in
  `config.vdf`, and falls back to the configured tool when the game is opened
  with a different one.
- Game detection by **`/proc/<pid>/comm`**, so games whose command line does
  not contain the executable name are detected too.
- **Assetto Corsa** gets a proper launcher (the game needs its own startup
  path); the bridge and plugin setup are unchanged.
- New `limit_available_games` handling: a CrewChief profile limited to a single
  game no longer hides the rest of the list.

## Bridges and app setup

- **Le Mans Ultimate**: the SMMP plugin is copied from CrewChief
  (`LMU.3.8.SharedMemoryMapPlugin.dll`) and loaded when the game starts; the
  bridge uses a raw SMMP sender that survives SDK updates. SimHub reports the
  game as configured.
- CrewChief's executable path is resolved correctly on current installs.
- `configurar_juego.sh` knows every supported game, including RaceRoom, so the
  gear button only applies that game's configuration.
- The payload patcher was extended for all of the above and still verifies the
  patched payload before packing it.

## Reliability

- **Single instance**: a second ETOL focuses the existing window instead of
  starting another watchdog.
- **CSP** and a security review of the Tauri commands and the embedded UI.
- Performance pass on the watchdog and the status polling.
- 11 unit tests, `clippy` clean and `cargo check --locked` green.
- Guides and README updated for the new game and the automatic group icon.

## Assets

| Asset | Size | SHA-256 |
|---|---|---|
| `ETOL-0.3.18-linux-x86_64-portable.AppImage` | 6.6 MB | `53824399f2c7f8180f3af4af73a60ac439ea100d1fb7a9d0ee6af5c4772d3187` |
| `ETOL-0.3.18-linux-x86_64-full.AppImage` | 112 MB | `7e913807d4ff0a2bdacacd308f4ebe396e2c46a2365c5c1df91249cab3fc58b5` |

## Requirements

- Linux x86_64, glibc **2.39+** (portable) or **2.44+** (full).
- `python3` and `curl` on the system.
- Portable build: `libwebkit2gtk-4.1` + `libgtk-3`.
- Steam with the games; Bottles with SimHub (ETOL can install and prepare both).
- KDE Plasma for dashboards above the game.
- A StatusNotifier host for the tray icon (Plasma, niri bar, GNOME extension).

## Install

```bash
chmod +x ETOL-0.3.18-linux-x86_64-portable.AppImage
./ETOL-0.3.18-linux-x86_64-portable.AppImage
```
