# ETOL 0.3.20

Follow-up to 0.3.19 for clean installs while GitHub's release CDN keeps
answering `504 Gateway Time-out`.

## Fix

- **Bottles components**: when `bottles-cli new` fails because Bottles cannot
  download its own components (dxvk, vkd3d, nvapi, latencyflex, winebridge),
  ETOL now downloads those same archives **through the GitHub API** and leaves
  them in Bottles' cache (`~/.var/…/bottles/temp`), verifying the MD5 from
  Bottles' own manifest. Then it retries the bottle creation, which now finds
  the files already cached and skips the broken downloads.
- This complements the 0.3.19 fixes: latest-version lookup, GE-Proton and
  SimHub downloads with API fallback, and the missing `runners` folder on a
  clean machine.

Nothing else changes: same payload, same games, same workflow.

## Assets

| Asset | Size | SHA-256 |
|---|---|---|
| `ETOL-0.3.20-linux-x86_64-portable.AppImage` | 6.6 MB | `8e740a7c640cf4ee062ac0e8465d49cb6d190b7eb2cf06d6ce29217f7952f504` |
| `ETOL-0.3.20-linux-x86_64-full.AppImage` | 112 MB | `1b0ea8fb56a6bf2bf34759a39280e92e68540764e53e0837776e396cd866e863` |

## Requirements

- Linux x86_64, glibc **2.39+** (portable) or **2.44+** (full).
- `python3` and `curl` on the system.
- Portable build: `libwebkit2gtk-4.1` + `libgtk-3`.
- Steam with the games; Bottles with SimHub (ETOL can install and prepare both).
- KDE Plasma for dashboards above the game.
- A StatusNotifier host for the tray icon (Plasma, niri bar, GNOME extension).

## Install

```bash
chmod +x ETOL-0.3.20-linux-x86_64-portable.AppImage
./ETOL-0.3.20-linux-x86_64-portable.AppImage
```
