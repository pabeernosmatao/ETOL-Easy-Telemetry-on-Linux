#!/usr/bin/env python3
"""Deja CrewChief en WASAPI con un dispositivo valido de Wine.

CrewChief guarda el ID del endpoint de audio en
`NAUDIO_DEVICE_GUID_MESSAGES` y `NAUDIO_DEVICE_GUID_BACKGROUND`. Si se elige
WASAPI y ese ID esta vacio, NAudio llama a `GetDevice("")`, lanza
`Value does not fall within the expected range` y CrewChief se queda sin
mensajes (solo funciona el fondo con WindowsMediaPlayer).

Wine publica los endpoints en `user.reg`, bajo
`Software\\Microsoft\\ActiveMovie\\devenum\\...\\DirectSound: <nombre>`, con
la GUID del dispositivo en `DSGuid`. El endpoint "PulseAudio Output" es el
que sigue al sink por defecto de PulseAudio/PipeWire.

Uso:
    configurar_audio_crewchief.py --bottle <botella> [--force-wasapi] [--json]

Sin `--force-wasapi` solo se toca CrewChief si ya esta configurado en WASAPI:
quien use WAVEOUT no nota ningun cambio. Si no se encuentra un endpoint, se
vuelve a WAVEOUT para no dejar la salida de mensajes rota.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

# Wine usa esta sesion de audio para el endpoint por defecto.
SESSION = "{0.0.0.00000000}"
SETTINGS = (
    "NAUDIO_DEVICE_GUID_MESSAGES",
    "NAUDIO_DEVICE_GUID_BACKGROUND",
    "naudio_output_interface_listprop",
    "naudio_wasapi_latency",
)
INTERFACE = "naudio_output_interface_listprop"


def audio_endpoints(reg_path: Path) -> list[tuple[str, str]]:
    """Devuelve (nombre, endpoint-id) de las salidas DirectSound de Wine."""
    if not reg_path.is_file():
        return []
    found: list[tuple[str, str]] = []
    section = ""
    for line in reg_path.read_text(encoding="utf-8", errors="replace").splitlines():
        if line.startswith("["):
            end = line.find("]")
            section = line[1:end] if end > 0 else ""
            continue
        if "devenum" not in section or "DirectSound: " not in section:
            continue
        match = re.search(r'"DSGuid"="(\{[0-9A-Fa-f-]+\})"', line)
        if not match:
            continue
        name = section.split("DirectSound: ", 1)[1]
        found.append((name, f"{SESSION}.{match.group(1).lower()}"))
    return found


def pick_endpoint(endpoints: list[tuple[str, str]]) -> tuple[str, str] | None:
    """Prefiere el endpoint generico de PulseAudio; si no, el primero."""
    for name, endpoint in endpoints:
        if "pulseaudio" in name.casefold():
            return name, endpoint
    return endpoints[0] if endpoints else None


def get_setting(text: str, name: str) -> str:
    match = re.search(
        r'<setting name="' + re.escape(name) + r'"[^>]*>\s*'
        r'<value(?:\s*/>|>(.*?)</value>)',
        text,
        re.S,
    )
    if not match:
        return ""
    return (match.group(1) or "").strip()


def set_setting(text: str, name: str, value: str) -> tuple[str, bool]:
    pattern = re.compile(
        r'(<setting name="' + re.escape(name) + r'"[^>]*>\s*<value)'
        r'(?:\s*/>|>.*?</value>)',
        re.S,
    )
    if not pattern.search(text):
        return text, False
    updated = pattern.sub(
        lambda match: match.group(1) + ">" + value + "</value>",
        text,
        count=1,
    )
    return updated, updated != text


def crewchief_configs(bottle: Path) -> list[Path]:
    configs = list(
        bottle.glob(
            "drive_c/users/*/AppData/Local/Britton_IT_Ltd/"
            "CrewChiefV4.exe_Url_*/*/user.config"
        )
    )
    exe_config = (
        bottle
        / "drive_c/Program Files (x86)/Britton IT Ltd/CrewChiefV4"
        / "CrewChiefV4.exe.config"
    )
    if exe_config.is_file():
        configs.append(exe_config)
    return sorted({path for path in configs if path.is_file()})


def read_text(path: Path) -> tuple[str, bool]:
    raw = path.read_bytes()
    bom = raw.startswith(b"\xef\xbb\xbf")
    return raw.decode("utf-8-sig"), bom


def write_text(path: Path, text: str, bom: bool) -> None:
    data = text.encode("utf-8")
    path.write_bytes((b"\xef\xbb\xbf" if bom else b"") + data)


def main() -> int:
    parser = argparse.ArgumentParser(description="Configura el audio de CrewChief")
    parser.add_argument("--bottle", required=True, help="ruta de la botella")
    parser.add_argument(
        "--force-wasapi",
        action="store_true",
        help="pasa a WASAPI aunque CrewChief este en WAVEOUT",
    )
    parser.add_argument("--json", action="store_true", help="salida JSON")
    args = parser.parse_args()

    bottle = Path(args.bottle).expanduser()
    configs = crewchief_configs(bottle)
    if not configs:
        print("CrewChief no esta instalado; no hay configuracion de audio.")
        return 0

    # La version efectiva manda: si la ultima esta en WASAPI, se prepara.
    wants_wasapi = args.force_wasapi
    for path in configs:
        text, _ = read_text(path)
        if get_setting(text, INTERFACE).upper() == "WASAPI":
            wants_wasapi = True
            break

    if not wants_wasapi:
        print("CrewChief: WAVEOUT configurado; no se toca el audio.")
        return 0

    endpoint = None
    for reg_name in ("user.reg", "system.reg"):
        endpoint = pick_endpoint(audio_endpoints(bottle / reg_name))
        if endpoint:
            break

    if not endpoint:
        # Sin endpoint valido, WASAPI dejaria CrewChief sin mensajes.
        changed = 0
        for path in configs:
            text, bom = read_text(path)
            if get_setting(text, INTERFACE).upper() != "WASAPI":
                continue
            updated, touched = set_setting(text, INTERFACE, "WAVEOUT")
            if touched:
                write_text(path, updated, bom)
                changed += 1
        message = (
            "CrewChief: no hay endpoint WASAPI en Wine; se deja WAVEOUT "
            f"({changed} config(s))."
        )
        print(message, file=sys.stderr)
        if args.json:
            print(json.dumps({"wasapi": False, "reason": "no-endpoint"}))
        return 0

    name, endpoint_id = endpoint
    checked = 0
    changed = 0
    for path in configs:
        text, bom = read_text(path)
        # No convertir a WASAPI configuraciones antiguas que siguen en WAVEOUT.
        if not args.force_wasapi and get_setting(text, INTERFACE).upper() != "WASAPI":
            continue
        checked += 1
        updated, touched = set_setting(text, INTERFACE, "WASAPI")
        for setting in ("NAUDIO_DEVICE_GUID_MESSAGES", "NAUDIO_DEVICE_GUID_BACKGROUND"):
            updated, changed_here = set_setting(updated, setting, endpoint_id)
            touched = touched or changed_here
        updated, changed_here = set_setting(updated, "naudio_wasapi_latency", "40")
        touched = touched or changed_here
        if touched:
            write_text(path, updated, bom)
            changed += 1

    state = f"{changed} actualizada(s)" if changed else "ya correcta(s)"
    summary = (
        f"CrewChief: WASAPI con {name} ({endpoint_id}); "
        f"{checked} config(s) {state}."
    )
    if args.json:
        print(
            json.dumps(
                {
                    "wasapi": True,
                    "device": name,
                    "endpoint": endpoint_id,
                    "configs": checked,
                    "changed": changed,
                },
                ensure_ascii=False,
            )
        )
    else:
        print(summary)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
