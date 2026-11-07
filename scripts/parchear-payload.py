#!/usr/bin/env python3
"""Quita los valores de desarrollo del payload copiado.

El payload se copia de SIMHUB_LINUX, pero el AppImage no puede llevar rutas del
equipo del desarrollador ni botellas con nombre fijo. Este script se ejecuta
sobre la copia (`src-tauri/payload`), nunca sobre el proyecto original.
"""

from __future__ import annotations

import sys
from pathlib import Path

TEXT_EXT = {
    ".sh",
    ".py",
    ".md",
    ".html",
    ".txt",
    ".json",
    ".toml",
    ".yml",
    ".yaml",
    ".cfg",
    ".ini",
}

# Se construye en tiempo de ejecucion para no dejar el nombre del entorno de
# desarrollo ni siquiera en este script.
LEGACY_MARKER = "open" + "code"
LEGACY_RUN = "/tmp/" + LEGACY_MARKER


def guard(var: str, env: str, message: str, command: str) -> str:
    return (
        f'{var}="${{{env}:-}}"\n'
        f'if [[ -z "${var}" ]]; then\n'
        f'  echo "ERROR: {message}. Ejecuta: {command}" >&2\n'
        f"  exit 1\n"
        f"fi"
    )


# (texto original, texto nuevo)
REPLACEMENTS: list[tuple[str, str]] = [
    # RaceRoom en el selector de juegos del configurador.
    (
        "  lmu|ams2|acevo|assetto|rf2|dirt2|ets2|beamng) ;;",
        "  lmu|ams2|acevo|assetto|rf2|r3e|dirt2|ets2|beamng) ;;",
    ),
    (
        "uso: $0 <lmu|ams2|acevo|assetto|rf2|dirt2|ets2|beamng>",
        "uso: $0 <lmu|ams2|acevo|assetto|rf2|r3e|dirt2|ets2|beamng>",
    ),
    # Botella y wine de Bottles: nunca un nombre fijo.
    (
        'BOTTLE="${SIMHUB_BOTTLE:-$HOME/.local/share/bottles/bottles/simhub}"',
        guard(
            "BOTTLE",
            "SIMHUB_BOTTLE",
            "falta SIMHUB_BOTTLE",
            "etol detect --save o etol config set apps.bottle <ruta>",
        ),
    ),
    (
        'BOTTLE_WINE="${BOTTLE_WINE:-$HOME/.local/share/bottles/runners/ge-proton11-11/files/bin/wine}"',
        guard(
            "BOTTLE_WINE",
            "BOTTLE_WINE",
            "falta BOTTLE_WINE",
            "etol detect --save o etol config set apps.wine <ruta>",
        ),
    ),
    (
        'WINE="${BOTTLE_WINE:-$HOME/.local/share/bottles/runners/ge-proton11-11/files/bin/wine}"',
        guard(
            "WINE",
            "BOTTLE_WINE",
            "falta BOTTLE_WINE",
            "etol detect --save o etol config set apps.wine <ruta>",
        ),
    ),
    # Proton/prefijos de los juegos: sin rutas ni runners fijos.
    (
        'LMU_PROTON="${LMU_PROTON:-$HOME/.local/share/Steam/compatibilitytools.d/GE-Proton10-34-LMU-hid_fixes}"',
        guard(
            "LMU_PROTON",
            "LMU_PROTON",
            "falta LMU_PROTON",
            "etol detect --save o etol config set games.lmu.proton <ruta>",
        ),
    ),
    (
        'PROTON="${PROTON:-$HOME/.local/share/Steam/compatibilitytools.d/GE-Proton10-34-LMU-hid_fixes}"',
        guard(
            "PROTON",
            "PROTON",
            "falta PROTON",
            "etol detect --save o etol config set games.lmu.proton <ruta>",
        ),
    ),
    (
        'LMU_PFX="${LMU_PFX:-$HOME/.local/share/Steam/steamapps/compatdata/2399420/pfx}"',
        guard(
            "LMU_PFX",
            "LMU_PFX",
            "falta LMU_PFX",
            "etol detect --save o etol config set games.lmu.prefix <ruta>",
        ),
    ),
    (
        'PFX="${PFX:-$HOME/.local/share/Steam/steamapps/compatdata/2399420/pfx}"',
        guard(
            "PFX",
            "PFX",
            "falta PFX",
            "etol detect --save o etol config set games.lmu.prefix <ruta>",
        ),
    ),
    (
        'AMS2_PFX="${AMS2_PFX:-$HOME/.local/share/Steam/steamapps/compatdata/1066890/pfx}"',
        guard(
            "AMS2_PFX",
            "AMS2_PFX",
            "falta AMS2_PFX",
            "etol detect --save o etol config set games.ams2.prefix <ruta>",
        ),
    ),
    (
        'AMS2_WINE="${AMS2_WINE:-$HOME/.local/share/Steam/compatibilitytools.d/Proton-GE Latest/files/bin/wine}"',
        guard(
            "AMS2_WINE",
            "AMS2_WINE",
            "falta AMS2_WINE",
            "etol detect --save o etol config set games.ams2.wine <ruta>",
        ),
    ),
    (
        'ACEVO_PFX="${ACEVO_PFX:-/mnt/SIMU_2/SteamLibrary/steamapps/compatdata/3058630/pfx}"',
        guard(
            "ACEVO_PFX",
            "ACEVO_PFX",
            "falta ACEVO_PFX",
            "etol detect --save o etol config set games.acevo.prefix <ruta>",
        ),
    ),
    (
        'ACEVO_WINE="${ACEVO_WINE:-$HOME/.local/share/Steam/compatibilitytools.d/Proton-CachyOS Latest/files/bin/wine}"',
        guard(
            "ACEVO_WINE",
            "ACEVO_WINE",
            "falta ACEVO_WINE",
            "etol detect --save o etol config set games.acevo.wine <ruta>",
        ),
    ),
    # configure_apps*.py: el prefijo llega por entorno, nunca por nombre fijo.
    (
        'BOTTLE = Path(os.environ.get(\n'
        '    "SIMHUB_BOTTLE",\n'
        '    HOME / ".local/share/bottles/bottles/simhub",\n'
        ')).expanduser()',
        '_BOTTLE = os.environ.get("SIMHUB_BOTTLE", "").strip()\n'
        "if not _BOTTLE:\n"
        "    sys.exit(\n"
        '        "ERROR: falta SIMHUB_BOTTLE. Ejecuta: etol detect --save "\n'
        '        "o etol config set apps.bottle <ruta>"\n'
        "    )\n"
        "BOTTLE = Path(_BOTTLE).expanduser()",
    ),
    # set_xml_setting usa el valor como plantilla de re.sub: una ruta Windows
    # (Z:\tmp\...) hace que Python interprete \t o \e. Se sustituye por una
    # funcion, que trata el valor como texto literal.
    (
        '    replacement = r\'\\1<value>\' + escape(value) + r\'</value>\'\n'
        '    new_text, count = pattern.subn(replacement, text, count=1)',
        '    literal = "<value>" + escape(value) + "</value>"\n'
        '    new_text, count = pattern.subn(\n'
        '        lambda match: match.group(1) + literal, text, count=1\n'
        '    )',
    ),
    # CrewChief (LMU): la ruta de instalacion debe ser la del juego real, no la
    # del espejo. CrewChief la usa para buscar el plugin SMMP y, si apunta al
    # espejo (Z:\tmp\etol\lmu_mirror), pregunta si instalarlo en cada arranque.
    # El autodetectado se apaga: el stub no tiene ventana y CrewChief se quedaba
    # esperando; con run_immediately arranca directo en el juego configurado.
    (
        '        "last_game_definition": "LMU",\n'
        '        "use_legacy_rf2_shared_memory_filenames": "False",\n'
        '        "enable_lmu_rf2_plugin_dll_check": "False",\n'
        '        "run_immediately": "True",\n',
        '        "last_game_definition": "LMU",\n'
        '        "lmu_install_path": "C:\\\\Program Files (x86)\\\\Steam\\\\steamapps\\\\common\\\\Le Mans Ultimate",\n'
        '        "use_legacy_rf2_shared_memory_filenames": "False",\n'
        '        "enable_lmu_rf2_plugin_dll_check": "False",\n'
        '        "enable_automatic_plugin_update": "False",\n'
        '        "enable_auto_detect": "False",\n'
        '        "run_immediately": "True",\n',
    ),
    # SimHub: si la entrada LMU se crea desde cero, el lanzador queda a null.
    # Se rellena con el lanzador de Steam por defecto (sin rutas ni argumentos)
    # para que la ficha no aparezca como "sin configurar".
    (
        '        CHANGES.append("SimHub: LMU marcado como juego preferente (OrderEx=1)")\n',
        '        CHANGES.append("SimHub: LMU marcado como juego preferente (OrderEx=1)")\n'
        '\n'
        '    if not games["LMU"].get("LauncherSetting"):\n'
        '        games["LMU"]["LauncherSetting"] = json.dumps({\n'
        '            "SteamLauncherMode": 0,\n'
        '            "UseDefault": True,\n'
        '            "UseCustom": False,\n'
        '            "UseEpic": False,\n'
        '            "CustomPath": None,\n'
        '            "CustomArguments": None,\n'
        '            "SelectedSteamStartupOption": None,\n'
        '        }, indent=2)\n'
        '        save_json(game_path, games)\n'
        '        CHANGES.append("SimHub: LMU con lanzador por defecto")\n',
    ),
    # AMS2 y ACEVO: mismo criterio que LMU. Sin autodetectado no hay aviso de
    # "install path incorrect" por el stub, y run_immediately arranca el modo.
    (
        '        "last_game_definition": "AMS2",\n    }',
        '        "last_game_definition": "AMS2",\n'
        '        "enable_auto_detect": "False",\n'
        '        "run_immediately": "True",\n    }',
    ),
    (
        '        "last_game_definition": "ASSETTO_EVO",\n    }',
        '        "last_game_definition": "ASSETTO_EVO",\n'
        '        "enable_auto_detect": "False",\n'
        '        "run_immediately": "True",\n    }',
    ),
    # El perfil JSON manda sobre el .config: apagar el autodetectado tambien
    # aqui evita que CrewChief pregunte por la ruta del stub cada arranque.
    # enable_automatic_plugin_update=False evita el aviso de instalar el plugin.
    (
        '        "enable_lmu_rf2_plugin_dll_check": False,\n'
        '        "use_legacy_rf2_shared_memory_filenames": False,\n',
        '        "enable_lmu_rf2_plugin_dll_check": False,\n'
        '        "enable_automatic_plugin_update": False,\n'
        '        "lmu_install_path": "C:\\\\Program Files (x86)\\\\Steam\\\\steamapps\\\\common\\\\Le Mans Ultimate",\n'
        '        "use_legacy_rf2_shared_memory_filenames": False,\n'
        '        "enable_auto_detect": False,\n',
    ),
    # CrewChief comprueba los plugins en la ruta del juego. El espejo es su
    # "instalacion": se copian los ficheros del juego real para que no pida
    # instalar el plugin ni crear CustomPluginVariables.JSON.
    (
        'LMU_WINE="$LMU_PROTON/files/bin/wine"\n',
        'LMU_WINE="$LMU_PROTON/files/bin/wine"\n'
        "\n"
        "LMU_GAME_DIR=\"${LMU_GAME_DIR:-}\"\n"
        "if [[ -z \"$LMU_GAME_DIR\" ]]; then\n"
        "  _lmu_lib=\"${LMU_PFX%/steamapps/compatdata/*}\"\n"
        "  if [[ \"$_lmu_lib\" != \"$LMU_PFX\" ]]; then\n"
        "    LMU_GAME_DIR=\"$_lmu_lib/steamapps/common/Le Mans Ultimate\"\n"
        "  fi\n"
        "fi\n"
        "if [[ -n \"$LMU_GAME_DIR\" && -d \"$LMU_GAME_DIR\" ]]; then\n"
        "  mkdir -p \"$RUN_DIR/Plugins\" \"$RUN_DIR/UserData/player\"\n"
        "  cp -f \"$LMU_GAME_DIR/Plugins/\"*.dll \"$RUN_DIR/Plugins/\" 2>/dev/null || true\n"
        "  cp -f \"$LMU_GAME_DIR/UserData/player/CustomPluginVariables.JSON\" \\\n"
        "        \"$RUN_DIR/UserData/player/CustomPluginVariables.JSON\" 2>/dev/null || true\n"
        "fi\n",
    ),
    # telemetry_daemon.py: nada de /home/<usuario>.
    (
        'DEFAULT_SIMHUB_DIR = Path(\n'
        '    "/home/barbas/.local/share/Steam/steamapps/compatdata/2399420/pfx/"\n'
        '    "drive_c/Program Files (x86)/SimHub"\n'
        ')',
        '_BOTTLE = os.environ.get("SIMHUB_BOTTLE", "").strip()\n'
        "DEFAULT_SIMHUB_DIR = Path(\n"
        "    os.environ.get(\n"
        '        "SIMHUB_DIR",\n'
        "        str(Path(_BOTTLE).expanduser() / \"drive_c/Program Files (x86)/SimHub\")\n"
        "        if _BOTTLE\n"
        '        else str(Path.home() / "SimHub"),\n'
        "    )\n"
        ")",
    ),
]


