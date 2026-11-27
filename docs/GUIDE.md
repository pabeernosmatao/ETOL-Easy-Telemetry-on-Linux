# ETOL · User guide

**English** · [Español](GUIA.md) · Version 0.5.2

Everything ETOL does, how to set up SimHub and CrewChief from scratch and how
to get the most out of each simulator.

---

## Contents

1. [Getting started](#1-getting-started)
2. [The window](#2-the-window)
3. [The Games tab](#3-the-games-tab)
4. [Playing](#4-playing)
5. [Bottles, SimHub and CrewChief from scratch](#5-bottles-simhub-and-crewchief-from-scratch)
6. [Supported simulators](#6-supported-simulators)
7. [Desktop: what works on each one](#7-desktop-what-works-on-each-one)
8. [Fanatec CSL DD](#8-fanatec-csl-dd-optional)
9. [HF8 pad](#9-hf8-pad-optional)
10. [The tray](#10-the-tray)
11. [Logs and diagnostics](#11-logs-and-diagnostics)
12. [Common problems](#12-common-problems)
13. [Where everything lives](#13-where-everything-lives)

---

## 1. Getting started

ETOL is a single application: it detects your Steam simulators, prepares SimHub
(and CrewChief if you want it) inside a Bottles bottle and launches everything
with one click. It never modifies your games.

### What you need

- Linux x86_64 with **glibc 2.39 or newer**.
- `python3` (the telemetry bridges) and `curl` (downloads).
- **Steam** with the games installed.
- **Bottles** with a bottle that has SimHub. If you do not have one, ETOL
  creates and prepares it for you.
- **Optional**: the HF8 pad, the DC2/DC3 pedals, the Fanatec CSL DD wheelbase and
  KDE Plasma for the dashboards. None of this is needed to open ETOL.

### Only if you use optional hardware or features

- **Fanatec CSL DD**: the `hid-fanatecff` driver (packaged as `fanatec-dkms`)
  must be loaded, and `pkexec` is used for the write rule. Without the driver
  ETOL works the same and the Fanatec card is not available.
- **DC Pedals**: an archive extractor (`bsdtar`/`7z`/`unrar`) for the official
  RAR and `pkexec` for the udev rule. ETOL installs the extractor if missing.
- **HF8**: no extra packages; `pkexec` only for the USB access rule.
- **Port 8888**: `ufw` or `firewalld` only for the Open port button.

### Atomic systems (Bazzite) — beta

**Support for immutable systems is beta.** ETOL detects the environment and
applies the fixes, but there may still be differences between versions. Classic
distros remain the primary target.

ETOL detects whether the system is Atomic (`/run/ostree-booted`, `rpm-ostree`)
and adapts: it uses `rpm-ostree install --apply-live` for missing packages,
accepts Bottles as a system Flatpak, normalizes `/home` → `/var/home` and
applies udev/Flatpak permissions to the right scope. **Options → Detected
environment** shows the result and lets you re-detect it after an
`rpm-ostree upgrade`.

### Opening the AppImage

```bash
chmod +x ETOL-0.5.2-linux-x86_64-portable.AppImage
./ETOL-0.5.2-linux-x86_64-portable.AppImage
```

The **portable** build uses the system WebKit and needs `libwebkit2gtk-4.1` and
`libgtk-3`. The **full** build bundles everything and needs glibc 2.44 or newer.

### The first launch

On first start ETOL checks what is missing and shows a banner at the top:

- **Bottles Flatpak is missing**: click the banner and accept. ETOL installs
  Bottles for your user (or uses the system copy on Atomic) and gives it the
  minimum permissions (home, host-etc, Steam libraries and HF8).
- **The SimHub bottle is missing**: ETOL downloads GE-Proton, creates the
  `simhub` bottle, installs the dependencies and opens the CrewChief and SimHub
  installers.

During installation a window opens with a **progress bar** and the current
step. The Windows installers are interactive: click “Next” and uncheck what
step 5 says.

---

## 2. The window

| Tab | What it is for |
|---|---|
| **Games** | One card per installed simulator: state, port, prefix, launch/app buttons and the configuration gear. |
| **Applications** | SimHub and CrewChief, FFB-HUB, DC Pedals and the Fanatec CSL DD card with one collapsible section per installed game, the selected bottle and the paths. |
| **Devices** | USB access, DC Pedals, Fanatec CSL DD (switch and udev rule) and the HF8 pad, with their state, COM port and the link inside the bottle. |
| **Diagnostics** | Checks with the exact fix next to them, including port 8888 and the button to open it to the local network only. |
| **Options** | Start with the system, language, automatic watchdog, per-game apps, dashboards, logs, USB and displays. |

The default language is **Automatic**: it uses Spanish or Catalan if the system
is in those languages and English for everything else.

---

## 3. The Games tab

Each installed simulator has its own card:

- **Port**: free, busy, or in use by the running bridge.
- **Prefix**: the game's Proton prefix, or the plugin on native Linux games.
- **Launch**: the lock ETOL uses to know the bridge is running.

### The gear

It configures that game: copies plugins, adjusts SimHub, CrewChief and the
bridges. It turns **red** when the game has never been configured or when ETOL
sees something wrong. Click it once per game.

### The buttons

- **Launch**: starts the bridge and the game's apps. If it is already running,
  it becomes **Stop**.
- **CrewChief** / **SimHub**: open or close only that application.
- **FFB-HUB** (in Applications): install or update it from its card and launch
  it manually. In automatic mode it only starts if the Applications switch and
  the game checkbox in Options are both enabled.
- **DC Pedals** (Applications + Devices): the Applications card installs the
  official DC Simracing utility in the bottle; the Devices card detects the
  `0444:20DC` HID and prepares the udev rule so Wine can open it.
- **Fanatec CSL DD** (Applications + Devices): the Applications card groups the
  settings per installed game; the Devices card enables the wheel and prepares
  the write rule. It does not touch firmware.

On games that only use SimHub (ETS2 and BeamNG.drive) the two buttons take
**half the width each**.

---

## 4. Playing

### Automatic watchdog (recommended)

With the watchdog on (Options → Automatic watchdog) you only have to open the
simulator. ETOL detects it, starts its bridge and opens the selected apps. When
you close the game it stops everything. If you switch games, it switches too.

### Manual launch

Disable the watchdog and use the card buttons or the tray menu.

ETOL must stay open to watch. You can leave it in the tray: closing the window
keeps it in the background.

When you exit ETOL it stops the bridges, closes the apps it opened and disables
the KWin rule used by the dashboards.

---

## 5. Bottles, SimHub and CrewChief from scratch

1. **Installs Bottles** as a user Flatpak if it is missing. It does not use
   `sudo`.
2. **Creates the `simhub` bottle** with a 64-bit Windows 11 prefix and
   GE-Proton. If it already exists, it is reused.
3. **Prepares the prefix** with `dotnet48`, `vcrun2022`, `d3dcompiler_43`,
   `d3dx9` and `corefonts` through winetricks. It can take several minutes.
4. **Downloads and opens the installers**: CrewChief (MSI) and SimHub (ZIP).

> **In the installers:** in CrewChief, when it finishes **do not check** “Run
> CrewChiefV4”. In SimHub, uncheck *Install .NET Runtime*, *Install USB Display
> Drivers* and *Install Visual C++ Redistributables*, and do not run it at the
> end.

> **CrewChief** comes from the official mirror and may be an older version: open
> it once afterwards and accept the update before using it with the games.

---

## 6. Supported simulators

| Game | Telemetry | Apps | Notes |
|---|---|---|---|
| Le Mans Ultimate | shared memory | SimHub + CrewChief (+ FFB-HUB) | |
| Automobilista 2 | shared memory | SimHub + CrewChief | |
| Assetto Corsa EVO | shared memory | SimHub + CrewChief | |
| Assetto Corsa | shared memory | SimHub + CrewChief | |
| Assetto Corsa Competizione | shared memory | SimHub + CrewChief | |
| rFactor 2 | shared memory | SimHub + CrewChief | bundles the SMMP plugin |
| RaceRoom Racing Experience | shared memory (`$R3E`) | SimHub + CrewChief (+ FFB-HUB) | raw mirror over UDP 8777 |
| DiRT Rally 2.0 | UDP (relay 20777) | SimHub + CrewChief | `extradata=3` |
| Euro Truck Simulator 2 | native Linux, `/dev/shm` | SimHub | installs the SCS plugin |
| BeamNG.drive | native Linux, OutGauge UDP | SimHub | |

The bridges are read-only: they read the game telemetry and publish it for
SimHub. They never write into the game.

---

## 7. Desktop: what works on each one

| Feature | KDE Plasma | GNOME | niri / other Wayland |
|---|---|---|---|
| App, detection and telemetry | Yes | Yes | Yes |
| Dashboards over the game | **Yes**, KWin rule | No | No |
| Is gamescope needed? | **No: works with or without gamescope** | Not applicable | Not applicable |
| Tray icon | Native | AppIndicator extension | Bar with a tray module |
| Close to tray | Yes | With the extension | With the bar |

The dashboards over the game are a **KWin rule** that places the SimHub window
above the simulator. On Plasma it works **with or without gamescope**; gamescope
is not required. GNOME and niri do not implement that rule, so there ETOL
provides telemetry and apps but no overlay.

ETOL also installs a **group icon** for SimHub and CrewChief (CrewChief on
top, SimHub below) and its `.desktop` entry for the `steam_proton` window
class, so the panel shows a single icon with both halves instead of the
CrewChief icon. It is created on startup; nothing has to be run by hand.

---

## 8. Fanatec CSL DD (optional)

ETOL can read and adjust the tuning menu of Fanatec wheelbases supported by the
`hid-fanatecff` driver (packaged as `fanatec-dkms`). It does not use Wine or the
official app, and **does not update firmware**.

### Enabling it

In **Devices → Fanatec CSL DD** turn the switch on. ETOL explains what it will
prepare and asks for confirmation: it creates
`/etc/udev/rules.d/70-etol-fanatec.rules` with `pkexec` to give you write access
to the driver settings. Without it the card is read-only. If the driver is not
loaded the card says so; install `hid-fanatecff`/`fanatec-dkms` before using it.

### Per-game profiles

In **Applications → Fanatec CSL DD** every installed game has its own collapsible
section with the active `SET` settings: `SEN`, `FF`, `FEI`, `FOR`, `SPR`, `DPR`,
`NDP`, `NFR`, `NIN`, `INT`, `FFS`, `FUL`, `SHO`, `BLI`, `brF`, `ACP`, `DRI` and
`advanced_mode`. Changes are saved per game in `[fanatec.profiles.<game>]`.

- **Load into wheel**: applies the profile now.
- **Copy from wheel**: saves the current values into that game's profile.
- Without a profile the sliders show the current values and the **No profile**
  tag; the first change creates a complete profile.
- When ETOL launches the game (or the watchdog starts the bridge), ETOL applies
  the profile before the game script. It applies to the **active SET**; ETOL does
  not change SET or touch firmware.

`SLOT` and `RESET` are not exposed by ETOL.

---

## 9. HF8 pad (optional)

The HF8 (ForceFeel) is a USB haptic pad. ETOL detects it, links its COM port
inside the Bottles bottle and writes the keys SimHub needs. Without it ETOL
works the same and the diagnostics only show a warning.

In **Devices** you can enable it, see the port and reconfigure it.

---

## 10. The tray

ETOL uses **StatusNotifierItem** (the Linux tray standard), so the icon is
drawn by your desktop: Plasma natively, the niri bar if it has a tray module, or
GNOME with the AppIndicator extension.

The menu has: Show window, one submenu per simulator (Launch / Stop everything),
Applications (open/close CrewChief and SimHub) and Quit.

In **Options → Start with the system** ETOL creates an XDG entry in
`~/.config/autostart/etol.desktop`. At login it opens in the tray, without a
window, so the automatic watchdog is ready. The switch reflects whether that
entry exists: if you delete it by hand, it shows as disabled.

If there is no tray host, ETOL opens anyway and logs it; in that case closing
the window quits the application.

---

## 11. Logs and diagnostics

In **Diagnostics** you have the checks with the fix next to them. In **Options**
you can enable log capture and choose the retention.

The logs live in `~/.config/etol/logs/`:

- `etol-YYYY-MM-DD.log`: app events.
- `setup-*.log` and `config-*.log`: full output of installations and
  configurations.
- `run-YYYY-MM-DD/`: copy of the bridge and SimHub/CrewChief logs.

### Port 8888 and wifi dashboards

In **Diagnostics** you can check SimHub's port 8888. If the firewall blocks it,
the result shows the exact command restricted to your local network and an
**Open port 8888** button:

- It detects whether you use `ufw` or `firewalld` and your local network (for
  example `192.168.1.0/24`).
- It shows the command and applies it with `pkexec`.
- It only opens the port to the LAN; never to the Internet.
- If no firewall is active it tells you the port is already open.

After applying it, press **Check 8888** again to confirm.

If something fails, send the whole logs folder. `recoger-logs.sh` packs it into
a `.tar.gz`.

---

## 12. Common problems

**No simulator appears.** Check that Steam has them installed and click
**Detect again**. ETOL reads all Steam libraries, including those on other
disks.

**The gear stays red.** That game has not been configured or ETOL sees
something wrong. Click the gear and read the diagnostics.

**SimHub does not start.** Check **Applications** and the diagnostics; if the
bottle does not have SimHub, run the prefix preparation again.

**There is no tray icon.** Your desktop has no StatusNotifier host. On GNOME
install the AppIndicator extension; on niri add a tray module to the bar.

**ETOL does not start with the system.** Enable **Options → Start with the
system**. If you moved or renamed the AppImage, turn it off and on again so the
entry points to the new path. The file is
`~/.config/autostart/etol.desktop`.

**The AppImage does not start.** The portable build needs
`libwebkit2gtk-4.1` and `libgtk-3`. With glibc 2.44 or newer try the **full**
build. Without FUSE2: `./ETOL-*.AppImage --appimage-extract-and-run`.

**The dashboards do not appear above the game.** That feature is exclusive to
KDE Plasma (KWin rule). On Plasma gamescope is not needed.

**Fanatec is read-only and I cannot edit it.** Enable it in Devices and
authorise the udev rule with `pkexec`. If the `hid-fanatecff`/`fanatec-dkms`
driver is not loaded, the card cannot read the tuning menu.

**The Fanatec profile is not loaded when the game starts.** Check that the game
has a saved profile, that the Fanatec switch is on and that you launch the game
from ETOL (or with the watchdog). The profile applies to the wheel's active SET.

**My phone cannot reach port 8888.** Press **Open port 8888** in Diagnostics;
the rule is restricted to your local network. If no firewall is active the
message says so. Then check the port again.

**I want to start over.** Close ETOL and delete `~/.config/etol`. Bottles
bottles and games are not touched.

---

## 13. Where everything lives

| What | Where |
|---|---|
| Configuration | `~/.config/etol/config.toml` |
| Scripts and bridges | `~/.config/etol/payload/` |
| Logs | `~/.config/etol/logs/` |
| Generated environment | `~/.config/etol/generated/entorno.sh` |
| Bottles, runners and prefixes | `~/.var/app/com.usebottles.bottles/` |
| Installer downloads | `~/Descargas/ETOL/` |
| Fanatec profiles | `~/.config/etol/config.toml` (`[fanatec.profiles.<game>]`) |
| Hardware udev rules | `/etc/udev/rules.d/70-etol-dcpedals.rules` and `/etc/udev/rules.d/70-etol-fanatec.rules` |
| Autostart | `~/.config/autostart/etol.desktop` |

ETOL does not store anything inside the Steam folder or the games, except the
plugins the user asks to install (SCS in ETS2 and SMMP in rFactor 2 and Le Mans
Ultimate) and the configuration files each game expects.
