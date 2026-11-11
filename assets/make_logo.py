"""Genera el icono de ClipVault: logo.svg, logo.png (256) e icon.ico (16-256).
Marca: triangulo de "play" apuntando hacia abajo sobre una bandeja = ver y guardar.
Uso: python assets/make_logo.py
"""
from pathlib import Path
from PIL import Image, ImageDraw

OUT = Path(__file__).parent
TOP, BOT = (10, 132, 214), (24, 200, 205)  # azul -> cian
TRI = [(72, 58), (184, 58), (128, 148)]  # coordenadas en rejilla 256 (ya centradas)
TRI_STROKE = 22
BAR = (56, 186, 200, 212)  # x0, y0, x1, y1
RADIUS = 58  # esquinas del azulejo

SVG = f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256">
<defs><linearGradient id="g" x1="0" y1="0" x2="0.35" y2="1"><stop offset="0" stop-color="rgb{TOP}"/><stop offset="1" stop-color="rgb{BOT}"/></linearGradient></defs>
<rect width="256" height="256" rx="{RADIUS}" fill="url(#g)"/>
<polygon points="{' '.join(f'{x},{y}' for x, y in TRI)}" fill="#fff" stroke="#fff" stroke-width="{TRI_STROKE}" stroke-linejoin="round"/>
<rect x="{BAR[0]}" y="{BAR[1]}" width="{BAR[2]-BAR[0]}" height="{BAR[3]-BAR[1]}" rx="13" fill="#fff"/>
</svg>
"""


def render(size: int = 1024) -> Image.Image:
    k = size / 256
    # degradado vertical-diagonal
    grad = Image.new("RGB", (size, size))
    px = grad.load()
    for y in range(size):
        for x in range(size):
            t = min(1.0, max(0.0, (y / size) * 0.9 + (x / size) * 0.1))
            px[x, y] = tuple(int(TOP[i] + (BOT[i] - TOP[i]) * t) for i in range(3))
    mask = Image.new("L", (size, size), 0)
    ImageDraw.Draw(mask).rounded_rectangle((0, 0, size - 1, size - 1), RADIUS * k, fill=255)
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    img.paste(grad, (0, 0), mask)

    d = ImageDraw.Draw(img)
    pts = [(x * k, y * k) for x, y in TRI]
    d.polygon(pts, fill="white")
    r = TRI_STROKE / 2 * k
    for i in range(3):
        a, b = pts[i], pts[(i + 1) % 3]
        d.line([a, b], fill="white", width=int(TRI_STROKE * k))
        d.ellipse((a[0] - r, a[1] - r, a[0] + r, a[1] + r), fill="white")
    d.rounded_rectangle([v * k for v in BAR], 13 * k, fill="white")
    return img


if __name__ == "__main__":
    big = render(1024)
    (OUT / "logo.svg").write_text(SVG, encoding="utf-8")
    big.resize((256, 256), Image.LANCZOS).save(OUT / "logo.png")
    big.resize((512, 512), Image.LANCZOS).save(OUT / "logo_512.png")
    big.save(OUT / "icon.ico", sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
    print("ok")