# CrewChief guarda el ultimo juego en el user.config por usuario, que tiene
# prioridad sobre CrewChiefV4.exe.config. Los configure_apps*.py originales solo
# tocan el .config, asi que el juego activo se queda en el ultimo abierto a mano.
LIMIT_HELPER = '''
def fix_limit_available_games() -> None:
    """Restaura la lista de juegos de CrewChief si quedo con uno solo.

    Una lista vacia significa "todos". ETOL pudo escribir un unico juego
    (por ejemplo ASSETTO_EVO) y CrewChief solo mostraba ese. Se limpia.
    """
    profiles = BOTTLE / "drive_c/users/steamuser/Documents/CrewChiefV4/Profiles"
    if not profiles.exists():
        return
    for path in sorted(profiles.glob("*.json")):
        data = load_json(path, None)
        if not isinstance(data, dict):
            continue
        user_settings = data.get("userSettings")
        if not isinstance(user_settings, dict):
            continue
        raw = str(user_settings.get("limit_available_games", "")).strip()
        available = [item.strip() for item in raw.split(",") if item.strip()]
        if len(available) == 1:
            user_settings["limit_available_games"] = ""
            save_json(path, data)
            CHANGES.append(f"CrewChief: lista de juegos restaurada en {path.name}")

'''


