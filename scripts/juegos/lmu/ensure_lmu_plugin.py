#!/usr/bin/env python3
"""Marca el plugin SMMP como activo en Le Mans Ultimate.

SimHub solo da LMU por configurado si
`UserData/player/CustomPluginVariables.JSON` contiene
`rFactor2SharedMemoryMapPlugin64.dll` con `" Enabled": 1`. CrewChief, además,
busca el DLL en `Plugins/` y ofrece instalarlo en cada arranque si no está.

El espejo de ETOL no necesita el plugin dentro del juego para publicar los
buffers, pero SimHub y CrewChief sí lo esperan. Este script copia el DLL que ya
viaja en el payload y activa la entrada del JSON, con copia de seguridad y sin
tocar nada más del juego.
"""

from __future__ import annotations

import json
import os
import shutil
import sys
from datetime import datetime
from pathlib import Path

PLUGIN = "rFactor2SharedMemoryMapPlugin64.dll"
LMU_PLUGIN = "LMU.3.8.SharedMemoryMapPlugin.dll"

LMU_DEFAULTS = {
    " Enabled": 1,
    "DebugISIInternals": 0,
    "DebugOutputLevel": 0,
    "DebugOutputSource": 1,
    "DedicatedServerMapGlobally": 0,
    "EnableDirectMemoryAccess": 0,
    "EnableHWControlInput": 1,
    "EnableRulesControlInput": 0,
    "EnableWeatherControlInput": 0,
    "UnsubscribedBuffersMask": 160,
}

# Los mismos valores que escribe SimHub al pulsar "Configure" en LMU.
DEFAULTS = {
    " Enabled": 1,
    "DebugISIInternals": 0,
    "DebugOutputLevel": 0,
    "DebugOutputSource": 1,
    "DedicatedServerMapGlobally": 0,
    "EnableDirectMemoryAccess": 0,
    "EnableHWControlInput": 1,
    "EnableRulesControlInput": 0,
    "EnableWeatherControlInput": 0,
    "UnsubscribedBuffersMask": 160,
}


def backup(path: Path) -> Path | None:
    if not path.is_file():
        return None
    root = Path(
        os.environ.get(
            "LMU_MIRROR_BACKUP_DIR",
            Path.home() / ".local/share/lmu_mirror/backups",
        )
    ).expanduser()
    root.mkdir(parents=True, exist_ok=True)
    dest = root / f"{path.name}.{datetime.now():%Y%m%d-%H%M%S}.bak"
    shutil.copy2(path, dest)
    return dest


def crewchief_lmu_dll(simhub: Path | None) -> Path | None:
    """DLL LMU que instala CrewChief (plugins/Le Mans Ultimate/Plugins)."""
    if not simhub:
        return None
    drive_c = simhub.parent.parent
    roots = [
        drive_c
        / "Program Files (x86)/Britton IT Ltd/CrewChiefV4/plugins/Le Mans Ultimate/Plugins",
        drive_c
        / "Program Files/Britton IT Ltd/CrewChiefV4/plugins/Le Mans Ultimate/Plugins",
    ]
    for root in roots:
        candidate = root / LMU_PLUGIN
        if candidate.is_file():
            return candidate
    for pattern in (
        "Program Files*/Britton IT Ltd/CrewChiefV4/plugins/*/Plugins/" + LMU_PLUGIN,
    ):
        for candidate in drive_c.glob(pattern):
            if candidate.is_file():
                return candidate
    return None


def install_lmu_dll(plugins: Path, simhub: Path | None) -> None:
    """Copia LMU.3.8.SharedMemoryMapPlugin.dll al juego si falta o difiere."""
    dest = plugins / LMU_PLUGIN
    source = crewchief_lmu_dll(simhub)
    if not source:
        print(
            f"AVISO: CrewChief no trae {LMU_PLUGIN}; "
            "actualiza CrewChief o copia el plugin a mano.",
            file=sys.stderr,
        )
        return
    same = dest.is_file() and dest.read_bytes() == source.read_bytes()
    if same:
        print(f"LMU: {LMU_PLUGIN} ya estaba al dia")
        return
    saved = backup(dest)
    if saved:
        print(f"LMU: copia del plugin anterior en {saved}")
    shutil.copy2(source, dest)
    print(f"LMU: {LMU_PLUGIN} copiado en {dest}")


