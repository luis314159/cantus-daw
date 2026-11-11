"""100 % crops of every theme's full shot (round 39, *themes*): the clip lanes, the mixer's
strips and the inspector, side by side at the shot's own pixels — where small text and fine
lines are judged, not on the reduced contact sheet.

    python tools/themes/crops.py docs/round39/themes
"""

import sys
from pathlib import Path

from PIL import Image

# At 1920 × 1080 and 125 %: (left, top, right, bottom) in the shot's pixels.
REGIONS = [
    (0, 120, 900, 500),  # the track headers and the clip lanes
    (0, 545, 720, 1060),  # the mixer's strips
    (1575, 55, 1920, 830),  # the inspector
]


def main(out_dir):
    out_dir = Path(out_dir)
    (out_dir / "crops").mkdir(exist_ok=True)
    for shot in sorted((out_dir / "full").glob("*.png")):
        im = Image.open(shot).convert("RGB")
        parts = [im.crop(r) for r in REGIONS]
        w = sum(p.width for p in parts) + 8 * (len(parts) - 1)
        h = max(p.height for p in parts)
        sheet = Image.new("RGB", (w, h), (128, 128, 128))
        x = 0
        for p in parts:
            sheet.paste(p, (x, 0))
            x += p.width + 8
        path = out_dir / "crops" / shot.name
        sheet.save(path, optimize=True)
        print(path)


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "docs/round39/themes")