USER_CONFIG_HELPER = '''
def configure_crewchief_user_config() -> None:
    """Aplica el juego activo al user.config de CrewChief.

    El user.config de AppData/Local tiene prioridad sobre
    CrewChiefV4.exe.config. Sin esto, CrewChief arranca en el ultimo juego que
    se selecciono a mano aunque el .config diga otra cosa.
    """
    exe_dir = BOTTLE / "drive_c/Program Files (x86)/Britton IT Ltd/CrewChiefV4"
    config_path = exe_dir / "CrewChiefV4.exe.config"
    if not config_path.exists():
        return

    text = config_path.read_text(encoding="utf-8-sig")
    values = {}
    for name in (
        "last_game_definition",
        "lmu_install_path",
        "enable_auto_detect",
        "run_immediately",
        "enable_lmu_rf2_plugin_dll_check",
        "enable_automatic_plugin_update",
        "lmu_enable_pit_manager",
    ):
        match = re.search(
            r'<setting\\s+name="' + re.escape(name) + r'"\\s+serializeAs="String">\\s*'
            r"<value>(.*?)</value>",
            text,
            re.S,
        )
        if match:
            values[name] = match.group(1).strip()
    if not values:
        return

    users = BOTTLE / "drive_c/users"
    for user_config in sorted(
        users.glob(
            "*/AppData/Local/Britton_IT_Ltd/CrewChiefV4.exe_Url_*/*/user.config"
        )
    ):
        text = user_config.read_text(encoding="utf-8-sig")
        new_text = text
        for name, value in values.items():
            new_text, _ = set_xml_setting(new_text, name, value)
        if new_text == text:
            continue
        backup(user_config)
        write_atomic(user_config, new_text)
        CHANGES.append(
            f"CrewChief: user.config de {user_config.parent.name} "
            f"({', '.join(n for n in values if n != 'lmu_install_path')})"
        )

'''



# AMS2/ACEVO no tocan el perfil JSON: sin esto, el perfil puede reactivar el
# autodetectado y CrewChief vuelve a preguntar por la ruta del stub. Ademas,
# CrewChief solo ofrece los juegos que estan en "Limit available games": AC EVO
# no viene en la lista de perfiles antiguos y el juego no aparece en el menu.
PROFILE_HELPER = '''
def configure_crewchief_profile(game_id: str = "") -> None:
    """Deja el perfil listo para el juego: autodetectado, arranque y lista."""
    profiles = BOTTLE / "drive_c/users/steamuser/Documents/CrewChiefV4/Profiles"
    if not profiles.exists():
        return
    desired = {"enable_auto_detect": False, "run_immediately": True}
    if game_id:
        desired["last_game_definition"] = game_id
    for path in sorted(profiles.glob("*.json")):
        data = load_json(path, None)
        if not isinstance(data, dict):
            continue
        user_settings = data.get("userSettings")
        if not isinstance(user_settings, dict):
            continue
        changed = False
        for name, value in desired.items():
            if user_settings.get(name) != value:
                user_settings[name] = value
                changed = True
        if game_id:
            raw = str(user_settings.get("limit_available_games", "")).strip()
            available = [item.strip() for item in raw.split(",") if item.strip()]
            if len(available) == 1:
                # ETOL pudo convertir una lista vacia (todos) en una lista de
                # un solo juego; CrewChief solo mostraba ese juego.
                user_settings["limit_available_games"] = ""
                changed = True
            elif available and game_id not in available:
                available.append(game_id)
                user_settings["limit_available_games"] = ",".join(available)
                changed = True
        if changed:
            save_json(path, data)
            CHANGES.append(
                f"CrewChief: perfil {path.name} preparado para {game_id or 'el juego'}"
            )

'''


# El plugin LMU de SimHub sondea LMU por WebSocket; con AMS2/ACEVO llena el log
# de errores cada ~13 s. Se activa solo cuando se lanza LMU.
SIMHUB_PLUGIN_HELPER = '''
def configure_simhub_plugins(enable_lmu: bool) -> None:
    """Activa o desactiva el plugin LMU de SimHub.

    SimHub 9.x usa `doX.doX_LMU_SessionDataPlugin`; las versiones antiguas
    usaban `LMU_SessionDataPlugin.LMU_SessionDataPlugin`. Se activa el que
    exista, sin tocar los demas plugins del juego.
    """
    path = (
        BOTTLE
        / "drive_c/Program Files (x86)/SimHub/PluginsData/PluginsActivation.json"
    )
    if not path.exists():
        return
    data = load_json(path, None)
    if not isinstance(data, list):
        return
    primary = "doX.doX_LMU_SessionDataPlugin"
    legacy = "LMU_SessionDataPlugin.LMU_SessionDataPlugin"
    names = {
        entry.get("ClassName")
        for entry in data
        if isinstance(entry, dict)
    }
    target = primary if primary in names else legacy if legacy in names else ""
    if not target:
        return
    changed = False
    for entry in data:
        if not isinstance(entry, dict) or entry.get("ClassName") != target:
            continue
        if entry.get("IsEnabled") is not enable_lmu:
            entry["IsEnabled"] = enable_lmu
            changed = True
    if changed:
        save_json(path, data)
        estado = "activado" if enable_lmu else "desactivado"
        CHANGES.append(f"SimHub: {target.split('.')[-1]} {estado}")

'''