def enable_lmu_plugin(game: Path) -> None:
    """Activa LMU.3.8.SharedMemoryMapPlugin.dll (buffers $LMU.3.8.SMMP_*)."""
    for rel in (
        "UserData/player/CustomPluginVariables.JSON",
        "Plugins/CustomPluginVariables.JSON",
    ):
        config = game / rel
        if not config.is_file():
            continue
        try:
            data = json.loads(config.read_text(encoding="utf-8-sig") or "{}")
        except Exception as exc:  # noqa: BLE001
            print(f"AVISO: no se pudo leer {config}: {exc}", file=sys.stderr)
            continue
        if not isinstance(data, dict):
            continue
        entry = data.get(LMU_PLUGIN)
        if not isinstance(entry, dict):
            entry = {}
        changed = False
        for name, value in LMU_DEFAULTS.items():
            if entry.get(name) != value:
                entry[name] = value
                changed = True
        if changed or data.get(LMU_PLUGIN) != entry:
            saved = backup(config)
            if saved:
                print(f"LMU: copia de {config.name} en {saved}")
            data[LMU_PLUGIN] = entry
            config.write_text(
                json.dumps(data, indent=2, ensure_ascii=False) + "\n",
                encoding="utf-8",
            )
            print(f"LMU: {LMU_PLUGIN} activado en {config}")
    print("LMU: reinicia el juego para que cargue el plugin si estaba desactivado.")


def report(simhub: Path | None) -> None:
    """Resume el estado que usa SimHub para dar LMU por configurado."""
    if not simhub or not simhub.is_dir():
        return
    print(f"SimHub: {simhub}")
    pak = simhub / "_Addons/GamePlugins/RFactor2/installLMU.pak"
    print(f"SimHub: installLMU.pak {'presente' if pak.is_file() else 'FALTA'}")
    data_dir = simhub / "PluginsData"
    try:
        hidden = json.loads((data_dir / "HiddenGames.json").read_text(encoding="utf-8-sig") or "{}")
        print(f"SimHub: HiddenGames LMU={hidden.get('LMU')!r}")
    except Exception as exc:  # noqa: BLE001
        print(f"SimHub: HiddenGames no legible ({exc})")
    try:
        games = json.loads((data_dir / "GameSettings.json").read_text(encoding="utf-8-sig") or "{}")
        lmu = games.get("LMU") if isinstance(games, dict) else None
        if isinstance(lmu, dict):
            print(
                "SimHub: GameSettings LMU "
                f"OrderEx={lmu.get('OrderEx')!r} "
                f"LauncherSetting={'si' if lmu.get('LauncherSetting') else 'no'}"
            )
        else:
            print("SimHub: GameSettings sin entrada LMU")
    except Exception as exc:  # noqa: BLE001
        print(f"SimHub: GameSettings no legible ({exc})")
    try:
        plugins = json.loads((data_dir / "PluginsActivation.json").read_text(encoding="utf-8-sig") or "[]")
        wanted = (
            "doX.doX_LMU_SessionDataPlugin",
            "NeoRed.lmuDataPlugin.LMU_NeoRedPlugin",
            "LMU_SessionDataPlugin.LMU_SessionDataPlugin",
        )
        for entry in plugins if isinstance(plugins, list) else []:
            if isinstance(entry, dict) and entry.get("ClassName") in wanted:
                print(f"SimHub: plugin {entry.get('ClassName')} IsEnabled={entry.get('IsEnabled')!r}")
    except Exception as exc:  # noqa: BLE001
        print(f"SimHub: PluginsActivation no legible ({exc})")


