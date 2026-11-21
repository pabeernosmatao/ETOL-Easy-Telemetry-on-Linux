# ETOL 0.5.0

DC Simracing pedals (DC2/DC3), native Fanatec CSL DD configuration, LAN-only
dashboards and a polished hardware/UI experience.

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

- **Applications → Fanatec CSL DD** groups the tuning menu exposed by
  `hid-fanatecff` (`SEN`, `FF`, `FEI`, `FOR`, `SPR`, `DPR`, `NDP`, `NFR`, `NIN`,
  `INT`, `FFS`, `FUL`, `SHO`, `BLI`, `brF`, `ACP`, `DRI`, `advanced_mode`) in a
  collapsible section per **installed game**. Each game has its own sliders;
  changes are saved under `[fanatec.profiles.<game>]` in the configuration.
- The game profile is applied to the active `SET` when ETOL launches the game
  (and when the daemon starts the bridge), before the game script. **Load into
  wheel** applies it immediately; **Copy from wheel** copies the current values
  into that game's profile. A game without a profile shows the current wheel
  values and is marked "No profile"; the first edit creates a complete profile
  from the current values plus the change.
- **Devices → Fanatec CSL DD** shows the hardware state (`/dev/hidraw*`, driver,
  active SET, range, firmware, access) and has an enable switch. Enabling it
  explains what will be prepared and asks for confirmation: ETOL installs
  `/etc/udev/rules.d/70-etol-fanatec.rules` with `pkexec`, which gives the user
  write access to the `ftec_tuning` attributes only.
- Enabling requires the out-of-tree `hid-fanatecff` driver (module
  `hid_fanatec`, packaged as `fanatec-dkms`); the popup warns when it is not
  detected. Without it the cards report that the tuning menu is not available.
- The card shows the setting key next to each slider (`SEN`, `FF`, `FEI`...).
- Writes go to the active SET only. Firmware updates are out of scope: ETOL
  does not install drivers or flash the wheel.

## Dashboards and port 8888

- The port check and the Diagnostics firewall suggestion now restrict the rule
  to the local IPv4 network detected with `ip` (private ranges only), for
  example `ufw allow from 192.168.1.0/24 to any port 8888 proto tcp`. The port
  is no longer opened to any origin.
- **Diagnostics → Open port 8888** detects the active `ufw`/`firewalld`, shows
  the exact LAN-scoped command and applies it with `pkexec`. If no firewall is
  active it only reports that the port is already open. It never enables
  Internet access.

## Hardware and UI

- Applications cards are collapsible and remember their state; FFB-HUB, DC
  Pedals and Fanatec are independent cards. Devices cards are collapsible too,
  with the same 14 px separation everywhere.
- DC Pedals and Fanatec have enable switches. Enabling Fanatec explains the
  udev write rule and the `hid-fanatecff`/`fanatec-dkms` requirement before
  applying it with `pkexec`.
- HF8 shows a **Ready** / **Check** / **Not detected** / **Disabled** tag, next
  to its switch.
- Hardware access and FFB-HUB/DC Pedals/Fanatec popups are translated into
  Spanish, Catalan and English. App switches and profiles survive a re-detection
  of the bottle or the games.

## Upgrading from 0.4.0

- Existing games, bridges, FFB-HUB, SimHub, CrewChief, HF8 configuration and
  Fanatec profiles are preserved. The Fanatec switch and profiles are not lost
  when ETOL re-detects the bottle or the installed games.
- DC Pedals is optional and starts disabled. Install it from Applications and
  prepare the HID access from Devices.
- Fanatec is optional and starts disabled. Enable it in Devices, authorise the
  write rule and create a profile per game in Applications.

## Requirements

- Linux x86_64, glibc **2.39+** (portable) or **2.44+** (full).
- `python3` and `curl`.
- `bsdtar` (libarchive), `7z` or `unrar` to install the DC Pedals RAR.
- Portable: `libwebkit2gtk-4.1` + `libgtk-3`.
- `pkexec` and a polkit agent to apply the udev rules and the LAN-only firewall
  button.
- `hid-fanatecff` (packaged as **fanatec-dkms**) and the module loaded for the
  Fanatec card. `ufw` or `firewalld` is only needed for the port 8888 button.
- Steam with the games; Bottles with SimHub (ETOL can prepare both).
- KDE Plasma for dashboards over the game.
- A StatusNotifier host for the tray icon.

## Assets

| Asset | Size | SHA-256 |
|---|---|---|
| `ETOL-0.5.0-linux-x86_64-portable.AppImage` | 6.9 MB | `ccede473576439c08424d7050292efb42f6bbc0934cdefc8755914116c60449b` |
| `ETOL-0.5.0-linux-x86_64-full.AppImage` | 113 MB | `9ba572ff22302b7e05f054c175c0b2217f293b80f29f8b3b960d549e5f331cb7` |

## Install

```bash
chmod +x ETOL-0.5.0-linux-x86_64-portable.AppImage
./ETOL-0.5.0-linux-x86_64-portable.AppImage
```