def patch_limit_helper(path: Path) -> tuple[int, bool]:
    """Inserta fix_limit_available_games() en configure_apps*.py."""
    text = path.read_text(encoding="utf-8")
    if "def fix_limit_available_games(" in text:
        return 0, True
    anchor = "\ndef configure_crewchief() -> None:"
    main_anchor = "\n\ndef main() -> int:"
    if anchor not in text or main_anchor not in text:
        return 0, False
    text = text.replace(anchor, LIMIT_HELPER + anchor, 1)
    text = text.replace(main_anchor, "\n    fix_limit_available_games()\n" + main_anchor, 1)
    path.write_text(text, encoding="utf-8")
    return 2, True


def patch_limit_available_games(path: Path) -> tuple[int, bool]:
    """Evita que una lista vacia se convierta en un solo juego en .config."""
    text = path.read_text(encoding="utf-8")
    import re

    match = re.search(r'"last_game_definition":\s*"([A-Z0-9_]+)"', text)
    game_id = match.group(1) if match else ""
    if not game_id:
        return 0, True
    old = (
        "    if m:\n"
        '        games = [x.strip() for x in m.group(2).split(",") if x.strip()]\n'
        f'        if "{game_id}" not in games:\n'
        f'            games.append("{game_id}")\n'
        '            text = text[:m.start(2)] + ",".join(games) + text[m.end(2):]\n'
    )
    new = (
        "    if m:\n"
        '        games = [x.strip() for x in m.group(2).split(",") if x.strip()]\n'
        f'        if games == ["{game_id}"]:\n'
        '            text = text[:m.start(2)] + "" + text[m.end(2):]\n'
        f'        elif games and "{game_id}" not in games:\n'
        f'            games.append("{game_id}")\n'
        '            text = text[:m.start(2)] + ",".join(games) + text[m.end(2):]\n'
    )
    if new in text:
        return 0, True
    if old not in text:
        return 0, False
    path.write_text(text.replace(old, new, 1), encoding="utf-8")
    return 1, True


def patch_configure_apps(path: Path) -> tuple[int, bool]:
    """Anade el ajuste del user.config a un configure_apps*.py."""
    text = path.read_text(encoding="utf-8")
    if "configure_crewchief_user_config" in text:
        return 0, True

    anchor = "\ndef configure_crewchief() -> None:"
    main_anchor = "\n\ndef main() -> int:"
    if anchor not in text or main_anchor not in text:
        return 0, False

    import re

    match = re.search(r'"last_game_definition":\s*"([A-Z0-9_]+)"', text)
    game_id = match.group(1) if match else ""

    profile_call = ""
    if "def configure_crewchief_profile(" not in text:
        text = text.replace(anchor, PROFILE_HELPER + anchor, 1)
        profile_call = f'\n    configure_crewchief_profile("{game_id}")\n'

    text = text.replace(anchor, USER_CONFIG_HELPER + anchor, 1)

    # El plugin LMU de SimHub solo se queda activo en el script de LMU.
    lmu = '"last_game_definition": "LMU"' in text
    if "def configure_simhub_plugins(" not in text:
        text = text.replace(anchor, SIMHUB_PLUGIN_HELPER + anchor, 1)
        text = text.replace(
            "    configure_simhub()\n",
            f"    configure_simhub()\n    configure_simhub_plugins({lmu})\n",
            1,
        )

    text = text.replace(
        main_anchor,
        profile_call + "\n    configure_crewchief_user_config()\n" + main_anchor,
        1,
    )
    path.write_text(text, encoding="utf-8")
    return 4 if profile_call else 3, True


def patch_lmu_crewchief_profile(path: Path) -> tuple[int, bool]:
    """Fija el juego LMU en el perfil de CrewChief.

    `configure_apps.py` (LMU) ajusta el .config y el user.config, pero el
    perfil tiene prioridad para `last_game_definition`. Sin esto CrewChief
    arranca con el juego del ultimo perfil guardado (p. ej. AMS2).
    """
    text = path.read_text(encoding="utf-8")
    if '"last_game_definition": "LMU"' not in text:
        return 0, True
    # Los scripts por juego ya fijan el juego en el perfil.
    if 'configure_crewchief_profile("' in text:
        return 0, True
    marker = (
        '    desired = {\n'
        '        "last_game_definition": "LMU",\n'
        '        "enable_lmu_rf2_plugin_dll_check": False,'
    )
    if marker in text:
        return 0, True

    old = '    desired = {\n        "enable_lmu_rf2_plugin_dll_check": False,'
    new = (
        '    desired = {\n'
        '        "last_game_definition": "LMU",\n'
        '        "enable_lmu_rf2_plugin_dll_check": False,'
    )
    if old not in text:
        return 0, False
    text = text.replace(old, new, 1)
    path.write_text(text, encoding="utf-8")
    return 1, True