def main() -> int:
    if len(sys.argv) < 2:
        print("uso: ensure_lmu_plugin.py <juego> [dll]", file=sys.stderr)
        return 2

    game = Path(sys.argv[1]).expanduser()
    dll_src = Path(sys.argv[2]).expanduser() if len(sys.argv) > 2 else None
    simhub = Path(sys.argv[3]).expanduser() if len(sys.argv) > 3 else None
    if not game.is_dir():
        print(f"AVISO: no existe la carpeta de LMU: {game}", file=sys.stderr)
        return 1

    # SimHub valida el DLL contra su copia de _Addons y usa installLMU.pak.
    # Si faltan (instalacion incompleta), se recrean con el DLL del payload.
    if simhub and simhub.is_dir() and dll_src and dll_src.is_file():
        addons = simhub / "_Addons/GamePlugins/RFactor2"
        addons_dll = addons / "Bin64/Plugins" / PLUGIN
        if not addons_dll.is_file():
            addons_dll.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(dll_src, addons_dll)
            print(f"SimHub: DLL de Addons copiado en {addons_dll}")
        pak = addons / "installLMU.pak"
        if not pak.is_file() and addons_dll.is_file():
            pak.parent.mkdir(parents=True, exist_ok=True)
            pak.write_text(
                "C;Bin64\\Plugins\\rFactor2SharedMemoryMapPlugin64.dll;"
                "{GameDir}\\Plugins\\rFactor2SharedMemoryMapPlugin64.dll\n",
                encoding="utf-8",
            )
            print(f"SimHub: installLMU.pak creado en {pak}")
        if addons_dll.is_file():
            dll_src = addons_dll

    plugins = game / "Plugins"
    player = game / "UserData/player"
    plugins.mkdir(parents=True, exist_ok=True)
    player.mkdir(parents=True, exist_ok=True)

    # LMU 2026 carga LMU.3.8.SharedMemoryMapPlugin.dll desde Plugins/.
    install_lmu_dll(plugins, simhub)

    dll = plugins / PLUGIN
    if dll_src and dll_src.is_file():
        # SimHub valida el MD5 del DLL contra el que trae en
        # _Addons/GamePlugins/RFactor2/Bin64/Plugins. Si el del juego no
        # coincide (o falta), se copia el de SimHub con copia de seguridad.
        same = dll.is_file() and dll.read_bytes() == dll_src.read_bytes()
        if not same:
            saved = backup(dll)
            if saved:
                print(f"LMU: copia del plugin anterior en {saved}")
            shutil.copy2(dll_src, dll)
            print(f"LMU: plugin copiado en {dll}")

    config = player / "CustomPluginVariables.JSON"
    data = {}
    if config.is_file():
        try:
            data = json.loads(config.read_text(encoding="utf-8-sig") or "{}")
        except Exception as exc:  # noqa: BLE001
            print(f"AVISO: no se pudo leer {config}: {exc}", file=sys.stderr)
            data = {}
        if not isinstance(data, dict):
            data = {}
        saved = backup(config)
        if saved:
            print(f"LMU: copia de {config.name} en {saved}")

    entry = data.get(PLUGIN)
    if not isinstance(entry, dict):
        entry = {}
    changed = False
    for name, value in DEFAULTS.items():
        if entry.get(name) != value:
            entry[name] = value
            changed = True
    if changed or data.get(PLUGIN) != entry:
        data[PLUGIN] = entry
        config.write_text(
            json.dumps(data, indent=2, ensure_ascii=False) + "\n",
            encoding="utf-8",
        )
        print(f"LMU: {PLUGIN} activado en {config}")
    else:
        print("LMU: el plugin ya estaba activo")

    enable_lmu_plugin(game)

    if dll.is_file() and dll_src and dll_src.is_file():
        print(
            "LMU: DLL del juego coincide con SimHub: "
            + ("si" if dll.read_bytes() == dll_src.read_bytes() else "NO")
        )
    report(simhub)

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
