#!/usr/bin/env python3
"""Genera los iconos de la aplicacion (PNG 512, 128 y 32)."""

from PIL import Image, ImageDraw

S = 512
SCALE = 4  # se dibuja grande y se reduce para suavizar bordes
W = S * SCALE

BG = (10, 14, 19, 255)
CARD = (16, 23, 32, 255)
BORDER = (31, 45, 60, 255)
TEAL = (56, 225, 176, 255)
TEAL_DIM = (32, 122, 104, 255)
AMBER = (255, 176, 74, 255)
WHITE = (232, 242, 250, 255)


def rr(draw, box, radius, fill=None, outline=None, width=1):
    draw.rounded_rectangle(box, radius=radius, fill=fill, outline=outline, width=width)


def main() -> None:
    img = Image.new("RGBA", (W, W), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    pad = 26 * SCALE
    rr(d, (pad, pad, W - pad, W - pad), radius=112 * SCALE, fill=CARD, outline=BORDER, width=6 * SCALE)

    # Arco del cuadro de instrumentos: 150 grados -> 30 grados (barrido de 240).
    box = (112 * SCALE, 118 * SCALE, 400 * SCALE, 406 * SCALE)
    d.arc(box, start=150, end=390, fill=(38, 54, 70, 255), width=26 * SCALE)

    # Zona activa del arco, con degradado de azul a verde.
    steps = 120
    for i in range(steps):
        t = i / (steps - 1)
        start = 150 + 240 * t
        end = start + 240 / steps + 1.2
        r = int(48 + (56 - 48) * t)
        g = int(130 + (225 - 130) * t)
        b = int(180 + (176 - 180) * t)
        d.arc(box, start=start, end=end, fill=(r, g, b, 255), width=26 * SCALE)

    # Aguja.
    import math

    angle = math.radians(-38)
    cx, cy = W / 2, 262 * SCALE
    length = 128 * SCALE
    d.line(
        (cx, cy, cx + length * math.cos(angle), cy + length * math.sin(angle)),
        fill=WHITE,
        width=13 * SCALE,
    )
    d.ellipse(
        (cx - 26 * SCALE, cy - 26 * SCALE, cx + 26 * SCALE, cy + 26 * SCALE),
        fill=TEAL,
    )
    d.ellipse(
        (cx - 10 * SCALE, cy - 10 * SCALE, cx + 10 * SCALE, cy + 10 * SCALE),
        fill=BG,
    )

    # Traza de etol en la parte baja.
    pts = [
        (128, 392),
        (168, 392),
        (190, 360),
        (212, 424),
        (236, 336),
        (262, 408),
        (286, 376),
        (310, 392),
        (384, 392),
    ]
    d.line([(x * SCALE, y * SCALE) for x, y in pts], fill=TEAL, width=12 * SCALE, joint="curve")
    d.ellipse((378 * SCALE, 386 * SCALE, 390 * SCALE, 398 * SCALE), fill=AMBER)

    icon = img.resize((S, S), Image.LANCZOS)
    icon.save("src-tauri/icons/icon.png")
    icon.resize((128, 128), Image.LANCZOS).save("src-tauri/icons/128x128.png")
    icon.resize((32, 32), Image.LANCZOS).save("src-tauri/icons/32x32.png")
    print("iconos generados")


if __name__ == "__main__":
    main()