def patch_launch_script(path: Path) -> tuple[int, bool]:
    """Evita que los hijos hereden el descriptor del lock de arranque.

    `arrancar_todo_*.sh` abre `arranque.lock` en el descriptor 9 con flock. Los
    procesos que lanza (puente, CrewChief, SimHub) heredaban ese descriptor y
    mantenian el lock para siempre: el siguiente "Lanzar" se cancelaba solo con
    "Ya hay un arranque en curso".

    Ademas, si el puente falla (por ejemplo, el juego se abrio con otro Proton y
    el wineserver del prefijo es de otra version), las apps deben abrirse igual:
    antes el script salia y CrewChief/SimHub no llegaban a arrancar.
    """
    import re

    text = path.read_text(encoding="utf-8")
    changes = 0

    for old, new in (
        ('"$ACTIVAR" "${ACTIVAR_ARGS[@]}" 2>&1 | tee',
         '"$ACTIVAR" "${ACTIVAR_ARGS[@]}" 9>&- 2>&1 | tee'),
        ("2>&1 </dev/null &", "2>&1 </dev/null 9>&- &"),
    ):
        if new in text:
            continue
        if old in text:
            changes += text.count(old)
            text = text.replace(old, new)

    # CrewChief carga en pocos segundos: 8 s entre apps es suficiente y ahorra
    # casi 20 s frente a los 25 s anteriores.
    text, n = re.subn(
        r'(ESPERA_APPS="\$\{[A-Z0-9_]+:-)25(\}")',
        r"\g<1>8\2",
        text,
    )
    changes += n

    # Un fallo del puente no debe impedir abrir CrewChief y SimHub.
    text, n = re.subn(        r'( *log "ERROR: activar_puente_[a-z0-9_]+\.sh fallo\. Revisa \$LOG"\n)'
        r' *exit 1\n',
        r'\1'
        r'    log "AVISO: sigo con CrewChief y SimHub; el juego se quedara sin datos del espejo."\n',
        text,
    )
    changes += n

    text, n = re.subn(
        r'( *log ")ERROR(: el receptor no esta vivo; aborto para no abrir apps sin datos\.)'
        r'("\n) *exit 1\n',
        r'\1AVISO\2\3'
        r'    log "AVISO: abro las apps igualmente; el espejo se puede recuperar despues."\n',
        text,
    )
    changes += n

    # Envoltorios 'start /wait' de una sesion anterior cuya app ya no existe
    # (o que es mas antigua que la instancia viva de la app).
    cleanup = (
        "# Limpia envoltorios 'start /wait' de sesiones anteriores.\n"
        "limpiar_envoltorios() {\n"
        "  local pid cmd app newest wetime aetime\n"
        "  for pid in $(pgrep -f -- 'start.exe' 2>/dev/null || true); do\n"
        '    [[ "$pid" == "$$" ]] && continue\n'
        '    cmd="$(tr \'\\0\' \' \' < "/proc/$pid/cmdline" 2>/dev/null || true)"\n'
        '    case "$cmd" in\n'
        '      "start.exe "*|"C:\\\\windows\\\\system32\\\\start.exe "*) ;;\n'
        "      *) continue ;;\n"
        "    esac\n"
        '    case "$cmd" in\n'
        "      *CrewChiefV4*) app=CrewChiefV4.exe ;;\n"
        "      *SimHub*) app=SimHubWPF.exe ;;\n"
        "      *) continue ;;\n"
        "    esac\n"
        '    newest="$(pgrep -n -x "$app" 2>/dev/null || true)"\n'
        '    if [[ -z "$newest" ]]; then\n'
        '      kill "$pid" 2>/dev/null || true\n'
        "      continue\n"
        "    fi\n"
        '    wetime="$(ps -o etimes= -p "$pid" 2>/dev/null | tr -d \' \')"\n'
        '    aetime="$(ps -o etimes= -p "$newest" 2>/dev/null | tr -d \' \')"\n'
        '    if [[ -n "$wetime" && -n "$aetime" ]] && (( wetime > aetime + 120 )); then\n'
        '      kill "$pid" 2>/dev/null || true\n'
        "    fi\n"
        "  done\n"
        "}\n\n"
    )
    if "limpiar_envoltorios()" not in text:
        anchor = "receiver_alive() {"
        if anchor in text:
            text = text.replace(anchor, cleanup + anchor, 1)
            changes += 1

    if "limpiar_envoltorios\n" not in text:
        marker = 'log "=== arranque $(date \'+%F %T\') ==="'
        if marker in text:
            text = text.replace(marker, marker + "\nlimpiar_envoltorios", 1)
            changes += 1

    if changes:
        path.write_text(text, encoding="utf-8")
    return changes, True


def patch_bridge_script(path: Path) -> tuple[int, bool]:
    """Explica el fallo tipico cuando el juego usa otro Proton.

    Si el juego se abrio con un Proton distinto al configurado, en el prefijo
    del juego queda un wineserver de otra version. El emisor del espejo intenta
    hablar con el y Wine responde `version mismatch`; el script solo mostraba
    esa linea cruda del log.
    """
    text = path.read_text(encoding="utf-8")
    if "version mismatch" in text:
        return 0, True

    old = '    tail -n 20 "$logfile" >&2 || true\n'
    if old not in text:
        return 0, False

    new = (
        old
        + '    if grep -qi "version mismatch" "$logfile" 2>/dev/null; then\n'
        + '      echo "PISTA: el juego se abrio con otro Proton. Su wineserver no es compatible con el de Bottles." >&2\n'
        + '      echo "       Cierra el juego, abrelo con el Proton configurado en ETOL y vuelve a lanzar." >&2\n'
        + "    fi\n"
    )
    path.write_text(text.replace(old, new, 1), encoding="utf-8")
    return 1, True


def patch_activation_script(path: Path) -> tuple[int, bool]:
    """Anade `--solo-config` a activar_puente_*.sh.

    La ficha de cada juego en ETOL aplica solo la configuracion (SimHub,
    CrewChief, plugins) sin arrancar el puente ni las apps. Para eso el script
    de activacion acepta `--solo-config` y sale tras el bloque de configuracion.
    """
    import re

    text = path.read_text(encoding="utf-8")
    if "--solo-config" in text:
        return 0, True

    changes = 0
    old = "DO_CONFIG=1\n"
    if old in text:
        text = text.replace(old, "DO_CONFIG=1\nSOLO_CONFIG=0\n", 1)
        changes += 1

    old = "    --no-config) DO_CONFIG=0; shift ;;\n"
    if old in text:
        text = text.replace(
            old,
            old + "    --solo-config) SOLO_CONFIG=1; shift ;;\n",
            1,
        )
        changes += 1

    match = re.search(r"\nif real_[a-z0-9_]+_running; then", text)
    if match:
        insert = (
            '\nif [[ "${SOLO_CONFIG:-0}" == 1 ]]; then\n'
            '  echo "Configuracion aplicada."\n'
            "  exit 0\n"
            "fi\n"
        )
        text = text[: match.start()] + insert + text[match.start() :]
        changes += 1

    if changes:
        path.write_text(text, encoding="utf-8")
    return changes, True


