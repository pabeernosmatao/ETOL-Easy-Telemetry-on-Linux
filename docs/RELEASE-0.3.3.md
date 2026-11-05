# ETOL 0.3.3

First release with native tray support, a built-in user guide and the fixes
from the clean-install tests. Everything from 0.3.2 is included.

## Tray without libappindicator

- The tray is now a **StatusNotifierItem** over D-Bus (`ksni`), so ETOL no
  longer needs `libappindicator` or `libayatana-appindicator`.
- Works on KDE Plasma, on niri/Wayland bars with a tray module, and on GNOME
  with the *AppIndicator and KStatusNotifierItem Support* extension.
- If the desktop has no StatusNotifier host, ETOL logs a warning and keeps
  working; closing the window then quits the app instead of hiding it.
- If the bar starts after ETOL (common on niri), it retries for a minute.
- `libappindicator` is no longer linked or bundled; the full AppImage stops
  shipping that GPL-3.0 library.

## Fixes from the clean-install tests

- **GitHub 403**: the runner and SimHub lookups no longer use `api.github.com`.
  The tag comes from the `releases/latest` redirect and the SimHub ZIP from the
  assets page, which have no API rate limit.
- **winetricks conflict**: only `vcrun2022` is installed (it includes the
  2015-2022 runtime) with `--force`, so re-running the prefix on a bottle that
  already had `vcrun2019` works.
- The setup scripts no longer need `python3`; missing `curl`, `tar`, `unzip`
  and `file` are reported with the install command for each distro.
- The full output of setup and per-game scripts is saved to
  `~/.config/etol/logs/setup-*.log` and `config-*.log`.
- Panics are written to the daily log, so a crash can be diagnosed without a
  terminal.
- Alt+F4 and closing the window now run the cleanup (stop bridges, capture
  logs, disable the KWin rule), not only the tray's *Quit*.

## Interface

- **User guide**: a *Guía de uso* button at the bottom right of the Games tab
  opens the new guide (Spanish) in the browser. The guide covers first run,
  the window, playing, Bottles/SimHub/CrewChief from scratch, the simulator
  table, desktop support, the HF8, the tray, logs and common problems.
- **Setup progress**: the installation window now has a real progress bar and
  the current stage (checking Bottles, downloading GE-Proton, creating the
  bottle, winetricks, CrewChief, SimHub, installers).
- **Half-width buttons** for simulators that only use SimHub (ETS2 and
  BeamNG.drive): *Launch* and *SimHub* take 50 % each.
- The README now has a **desktop support** table separating what works on KDE
  Plasma, GNOME and niri/Wayland.

## Assets

| Asset | Size | SHA-256 |
|---|---|---|
| `ETOL-0.3.3-linux-x86_64-portable.AppImage` | 6.1 MB | `8edd8d30cd482fe76abb34a8c119b7f09c8319023c6d9a7c2ed510507bc84e29` |
| `ETOL-0.3.3-linux-x86_64-full.AppImage` | 112 MB | `d53735638addfa37b140b6a3a1a3611f065515d18f315058fdfb699166a5c2f4` |

## Requirements

- Linux x86_64, glibc **2.39+** (portable) or **2.44+** (full).
- `python3` and `curl` on the system.
- Portable build: `libwebkit2gtk-4.1` + `libgtk-3`.
- Steam with the games; Bottles with SimHub (ETOL can install and prepare both).
- KDE Plasma for dashboards above the game.
- A StatusNotifier host for the tray icon (Plasma, niri bar, GNOME extension).

## Install

```bash
chmod +x ETOL-0.3.3-linux-x86_64-portable.AppImage
./ETOL-0.3.3-linux-x86_64-portable.AppImage
```
