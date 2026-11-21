#!/usr/bin/env python3
"""Instala o actualiza DC Pedals Calibration en la botella de ETOL.

El programa es la utilidad oficial de DC Simracing para calibrar los pedales
DC2/DC3. ETOL no la redistribuye: descarga el RAR publicado en dcsimracing.com
y copia el ejecutable y `mcHID.dll` a `C:\\DC Pedals Calibration`.

Los presets del usuario (`normal.txt`, `75.txt`, ...) viven en esa misma
carpeta. El instalador solo reemplaza el programa, la DLL y la nota oficial,
asi que no los borra.

No ejecuta el programa ni abre el dispositivo HID.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import subprocess
import sys
import tempfile
import urllib.request
from pathlib import Path

URL = "https://dcsimracing.com/dw/software/dc_pedals_calibration_1.0.2.rar"
VERSION = "1.0.2"
USER_AGENT = "ETOL"

# SHA-256 de los dos ficheros del RAR oficial 1.0.2. Si DC Simracing cambia el
# paquete, el instalador se detiene en vez de mezclar versiones.
SHA_EXE = "39bed3923c1fddaca673bf89a63da9c8f6d50b181a5ff41e26eea8c481ca7921"
SHA_DLL = "e705e13593c869497d635861428d10c915d3a760ab40c914974f3521dc45defd"


def fetch(url: str) -> bytes:
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    with urllib.request.urlopen(request, timeout=180) as response:
        return response.read()


def extract(archive: Path, target: Path) -> None:
    """Extrae el RAR con la primera herramienta disponible del sistema."""
    target.mkdir(parents=True, exist_ok=True)

    if shutil.which("bsdtar"):
        subprocess.run(
            ["bsdtar", "-xf", str(archive), "-C", str(target)],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        return
    if shutil.which("7z"):
        subprocess.run(
            ["7z", "x", "-y", f"-o{target}", str(archive)],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        return
    if shutil.which("unrar"):
        subprocess.run(
            ["unrar", "x", "-y", str(archive), str(target) + "/"],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        return
    if shutil.which("unar"):
        subprocess.run(
            ["unar", "-f", "-o", str(target), str(archive)],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        return
    raise RuntimeError(
        "no hay extractor de RAR. Instala libarchive (bsdtar), p7zip (7z) "
        "o unrar y vuelve a intentarlo."
    )


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def install(bottle: Path, dest: Path | None = None) -> dict[str, str]:
    dest = (dest or (bottle / "drive_c" / "DC Pedals Calibration")).expanduser()
    dest.mkdir(parents=True, exist_ok=True)

    with tempfile.TemporaryDirectory(prefix="etol-dcpedals-") as tmp_name:
        tmp = Path(tmp_name)
        archive = tmp / "dc_pedals_calibration.rar"
        archive.write_bytes(fetch(URL))
        content = tmp / "content"
        extract(archive, content)

        exe = next(content.rglob("DC Pedals Calibration.exe"), None)
        if exe is None:
            raise RuntimeError("el RAR no contiene DC Pedals Calibration.exe")
        dll = exe.parent / "mcHID.dll"
        if not dll.is_file():
            raise RuntimeError("el RAR no contiene mcHID.dll junto al ejecutable")

        got_exe = sha256(exe)
        got_dll = sha256(dll)
        if got_exe != SHA_EXE or got_dll != SHA_DLL:
            raise RuntimeError(
                "el paquete descargado no coincide con la version 1.0.2 "
                f"(exe {got_exe[:12]}..., dll {got_dll[:12]}...)"
            )

        # Solo se reemplaza el programa. Los *.txt del usuario (presets) y
        # cualquier otro fichero de la carpeta se quedan intactos.
        shutil.copy2(exe, dest / exe.name)
        shutil.copy2(dll, dest / dll.name)

        note = next(content.rglob("nota importante.txt"), None)
        if note is not None and note.is_file():
            shutil.copy2(note, dest / note.name)

        (dest / "version.txt").write_text(VERSION + "\n", encoding="utf-8")

    installed = dest / "DC Pedals Calibration.exe"
    if not installed.is_file():
        raise RuntimeError(f"no se encontro {installed} tras la instalacion")
    return {"version": VERSION, "dest": str(dest), "exe": str(installed)}


def main() -> int:
    parser = argparse.ArgumentParser(description="Instala DC Pedals Calibration")
    parser.add_argument("--bottle", required=True, help="ruta de la botella")
    parser.add_argument("--dest", help="carpeta destino (por defecto C:\\DC Pedals Calibration)")
    parser.add_argument(
        "--json",
        action="store_true",
        help="imprime el resultado como JSON en stdout",
    )
    args = parser.parse_args()

    bottle = Path(args.bottle).expanduser()
    if not (bottle / "drive_c").is_dir():
        print(f"ERROR: no parece una botella: {bottle}", file=sys.stderr)
        return 1

    try:
        result = install(bottle, Path(args.dest) if args.dest else None)
    except Exception as exc:  # noqa: BLE001
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1

    if args.json:
        print(json.dumps(result, ensure_ascii=False))
    else:
        print(f"DC Pedals Calibration {result['version']} instalado en {result['dest']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