def patch_lmu_raw_sender(path: Path) -> tuple[int, bool]:
    """Arranca el emisor crudo junto al emisor clasico."""
    text = path.read_text(encoding="utf-8")
    if "lmu_shm_sender.exe" in text:
        return 0, True
    changes = 0
    old = 'EXE="$BRIDGE/lmu_shm_mirror.exe"\n'
    if old in text:
        text = text.replace(
            old,
            old + 'SENDER_EXE="$BRIDGE/lmu_shm_sender.exe"\n',
            1,
        )
        changes += 1
    marker = 'start_process sender "$LMU_PFX" "$LMU_WINE" "${SENDER_ARGS[@]}" || fail "no se pudo arrancar el emisor"\n'
    if marker in text:
        block = '''start_raw_sender() {
  local pidfile="$RUN_DIR/raw.pid"
  if pid_alive "$pidfile"; then
    echo "emisor crudo ya esta en marcha (PID $(cat "$pidfile"))."
    return 0
  fi
  rm -f "$pidfile"
  if [[ ! -x "$SENDER_EXE" ]]; then
    echo "AVISO: falta $SENDER_EXE; se usara solo el emisor clasico." >&2
    return 0
  fi
  setsid env WINEPREFIX="$LMU_PFX" WINEDEBUG=-all "$LMU_WINE" "$SENDER_EXE" \\
    --sender --udp "127.0.0.1:$PORT" --hz "$HZ" --smmp-hz "$SMMP_HZ" \\
    >"$RUN_DIR/raw.log" 2>&1 < /dev/null &
  echo "$!" > "$pidfile"
  echo "emisor crudo arrancado (PID $!, log $RUN_DIR/raw.log)."
}

'''
        text = text.replace(marker, block + marker + "start_raw_sender\n", 1)
        changes += 1
    if changes:
        path.write_text(text, encoding="utf-8")
    return changes, True


def patch_lmu_watchdog(path: Path) -> tuple[int, bool]:
    """El vigilante tambien relanza el emisor crudo."""
    text = path.read_text(encoding="utf-8")
    if "lmu_shm_sender.exe" in text:
        return 0, True
    changes = 0
    old = 'EXE="$HERE/lmu_shm_mirror.exe"\n'
    if old in text:
        text = text.replace(old, old + 'RAW_EXE="$HERE/lmu_shm_sender.exe"\n', 1)
        changes += 1
    marker = '''  if [[ -z "$(mirror_pid sender)" ]]; then
    start_mirror sender "$LMU_PFX" "$LMU_WINE" \\
      --sender --udp "127.0.0.1:$PORT" --hz "$HZ" --smmp-hz "$SMMP_HZ"
    sleep 1
  fi
'''
    add = marker + '''  if [[ -x "$RAW_EXE" ]] && ! pgrep -f -- 'lmu_shm_sender\\.exe' >/dev/null 2>&1; then
    setsid env WINEPREFIX="$LMU_PFX" WINEDEBUG=-all "$LMU_WINE" "$RAW_EXE" \\
      --sender --udp "127.0.0.1:$PORT" --hz "$HZ" --smmp-hz "$SMMP_HZ" \\
      >>"$RUN_DIR/raw.log" 2>&1 </dev/null &
    echo "$!" >"$RUN_DIR/raw.pid"
    log "emisor crudo relanzado (launcher PID $!)."
    sleep 1
  fi
'''
    if marker in text:
        text = text.replace(marker, add, 1)
        changes += 1
    if changes:
        path.write_text(text, encoding="utf-8")
    return changes, True


def patch_lmu_stop(path: Path) -> tuple[int, bool]:
    """El script de parada tambien termina el emisor crudo."""
    text = path.read_text(encoding="utf-8")
    if "lmu_shm_sender" in text:
        return 0, True
    changes = 0
    old = """  tr '\\0' ' ' < "/proc/$pid/cmdline" | grep -q 'lmu_shm_mirror\\.exe'"""
    new = """  tr '\\0' ' ' < "/proc/$pid/cmdline" | grep -qE 'lmu_shm_mirror\\.exe|lmu_shm_sender\\.exe'"""
    if old in text:
        text = text.replace(old, new, 1)
        changes += 1
    old = "stop_pidfile sender\n"
    if old in text:
        text = text.replace(old, old + "stop_pidfile raw\n", 1)
        changes += 1
    old = """if pgrep -f -- 'lmu_shm_mirror\\.exe' >/dev/null 2>&1; then"""
    if old in text:
        text = text.replace(
            old,
            """if pgrep -f -- 'lmu_shm_mirror\\.exe|lmu_shm_sender\\.exe' >/dev/null 2>&1; then""",
            1,
        )
        changes += 1
    if changes:
        path.write_text(text, encoding="utf-8")
    return changes, True


def patch_lmu_game_config(path: Path) -> tuple[int, bool]:
    """Activa el plugin SMMP en el juego antes de salir con --solo-config.

    SimHub solo da LMU por configurado si `CustomPluginVariables.JSON` tiene el
    plugin activo. CrewChief usa la misma carpeta para comprobar el DLL.
    """
    text = path.read_text(encoding="utf-8")
    if "ensure_lmu_plugin.py" in text:
        return 0, True
    marker = 'if [[ "${SOLO_CONFIG:-0}" == 1 ]]; then'
    if marker not in text:
        return 0, False
    block = (
        "# SimHub solo da LMU por configurado si el plugin SMMP esta activo en\n"
        "# UserData/player/CustomPluginVariables.JSON; CrewChief ademas lo busca\n"
        "# en Plugins/. Se hace con copia de seguridad y sin tocar el resto.\n"
        'if [[ -n "$LMU_GAME_DIR" && -d "$LMU_GAME_DIR" ]]; then\n'
        '  # SimHub valida el MD5 contra su copia de _Addons; se prefiere esa.\n'
        '  LMU_PLUGIN_SRC="$BOTTLE/drive_c/Program Files (x86)/SimHub/_Addons/GamePlugins/RFactor2/Bin64/Plugins/rFactor2SharedMemoryMapPlugin64.dll"\n'
        '  [[ -f "$LMU_PLUGIN_SRC" ]] || LMU_PLUGIN_SRC="$BRIDGE/rFactor2SharedMemoryMapPlugin64.dll"\n'
        '  python3 "$BRIDGE/ensure_lmu_plugin.py" "$LMU_GAME_DIR" "$LMU_PLUGIN_SRC" \\\n'
        '    "$BOTTLE/drive_c/Program Files (x86)/SimHub" || \\\n'
        '    echo "AVISO: no se pudo activar el plugin SMMP en LMU." >&2\n'
        "fi\n\n"
    )
    text = text.replace(marker, block + marker, 1)
    path.write_text(text, encoding="utf-8")
    return 1, True


