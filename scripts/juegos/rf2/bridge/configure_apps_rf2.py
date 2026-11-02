#!/usr/bin/env python3
"""Configura SimHub y CrewChief para consumir el espejo de rFactor 2.

- SimHub: rFactor 2 visible y como juego preferente, lector por memoria
  compartida (CurrentReader null).
- CrewChief: juego activo RF2, ruta de instalacion en el espejo y
  RF2 en la lista de juegos disponibles.

No toca ninguna opcion de LMU/AMS2/AC EVO, no arranca ni reinicia aplicaciones
y no modifica archivos de una app que este abierta (esa app los reescribiria al
cerrarse). Hace copia de seguridad antes de cada cambio.
"""

from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import sys
from datetime import datetime
from pathlib import Path
from xml.sax.saxutils import escape

HOME = Path.home()
BOTTLE = Path(os.environ.get(
    "SIMHUB_BOTTLE",
    HOME / ".local/share/bottles/bottles/simhub",
)).expanduser()
BACKUP_DIR = Path(os.environ.get(
    "RF2_MIRROR_BACKUP_DIR",
    HOME / ".local/share/rf2_mirror/backups",
)).expanduser()
RUN_DIR = os.environ.get("RF2_MIRROR_RUN_DIR", "/tmp/etol/rf2_mirror")
STAMP = datetime.now().strftime("%Y%m%d-%H%M%S")
CHANGES: list[str] = []
WARNINGS: list[str] = []


def app_running(exe_name: str) -> bool:
    """True si el .exe real de la app corre en algun wineserver."""
    needle = exe_name.encode()
    proc = Path("/proc")
    try:
        entries = list(proc.iterdir())
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
        if any(marker in cmd for marker in (
            b"start.exe", b"wine", b"pv-adverb", b"pressure-vessel",
            b"proton", b"reaper", b"steamlaunch",
        )):
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
    tmp = path.with_name(path.name + ".tmp-rf2-mirror")
    tmp.write_text(text, encoding="utf-8", newline="")
    os.replace(tmp, path)


def load_json(path: Path, default):
    if not path.exists():
        return default
    try:
        with path.open("r", encoding="utf-8-sig") as fh:
            return json.load(fh)
    except Exception as exc:  # noqa: BLE001
        print(f"AVISO: no se pudo leer {path}: {exc}", file=sys.stderr)
        return default


def save_json(path: Path, data) -> None:
    backup(path)
    write_atomic(path, json.dumps(data, indent=2, ensure_ascii=False) + "\n")


def configure_simhub() -> None:
    simhub_running = app_running("SimHubWPF.exe")
    plugins = BOTTLE / "drive_c/Program Files (x86)/SimHub/PluginsData"

    hidden_path = plugins / "HiddenGames.json"
    hidden = load_json(hidden_path, {})
    if hidden.get("RFactor2") is not False:
        if simhub_running:
            WARNINGS.append(
                "SimHub esta abierto: no toco HiddenGames.json; "
                "rFactor 2 deberia estar visible."
            )
        else:
            hidden["RFactor2"] = False
            save_json(hidden_path, hidden)
            CHANGES.append("SimHub: rFactor 2 visible en HiddenGames.json")
    else:
        CHANGES.append("SimHub: rFactor 2 ya estaba visible")

    game_path = plugins / "GameSettings.json"
    games = load_json(game_path, {})
    entry = games.get("RFactor2")
    if not isinstance(entry, dict):
        if simhub_running:
            WARNINGS.append(
                "SimHub esta abierto y no tiene entrada RFactor2; "
                "se creara en el proximo arranque con --no-config desactivado."
            )
        else:
            games["RFactor2"] = {
                "UDPPort": None,
                "UDPForwardPort": None,
                "UDPForwardIpAddress": "127.0.0.1",
                "TyreWearMinimum": None,
                "UDPForwardActive": False,
                "DisableConfigAlert": False,
                "OrderEx": 1,
                "LauncherSetting": None,
                "CurrentReader": None,
                "ManualConfigurationDismissed": False,
                "AddictionnalUDPRedirects": [],
                "AutomaticConfigurationDismissed": False,
                "TargetIP": None,
                "LastActivations": [],
            }
            save_json(game_path, games)
            CHANGES.append("SimHub: entrada RFactor2 creada")
    else:
        changed = False
        if entry.get("OrderEx") != 1:
            if simhub_running:
                WARNINGS.append(
                    "SimHub esta abierto: no cambio OrderEx; selecciona "
                    "rFactor 2 manualmente si hace falta."
                )
            else:
                entry["OrderEx"] = 1
                changed = True
        if entry.get("CurrentReader") is not None:
            WARNINGS.append(
                "SimHub tiene un lector explicito para RFactor2; "
                "el espejo usa memoria compartida (rF2SMMP_*)."
            )
        if changed:
            save_json(game_path, games)
            CHANGES.append("SimHub: rFactor 2 marcado como preferente (OrderEx=1)")
        else:
            CHANGES.append("SimHub: rFactor 2 ya estaba configurado")


