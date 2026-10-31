"""Build PIRA's app icons from the round logo (assets/app-icon/pira-logo-round.png).

Writes every PNG size in assets/app-icon/hicolor/ (the title bar mark, the Android launcher and
the desktop builds all load these), the padded 1024 px macOS master, the Windows .ico and the Android adaptive icon's foreground. The
logo is masked to its circle first, so faint stray paint outside the disc never shows.

Needs Pillow:

    python tools/pira-icon/from_logo.py
"""

import os

from PIL import Image, ImageDraw

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
ICONS = os.path.join(ROOT, "assets", "app-icon")
LOGO = os.path.join(ICONS, "pira-logo-round.png")
NAME = "ai.storyteller.photocraft.png"
SIZES = [16, 24, 32, 48, 64, 128, 256, 512]
ICO_SIZES = [16, 20, 24, 32, 40, 48, 64, 128, 256]


def masked(path):
    """The logo cut to its circle, with an antialiased edge (drawn 4x and scaled down)."""
    im = Image.open(path).convert("RGBA")
    w, h = im.size
    big = Image.new("L", (w * 4, h * 4), 0)
    ImageDraw.Draw(big).ellipse((0, 0, w * 4 - 1, h * 4 - 1), fill=255)
    circle = big.resize((w, h), Image.LANCZOS)
    alpha = Image.composite(im.getchannel("A"), Image.new("L", (w, h), 0), circle)
    im.putalpha(alpha)
    return im


def main():
    logo = masked(LOGO)
    for s in SIZES:
        out = os.path.join(ICONS, "hicolor", f"{s}x{s}", "apps")
        os.makedirs(out, exist_ok=True)
        logo.resize((s, s), Image.LANCZOS).save(os.path.join(out, NAME), optimize=True)
    # macOS: the disc on Apple's 824/1024 body grid, with a transparent margin.
    mac = Image.new("RGBA", (1024, 1024), (0, 0, 0, 0))
    mac.paste(logo.resize((824, 824), Image.LANCZOS), (100, 100))
    mac.save(os.path.join(ICONS, "photocraft-1024.png"), optimize=True)
    # Windows .ico, every size drawn from the full-resolution logo.
    big = logo.resize((256, 256), Image.LANCZOS)
    big.save(os.path.join(ICONS, "photocraft.ico"), sizes=[(s, s) for s in ICO_SIZES])
    # Android adaptive icon foreground: 108 dp layer (432 px at xxxhdpi) with the disc filling the
    # 72 dp the launcher's mask shows, so the launcher draws the logo itself, not a shrunk tile
    # on a white plate. The background is the disc's navy (res/values/ic_launcher_background.xml).
    fg = Image.new("RGBA", (432, 432), (0, 0, 0, 0))
    disc = 290
    fg.paste(logo.resize((disc, disc), Image.LANCZOS), ((432 - disc) // 2,) * 2)
    os.makedirs(os.path.join(ICONS, "android"), exist_ok=True)
    fg.save(os.path.join(ICONS, "android", "ic_launcher_foreground.png"), optimize=True)
    print("icons written to", ICONS)


if __name__ == "__main__":
    main()
