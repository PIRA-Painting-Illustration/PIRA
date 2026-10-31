"""3D treatments of the PIRA Line Bold Italic wordmark: extrusions, bevels and shadows.

    python mark3d.py <fonts dir> <round logo> <out dir>
"""
import os
import sys

from PIL import Image, ImageChops, ImageDraw, ImageFilter

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import geo_mark as g  # noqa: E402  (reads the same argv)

OUT = sys.argv[3]
os.makedirs(OUT, exist_ok=True)
NAVY, CREAM, TEAL = g.NAVY, g.CREAM, g.TEAL
CORAL, YELLOW, VIOLET = (255, 107, 90), (255, 209, 102), (123, 97, 255)


def mask(cap):
    return g.word("linebold", (255, 255, 255), cap, italic=True).getchannel("A")


def lerp(a, b, t):
    return tuple(int(a[i] + (b[i] - a[i]) * t) for i in range(3))


def vgrad(size, top, bottom):
    w, h = size
    col = Image.new("RGB", (1, h))
    for y in range(h):
        col.putpixel((0, y), lerp(top, bottom, y / max(1, h - 1)))
    return col.resize((w, h))


def hgrad(size, stops):
    w, h = size
    row = Image.new("RGB", (w, 1))
    for x in range(w):
        t = x / max(1, w - 1)
        for (a, ca), (b, cb) in zip(stops, stops[1:]):
            if a <= t <= b:
                row.putpixel((x, 0), lerp(ca, cb, (t - a) / (b - a)))
                break
    return row.resize((w, h))


def render(cap, face, side, depth_k=0.10, direction=(1.0, 0.75), bevel=True, shadow=True, outline=None):
    """face/side: callables (size) -> RGB image. Returns RGBA."""
    m = mask(cap)
    depth = int(cap * depth_k)
    dx, dy = direction
    pad = depth + int(cap * 0.25)
    W, H = m.width + pad * 2, m.height + pad * 2
    out = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    ox, oy = pad - int(depth * dx * 0.5), pad - int(depth * dy * 0.5)
    if shadow:
        sh = Image.new("L", (W, H), 0)
        sh.paste(m, (ox + int(depth * dx * 1.6), oy + int(depth * dy * 1.6)))
        sh = sh.filter(ImageFilter.GaussianBlur(cap * 0.06)).point(lambda v: int(v * 0.55))
        out.alpha_composite(Image.merge("RGBA", (*Image.new("RGB", (W, H), (0, 0, 0)).split(), sh)))
    # Extrusion: the mask stepped back from depth to 1, shading darker with distance.
    side_img = side((W, H))
    for i in range(depth, 0, -1):
        t = i / depth
        layer = Image.new("L", (W, H), 0)
        layer.paste(m, (ox + int(round(i * dx)), oy + int(round(i * dy))))
        shade = ImageChops.multiply(side_img, Image.new("RGB", (W, H), lerp((255, 255, 255), (120, 120, 140), t)))
        out.paste(shade, (0, 0), layer)
    fm = Image.new("L", (W, H), 0)
    fm.paste(m, (ox, oy))
    out.paste(face((W, H)), (0, 0), fm)
    if bevel:
        # Light from the top left: a bright inner edge on top/left, a darker one bottom/right.
        e = max(1, int(cap * 0.012))
        hi = ImageChops.subtract(fm, ImageChops.offset(fm, e, e)).filter(ImageFilter.GaussianBlur(0.6))
        lo = ImageChops.subtract(fm, ImageChops.offset(fm, -e, -e)).filter(ImageFilter.GaussianBlur(0.6))
        out.paste(Image.new("RGB", (W, H), (255, 255, 255)), (0, 0), hi.point(lambda v: int(v * 0.85)))
        out.paste(Image.new("RGB", (W, H), (0, 0, 0)), (0, 0), lo.point(lambda v: int(v * 0.35)))
    if outline:
        edge = ImageChops.subtract(fm.filter(ImageFilter.MaxFilter(3)), fm)
        out.paste(Image.new("RGB", (W, H), outline), (0, 0), edge)
    return out


TREATMENTS = [
    ("A  Cream on teal extrusion", lambda s: vgrad(s, (250, 246, 236), (214, 206, 188)), lambda s: vgrad(s, (42, 157, 143), (20, 90, 84)), {}),
    ("B  Cream on brand-gradient extrusion", lambda s: vgrad(s, (250, 246, 236), (214, 206, 188)),
     lambda s: hgrad(s, [(0.0, (42, 157, 143)), (0.35, VIOLET), (0.7, CORAL), (1.0, YELLOW)]), {}),
    ("C  Gradient face, deep navy extrusion", lambda s: hgrad(s, [(0.0, (90, 210, 195)), (0.4, (160, 140, 255)), (0.75, CORAL), (1.0, YELLOW)]),
     lambda s: vgrad(s, (24, 52, 64), (8, 18, 24)), dict(depth_k=0.12)),
    ("D  Chrome face, steel extrusion", lambda s: vgrad(s, (255, 255, 255), (150, 165, 175)),
     lambda s: vgrad(s, (90, 105, 115), (40, 50, 58)), dict(depth_k=0.08)),
]

if __name__ == "__main__":
    lab = g.font("montserrat-Montserrat[wght].ttf", 22, 600)
    rows = []
    for name, face, side, kw in TREATMENTS:
        rows.append((name, render(150, face, side, **kw)))
    W = max(r.width for _, r in rows) + 80
    H = sum(r.height + 40 for _, r in rows) + 30
    sheet = Image.new("RGBA", (W, H), NAVY + (255,))
    y = 20
    for name, r in rows:
        ImageDraw.Draw(sheet).text((40, y), name, font=lab, fill=TEAL)
        sheet.alpha_composite(r, (40, y + 10))
        y += r.height + 40
    sheet.convert("RGB").save(os.path.join(OUT, "PIRA 3D options.png"), optimize=True)
    for i, (name, face, side, kw) in enumerate(TREATMENTS):
        render(300, face, side, **kw).save(os.path.join(OUT, f"PIRA 3D {name[0]} - wordmark.png"), optimize=True)
    print("ok")


def lockup3d(word_img, cap, bg=NAVY, sub=TEAL):
    t = g.tagline(sub, int(cap * 0.2))
    logo_d, pad, gap = int(cap * 2.1), int(cap * 0.45), int(cap * 0.2)
    block = word_img.height + t.height
    W = pad * 2 + logo_d + gap + max(word_img.width, t.width)
    H = pad * 2 + max(logo_d, block)
    im = Image.new("RGBA", (W, H), bg + (255,) if bg else (0, 0, 0, 0))
    im.alpha_composite(g.logo.resize((logo_d, logo_d), Image.LANCZOS), (pad, (H - logo_d) // 2))
    x, y = pad + logo_d + gap, (H - block) // 2
    im.alpha_composite(word_img, (x, y))
    im.alpha_composite(t, (x + int(cap * 0.3), y + word_img.height - int(cap * 0.12)))
    return im


if __name__ == "__main__":
    for name, face, side, kw in TREATMENTS:
        w = render(110, face, side, **kw)
        lockup3d(w, 110).convert("RGB").save(os.path.join(OUT, f"PIRA 3D {name[0]} - horizontal navy.png"), optimize=True)
        lockup3d(w, 110, (237, 231, 217), (30, 120, 110)).convert("RGB").save(os.path.join(OUT, f"PIRA 3D {name[0]} - horizontal cream.png"), optimize=True)
    print("lockups ok")
