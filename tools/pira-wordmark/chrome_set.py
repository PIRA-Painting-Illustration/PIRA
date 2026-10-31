"""The PIRA chrome wordmark (3D option D): the full set of lockups."""
import os
import sys

from PIL import Image

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import mark3d as m  # noqa: E402
g = m.g

OUT = sys.argv[3]
os.makedirs(OUT, exist_ok=True)
name, face, side, kw = m.TREATMENTS[3]


def word(cap):
    return m.render(cap, face, side, **kw)


def stacked(cap, bg, sub):
    w = word(cap)
    t = g.tagline(sub, int(cap * 0.19))
    logo_d, pad = int(cap * 3.0), int(cap * 0.5)
    W = pad * 2 + max(logo_d, w.width, t.width)
    H = pad * 2 + logo_d + int(cap * 0.15) + w.height + t.height
    im = Image.new("RGBA", (W, H), bg + (255,) if bg else (0, 0, 0, 0))
    y = pad
    im.alpha_composite(g.logo.resize((logo_d, logo_d), Image.LANCZOS), ((W - logo_d) // 2, y))
    y += logo_d + int(cap * 0.15)
    im.alpha_composite(w, ((W - w.width) // 2, y))
    y += w.height - int(cap * 0.1)
    im.alpha_composite(t, ((W - t.width) // 2, y))
    return im


CREAM_BG, SUB_LIGHT = (237, 231, 217), (30, 120, 110)
w = word(160)
m.lockup3d(w, 160).convert("RGB").save(os.path.join(OUT, "PIRA chrome horizontal - navy.png"), optimize=True)
m.lockup3d(w, 160, CREAM_BG, SUB_LIGHT).convert("RGB").save(os.path.join(OUT, "PIRA chrome horizontal - cream.png"), optimize=True)
m.lockup3d(w, 160, None, g.TEAL).save(os.path.join(OUT, "PIRA chrome horizontal - transparent, dark backgrounds.png"), optimize=True)
m.lockup3d(w, 160, None, SUB_LIGHT).save(os.path.join(OUT, "PIRA chrome horizontal - transparent, light backgrounds.png"), optimize=True)
stacked(170, g.NAVY, g.TEAL).convert("RGB").save(os.path.join(OUT, "PIRA chrome stacked - navy.png"), optimize=True)
stacked(170, CREAM_BG, SUB_LIGHT).convert("RGB").save(os.path.join(OUT, "PIRA chrome stacked - cream.png"), optimize=True)
stacked(170, None, g.TEAL).save(os.path.join(OUT, "PIRA chrome stacked - transparent.png"), optimize=True)
word(400).save(os.path.join(OUT, "PIRA chrome wordmark only - transparent.png"), optimize=True)
print("ok")
