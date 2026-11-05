#!/usr/bin/env python3
"""Genera el icono de grupo de SimHub + CrewChief y lo instala.

Las dos apps corren bajo Wine/Proton y comparten la clase de ventana
`steam_proton`, asi que KDE las agrupa en una sola entrada del panel. Este
script crea un icono fusionado (CrewChief arriba, SimHub abajo, corte diagonal)
y una entrada .desktop que captura esa clase para que el grupo lo use.

Los logos se extraen de los propios ejecutables (no se descargan). No modifica
ni SimHub ni CrewChief.

Uso:
  ./scripts/icono-apps.py                 instala icono y entrada .desktop
  ./scripts/icono-apps.py --solo-preview  solo escribe /tmp/etol-apps-preview.png
"""

from __future__ import annotations

import io
import os
import struct
import sys
from pathlib import Path

import pefile
from PIL import Image, ImageDraw

HOME = Path.home()
BOTTLE = Path(
    os.environ.get(
        "SIMHUB_BOTTLE",
        HOME / ".var/app/com.usebottles.bottles/data/bottles/bottles/simhub",
    )
).expanduser()

SIMHUB_EXE = BOTTLE / "drive_c/Program Files (x86)/SimHub/SimHubWPF.exe"
CREW_EXE = (
    BOTTLE
    / "drive_c/Program Files (x86)/Britton IT Ltd/CrewChiefV4/CrewChiefV4.exe"
)

ICON_NAME = "etol-apps"
ICON_DIR = HOME / ".local/share/icons/hicolor"
DESKTOP_FILE = HOME / ".local/share/applications/etol-apps.desktop"
# Clase de ventana que comparten las apps de Wine en el prefijo de Bottles.
WINDOW_CLASS = "steam_proton"

S, F = 512, 4
W = S * F
BG = (13, 20, 29, 255)
BORDER = (31, 45, 60, 255)
TEAL = (56, 225, 176)
GOLD = (198, 160, 60)
BLUE = (47, 126, 216)


def iconos_del_exe(path: Path) -> list[bytes]:
    """Saca los RT_ICON de un .exe de Windows."""
    pe = pefile.PE(str(path), fast_load=True)
    pe.parse_data_directories(
        directories=[pefile.DIRECTORY_ENTRY["IMAGE_DIRECTORY_ENTRY_RESOURCE"]]
    )
    out: list[bytes] = []
    for entry in pe.DIRECTORY_ENTRY_RESOURCE.entries:
        if entry.id != 3:  # RT_ICON
            continue
        for item in entry.directory.entries:
            data = item.directory.entries[0].data
            out.append(pe.get_data(data.struct.OffsetToData, data.struct.Size))
    return out


def decodificar(data: bytes) -> Image.Image | None:
    """Convierte un RT_ICON (PNG o DIB) en imagen RGBA."""
    if data[:4] == b"\x89PNG":
        im = Image.open(io.BytesIO(data))
        im.load()
        return im.convert("RGBA")
    if data[:4] != b"\x28\x00\x00\x00":
        return None
    w = struct.unpack_from("<i", data, 4)[0]
    h = abs(struct.unpack_from("<i", data, 8)[0]) // 2
    bpp = struct.unpack_from("<H", data, 14)[0]
    if w <= 0 or h <= 0:
        return None
    # ICO de una sola imagen: cabecera + entrada + DIB tal cual.
    ico = (
        struct.pack("<HHH", 0, 1, 1)
        + struct.pack("<BBBBHHII", w & 0xFF, h & 0xFF, 0, 0, 1, bpp, len(data), 22)
        + data
    )
    try:
        im = Image.open(io.BytesIO(ico))
        im.load()
        return im.convert("RGBA")
    except Exception:  # noqa: BLE001
        return None


def logo(path: Path, quitar_fondo: bool) -> Image.Image:
    mejor = None
    for data in iconos_del_exe(path):
        im = decodificar(data)
        if im is not None and (mejor is None or im.size[0] > mejor.size[0]):
            mejor = im
    if mejor is None:
        raise SystemExit(f"no se pudo extraer el icono de {path}")

    if quitar_fondo:
        # CrewChief trae fondo marron oscuro; se hace transparente.
        px = mejor.load()
        for y in range(mejor.height):
            for x in range(mejor.width):
                r, g, b, _ = px[x, y]
                if r < 95 and g < 80 and b < 55:
                    px[x, y] = (0, 0, 0, 0)
    return mejor.crop(mejor.getbbox())


