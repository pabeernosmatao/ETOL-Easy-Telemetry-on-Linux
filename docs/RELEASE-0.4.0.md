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

## FFB-HUB (LMUFFB)

- New **FFB-HUB** card in Applications: description, enable switch, manual
  **Launch** and **Update** buttons.
- The first activation offers the automatic installation. ETOL downloads the
  latest official release (`coasting-nc/LMUFFB`) and installs it in the ETOL
  bottle (`C:\FFBHUB`), not in the game prefix. Updates preserve `config.ini`
  and the user profiles.
- **Automatic mode** launches FFB-HUB with LMU or RaceRoom only when it is
  enabled in Applications **and** its per-game checkbox in Options is on.
- Works with the shared memory that ETOL already mirrors for LMU and RaceRoom.
  The LMU receiver now also publishes the native `HoldEvent`/`DataEvent`, the
  shared-memory lock and a valid app window, which FFB-HUB needs to consider
  the simulator connected from the ETOL bottle.
- FFB-HUB is proprietary and is downloaded at runtime; it is not bundled.

## From 0.3.23

- **HF8 USB access** card in Devices: serial group (`uucp`/`dialout`),
  udev `uaccess` rule and `brltty` mask with one button.
- **Auto-close fixes**: Proton's `steam.exe` wrapper, the LMU bridge name,
  BeamNG's detection stub and Steam wrappers for native games.
- **Auto-close waits 5 s** after the game exits (was 8 s) before stopping the
  bridge and closing the apps.
- Orphan bridge cleanup when ETOL starts.

## Assets

| Asset | Size | SHA-256 |
|---|---|---|
| `ETOL-0.4.0-linux-x86_64-portable.AppImage` | 6.8 MB | `d2a21f599d83d6df207d7a78086dcae406aed000f75fe0c68f95c1191b90af45` |
| `ETOL-0.4.0-linux-x86_64-full.AppImage` | 112 MB | `713a889f253fa97a18ae753ee80f74d859e3e9d15c3fbe53dc798003cb96d3ed` |

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
