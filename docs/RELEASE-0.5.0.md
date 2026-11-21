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
- The rule is written to `/etc/udev/rules.d/70-etol-dcpedals.rules` with
  `pkexec` (the `70-` prefix runs before systemd's `uaccess` builtin). It only
  grants user access: **no report is written to the pedals**. If access remains
  denied, reconnect the USB cable.
- ETOL also adjusts the Wine bottle used by the utilities:
  `Enable SDL=0` and `EnableHidraw=0444:20DC` in
  `HKLM\System\CurrentControlSet\Services\WineBus`. Wine 11 exposed the pedals
  through its SDL backend, which replaces the real HID descriptor with a
  generic empty joystick; with these values Wine uses `hidraw` and delivers the
  pedal's 57-byte reports. It only affects that bottle (games use their own
  prefix). The change is applied at install time and before launching the
  utility; if SimHub/CrewChief are open, they must be closed once so Wine can
  reload the setting.
- The calibration utility is **manual only**; it is not started with games.
  The Diagnostics tab warns when the pedals are present but the udev rule or
  access is missing.

## Fanatec CSL DD

- **Applications → Fanatec CSL DD** is the native configuration card. It reads
  the tuning menu exposed by the out-of-tree `hid-fanatecff` driver at
  `/sys/class/ftec_tuning/<device>` (`SEN`, `FF`, `FEI`, `FOR`, `SPR`, `DPR`,
  `NDP`, `NFR`, `NIN`, `INT`, `FFS`, `FUL`, `SHO`, `BLI`, `brF`, `ACP`, `DRI`
  and `advanced_mode`) and shows sliders/switches for the active `SET`.
- **Devices → Fanatec CSL DD** shows the hardware state (`/dev/hidraw*`, driver,
  active SET, range, firmware, access) and has an enable switch. Enabling it
  explains what will be prepared and asks for confirmation: ETOL installs
  `/etc/udev/rules.d/70-etol-fanatec.rules` with `pkexec`, which gives the user
  write access to the `ftec_tuning` attributes only.
- Enabling requires the out-of-tree `hid-fanatecff` driver (module
  `hid_fanatec`, packaged as `fanatec-dkms`); the popup warns when it is not
  detected. Without it the cards report that the tuning menu is not available.
- Writes go to the active SET only. Firmware updates are out of scope: ETOL
  does not install drivers or flash the wheel.

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
| `ETOL-0.5.0-linux-x86_64-portable.AppImage` | 6.9 MB | `dc9621ac51012f9112b5c3bb7f7e013ee9f0063fd2acc2a3cd7fbf89713f0763` |
| `ETOL-0.5.0-linux-x86_64-full.AppImage` | 113 MB | `f90b11ea99e7b7b0e6b5c7093410406d3692444fe1f7b4bfe549354c6a170c50` |

## Install

```bash
chmod +x ETOL-0.5.0-linux-x86_64-portable.AppImage
./ETOL-0.5.0-linux-x86_64-portable.AppImage
```
