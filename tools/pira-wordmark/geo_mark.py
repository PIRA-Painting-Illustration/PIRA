"""PIRA as custom geometric letters (no font): P and R share a bowl that opens at the top left.

Three styles drawn from the same skeleton:
  stencil  - heavy, square corners, one cut per letter (P/R bowl opening, A split apex)
  chamfer  - heavy, 45-degree chamfers on outer corners
  line     - thin monoline, rounded bowls, A with a floating triangle instead of a crossbar
"""
import os
import sys

from PIL import Image, ImageChops, ImageDraw, ImageFont

FONTS, LOGO, OUT = sys.argv[1], sys.argv[2], sys.argv[3]
os.makedirs(OUT, exist_ok=True)
SS = 4
CREAM, NAVY, TEAL = (237, 231, 217), (15, 30, 37), (42, 157, 143)
U = 4 * SS  # pixels per design unit (cap height = 100 units)

STYLES = {
    "stencil": dict(s=15, wP=76, wR=80, wA=98, gap=9, round=False, chamfer=0, track=14),
    "chamfer": dict(s=15, wP=76, wR=80, wA=98, gap=9, round=False, chamfer=17, track=14),
    "line": dict(s=8, wP=72, wR=76, wA=92, gap=9, round=True, chamfer=0, track=16),
    "linebold": dict(s=12.5, wP=74, wR=78, wA=96, gap=10, round=True, chamfer=0, track=15),
}
ITALIC = 0.21  # shear: about 12 degrees
B = 60  # bowl height


def canvas(w):
    return Image.new("L", (int(w * U) + 2, 100 * U + 2), 0)


def poly(m, pts, fill=255):
    ImageDraw.Draw(m).polygon([(x * U, y * U) for x, y in pts], fill=fill)


def shape_box(m, x0, y0, x1, y1, st, fill=255, inner=False):
    """A box whose right corners are rounded (line), chamfered (chamfer) or square (stencil)."""
    d = ImageDraw.Draw(m)
    if st["round"]:
        r = (y1 - y0) / 2
        d.rounded_rectangle((x0 * U, y0 * U, x1 * U, y1 * U), radius=r * U - 2, fill=fill, corners=(False, True, True, False))
    elif st["chamfer"]:
        c = st["chamfer"] - (st["s"] * 0.42 if inner else 0)
        c = max(c, 1)
        poly(m, [(x0, y0), (x1 - c, y0), (x1, y0 + c), (x1, y1 - c), (x1 - c, y1), (x0, y1)], fill)
    else:
        d.rectangle((x0 * U, y0 * U, x1 * U, y1 * U), fill=fill)


def stem(m, x, st, cut=True):
    s = st["s"]
    poly(m, [(x, 0), (x + s, 0), (x + s, 100), (x, 100)])
    if cut and not st["round"]:  # angled top-left on the heavy styles, echoing the bowl's cut
        poly(m, [(x - 0.1, -0.1), (x + s * 0.45, -0.1), (x - 0.1, s * 0.9)], 0)


def bowl(m, x, w, st):
    s = st["s"]
    outer = Image.new("L", m.size, 0)
    inner = Image.new("L", m.size, 0)
    shape_box(outer, x, 0, x + w, B, st)
    shape_box(inner, x - 1, s, x + w - s, B - s, st, inner=True)
    b = ImageChops.subtract(outer, inner)
    # The opening: the top bar starts a gap after the stem, its end cut on a slant.
    g = st["gap"]
    cut = Image.new("L", m.size, 0)
    slant = 0 if st["round"] else s * 0.4
    poly(cut, [(x + s, -1), (x + s + g + slant, -1), (x + s + g, s + 0.5), (x + s, s + 0.5)])
    return ImageChops.lighter(m, ImageChops.subtract(b, cut))


def glyph_P(st):
    m = canvas(st["wP"])
    stem(m, 0, st)
    return bowl(m, 0, st["wP"], st)


def glyph_I(st):
    m = canvas(st["s"])
    stem(m, 0, st, cut=False)  # a cut I reads as a 1
    return m


def glyph_R(st):
    w, s = st["wR"], st["s"]
    m = canvas(w)
    stem(m, 0, st)
    m = bowl(m, 0, w - 2, st)
    # Leg: from the bowl's lower bar down to the bottom right.
    lx = w * 0.42
    poly(m, [(lx, B - s), (lx + s * 1.25, B - s), (w, 100), (w - s * 1.25, 100)])
    return m


