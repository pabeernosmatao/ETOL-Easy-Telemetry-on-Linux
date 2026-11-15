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
- The RaceRoom mirror now publishes on every change of the mapping instead of
  a fixed 30 Hz timer. RaceRoom mixes 200 Hz telemetry with a 400 Hz FFB
  channel; keeping the native cadence avoids FFB-HUB discarding the data.
- RaceRoom configures the **HF8** after its detection stub, so the stub cannot
  leave `COM33` pointing at the wrong tty and SimHub keeps the pad configured.
- The per-game app switches are ordered as FFB-HUB, CrewChief and SimHub so
  the columns stay aligned on games without FFB-HUB.
- FFB-HUB is proprietary and is downloaded at runtime; it is not bundled.

## Display settings

- The display-settings helper now has independent switches for **Wayland**
  (`PROTON_ENABLE_WAYLAND=1`, `SDL_VIDEODRIVER=wayland`), **HDR**
  (`--hdr-enabled`, `DXVK_HDR=1`) and **Adaptive Sync** (`--adaptive-sync`).
- New **refresh-rate** selector (60/120/144/165/240 Hz) for `gamescope -r`.
- More common options: AMD multithreaded OpenGL (`mesa_glthread=true`) and RADV
  Pipeline Library (`RADV_PERFTEST=gpl`); NVIDIA OpenGL VSync off
  (`__GL_SYNC_TO_VBLANK=0`) and PRIME GPU selection
  (`__GLX_VENDOR_LIBRARY_NAME=nvidia`).
- Common options apply to any GPU; gamescope-only switches are omitted in
  without-gamescope mode. HDR and Adaptive Sync stay enabled by default.

## CrewChief audio

- Before opening CrewChief, ETOL ensures **WASAPI** has a valid device.
  CrewChief could keep an empty endpoint ID after switching from WaveOut, so
  NAudio called `GetDevice("")` and message playback died while background
  sounds fell back to WindowsMediaPlayer.
- ETOL reads Wine's `ActiveMovie\devenum` keys from `user.reg` and writes the
  **PulseAudio Output** endpoint into both message and background devices.
  WaveOut configurations are left untouched; if no endpoint exists, ETOL
  reverts WASAPI to WaveOut instead of leaving CrewChief silent.

## CrewChief LMU telemetry

- The LMU bridge now publishes `$LMU.3.8.*` with the buffer sizes CrewChief
  reserves (`241680`, `75312`, `45272`, `10152`) and keeps the full LMU 3.8
  buffers under `$rFactor2SMMP_*` for SimHub. This removes CrewChief's
  `Destination array was not long enough` exception and restores its live
  telemetry in LMU.

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
| `ETOL-0.4.0-linux-x86_64-portable.AppImage` | 6.8 MB | `08df1f45e30e040c0ea3a73d83f53de5f9f832730eb97607012d1f2519045b0a` |
| `ETOL-0.4.0-linux-x86_64-full.AppImage` | 112 MB | `af6b72df53162f3750208c975dd06f62a3e23a3b0879efd1a4791a8685ec5740` |

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
