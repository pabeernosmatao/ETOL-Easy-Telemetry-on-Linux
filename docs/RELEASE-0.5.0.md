# ETOL 0.5.0

DC Simracing pedals (DC2/DC3) support, with an optional official calibration
app and a hardware access card.

## DC Pedals

- **Applications → DC Pedals**: installs, updates and launches the official
  **DC Pedals Calibration** utility (DC Simracing) inside the ETOL bottle. On
  first activation ETOL explains that it downloads the official RAR from
  `dcsimracing.com`, where it installs it, that the `.txt` presets are kept and
  that ETOL does not redistribute the program.
- The installer verifies the SHA-256 of the executable and `mcHID.dll`, uses
  `bsdtar`, `7z`, `unrar` or `unar` to open the RAR, and only replaces those
  two files plus the official note. User presets stay in place.
- **Devices → DC Pedals**: detects the HID device (`VID 0444`, `PID 20DC`),
  shows its `/dev/hidraw*` node and the user's access, and explains the exact
  udev rule before applying it:
  `KERNEL=="hidraw*", SUBSYSTEM=="hidraw", ATTRS{idVendor}=="0444", ATTRS{idProduct}=="20dc", TAG+="uaccess"`.
- The rule is written to `/etc/udev/rules.d/99-etol-dcpedals.rules` with
  `pkexec`. It only grants user access: **no report is written to the pedals**.
  If access remains denied, reconnect the USB cable.
- The calibration utility is **manual only**; it is not started with games.
  The Diagnostics tab warns when the pedals are present but the udev rule or
  access is missing.

## Upgrading from 0.4.0

- Existing games, bridges, FFB-HUB, SimHub, CrewChief and the HF8 configuration
  are preserved.
- DC Pedals is optional and starts disabled. Install it from Applications and
  prepare the HID access from Devices.

## Requirements

- Linux x86_64, glibc **2.39+** (portable) or **2.44+** (full).
- `python3` and `curl`.
- `bsdtar` (libarchive), `7z` or `unrar` to install the DC Pedals RAR.
- Portable: `libwebkit2gtk-4.1` + `libgtk-3`.
- `pkexec` and a polkit agent to apply the udev rule.
- Steam with the games; Bottles with SimHub (ETOL can prepare both).
- KDE Plasma for dashboards over the game.
- A StatusNotifier host for the tray icon.

## Assets

| Asset | Size | SHA-256 |
|---|---|---|
| `ETOL-0.5.0-linux-x86_64-portable.AppImage` | 6.9 MB | `882c0539a03e3fad2681a2e32a1d5ceb2a8a1177b30cc0b79131fd54cf31dae9` |
| `ETOL-0.5.0-linux-x86_64-full.AppImage` | 113 MB | `501f4f3b930c4ceb0593dde98d1edc766ddccca77d844c9964d297ef80ebeed1` |

## Install

```bash
chmod +x ETOL-0.5.0-linux-x86_64-portable.AppImage
./ETOL-0.5.0-linux-x86_64-portable.AppImage
```
