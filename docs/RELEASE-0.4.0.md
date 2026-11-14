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
  `run_immediately=True` so the stub does not leave it waiting. The setting is
  written to both `CrewChiefV4.exe.config` and the effective `user.config`.
- **Rivals, relative and leaderboard**: ETOL writes ACC's `broadcasting.json`
  (`127.0.0.1:9000`, connection password `asd`). SimHub and CrewChief use it
  for opponents and race-control messages. Restart ACC once after the first
  configuration so the game reads the file.
- **CrewChief audio**: ETOL sets Wine's audio driver to `pulse`
  (PipeWire-Pulse) in the Bottles prefix. Without it the runner exposes no
  output devices, CrewChief plays its messages internally and nothing is heard.
- Auto-close uses the real `AC2-Win64-Shipping.exe` process: closing ACC stops
  the bridge, the stubs, CrewChief and SimHub.
- Tested with Proton-CachyOS, the Bottles GE-Proton runner and SimHub/CrewChief.

## From 0.3.23

- **HF8 USB access** card in Devices: serial group (`uucp`/`dialout`),
  udev `uaccess` rule and `brltty` mask with one button.
- **Auto-close fixes**: Proton's `steam.exe` wrapper, the LMU bridge name,
  BeamNG's detection stub and Steam wrappers for native games.
- **Auto-close waits 5 s** after the game exits (was 8 s) before stopping the
  bridge and closing the apps.
- **LMU plugins**: stop installing `LMU.3.8.SharedMemoryMapPlugin.dll`, which
  LMU 1.4.2 rejects and removes from `CustomPluginVariables.JSON`; keep
  `rFactor2SharedMemoryMapPlugin64.dll` and set CrewChief to the legacy
  `$rFactor2SMMP_*` names.
- Orphan bridge cleanup when ETOL starts.

## Assets

| Asset | Size | SHA-256 |
|---|---|---|
| `ETOL-0.4.0-linux-x86_64-portable.AppImage` | 6.8 MB | `093c6c04fefd73a088352143c48400515469c966eedf9c7ec3dbf9db96e52972` |
| `ETOL-0.4.0-linux-x86_64-full.AppImage` | 112 MB | `8e36c235ff61cb4dad0a36f53ec26445477265e24e9077fd60cf6e4a582dbc1e` |

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
