# ETOL 0.3.4

Small follow-up to 0.3.3, focused on the user guide and desktop support.

## User guide

- The *Guía de uso* button now opens the guide **in the interface language**
  (Spanish, Catalan or English), not always Spanish.
- The guide is published in three languages and each version has a language
  selector at the top.
- A **Markdown version** is available for GitHub:
  [`docs/GUIA.md`](https://github.com/pabeernosmatao/ETOL-Easy-Telemetry-on-Linux/blob/main/docs/GUIA.md)
  (Spanish) and
  [`docs/GUIDE.md`](https://github.com/pabeernosmatao/ETOL-Easy-Telemetry-on-Linux/blob/main/docs/GUIDE.md)
  (English). They are linked next to the language selector in the READMEs.
- The old `docs/guia-reinstalacion.html` URL redirects to the new Spanish guide,
  so existing links keep working.

## Gamescope clarification

The guide and the README now say explicitly that the dashboards above the game
on KDE Plasma work **with or without gamescope**; gamescope is not required.
GNOME and niri still do not have the overlay (it is a KWin rule).

## Assets

| Asset | Size | SHA-256 |
|---|---|---|
| `ETOL-0.3.4-linux-x86_64-portable.AppImage` | 6.1 MB | `607ab8642a459ed3a2acf73565fdf353a5825dde863fd4a770b76c3dfbc6ca3a` |
| `ETOL-0.3.4-linux-x86_64-full.AppImage` | 112 MB | `67eebc49a4d570aef462091e9ef36e86f1322bbf9725514f73681d99db1484cc` |

## Requirements

- Linux x86_64, glibc **2.39+** (portable) or **2.44+** (full).
- `python3` and `curl` on the system.
- Portable build: `libwebkit2gtk-4.1` + `libgtk-3`.
- Steam with the games; Bottles with SimHub (ETOL can install and prepare both).
- KDE Plasma for dashboards above the game.
- A StatusNotifier host for the tray icon (Plasma, niri bar, GNOME extension).

## Install

```bash
chmod +x ETOL-0.3.4-linux-x86_64-portable.AppImage
./ETOL-0.3.4-linux-x86_64-portable.AppImage
```