def encajar(im: Image.Image, max_w: float, max_h: float) -> Image.Image:
    escala = min(max_w / im.width, max_h / im.height)
    return im.resize(
        (max(1, int(im.width * escala)), max(1, int(im.height * escala))),
        Image.LANCZOS,
    )


def componer() -> Image.Image:
    crew = logo(CREW_EXE, quitar_fondo=True)
    sim = logo(SIMHUB_EXE, quitar_fondo=False)

    y_left, y_right = int(0.34 * W), int(0.54 * W)
    arriba = [(0, 0), (W, 0), (W, y_right), (0, y_left)]
    abajo = [(0, y_left), (W, y_right), (W, W), (0, W)]

    base = Image.new("RGBA", (W, W), (0, 0, 0, 0))
    d = ImageDraw.Draw(base)
    d.rounded_rectangle(
        (26 * F, 26 * F, W - 26 * F, W - 26 * F),
        radius=112 * F,
        fill=BG,
        outline=BORDER,
        width=6 * F,
    )

    # Tinte suave por mitad, recortado a la tarjeta.
    forma = Image.new("L", (W, W), 0)
    ImageDraw.Draw(forma).rounded_rectangle(
        (26 * F, 26 * F, W - 26 * F, W - 26 * F), radius=112 * F, fill=255
    )
    tinte = Image.new("RGBA", (W, W), (0, 0, 0, 0))
    td = ImageDraw.Draw(tinte)
    td.polygon(arriba, fill=GOLD + (42,))
    td.polygon(abajo, fill=BLUE + (30,))
    tinte.putalpha(Image.composite(tinte.getchannel("A"), Image.new("L", (W, W), 0), forma))
    base = Image.alpha_composite(base, tinte)

    # Corte diagonal, recortado a la tarjeta.
    linea = Image.new("RGBA", (W, W), (0, 0, 0, 0))
    ImageDraw.Draw(linea).line(
        [(0, y_left), (W, y_right)], fill=TEAL + (150,), width=3 * F
    )
    linea.putalpha(Image.composite(linea.getchannel("A"), Image.new("L", (W, W), 0), forma))
    base = Image.alpha_composite(base, linea)

    # Logos contenidos en su mitad.
    centro = (y_left + y_right) // 2
    c = encajar(crew, W * 0.46, centro * 0.70)
    base.alpha_composite(c, (int(W / 2 - c.width / 2), int(centro * 0.56 - c.height / 2)))

    alto_abajo = W - centro
    s = encajar(sim, W * 0.46, alto_abajo * 0.74)
    base.alpha_composite(
        s, (int(W / 2 - s.width / 2), int(centro + alto_abajo / 2 - s.height / 2))
    )

    return base.resize((S, S), Image.LANCZOS)


def appimage() -> str:
    """AppImage instalado mas reciente (para el Exec de la entrada)."""
    instalados = sorted(
        (HOME / ".local/bin").glob("ETOL*.AppImage"),
        key=lambda path: path.stat().st_mtime,
        reverse=True,
    )
    if instalados:
        return str(instalados[0])
    raiz = Path(__file__).resolve().parent.parent
    try:
        version = json.loads((raiz / "src-tauri/tauri.conf.json").read_text())["version"]
    except Exception:
        version = "0.3.4"
    return str(HOME / f".local/bin/ETOL-{version}-linux-x86_64-portable.AppImage")


def instalar(icono: Image.Image) -> None:
    for size in (512, 256, 128, 64, 32):
        destino = ICON_DIR / f"{size}x{size}/apps"
        destino.mkdir(parents=True, exist_ok=True)
        icono.resize((size, size), Image.LANCZOS).save(destino / f"{ICON_NAME}.png")

    DESKTOP_FILE.parent.mkdir(parents=True, exist_ok=True)
    DESKTOP_FILE.write_text(
        "[Desktop Entry]\n"
        "Type=Application\n"
        "Name=SimHub + CrewChief\n"
        "Comment=Apps de telemetria que lanza ETOL (icono de grupo)\n"
        f"Exec={appimage()}\n"
        f"Icon={ICON_NAME}\n"
        f"StartupWMClass={WINDOW_CLASS}\n"
        "NoDisplay=true\n"
        "Terminal=false\n"
        "Categories=Utility;\n",
        encoding="utf-8",
    )
    print(f"icono instalado en {ICON_DIR}")
    print(f"entrada creada: {DESKTOP_FILE}")


def main() -> int:
    icono = componer()
    if "--solo-preview" in sys.argv:
        destino = Path("/tmp/etol-apps-preview.png")
        icono.save(destino)
        print(f"preview: {destino}")
        return 0
    instalar(icono)
    print("reinicia el panel o vuelve a abrir las apps para ver el cambio")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
