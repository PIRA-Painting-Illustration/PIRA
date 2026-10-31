"""Turn the owner's squircle PIRA tile into the round logo (assets/app-icon/pira-logo-round.png).

The tile sits at (148, 123) in a 3000 px canvas; its painted strokes are lifted off the tile's
navy, placed on a disc of the same navy extended past the tile, and ringed with a thin
teal-violet-rose rim. Then run from_logo.py to make every icon size.

    python tools/pira-icon/round_from_tile.py "piralogo 2.png" assets/app-icon/pira-logo-round.png
"""
import math
import sys

from PIL import Image, ImageChops, ImageDraw, ImageFilter

SRC, OUT = sys.argv[1], sys.argv[2]
SS = 4  # supersampling for the edges

full = Image.open(SRC).convert("RGBA")
tile = full.crop((148, 123, 148 + 585, 123 + 646))  # the tile; the stray strokes lie further right
W, H = tile.size

# The tile's background: a vertical navy gradient, read from two art-free columns.
def bg_at(y):
    yy = min(max(int(y), 100), 550)
    a, b = tile.getpixel((20, yy)), tile.getpixel((565, yy))
    c = [(a[i] + b[i]) / 2 for i in range(3)]
    if y < 100:  # continue the lighter top a little past the tile
        top = [(tile.getpixel((20, 100))[i] + tile.getpixel((565, 100))[i]) / 2 for i in range(3)]
        nxt = [(tile.getpixel((20, 140))[i] + tile.getpixel((565, 140))[i]) / 2 for i in range(3)]
        k = min((100 - y) / 40.0, 3.0)
        c = [top[i] + (top[i] - nxt[i]) * k for i in range(3)]
    return tuple(int(round(max(0, min(255, v)))) for v in c)

# Disc geometry in tile pixels: centred on the artwork with the tail tip just inside the rim.
CX, CY, R = 300.0, 338.0, 392.0
D = int(round(2 * R))
ox, oy = CX - R, CY - R  # disc origin in tile coordinates

disc = Image.new("RGBA", (D, D))
px = disc.load()
for y in range(D):
    col = bg_at(oy + y)
    for x in range(D):
        px[x, y] = col + (255,)

# The tile's artwork, without its blue rim: its own alpha eroded and feathered so it melts into
# the extended background.
# Only what differs from the background row colour counts as art (strokes, outlines, glow), so
# the tile's own edge never shows; its blue rim is cut by an eroded tile mask.
inside = tile.getchannel("A").point(lambda v: 255 if v > 200 else 0).filter(ImageFilter.MinFilter(33)).filter(ImageFilter.GaussianBlur(6))
art = tile.copy()
ap = art.load()
ip = inside.load()
for y in range(H):
    bg = bg_at(y)
    for x in range(W):
        if ip[x, y] < 2:
            ap[x, y] = (0, 0, 0, 0)
            continue
        r, g, b_, _ = ap[x, y]
        diff = max(abs(r - bg[0]), abs(g - bg[1]), abs(b_ - bg[2]))
        ap[x, y] = (r, g, b_, int(ip[x, y] * max(0.0, min(1.0, (diff - 3) / 22.0))))
disc.alpha_composite(art, (int(round(-ox)), int(round(-oy))))

# Rim: a thin ring sweeping teal (top left) → blue → violet (right) → rose (bottom) → teal.
stops = [(0.0, (60, 150, 160)), (0.25, (96, 151, 190)), (0.45, (110, 105, 200)), (0.7, (190, 110, 120)), (0.9, (120, 120, 130)), (1.0, (60, 150, 160))]
def ring_col(t):
    for (a, ca), (b, cb) in zip(stops, stops[1:]):
        if a <= t <= b:
            k = (t - a) / (b - a)
            return tuple(int(ca[i] + (cb[i] - ca[i]) * k) for i in range(3))
    return stops[-1][1]
ring_w = max(3.0, D * 0.009)
big = D * SS
ring = Image.new("RGBA", (D, D))
rp = ring.load()
for y in range(D):
    for x in range(D):
        dx, dy = x + 0.5 - D / 2, y + 0.5 - D / 2
        d = math.hypot(dx, dy)
        edge = D / 2 - 0.5
        cover = max(0.0, min(1.0, ring_w - (edge - d))) * max(0.0, min(1.0, edge + 1 - d))
        if cover > 0:
            t = ((math.atan2(dy, dx) + math.pi * 0.75) / (2 * math.pi)) % 1.0
            rp[x, y] = ring_col(t) + (int(255 * cover),)
disc.alpha_composite(ring)

# Circle mask, antialiased.
m = Image.new("L", (big, big), 0)
ImageDraw.Draw(m).ellipse((0, 0, big - 1, big - 1), fill=255)
disc.putalpha(ImageChops.multiply(disc.getchannel("A"), m.resize((D, D), Image.LANCZOS)))
disc.save(OUT, optimize=True)
print(OUT, disc.size)
