#!/usr/bin/env python3
"""Configura SimHub para Euro Truck Simulator 2 en su version nativa Linux.

No hay nada que configurar en el juego: el plugin SCS publica la telemetria en
/dev/shm/SCSTelemetry y el espejo de ETOL la republica dentro del prefijo de
Bottles. Aqui solo se deja SimHub listo:

  - que ETS2 no este oculto en HiddenGames.json;
  - que su entrada de GameSettings.json no muestre avisos de configuracion.

CrewChief no aparece: no tiene lector de camiones.
"""

from __future__ import annotations

import json
import os
import shutil
import sys
from datetime import datetime
from pathlib import Path

HOME = Path.home()
BOTTLE = Path(
    os.environ.get("SIMHUB_BOTTLE", HOME / ".local/share/bottles/bottles/simhub")
).expanduser()
RUN_DIR = os.environ.get("ETS2_MIRROR_RUN_DIR", "/tmp/etol/ets2_bridge")
GAME_DIR = os.environ.get("ETS2_GAME_DIR", "")
BACKUP_DIR = Path(
    os.environ.get("ETS2_BACKUP_DIR", HOME / ".local/share/etol/ets2/backups")
).expanduser()
STAMP = datetime.now().strftime("%Y%m%d-%H%M%S")

PLUGINS = BOTTLE / "drive_c/Program Files (x86)/SimHub/PluginsData"
GAME_KEY = "ETS2"

CHANGES: list[str] = []
WARNINGS: list[str] = []


def app_running(exe_name: str) -> bool:
    """True si el .exe real de la app corre en algun wineserver."""
    needle = exe_name.encode()
    try:
        entries = list(Path("/proc").iterdir())
    except OSError:
        return False
    for entry in entries:
        if not entry.name.isdigit():
            continue
        try:
            raw = (entry / "cmdline").read_bytes()
        except OSError:
            continue
        if needle not in raw:
            continue
        cmd = raw.replace(b"\0", b" ").strip().lower()
        if any(
            marker in cmd
            for marker in (
                b"start.exe",
                b"wine",
                b"pv-adverb",
                b"pressure-vessel",
                b"proton",
                b"reaper",
                b"steamlaunch",
            )
        ):
            continue
        return True
    return False


def backup(path: Path) -> None:
    if not path.exists():
        return
    BACKUP_DIR.mkdir(parents=True, exist_ok=True)
    dest = BACKUP_DIR / f"{path.name}.{STAMP}.bak"
    shutil.copy2(path, dest)
    CHANGES.append(f"backup: {dest}")


def write_atomic(path: Path, text: str) -> None:
    tmp = path.with_name(path.name + ".tmp-etol-ets2")
    tmp.write_text(text, encoding="utf-8", newline="")
    os.replace(tmp, path)


def load_json(path: Path, default):
    if not path.exists():
        return default
    try:
        with path.open("r", encoding="utf-8-sig") as fh:
            return json.load(fh)
    except Exception as exc:  # noqa: BLE001
        WARNINGS.append(f"no se pudo leer {path}: {exc}")
        return default


def save_json(path: Path, data) -> None:
    backup(path)
    write_atomic(path, json.dumps(data, indent=2, ensure_ascii=False) + "\n")


def check_plugin() -> None:
    if not GAME_DIR:
        WARNINGS.append(
            "no se conoce la carpeta de ETS2; no puedo comprobar el plugin SCS"
        )
        return
    plugin = Path(GAME_DIR) / "bin/linux_x64/plugins/libscs-telemetry.so"
    if plugin.is_file():
        CHANGES.append(f"plugin SCS presente: {plugin}")
    else:
        WARNINGS.append(f"falta el plugin SCS en {plugin}")


def configure_simhub() -> None:
    if not PLUGINS.is_dir():
        WARNINGS.append(f"no existe {PLUGINS}")
        return

    simhub_running = app_running("SimHubWPF.exe")

    hidden_path = PLUGINS / "HiddenGames.json"
    hidden = load_json(hidden_path, {})
    if hidden.get(GAME_KEY) is not False:
        if simhub_running:
            WARNINGS.append(
                "SimHub esta abierto: no toco HiddenGames.json; "
                "Euro Truck Simulator 2 deberia estar visible."
            )
        else:
            hidden[GAME_KEY] = False
            save_json(hidden_path, hidden)
            CHANGES.append("SimHub: Euro Truck Simulator 2 visible en HiddenGames.json")
    else:
        CHANGES.append("SimHub: Euro Truck Simulator 2 ya estaba visible")

    game_path = PLUGINS / "GameSettings.json"
    games = load_json(game_path, {})
    if not isinstance(games, dict) or not games:
        WARNINGS.append(f"no se pudo leer {game_path}")
        return

    entry = games.get(GAME_KEY)
    if isinstance(entry, dict):
        if simhub_running:
            WARNINGS.append(
                "SimHub esta abierto: no toco GameSettings.json; "
                "reinicialo para aplicar los cambios."
            )
            return
        desired = {
            "DisableConfigAlert": True,
            "AutomaticConfigurationDismissed": True,
            "ManualConfigurationDismissed": True,
        }
        changed = False
        for key, value in desired.items():
            if entry.get(key) != value:
                entry[key] = value
                changed = True
        if changed:
            save_json(game_path, games)
            CHANGES.append("SimHub: avisos de configuracion de ETS2 desactivados")
        else:
            CHANGES.append("SimHub: ETS2 ya estaba configurado")
        return

    # La entrada existe en cualquier instalacion de SimHub; si falta, no la
    # inventamos: se crea sola la primera vez que SimHub vea el juego.
    WARNINGS.append(
        "SimHub no tiene todavia la entrada ETS2 en GameSettings.json; "
        "se creara sola cuando SimHub detecte el juego."
    )


def main() -> int:
    check_plugin()
    configure_simhub()

    print("Euro Truck Simulator 2 (Linux nativo) -> SimHub")
    print(f"  espejo: /dev/shm/SCSTelemetry -> Local\\SCSTelemetry")
    if GAME_DIR:
        print(f"  juego:  {GAME_DIR}")
    print(f"  logs:   {RUN_DIR}")
    for line in CHANGES:
        print(f"  · {line}")
    for line in WARNINGS:
        print(f"  ! {line}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