def glyph_A(st, kind):
    w, s = st["wA"], st["s"]
    m = canvas(w)
    a = s * 0.35  # half the flat apex
    sw = s * 1.2  # horizontal leg thickness
    poly(m, [(0, 100), (w / 2 - a, 0), (w / 2 + a, 0), (w, 100)])
    yi = 100 * (1 - (w / 2 - sw) / (w / 2 - a))
    poly(m, [(sw, 100.5), (w / 2, yi), (w - sw, 100.5)], 0)
    if kind == "stencil":
        # A crossbar that stops short of the right leg.
        y = 64
        poly(m, [(w * 0.22, y), (w * 0.66, y), (w * 0.66, y + s * 0.8), (w * 0.20, y + s * 0.8)])
        inner = Image.new("L", m.size, 0)
        poly(inner, [(0, 100), (w / 2 - a, 0), (w / 2 + a, 0), (w, 100)])
        m = ImageChops.multiply(m, inner)
    elif kind == "chamfer":
        y = 64
        bar = Image.new("L", m.size, 0)
        poly(bar, [(0, y), (w, y), (w, y + s * 0.8), (0, y + s * 0.8)])
        inner = Image.new("L", m.size, 0)
        poly(inner, [(0, 100), (w / 2 - a, 0), (w / 2 + a, 0), (w, 100)])
        m = ImageChops.lighter(m, ImageChops.multiply(bar, inner))
    else:
        # A floating triangle where the crossbar would be.
        t = 13 + (st["s"] - 8) * 0.5
        cy = 74 + (st["s"] - 8) * 0.3
        poly(m, [(w / 2, cy - t), (w / 2 + t * 0.9, cy + t * 0.6), (w / 2 - t * 0.9, cy + t * 0.6)])
    return m


def word(kind, fg, cap_px, italic=False):
    st = STYLES[kind]
    gl = [glyph_P(st), glyph_I(st), glyph_R(st), glyph_A(st, kind)]
    tr = st["track"] * U
    W = sum(g.width for g in gl) + tr * 3
    m = Image.new("L", (int(W), 100 * U + 2), 0)
    x = 0
    for g in gl:
        m.paste(g, (int(x), 0))
        x += g.width + tr
    if italic:
        h = m.height
        k = ITALIC
        m = m.transform((m.width + int(k * h), h), Image.AFFINE, (1, k, -k * h, 0, 1, 0), resample=Image.BICUBIC)
    scale = cap_px / (100 * U)
    m = m.resize((max(1, int(m.width * scale)), max(1, int(m.height * scale))), Image.LANCZOS)
    im = Image.new("RGBA", m.size, fg + (0,))
    im.putalpha(m)
    return im


def font(name, size, weight=None):
    f = ImageFont.truetype(os.path.join(FONTS, name), size)
    if weight is not None:
        f.set_variation_by_axes([weight])
    return f