def set_xml_setting(text: str, name: str, value: str) -> tuple[str, bool]:
    pattern = re.compile(
        r'(<setting\s+name="' + re.escape(name) +
        r'"\s+serializeAs="String">\s*)'
        r'(<value\s*/>|<value>.*?</value>)',
        re.S,
    )
    replacement = r'\1<value>' + escape(value) + r'</value>'
    new_text, count = pattern.subn(replacement, text, count=1)
    if count:
        return new_text, True

    marker = "    </CrewChiefV4.Properties.Settings>"
    if marker not in new_text:
        return new_text, False
    block = (
        f'      <setting name="{name}" serializeAs="String">\n'
        f'        <value>{escape(value)}</value>\n'
        f'      </setting>\n'
    )
    return new_text.replace(marker, block + marker, 1), True


def configure_crewchief() -> None:
    exe_dir = BOTTLE / "drive_c/Program Files (x86)/Britton IT Ltd/CrewChiefV4"
    config_path = exe_dir / "CrewChiefV4.exe.config"
    if not config_path.exists():
        WARNINGS.append(f"no existe {config_path}")
        return

    if app_running("CrewChiefV4.exe"):
        WARNINGS.append(
            "CrewChief esta abierto: no toco su configuracion (la reescribe al "
            "cerrarse). Si no esta ya en rFactor 2, cambialo en la app "
            "cuando no estes en pista."
        )
        return

    text = config_path.read_text(encoding="utf-8-sig")
    original = text
    desired = {
        "last_game_definition": "RF2",
        "rf2_install_path": "Z:" + RUN_DIR.replace("/", "\\"),
        "enable_auto_detect": "False",
        "run_immediately": "True",
        "use_legacy_rf2_shared_memory_filenames": "True",
        "enable_lmu_rf2_plugin_dll_check": "False",
    }
    for name, value in desired.items():
        text, ok = set_xml_setting(text, name, value)
        if not ok:
            WARNINGS.append(f"no se pudo ajustar {name}")

    m = re.search(
        r'(<setting\s+name="limit_available_games"\s+serializeAs="String">\s*'
        r'<value>)(.*?)(</value>)',
        text,
        re.S,
    )
    if m:
        games = [x.strip() for x in m.group(2).split(",") if x.strip()]
        if "RF2" not in games:
            games.append("RF2")
            text = text[:m.start(2)] + ",".join(games) + text[m.end(2):]

    if text != original:
        backup(config_path)
        write_atomic(config_path, text)
        CHANGES.append("CrewChief: juego activo RF2")
    else:
        CHANGES.append("CrewChief: ya estaba configurado para RF2")


def configure_crewchief_profile_extra() -> None:
    """Deja la ruta de AC y RF2 tambien en el perfil JSON.

    CrewChief lee el perfil con prioridad sobre el .config. Sin
    rf2_install_path en el perfil, busca el juego real y muestra el aviso
    "Plugin Needed" que bloquea el arranque.
    """
    profiles = BOTTLE / "drive_c/users/steamuser/Documents/CrewChiefV4/Profiles"
    if not profiles.exists():
        return
    if app_running("CrewChiefV4.exe"):
        WARNINGS.append(
            "CrewChief esta abierto: no toco su perfil; si pide el plugin, "
            "cierra CrewChief y vuelve a lanzar."
        )
        return
    install = "Z:" + RUN_DIR.replace("/", "\\")
    for path in sorted(profiles.glob("*.json")):
        data = load_json(path, None)
        if not isinstance(data, dict):
            continue
        user_settings = data.get("userSettings")
        if not isinstance(user_settings, dict):
            continue
        changed = False
        if user_settings.get("rf2_install_path") != install:
            user_settings["rf2_install_path"] = install
            changed = True
        if user_settings.get("use_legacy_rf2_shared_memory_filenames") is not True:
            user_settings["use_legacy_rf2_shared_memory_filenames"] = True
            changed = True
        if user_settings.get("enable_lmu_rf2_plugin_dll_check") is not False:
            user_settings["enable_lmu_rf2_plugin_dll_check"] = False
            changed = True
        games = [
            item.strip()
            for item in str(user_settings.get("limit_available_games", "")).split(",")
            if item.strip()
        ]
        if "RF2" not in games:
            games.append("RF2")
            user_settings["limit_available_games"] = ",".join(games)
            changed = True
        if changed:
            save_json(path, data)
            CHANGES.append(
                f"CrewChief: perfil {path.name} con rf2_install_path y RF2"
            )


