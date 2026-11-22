# ETOL 0.5.1

Start ETOL automatically with your desktop session, straight to the tray.

## Start with the system

- **Options → Start with the system**, the first option before **Language**,
  creates the standard XDG autostart entry
  `~/.config/autostart/etol.desktop`. Nothing is added to `config.toml`.
- The switch reflects the file itself: if you delete the entry by hand it shows
  as disabled, and enabling it again rewrites it. No config/file drift.
- `Exec=` points to the real AppImage (`APPIMAGE`), not the temporary mount
  inside `/tmp`, and adds `--autostart`.
- On login ETOL starts hidden in the tray, so the automatic watchdog is ready
  without opening a window over your desktop.
- If the desktop has no tray host, ETOL opens the window instead: it never
  hides without a way to bring it back.
- If you move or rename the AppImage, turn the option off and on so the entry
  points to the new path.
- Existing ETOL autostart entries from AppImage integration are upgraded in
  place with `--autostart`; their original `Exec=` path is preserved. If that
  path no longer exists, the entry is rewritten with the current AppImage.

## Upgrading from 0.5.0

- Games, bridges, FFB-HUB, SimHub, CrewChief, the HF8 configuration, Fanatec
  profiles and the udev rules are preserved.
- The option shows the real state of `~/.config/autostart/etol.desktop`: if an
  entry already existed, it appears enabled. Toggle it off and on to point it
  at the copy of ETOL you are using now.

## Requirements

### Always required

- Linux x86_64, glibc **2.39+** (portable) or **2.44+** (full).
- `python3` and `curl`.
- Portable: `libwebkit2gtk-4.1` + `libgtk-3`.
- Steam with the games; Bottles with SimHub (ETOL can prepare both).

### Only if you use that hardware or feature

- **Fanatec CSL DD**: `hid-fanatecff` (packaged as **fanatec-dkms**) loaded and
  `pkexec` for the write rule. ETOL runs normally without it; the Fanatec card
  is simply unavailable.
- **DC Pedals**: `bsdtar`/`7z`/`unrar` for the official RAR and `pkexec` for the
  udev rule.
- **HF8**: no extra packages; `pkexec` only for the optional USB access rule.
- **Port 8888 / wifi dashboards**: `ufw` or `firewalld` only for the **Open port
  8888** button.
- **KDE Plasma** for dashboards over the game and a **StatusNotifier** host for
  the tray icon. Without them ETOL works the same; only the overlay or tray icon
  is missing.

## Assets

| Asset | Size | SHA-256 |
|---|---|---|
| `ETOL-0.5.1-linux-x86_64-portable.AppImage` | 7.0 MB | `0139cd14092d4cb935af03f3a6434e94ec23931508215687bbd8ba705a82c514` |
| `ETOL-0.5.1-linux-x86_64-full.AppImage` | 112 MB | `34c8bd942ede17cb14440fef19c60374201b5271b6f79ed27810d503619c004e` |

## Install

```sh
chmod +x ETOL-0.5.1-linux-x86_64-portable.AppImage
./ETOL-0.5.1-linux-x86_64-portable.AppImage
```

Then go to **Options → Start with the system** and turn the switch on.
