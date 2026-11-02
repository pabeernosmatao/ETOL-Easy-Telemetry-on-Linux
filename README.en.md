# ETOL · Easy Telemetry On Linux

[Español](README.md) · **English**

[![Version](https://img.shields.io/badge/version-0.3.0-38e1b0)](https://github.com/pabeernosmatao/ETOL-Easy-Telemetry-on-Linux/releases)
[![License](https://img.shields.io/badge/license-MIT-4aa8ff)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Linux%20x86__64-8ea3b9)](#requirements)
[![Tauri](https://img.shields.io/badge/Tauri-2-24c8db)](https://tauri.app)

**Telemetry for your simulators on Linux without fighting Wine.**
ETOL detects your games, sets up SimHub (and CrewChief) and launches everything
with one click.

---

## What it does

- **Auto-detects** your Steam simulators (any library, any disk), their Proton
  build and the Bottles bottle with SimHub.
- **Per-game auto-configuration** with one button. The gear turns **red** when a
  game has never been configured or ETOL finds something wrong.
- **Per-game telemetry bridge**: shared memory or UDP, whichever the simulator
  uses. Always read-only: it never writes into the game.
- **Automatic watchdog**: opening a simulator starts its bridge and apps;
  closing it stops everything. Switching games switches everything.
- **HF8 pad** (ForceFeel) with its COM port wired inside the bottle.
- **SimHub dashboards over the game** on KDE/KWin, no gamescope needed.
- Interface in **Spanish, Catalan and English**.

## Supported simulators

| Game | Telemetry | Apps | Notes |
|---|---|---|---|
| **Le Mans Ultimate** | shared memory | SimHub + CrewChief | |
| **Automobilista 2** | shared memory | SimHub + CrewChief | |
| **Assetto Corsa EVO** | shared memory | SimHub + CrewChief | |
| **Assetto Corsa** | shared memory | SimHub + CrewChief | |
| **rFactor 2** | shared memory | SimHub + CrewChief | bundles the SMMP plugin |
| **DiRT Rally 2.0** | UDP relay (20777) | SimHub + CrewChief | `extradata=3` |
| **Euro Truck Simulator 2** | native Linux, `/dev/shm` | SimHub | installs the SCS plugin |
| **BeamNG.drive** | native Linux, OutGauge UDP | SimHub | |

## How to use it

1. **Detect.** ETOL finds Steam, Bottles, SimHub and the HF8 pad.
2. **Configure.** Press the gear on each card (once per game).
3. **Play.** Open the simulator and press **Launch**, or let the automatic
   watchdog do it when it sees the game.

## Installation

Download the AppImage from the latest release and make it executable:

```bash
chmod +x ETOL-0.3.0-linux-x86_64.AppImage
./ETOL-0.3.0-linux-x86_64.AppImage
```

Two builds are published:

| Build | Size | When to use |
|---|---|---|
| `ETOL-0.3.0-linux-x86_64.AppImage` | ~6 MB | **Recommended.** Uses the system WebKit; works on Ubuntu 26.04+, Fedora, Arch and CachyOS |
| `ETOL_0.3.0_amd64.AppImage` | ~111 MB | Everything bundled; needs glibc 2.44 (current CachyOS/Arch/Fedora) |

The portable build needs `libwebkit2gtk-4.1` and `libgtk-3`:

```bash
sudo apt install libwebkit2gtk-4.1-0 libgtk-3-0    # Ubuntu/Debian
sudo dnf install webkit2gtk4.1 gtk3                # Fedora
sudo pacman -S webkit2gtk-4.1 gtk3                 # Arch/CachyOS
```

Without FUSE2:

```bash
./ETOL-0.3.0-linux-x86_64.AppImage --appimage-extract-and-run
```

## Requirements

- Linux x86_64 with **glibc 2.39 or newer**.
- **Steam** with the games installed.
- **Bottles** (Flatpak or native) with a bottle that has **SimHub**; CrewChief
  is optional.
- **KDE Plasma** for dashboards above the game.
- The **HF8** pad is optional.

## The window

- **Games**: one card per simulator with its status, app buttons and the
  configuration gear.
- **Applications**: SimHub and CrewChief with their processes.
- **Devices**: the HF8 pad and its COM link.
- **Diagnostics**: checks with the exact fix next to them.
- **Options**: language, automatic watchdog, dashboards, logs and USB.

## Documentation

- [`docs/TECNICO.md`](docs/TECNICO.md): CLI, configuration, detection, building.
- [`docs/guia-reinstalacion.html`](docs/guia-reinstalacion.html): reinstall from scratch (Spanish).
- [`THIRD-PARTY.md`](THIRD-PARTY.md): licenses of the bundled components.

## License

ETOL is MIT. The AppImage bundles third-party components; see
[THIRD-PARTY.md](THIRD-PARTY.md).
