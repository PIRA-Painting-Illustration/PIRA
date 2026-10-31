"""Superseded: PIRA's icons now come from the real logo (tools/pira-icon/from_logo.py).
Running this would overwrite them with the old placeholder.

Draw PIRA's placeholder app icon (pure Python, no dependencies).

A stand-in until the real logo is ready: a rounded teal-navy tile with a brush-stroke "P"
(violet-to-teal stem, coral-to-yellow bowl) in the PIRA palette. It overwrites the PNGs in
assets/app-icon/hicolor/ under their existing names, so every place that loads the icon (the
title bar mark, the Android launcher, the desktop builds) picks it up.

    python tools/pira-icon/make_icon.py
"""

import math
import os
import struct
import zlib

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
OUT = os.path.join(ROOT, "assets", "app-icon", "hicolor")
NAME = "ai.storyteller.photocraft.png"
SIZES = [16, 24, 32, 48, 64, 128, 256, 512]

BG = (15, 30, 37)
TEAL = (42, 157, 143)
VIOLET = (123, 97, 255)
CORAL = (255, 107, 90)
YELLOW = (255, 209, 102)


def mix(a, b, t):
    return tuple(a[i] + (b[i] - a[i]) * t for i in range(3))


def rounded_rect_inside(x, y, size=512.0, r=112.0):
    cx = min(max(x, r), size - r)
    cy = min(max(y, r), size - r)
    return (x - cx) ** 2 + (y - cy) ** 2 <= r * r and 0 <= x <= size and 0 <= y <= size


def capsule(x, y, ax, ay, bx, by, rad):
    """Distance test to a segment; returns (inside, t along the segment)."""
    dx, dy = bx - ax, by - ay
    t = max(0.0, min(1.0, ((x - ax) * dx + (y - ay) * dy) / (dx * dx + dy * dy)))
    px, py = ax + dx * t, ay + dy * t
    return (x - px) ** 2 + (y - py) ** 2 <= rad * rad, t


def sample(x, y):
    """Colour (r, g, b, a) at a point of the 512-unit tile."""
    if not rounded_rect_inside(x, y):
        return (0, 0, 0, 0)
    col = BG
    # Bowl: a ring open on the left, where it meets the stem.
    cx, cy, rad, half = 284.0, 214.0, 92.0, 30.0
    d = math.hypot(x - cx, y - cy)
    ang = math.degrees(math.atan2(cy - y, x - cx)) % 360.0  # 0 = right, counter-clockwise
    if abs(d - rad) <= half and not (150.0 < ang < 214.0):
        # Coral at the top-left start, yellow where it returns to the stem.
        sweep = ((150.0 - ang) % 360.0) / (360.0 - 64.0)
        col = mix(CORAL, YELLOW, max(0.0, min(1.0, sweep)))
    # Stem: a slightly slanted stroke, tapering towards the bottom.
    inside, t = capsule(x, y, 205.0, 120.0, 178.0, 412.0, 31.0 - 10.0 * max(0.0, (y - 300.0) / 112.0))
    if inside:
        col = mix(TEAL, VIOLET, t)
    return (col[0], col[1], col[2], 255)


def render(px):
    ss = 4 if px <= 64 else 3
    scale = 512.0 / px
    rows = []
    for j in range(px):
        row = bytearray()
        for i in range(px):
            acc = [0.0, 0.0, 0.0, 0.0]
            for sj in range(ss):
                for si in range(ss):
                    r, g, b, a = sample((i + (si + 0.5) / ss) * scale, (j + (sj + 0.5) / ss) * scale)
                    acc[0] += r * a
                    acc[1] += g * a
                    acc[2] += b * a
                    acc[3] += a
            n = ss * ss
            a = acc[3] / n
            if a > 0:
                row += bytes([round(acc[0] / acc[3]), round(acc[1] / acc[3]), round(acc[2] / acc[3]), round(a)])
            else:
                row += bytes([0, 0, 0, 0])
        rows.append(bytes(row))
    return rows


def write_png(path, px, rows):
    raw = b"".join(b"\x00" + r for r in rows)

    def chunk(kind, data):
        c = struct.pack(">I", len(data)) + kind + data
        return c + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)

    png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", px, px, 8, 6, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")
    with open(path, "wb") as f:
        f.write(png)


def main():
    for px in SIZES:
        d = os.path.join(OUT, f"{px}x{px}", "apps")
        os.makedirs(d, exist_ok=True)
        write_png(os.path.join(d, NAME), px, render(px))
        print("wrote", px)


if __name__ == "__main__":
    main()
