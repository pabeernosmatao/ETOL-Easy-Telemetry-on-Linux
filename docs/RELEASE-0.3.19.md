# ETOL 0.3.19

Fix release for **installing from scratch**. The 0.3.18 setup could stop with
`El script terminó con errores (código 1)` and no explanation when GitHub was
not serving release downloads.

## Fixes

- **Latest version lookup**: if `github.com/…/releases/latest` fails (it has
  been answering `504 Gateway Time-out`), ETOL asks `api.github.com` instead.
  This affects GE-Proton and SimHub.
- **Release downloads**: if `github.com/…/releases/download/…` fails, the file
  is fetched through the GitHub API, which redirects to the same asset. Used
  for the GE-Proton runner and the SimHub ZIP.
- **Clean installs**: when there is no `runners` folder yet, the search for an
  already-installed runner no longer aborts the script silently. That was the
  real cause of the error at 10 %.
- **Clear failure**: if Bottles cannot create the bottle because its own
  components (dxvk, vkd3d, nvapi…) return `504`, the message now says it is a
  temporary GitHub outage and invites you to retry.
- Unexpected failures now print the line number and exit code instead of only
  `código 1`.

Bottles downloads its own components directly from GitHub; if that CDN is
failing, retry in a few minutes. Everything else in the setup is unchanged.

## Assets

| Asset | Size | SHA-256 |
|---|---|---|
| `ETOL-0.3.19-linux-x86_64-portable.AppImage` | 6.6 MB | `453b689bc085c0d68f76e269b60d677dc103a2462d82366a858658694006797c` |
| `ETOL-0.3.19-linux-x86_64-full.AppImage` | 112 MB | `f615baf7a05af400a0dca8d678e2452a0c0084a6c479120831a10e47587c3190` |

## Requirements

- Linux x86_64, glibc **2.39+** (portable) or **2.44+** (full).
- `python3` and `curl` on the system.
- Portable build: `libwebkit2gtk-4.1` + `libgtk-3`.
- Steam with the games; Bottles with SimHub (ETOL can install and prepare both).
- KDE Plasma for dashboards above the game.
- A StatusNotifier host for the tray icon (Plasma, niri bar, GNOME extension).

## Install

```bash
chmod +x ETOL-0.3.19-linux-x86_64-portable.AppImage
./ETOL-0.3.19-linux-x86_64-portable.AppImage
```
