# ETOL 0.3.22

Two fixes since 0.3.20: MangoHud outside gamescope and the Le Mans Ultimate
shared-memory plugin now that CrewChief stopped shipping it.

## MangoHud without gamescope

- In **Display settings**, MangoHud now works in both modes:
  `--mangoapp` with gamescope and `MANGOHUD=1` without it.
- The options that only exist inside gamescope (mouse grab, internal
  fullscreen, real-time priority, HDR ITM, FSR/NIS scaling) are marked
  **«solo gamescope»** and the note explains that they are omitted without it.
- AMD/NVIDIA variables (FSR 4, Anti-Lag, RADV, NVAPI, DLSS, shader cache) keep
  working in both modes, as before.

## Le Mans Ultimate plugin

- CrewChief **4.18 and newer no longer include**
  `LMU.3.8.SharedMemoryMapPlugin.dll`. ETOL now carries its own unmodified copy
  (85 KB, GPLv3, TheIronWolfModding) and installs it in
  `<game>/Plugins/` when CrewChief does not provide it.
- `ensure_lmu_plugin.py` uses CrewChief's copy when available and the bundled
  one otherwise; it enables the JSON entry only when the DLL is really there
  and removes stale entries.
- The warning in the LMU gear no longer asks for the old name only: it accepts
  the rFactor 2 plugin as well and checks the file that ETOL has to install.
- The new DLL is documented in `THIRD-PARTY.md` with its GPL v3 notice.

## Installer window

- The *Create the SimHub + CrewChief bottle* dialog now explains that CrewChief
  is downloaded from the official mirror and **may be an older version**: open
  it once when the installation finishes and accept the update before using it
  with the games.
- The same notice is printed by the setup script and added to the guides in
  Spanish, Catalan and English.

## Assets

| Asset | Size | SHA-256 |
|---|---|---|
| `ETOL-0.3.22-linux-x86_64-portable.AppImage` | 6.6 MB | `e5f340efb6fb76043c1fd3ccb253b17887b27cfeead3f2ee1eaa4d130b3bd6d1` |
| `ETOL-0.3.22-linux-x86_64-full.AppImage` | 112 MB | `b0af106edd59921e8bfb11417b9b4042bfce6c8e5a83bcef293fb2649e7b714b` |

## Requirements

- Linux x86_64, glibc **2.39+** (portable) or **2.44+** (full).
- `python3` and `curl` on the system.
- Portable build: `libwebkit2gtk-4.1` + `libgtk-3`.
- Steam with the games; Bottles with SimHub (ETOL can install and prepare both).
- KDE Plasma for dashboards above the game.
- A StatusNotifier host for the tray icon (Plasma, niri bar, GNOME extension).

## Install

```bash
chmod +x ETOL-0.3.22-linux-x86_64-portable.AppImage
./ETOL-0.3.22-linux-x86_64-portable.AppImage
```
