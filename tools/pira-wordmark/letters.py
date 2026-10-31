"""The chrome PIRA letters one at a time, for the landing page's hover effect.

    python letters.py <fonts dir> ../../assets/app-icon/pira-logo-round.png ../../assets/brand/letters

Writes P.png, I.png, R.png and A.png. They share one height and baseline, so they line up when
set side by side at the same CSS height.
"""
import os
import sys

from PIL import Image

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import mark3d as m  # noqa: E402

g = m.g
OUT = sys.argv[3]
os.makedirs(OUT, exist_ok=True)
name, face, side, kw = m.TREATMENTS[3]
CAP = 160


def one_glyph(index):
    """g.word, but drawing only one of the four letters."""
    full = g.word

    def word(kind, fg, cap_px, italic=False):
        st = g.STYLES[kind]
        gl = [g.glyph_P(st), g.glyph_I(st), g.glyph_R(st), g.glyph_A(st, kind)][index]
        mask = Image.new("L", gl.size, 0)
        mask.paste(gl, (0, 0))
        if italic:
            h = mask.height
            k = g.ITALIC
            mask = mask.transform((mask.width + int(k * h), h), Image.AFFINE, (1, k, -k * h, 0, 1, 0), resample=Image.BICUBIC)
        scale = cap_px / (100 * g.U)
        mask = mask.resize((max(1, int(mask.width * scale)), max(1, int(mask.height * scale))), Image.LANCZOS)
        im = Image.new("RGBA", mask.size, fg + (0,))
        im.putalpha(mask)
        return im

    g.word = word
    try:
        return m.render(CAP, face, side, **kw)
    finally:
        g.word = full


ims = [one_glyph(i) for i in range(4)]
boxes = [im.getchannel("A").point(lambda v: 255 if v > 6 else 0).getbbox() for im in ims]
# One top and bottom for all four, so they share a baseline; each trimmed to its own width.
top, bottom = min(b[1] for b in boxes), max(b[3] for b in boxes)
for ch, im, (x0, _, x1, _) in zip("PIRA", ims, boxes):
    im.crop((x0, top, x1, bottom)).save(os.path.join(OUT, f"{ch}.png"), optimize=True)
    print(ch, x1 - x0, bottom - top)