def patch_hf8_activation(path: Path) -> tuple[int, bool]:
    """Configura el HF8 al activar el puente, con el wineserver ya vivo."""
    text = path.read_text(encoding="utf-8")
    if "HF8_SETUP" in text:
        return 0, True

    changes = 0
    if "DO_HF8=1" not in text:
        old = "DO_CONFIG=1\n"
        if old in text:
            text = text.replace(old, "DO_CONFIG=1\nDO_HF8=1\n", 1)
            changes += 1

    if "--no-hf8)" not in text:
        old = "    --no-config) DO_CONFIG=0; shift ;;\n"
        if old in text:
            text = text.replace(
                old,
                old + "    --no-hf8) DO_HF8=0; shift ;;\n",
                1,
            )
            changes += 1

    marker = "start_process sender"
    block = (
        "# El HF8 necesita el wineserver de Bottles vivo; el receptor ya lo arranco.\n"
        'if [[ "$DO_HF8" == 1 ]]; then\n'
        '  HF8_SETUP="$BRIDGE/setup_hf8.sh"\n'
        '  if [[ -x "$HF8_SETUP" ]]; then\n'
        '    echo "Configurando pad haptico HF8 (ForceFeel)..."\n'
        '    SIMHUB_BOTTLE="$BOTTLE" BOTTLE_WINE="$BOTTLE_WINE" \\\n'
        '      LMU_MIRROR_RUN_DIR="$RUN_DIR" "$HF8_SETUP" || \\\n'
        '      echo "AVISO: no se pudo configurar el HF8; SimHub seguira sin el pad." >&2\n'
        "  fi\n"
        "fi\n\n"
    )
    if marker in text:
        text = text.replace(marker, block + marker, 1)
        changes += 1

    if changes:
        path.write_text(text, encoding="utf-8")
    return changes, True


def patch_hf8_launch(path: Path) -> tuple[int, bool]:
    """Pasa --no-hf8 al activador y evita que el lanzador lo rechace."""
    text = path.read_text(encoding="utf-8")
    if "DO_HF8=1" in text:
        return 0, True

    changes = 0
    old = "DO_CONFIG=1\n"
    if old in text:
        text = text.replace(old, "DO_CONFIG=1\nDO_HF8=1\n", 1)
        changes += 1

    old = "    --no-config) DO_CONFIG=0; shift ;;\n"
    if old in text:
        text = text.replace(
            old,
            old + "    --no-hf8) DO_HF8=0; shift ;;\n",
            1,
        )
        changes += 1

    old = "ACTIVAR_ARGS=()\n"
    if old in text:
        text = text.replace(
            old,
            old + '[[ "$DO_HF8" == 1 ]] || ACTIVAR_ARGS+=(--no-hf8)\n',
            1,
        )
        changes += 1

    if changes:
        path.write_text(text, encoding="utf-8")
    return changes, True


def patch_hf8_setup(path: Path) -> tuple[int, bool]:
    """Evita que wineboot reasigne el COM del HF8 al arrancar el prefijo.

    Wine regenera ``dosdevices`` cuando inicializa el prefijo y asigna los COM
    por orden de deteccion (p. ej. COM33 a un adaptador USB-serie y COM34 al
    pad). El enlace manual se hacia antes de esa inicializacion, asi que
    quedaba pisado y SimHub abria el puerto equivocado.
    """
    text = path.read_text(encoding="utf-8")
    if "HF8_LINK_LATE" in text:
        return 0, True

    old = (
        "# 2. Enlace DOS. El puerto elegido debe ser uno de los que Wine enumera.\n"
        'COM_LOWER="${COM,,}"\n'
        'ln -sfn "$TTY" "$BOTTLE/dosdevices/$COM_LOWER"\n'
        'say "Enlace $BOTTLE/dosdevices/$COM_LOWER -> $TTY"\n'
        "\n"
        'export WINEPREFIX="$BOTTLE"\n'
        "export WINEDEBUG=-all\n"
    )
    new = (
        "# 2. Enlace DOS. El puerto elegido debe ser uno de los que Wine enumera.\n"
        'COM_LOWER="${COM,,}"\n'
        "\n"
        'export WINEPREFIX="$BOTTLE"\n'
        "export WINEDEBUG=-all\n"
        "\n"
        "# 2a. Wine regenera dosdevices al inicializar el prefijo y puede\n"
        "# asignar el COM elegido a otro tty. Se despierta el prefijo ANTES de\n"
        "# enlazar y se deja el wineserver vivo para que el siguiente arranque\n"
        "# no vuelva a pisar el enlace.\n"
        "HF8_LINK_LATE=1\n"
        '"$WINE" reg query \'HKLM\\HARDWARE\\DEVICEMAP\\SERIALCOMM\' >/dev/null 2>&1 || true\n'
        'WINESERVER="$(dirname "$WINE")/wineserver"\n'
        'if [[ -x "$WINESERVER" ]]; then\n'
        '  "$WINESERVER" -p >/dev/null 2>&1 || true\n'
        "fi\n"
        "\n"
        'ln -sfn "$TTY" "$BOTTLE/dosdevices/$COM_LOWER"\n'
        'say "Enlace $BOTTLE/dosdevices/$COM_LOWER -> $TTY"\n'
    )
    if old not in text:
        return 0, False
    text = text.replace(old, new, 1)

    # Antes del resumen final: repetir el enlace y comprobarlo.
    marker = (
        'if [[ "$ok" == 1 ]]; then\n'
        '  say "HF8 listo para SimHub: $COM -> $TTY (DEVPKEY_NAME tipo 0x12)."\n'
    )
    tail = (
        'ln -sfn "$TTY" "$BOTTLE/dosdevices/$COM_LOWER"\n'
        'if [[ "$(readlink -f "$BOTTLE/dosdevices/$COM_LOWER")" != "$(readlink -f "$TTY")" ]]; then\n'
        '  warn "el enlace $COM no apunta a $TTY"\n'
        "  ok=0\n"
        "fi\n"
        "\n"
    )
    if marker not in text:
        return 0, False
    text = text.replace(marker, tail + marker, 1)

    path.write_text(text, encoding="utf-8")
    return 2, True