def tagline(fg, size):
    f = font("montserrat-Montserrat[wght].ttf", size, 500)
    tag = "PAINTING · ILLUSTRATION · RASTER ART"
    tr = size * 0.2
    w = sum(f.getlength(c) for c in tag) + tr * (len(tag) - 1)
    b = f.getbbox("A")
    im = Image.new("RGBA", (int(w) + 4, b[3] - b[1] + 6), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    x = 0
    for c in tag:
        d.text((x, 3 - b[1]), c, font=f, fill=fg)
        x += f.getlength(c) + tr
    return im


logo = Image.open(LOGO).convert("RGBA")


def lockup(kind, bg, fg, sub, cap=80, italic=False):
    w = word(kind, fg, cap, italic)
    t = tagline(sub, int(cap * 0.21))
    if t.width > w.width:  # keep the tagline no wider than the word
        t = t.resize((w.width, int(t.height * w.width / t.width)), Image.LANCZOS)
    pad, logo_d, gap = int(cap * 0.5), int(cap * 2.0), int(cap * 0.4)
    block = w.height + int(cap * 0.3) + t.height
    W = pad * 2 + logo_d + gap + max(w.width, t.width)
    H = pad * 2 + max(logo_d, block)
    im = Image.new("RGBA", (W, H), bg + (255,) if bg else (0, 0, 0, 0))
    im.alpha_composite(logo.resize((logo_d, logo_d), Image.LANCZOS), (pad, (H - logo_d) // 2))
    x, y = pad + logo_d + gap, (H - block) // 2
    im.alpha_composite(w, (x, y))
    im.alpha_composite(t, (x, y + w.height + int(cap * 0.3)))
    return im


def stacked(kind, bg, fg, sub, cap=110, italic=False):
    w = word(kind, fg, cap, italic)
    t = tagline(sub, int(cap * 0.19))
    if t.width > w.width * 1.15:
        t = t.resize((int(w.width * 1.15), int(t.height * w.width * 1.15 / t.width)), Image.LANCZOS)
    logo_d, pad = int(cap * 3.2), int(cap * 0.55)
    W = pad * 2 + max(logo_d, w.width, t.width)
    H = pad * 2 + logo_d + int(cap * 0.5) + w.height + int(cap * 0.35) + t.height
    im = Image.new("RGBA", (W, H), bg + (255,) if bg else (0, 0, 0, 0))
    y = pad
    im.alpha_composite(logo.resize((logo_d, logo_d), Image.LANCZOS), ((W - logo_d) // 2, y))
    y += logo_d + int(cap * 0.5)
    im.alpha_composite(w, ((W - w.width) // 2, y))
    y += w.height + int(cap * 0.35)
    im.alpha_composite(t, ((W - t.width) // 2, y))
    return im


if __name__ == "__main__":
    D = OUT
    variants = [("Line (current)", "line", False), ("Line Bold", "linebold", False), ("Line Bold Italic", "linebold", True)]
    big = [(n, word(k, CREAM, 150, it)) for n, k, it in variants]
    lab = font("montserrat-Montserrat[wght].ttf", 22, 600)
    st = Image.new("RGBA", (max(b.width for _, b in big) + 80, sum(b.height + 90 for _, b in big) + 20), NAVY + (255,))
    y = 20
    for n, b in big:
        ImageDraw.Draw(st).text((40, y), n, font=lab, fill=TEAL)
        y += 40
        st.alpha_composite(b, (40, y))
        y += b.height + 50
    st.convert("RGB").save(os.path.join(D, "PIRA line bold letters.png"), optimize=True)
    for it, tag in [(False, "bold"), (True, "bold italic")]:
        lockup("linebold", NAVY, CREAM, TEAL, 90, it).convert("RGB").save(os.path.join(D, f"PIRA {tag} horizontal - navy.png"), optimize=True)
        lockup("linebold", None, CREAM, TEAL, 90, it).save(os.path.join(D, f"PIRA {tag} horizontal - for dark backgrounds.png"), optimize=True)
        lockup("linebold", None, NAVY, (30, 120, 110), 90, it).save(os.path.join(D, f"PIRA {tag} horizontal - for light backgrounds.png"), optimize=True)
        stacked("linebold", NAVY, CREAM, TEAL, 120, it).convert("RGB").save(os.path.join(D, f"PIRA {tag} stacked - navy.png"), optimize=True)
        stacked("linebold", None, CREAM, TEAL, 120, it).save(os.path.join(D, f"PIRA {tag} stacked - for dark backgrounds.png"), optimize=True)
        word("linebold", CREAM, 200, it).save(os.path.join(D, f"PIRA {tag} wordmark only - cream.png"), optimize=True)
        word("linebold", NAVY, 200, it).save(os.path.join(D, f"PIRA {tag} wordmark only - navy.png"), optimize=True)
    print("ok")


LABELS_OLD = {"stencil": "A  Stencil (after Reiko)", "chamfer": "B  Chamfer (after Quantum)", "line": "C  Line (after Charito)"}

if False:
    kinds = list(STYLES)
    rows = [(LABELS[k], lockup(k, NAVY, CREAM, TEAL), lockup(k, CREAM, NAVY, (30, 120, 110))) for k in kinds]
    rw = max(r[1].width for r in rows) * 2 + 60
    rh = sum(r[1].height + 50 for r in rows) + 30
    sheet = Image.new("RGBA", (rw, rh), (250, 248, 244, 255))
    sd = ImageDraw.Draw(sheet)
    lab = font("montserrat-Montserrat[wght].ttf", 22, 600)
    y = 20
    for name, dark, light in rows:
        sd.text((20, y), name, font=lab, fill=NAVY)
        y += 32
        sheet.alpha_composite(dark, (20, y))
        sheet.alpha_composite(light, (40 + dark.width, y))
        y += dark.height + 18
    sheet.convert("RGB").save(os.path.join(OUT, "PIRA geometric options.png"), optimize=True)
    # Large letter study.
    big = [word(k, CREAM, 150) for k in kinds]
    st = Image.new("RGBA", (max(b.width for b in big) + 80, sum(b.height + 50 for b in big) + 40), NAVY + (255,))
    y = 40
    for b in big:
        st.alpha_composite(b, (40, y))
        y += b.height + 50
    st.convert("RGB").save(os.path.join(OUT, "PIRA geometric letters.png"), optimize=True)
    print("ok")
