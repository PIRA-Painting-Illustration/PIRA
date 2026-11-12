"""Build PIRA Pose's icon: PIRA's round logo with the figure badge (assets/app-icon/pira-pose-badge.png)
in its lower right, and the Android adaptive icon's foreground from it. Run after from_logo.py.

    python tools/pira-icon/pose_from_logo.py
"""

import os

from PIL import Image

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
ICONS = os.path.join(ROOT, "assets", "app-icon")
# The badge's centre on the 784 px logo, kept clear of round launcher masks.
BADGE_AT = (585, 585)


def main():
    logo = Image.open(os.path.join(ICONS, "pira-logo-round.png")).convert("RGBA").resize((784, 784), Image.LANCZOS)
    badge = Image.open(os.path.join(ICONS, "pira-pose-badge.png")).convert("RGBA")
    logo.alpha_composite(badge, (BADGE_AT[0] - badge.width // 2, BADGE_AT[1] - badge.height // 2))
    logo.save(os.path.join(ICONS, "pira-pose-logo-round.png"), optimize=True)
    # Adaptive icon foreground: 432 px (108 dp at xxxhdpi) with the disc filling the 72 dp the mask shows.
    fg = Image.new("RGBA", (432, 432), (0, 0, 0, 0))
    disc = 290
    fg.paste(logo.resize((disc, disc), Image.LANCZOS), ((432 - disc) // 2,) * 2)
    out = os.path.join(ROOT, "apps", "pira-pose", "android", "app", "src", "main", "res", "mipmap-xxxhdpi")
    fg.save(os.path.join(out, "ic_launcher_foreground.png"), optimize=True)
    print("PIRA Pose icon written")


if __name__ == "__main__":
    main()
