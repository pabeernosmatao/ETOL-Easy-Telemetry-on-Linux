# ETOL 0.4.0

Assetto Corsa Competizione support, plus all the fixes from 0.3.23.

## Assetto Corsa Competizione

- ETOL detects **Assetto Corsa Competizione** (Steam app `805550`) and its
  Proton prefix.
- New read-only bridge: it copies the three ACC shared-memory pages
  `Local\acpmf_physics`, `Local\acpmf_graphics` and `Local\acpmf_static`
  (4096 bytes each) from the game prefix to the Bottles prefix over UDP
  `127.0.0.1:8779`.
- The bridge creates two inert detection stubs in Bottles (`acc.exe` and
  `AC2-Win64-Shipping.exe`) so SimHub and CrewChief can see the game. The stubs
  are filtered out of ETOL's game detection and stopped together with the
  bridge.
- **SimHub**: `AssettoCorsaCompetizione` is visible, preferred (`OrderEx=1`) and
  uses the shared-memory reader (`CurrentReader=null`).
- **CrewChief**: active game `ACC`, with `enable_auto_detect=False` and
  `run_immediately=True` so the stub does not leave it waiting.
- Auto-close uses the real `AC2-Win64-Shipping.exe` process: closing ACC stops
  the bridge, the stubs, CrewChief and SimHub.
- Tested with Proton-CachyOS, the Bottles GE-Proton runner and SimHub/CrewChief.

## From 0.3.23

- **HF8 USB access** card in Devices: serial group (`uucp`/`dialout`),
  udev `uaccess` rule and `brltty` mask with one button.
- **Auto-close fixes**: Proton's `steam.exe` wrapper, the LMU bridge name,
  BeamNG's detection stub and Steam wrappers for native games.
- Orphan bridge cleanup when ETOL starts.

## Assets

| Asset | Size | SHA-256 |
|---|---|---|
| `ETOL-0.4.0-linux-x86_64-portable.AppImage` | 6.8 MB | `4e0a03e7217ae82f8d8c6b9ef72ec668e6e85cf8570f0ea1a264d404824c3806` |
| `ETOL-0.4.0-linux-x86_64-full.AppImage` | 112 MB | `60ba8d8515df8dabace36c7dcfabbd3221f66bc962b830b2a2dc26f69455df94` |

## Install

```bash
chmod +x ETOL-0.4.0-linux-x86_64-portable.AppImage
./ETOL-0.4.0-linux-x86_64-portable.AppImage
```

## Requirements

- Linux x86_64, glibc **2.39+** (portable) or **2.44+** (full).
- `python3` and `curl` on the system.
- Portable: `libwebkit2gtk-4.1` + `libgtk-3`.
- `pkexec` and a polkit agent to apply the USB fix (KDE includes one).
- Steam with the games; Bottles with SimHub (ETOL can install and prepare
  both).
- KDE Plasma for the in-game dashboards.
- A StatusNotifier host for the tray icon.