def configure_simhub_steam_path() -> None:
    """Hace que SimHub encuentre la instalacion real de rFactor 2.

    SimHub escanea Steam para localizar el juego. En la botella no hay Steam,
    asi que se crea/actualiza un `libraryfolders.vdf` con la biblioteca real y
    se marca la app 365960 como instalada. Se conservan las bibliotecas que ya
    estuvieran anadidas (por ejemplo la de Assetto Corsa).
    """
    pfx = os.environ.get("RF2_PFX", "").strip()
    wine = os.environ.get("BOTTLE_WINE", "").strip()
    if not pfx or not wine:
        return
    marker = "/steamapps/compatdata/"
    if marker not in pfx:
        return
    library = pfx.split(marker, 1)[0]
    if not Path(library).is_dir():
        return
    winlib = "Z:" + library.replace("/", "\\\\")

    steam = BOTTLE / "drive_c/Program Files (x86)/Steam"
    apps = steam / "steamapps"
    apps.mkdir(parents=True, exist_ok=True)
    vdf_path = apps / "libraryfolders.vdf"

    paths: list[str] = []
    if vdf_path.exists():
        for line in vdf_path.read_text(encoding="utf-8", errors="ignore").splitlines():
            match = re.search(r'"path"\s+"([^"]+)"', line, re.I)
            if match and match.group(1) not in paths:
                paths.append(match.group(1))
    for candidate in (r"C:\\Program Files (x86)\\Steam", winlib):
        if candidate not in paths:
            paths.append(candidate)

    body = ['"libraryfolders"', "{"]
    for index, path in enumerate(paths):
        body.append(f'\t"{index}"')
        body.append("\t{")
        body.append(f'\t\t"path"\t\t"{path}"')
        body.append("\t}")
    body.append("}")
    vdf = "\n".join(body) + "\n"
    if not vdf_path.exists() or vdf_path.read_text(encoding="utf-8") != vdf:
        vdf_path.write_text(vdf, encoding="utf-8")
        CHANGES.append("SimHub: biblioteca de Steam con rFactor 2")

    env = dict(os.environ, WINEPREFIX=str(BOTTLE), WINEDEBUG="-all")
    for name, value, kind in (
        ("Installed", "1", "REG_DWORD"),
        ("Name", "rFactor 2", "REG_SZ"),
    ):
        subprocess.run(
            [
                wine, "reg", "add", r"HKCU\Software\Valve\Steam\Apps\365960",
                "/v", name, "/t", kind, "/d", value, "/f",
            ],
            env=env,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )

    # Los ficheros de configuracion del juego viven en su prefijo; SimHub los
    # busca en Documentos de la botella.
    docs = BOTTLE / "drive_c/users/steamuser/Documents/rFactor 2"
    real_docs = Path(pfx) / "drive_c/users/steamuser/Documents/rFactor 2"
    if real_docs.exists() and not docs.exists():
        docs.parent.mkdir(parents=True, exist_ok=True)
        try:
            docs.symlink_to(real_docs)
        except OSError:
            pass


def main() -> int:
    if not BOTTLE.exists():
        print(f"ERROR: no existe la botella: {BOTTLE}", file=sys.stderr)
        return 1
    configure_simhub()
    configure_simhub_steam_path()
    configure_crewchief()
    configure_crewchief_profile_extra()

    if CHANGES:
        print("Configuracion aplicada:")
        for item in CHANGES:
            print(f"  - {item}")
    else:
        print("No habia nada que cambiar.")
    if WARNINGS:
        print("Avisos:")
        for item in WARNINGS:
            print(f"  - {item}")
    print(f"Copias de seguridad: {BACKUP_DIR}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
