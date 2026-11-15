#!/usr/bin/env python3
"""Instala o actualiza FFB-HUB (LMUFFB) en la botella de ETOL.

FFB-HUB es una aplicacion propietaria de coasting-nc que se descarga del
repositorio oficial. ETOL no la redistribuye: solo baja el ultimo release y lo
descomprime en la botella, donde estan SimHub/CrewChief y la memoria
compartida que publican los puentes.

Uso:
    instalar_ffbhub.py --bottle <botella> [--dest <ruta>] [--json]

El zip de FFB-HUB usa barras invertidas en algunos nombres de entrada; se
normalizan para que `zipfile` lo extraiga bien en Linux. Se conservan
`config.ini`, `profiles/` y `presets/` al actualizar.
"""

from __future__ import annotations

import argparse
import json
import shutil
import sys
import tempfile
import urllib.request
import zipfile
from pathlib import Path

API = "https://api.github.com/repos/coasting-nc/LMUFFB/releases/latest"
USER_AGENT = "ETOL"
PRESERVE = ("config.ini", "profiles", "presets")


def fetch(url: str) -> bytes:
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    with urllib.request.urlopen(request, timeout=120) as response:
        return response.read()


def latest_release() -> tuple[str, str]:
    data = json.loads(fetch(API).decode("utf-8"))
    tag = str(data.get("tag_name") or "").strip()
    if not tag:
        raise RuntimeError("el release no tiene tag_name")
    asset = next(
        (
            item
            for item in data.get("assets") or []
            if str(item.get("name", "")).lower().startswith("ffbhub-")
            and str(item.get("name", "")).lower().endswith(".zip")
        ),
        None,
    )
    if not asset:
        raise RuntimeError("el release no incluye un zip FFBHUB-*.zip")
    url = str(asset.get("browser_download_url") or "").strip()
    if not url:
        raise RuntimeError("el asset no tiene browser_download_url")
    return tag, url


def extract(zip_path: Path, target: Path) -> None:
    """Extrae el zip normalizando las barras invertidas de Windows."""
    with zipfile.ZipFile(zip_path) as archive:
        for info in archive.infolist():
            name = info.filename.replace("\\", "/")
            if not name or name.endswith("/"):
                continue
            # Evita rutas absolutas o con .. que salgan de target.
            parts = [part for part in name.split("/") if part not in ("", ".", "..")]
            if not parts:
                continue
            dest = target.joinpath(*parts)
            dest.parent.mkdir(parents=True, exist_ok=True)
            with archive.open(info) as source, dest.open("wb") as output:
                shutil.copyfileobj(source, output)


def install(bottle: Path, dest: Path | None = None) -> dict[str, str]:
    tag, url = latest_release()
    dest = (dest or (bottle / "drive_c" / "FFBHUB")).expanduser()
    dest.mkdir(parents=True, exist_ok=True)

    with tempfile.TemporaryDirectory(prefix="etol-ffbhub-") as tmp_name:
        tmp = Path(tmp_name)
        zip_path = tmp / "ffbhub.zip"
        zip_path.write_bytes(fetch(url))

        content = tmp / "content"
        content.mkdir()
        extract(zip_path, content)

        root = content
        if not (root / "FFBHUB.exe").is_file():
            found = next(content.rglob("FFBHUB.exe"), None)
            if found is None:
                raise RuntimeError("el zip no contiene FFBHUB.exe")
            root = found.parent

        kept = tmp / "keep"
        kept.mkdir()
        for name in PRESERVE:
            source = dest / name
            if not source.exists():
                continue
            target = kept / name
            if source.is_dir():
                shutil.copytree(source, target)
            else:
                shutil.copy2(source, target)

        for item in root.iterdir():
            target = dest / item.name
            if item.is_dir():
                if target.exists():
                    shutil.rmtree(target)
                shutil.copytree(item, target)
            else:
                shutil.copy2(item, target)

        for name in PRESERVE:
            source = kept / name
            if not source.exists():
                continue
            target = dest / name
            if source.is_dir():
                if target.exists():
                    shutil.rmtree(target)
                shutil.copytree(source, target)
            else:
                shutil.copy2(source, target)

        (dest / "version.txt").write_text(tag + "\n", encoding="utf-8")

    exe = dest / "FFBHUB.exe"
    if not exe.is_file():
        raise RuntimeError(f"no se encontro {exe} tras la instalacion")
    return {"version": tag, "dest": str(dest), "exe": str(exe)}


def main() -> int:
    parser = argparse.ArgumentParser(description="Instala o actualiza FFB-HUB")
    parser.add_argument("--bottle", required=True, help="ruta de la botella")
    parser.add_argument("--dest", help="carpeta destino (por defecto C:\\FFBHUB)")
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
        print(f"FFB-HUB {result['version']} instalado en {result['dest']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
