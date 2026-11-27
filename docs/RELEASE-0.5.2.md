# ETOL 0.5.2

Tray icon polish and first-class support for Atomic systems such as **Bazzite**.

## Tray icon

- The StatusNotifierItem icon now uses a dedicated set of sizes (16, 22, 24,
  32, 48, 64 and 128 px) instead of a single 32x32 PNG.
- The gauge and heartbeat fill the icon and the dark square background is
  gone, so ETOL blends with the panel instead of looking like a small tile.
- The application icon, window icon and the rest of the interface are
  unchanged.

## Atomic / Bazzite

> **Beta:** support for immutable systems is **beta** in 0.5.2. ETOL detects the
> environment and applies the fixes below, but there may still be differences
> between immutable versions. Classic distros remain the primary target.

- **Environment probe on first run**, persisted in `[system]`:
  `kind` (`atomic`/`classic`), `distro`, `home_real`, `flatpak_scope`,
  `package_backend` and `needs_path_normalization`. It is shown in
  **Options → Entorno detectado** with a **Redetectar** button. It is refreshed
  at startup, so it also picks up changes after `rpm-ostree upgrade`.
- **Serial group on Atomic**: `/etc/group` can be minimal and the group can come
  from `/usr/lib/group` (NSS `altfiles`). ETOL now creates the local group entry
  before `usermod`, verifies `id -nG` as a mandatory postcondition and reports an
  error if the user did not really enter the group. With the `uaccess` udev rule
  installed it no longer asks to log out: restarting ETOL is enough.
- **`/home` vs `/var/home`**: the home and all compared paths are
  canonicalized. Steam libraries under `/var/home/<user>` are no longer
  reported as outside the home or as new missing libraries.
- **Bottles as a system Flatpak**: the active scope is `user` if present,
  otherwise `system`. Permissions are read and applied from that scope, the UI
  no longer asks for a duplicate user copy, and system overrides are applied
  with `pkexec` (or `sudo`) after verifying the postcondition.
- **DC Pedals / RAR5**: the installer tries `bsdtar`, `7z`, `7za`, `unrar` and
  `unar` in order. Each attempt is accepted only if it exits cleanly and the
  executable and `mcHID.dll` match the declared SHA-256; the final error lists
  every attempted tool and its reason.
- If no RAR extractor exists, ETOL installs the correct package with `pkexec`
  and verifies it: `bsdtar` on rpm-ostree/Fedora, `libarchive` on Arch/CachyOS,
  `libarchive-tools` on Debian/Ubuntu.
- **Package hints** in diagnostics and setup scripts now follow the detected
  backend: `rpm-ostree install --apply-live`, `pacman`, `apt` or `dnf`.
- **Preserved on purpose**: the `70-etol-dcpedals.rules` name and prefix,
  `99-etol-serial.rules` with `uaccess`, brltty masking and the `pkexec`/polkit
  elevation mechanism.
- **CachyOS is unchanged**: it is detected as `classic` with `pacman` and the
  user Flatpak, and keeps the previous behaviour.

## Upgrading from 0.5.1

- Games, bridges, FFB-HUB, SimHub, CrewChief, HF8, Fanatec profiles, udev rules
  and the autostart entry are preserved.
- On Bazzite, a Bottles system Flatpak is now accepted; no duplicate user copy
  is required.
- If a system change is not reflected immediately, use **Options →
  Entorno detectado → Redetectar**.

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
- **DC Pedals**: an archive extractor (`bsdtar`, `7z`, `7za`, `unrar` or
  `unar`) for the official RAR and `pkexec` for the udev rule. If no extractor
  is present, ETOL installs the correct package with `pkexec`.
- **HF8**: no extra packages; `pkexec` only for the optional USB access rule.
- **Port 8888 / wifi dashboards**: `ufw` or `firewalld` only for the **Open port
  8888** button.
- **KDE Plasma** for dashboards over the game and a **StatusNotifier** host for
  the tray icon. Without them ETOL works the same; only the overlay or tray icon
  is missing.

## Assets

| Asset | Size | SHA-256 |
|---|---|---|
| `ETOL-0.5.2-linux-x86_64-portable.AppImage` | 7.0 MB | `f8131332bd57166f5dd4243ccb27da994bf3cfb28ab5d6c49bdb724d2ed6c33b` |
| `ETOL-0.5.2-linux-x86_64-full.AppImage` | 112 MB | `32cc640849ee5bed83b5beb136cf7ba398039e386ffa2c083870afa59949e4e7` |

## Install

```sh
chmod +x ETOL-0.5.2-linux-x86_64-portable.AppImage
./ETOL-0.5.2-linux-x86_64-portable.AppImage
```

Then open **Options → Entorno detectado** to see how ETOL recognised the system.