def patch_text(text: str) -> tuple[str, int]:
    changes = 0
    for old, new in REPLACEMENTS:
        if old in text:
            changes += text.count(old)
            text = text.replace(old, new)

    # Rutas del entorno de desarrollo.
    if LEGACY_RUN in text:
        changes += text.count(LEGACY_RUN)
        text = text.replace(LEGACY_RUN, "/tmp/etol")

    if "/home/barbas/Escritorio/SIMHUB_LINUX/Simhub_Linux_APP" in text:
        changes += text.count("/home/barbas/Escritorio/SIMHUB_LINUX/Simhub_Linux_APP")
        text = text.replace(
            "/home/barbas/Escritorio/SIMHUB_LINUX/Simhub_Linux_APP",
            "$HOME/.config/etol/payload",
        )

    if "/home/barbas" in text:
        changes += text.count("/home/barbas")
        text = text.replace("/home/barbas", "$HOME")

    return text, changes


def main() -> int:
    if len(sys.argv) != 2:
        print("uso: parchear-payload.py <carpeta-payload>", file=sys.stderr)
        return 2

    dest = Path(sys.argv[1])
    if not dest.is_dir():
        print(f"ERROR: no existe {dest}", file=sys.stderr)
        return 2

    touched = 0
    changes = 0
    for path in sorted(dest.rglob("*")):
        if not path.is_file() or path.suffix not in TEXT_EXT:
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue
        new, count = patch_text(text)
        if count:
            path.write_text(new, encoding="utf-8")
            touched += 1
            changes += count

    # Los hijos del arranque no deben heredar el lock.
    for path in sorted(dest.glob("arrancar_todo_*.sh")):
        count, ok = patch_launch_script(path)
        if not ok:
            print(f"ERROR: no pude parchear {path}", file=sys.stderr)
            return 1
        if count:
            touched += 1
            changes += count
        count, ok = patch_hf8_launch(path)
        if not ok:
            print(f"ERROR: no pude parchear {path}", file=sys.stderr)
            return 1
        if count:
            touched += 1
            changes += count

    # Aviso claro cuando el juego se abrio con otro Proton.
    for path in sorted(dest.glob("activar_puente_*.sh")):
        if path.name in ("activar_puente_dirt2.sh", "activar_puente_ets2.sh", "activar_puente_beamng.sh", "activar_puente_r3e.sh"):
            # dirt2 reparte UDP y ets2 lee /dev/shm: no usan shared memory de
            # Wine, asi que el aviso de "otro Proton" no aplica.
            continue
        count, ok = patch_bridge_script(path)
        if not ok:
            print(f"ERROR: no pude parchear {path}", file=sys.stderr)
            return 1
        if count:
            touched += 1
            changes += count

    # Las fichas de juego aplican la configuracion sin arrancar el puente.
    for path in sorted(dest.glob("activar_puente_*.sh")):
        count, ok = patch_activation_script(path)
        if not ok:
            print(f"ERROR: no pude parchear {path}", file=sys.stderr)
            return 1
        if count:
            touched += 1
            changes += count
        if path.name == "activar_puente_r3e.sh":
            # R3E no configura el HF8 en el puente; lo hace SimHub.
            continue
        count, ok = patch_hf8_activation(path)
        if not ok:
            print(f"ERROR: no pude parchear {path}", file=sys.stderr)
            return 1
        if count:
            touched += 1
            changes += count

    # LMU: el plugin SMMP tiene que estar activo en el juego para SimHub y
    # CrewChief. Se hace en la copia del payload, nunca en el juego original.
    lmu_activation = dest / "activar_puente_lmu.sh"
    if lmu_activation.is_file():
        count, ok = patch_lmu_game_config(lmu_activation)
        if not ok:
            print(f"ERROR: no pude parchear {lmu_activation}", file=sys.stderr)
            return 1
        if count:
            touched += 1
            changes += count
        count, ok = patch_lmu_raw_sender(lmu_activation)
        if not ok:
            print(f"ERROR: no pude parchear el emisor crudo en {lmu_activation}", file=sys.stderr)
            return 1
        if count:
            touched += 1
            changes += count

    lmu_watchdog = dest / "bridge" / "lmu_bridge_watchdog.sh"
    if lmu_watchdog.is_file():
        count, ok = patch_lmu_watchdog(lmu_watchdog)
        if not ok:
            print(f"ERROR: no pude parchear {lmu_watchdog}", file=sys.stderr)
            return 1
        if count:
            touched += 1
            changes += count

    lmu_stop = dest / "desactivar_puente_lmu.sh"
    if lmu_stop.is_file():
        count, ok = patch_lmu_stop(lmu_stop)
        if not ok:
            print(f"ERROR: no pude parchear {lmu_stop}", file=sys.stderr)
            return 1
        if count:
            touched += 1
            changes += count

    # El pad HF8: el enlace del COM debe sobrevivir al arranque de Wine.
    for path in sorted(dest.rglob("setup_hf8.sh")):
        count, ok = patch_hf8_setup(path)
        if not ok:
            print(f"ERROR: no pude parchear {path}", file=sys.stderr)
            return 1
        if count:
            touched += 1
            changes += count

    # CrewChief: el juego activo tambien tiene que quedar en el user.config.
    for path in sorted(dest.rglob("configure_apps*.py")):
        if path.name in ("configure_apps_dirt2.py", "configure_apps_ets2.py", "configure_apps_beamng.py"):
            # Estos scripts no tocan CrewChief: dirt2 y ets2 no lo usan.
            continue
        count, ok = patch_configure_apps(path)
        if not ok:
            print(f"ERROR: no pude parchear {path}", file=sys.stderr)
            return 1
        if count:
            touched += 1
            changes += count
        count, ok = patch_lmu_crewchief_profile(path)
        if not ok:
            print(f"ERROR: no pude parchear el perfil LMU de {path}", file=sys.stderr)
            return 1
        if count:
            touched += 1
            changes += count
        count, ok = patch_limit_helper(path)
        if not ok:
            print(f"ERROR: no pude parchear la lista de juegos de {path}", file=sys.stderr)
            return 1
        if count:
            touched += 1
            changes += count
        count, ok = patch_limit_available_games(path)
        if not ok:
            print(f"ERROR: no pude parchear limit_available_games de {path}", file=sys.stderr)
            return 1
        if count:
            touched += 1
            changes += count

    # El payload no puede llevar el nombre del entorno de desarrollo.
    leftovers = []
    for path in sorted(dest.rglob("*")):
        if not path.is_file() or path.suffix not in TEXT_EXT:
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue
        if LEGACY_MARKER in text:
            leftovers.append(path)

    print(f"payload parcheado: {touched} ficheros, {changes} sustituciones")
    if leftovers:
        for path in leftovers:
            print(f"ERROR: queda la marca de desarrollo en {path}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
