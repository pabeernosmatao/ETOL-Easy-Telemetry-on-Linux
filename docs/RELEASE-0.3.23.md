# ETOL 0.3.23

An HF8 fix: USB serial access without manual commands.

## HF8 USB access

On a clean install SimHub could see the HF8 but not open it. The usual cause is
that the user is not in the group that owns `/dev/ttyACM*` and `/dev/ttyUSB*`:

- Arch/CachyOS: `uucp`.
- Debian, Ubuntu, Fedora: `dialout`.

`brltty` can also claim CDC-ACM USB devices such as the HF8.

The **Devices** tab now has a **USB device access** card as the first card,
collapsed by default. Its header shows the status ("OK" or "Check") without
taking up space. Expanding it shows:

- the detected serial group and whether the user belongs to it;
- the `brltty` state (not installed, masked or active);
- the udev rule state;
- the detected serial ports and whether the user can open them.

The **Set up USB access** button asks for authorization with `pkexec` and:

1. adds the user to the group that owns the serial ports (`uucp` or `dialout`);
2. installs `/etc/udev/rules.d/99-etol-serial.rules` with `TAG+="uaccess"` for
   `ttyACM*` and `ttyUSB*`, so access works without depending on the group;
3. masks `brltty-udev.service` and `brltty.service` if they are installed;
4. reloads and applies the udev rules.

The udev rule grants immediate access to the session user. The group is the
persistent fallback: if it is added, log out or reboot for the session to
receive it.

On the test machine the only required change was adding the user to `uucp`:
`brltty` was not installed and the `ttyUSB` rule used for the LEDs does not
affect the HF8, which is `ttyACM`.

## Auto-close when the game exits

The watcher considered some games open after they had been closed. Proton
leaves a `steam.exe` process whose first argument is
`c:\windows\system32\steam.exe` and which carries the game path in its command
line; detection confused it with the real game executable.

Now only the process whose first argument (or `comm`) is the game executable
counts. The prefix's `steam.exe` no longer prevents the bridge, CrewChief and
SimHub from stopping when the simulator closes.

LMU auto-close was also fixed: the bridge name now includes `.exe` (it was not
recognized before), and the manual-start window no longer prevents closing
CrewChief and SimHub if the game closes within those 90 seconds.

In BeamNG the bridge is the `BeamNG.drive.x64.exe` stub that SimHub uses to
detect the native game. ETOL now recognizes that process and stops it when
BeamNG closes; before it stayed alive and SimHub always switched back to BeamNG
even if you changed the game manually.

For native games (ETS2, BeamNG), detection no longer accepts any command-line
argument: Steam's `reaper`/`python3` wrappers carry the game path and could keep
it marked as open after closing. It checks the process `comm` or its real first
argument (read from `/proc/<pid>/cmdline`, which respects spaces in paths such
as `Euro Truck Simulator 2`).

ETOL also stops bridges left over from a previous session at startup if their
game is not open.

## Diagnostics

The **Diagnostics** tab includes a **USB access** check. If the user cannot open
the device or `brltty` may claim it, a warning with the fix is shown.

## Release notes

- New card in Devices and new `fix_usb_access` command.
- New `usbaccess` module with detection and fix.
- Spanish, Catalan and English translations.
- Guides updated in all three languages.

## Assets

| Asset | Size | SHA-256 |
|---|---|---|
| `ETOL-0.3.23-linux-x86_64-portable.AppImage` | 6.6 MB | `9fa40c146dede2d17ac82dc837e98f2f5855bb02e1c94bbbcb5e52e33665ac0e` |
| `ETOL-0.3.23-linux-x86_64-full.AppImage` | 112 MB | `125531bcc47a0dd6dc9e7c050e3153aae16037b171bf382c20b843e76b732999` |

## Install

```bash
chmod +x ETOL-0.3.23-linux-x86_64-portable.AppImage
./ETOL-0.3.23-linux-x86_64-portable.AppImage
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
