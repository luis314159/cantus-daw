"""Round 39 close-out (lo-mixer): des-mixer's inspector sheet beside the built inspector.

`python tools/look/lomixer_compare.py <des-mixer sheets dir> <shots dir>` writes
`compare-inspector-{night,paper}.png` into the shots dir: the sheet's first and second
columns (head, source, inserts with the mini view, EQ, sends; routing, sidechain,
recording, automation) at 1.25x beside the same sections of `lomix-insp-strip-open-*`
(1920 x 2700 at 125 %), and its third column (level, freeze) beside ours.
"""
import sys
from pathlib import Path

from PIL import Image, ImageDraw

sheets, shots = Path(sys.argv[1]), Path(sys.argv[2])
for theme in ["night", "paper"]:
    sheet = Image.open(sheets / f"inspector-{theme}-1x.png").convert("RGB")
    ours = Image.open(shots / f"lomix-insp-strip-open-slate_{theme}.png").convert("RGB")
    k = 1.25
    cols = [sheet.crop((24, 72, 270, 712)), sheet.crop((294, 72, 540, 712)),
            sheet.crop((564, 72, 810, 712))]
    cols = [c.resize((int(c.width * k), int(c.height * k)), Image.LANCZOS) for c in cols]
    built = ours.crop((1568, 88, 1912, 2400))
    h = max(sum(c.height for c in cols) + 40, built.height)
    out = Image.new("RGB", (cols[0].width + built.width + 60, h + 40), (128, 128, 128))
    y = 30
    for c in cols:
        out.paste(c, (20, y))
        y += c.height + 10
    out.paste(built, (cols[0].width + 40, 30))
    d = ImageDraw.Draw(out)
    d.text((20, 8), "des-mixer's sheet (x1.25)", fill=(0, 0, 0))
    d.text((cols[0].width + 40, 8), "built (125 %)", fill=(0, 0, 0))
    out.save(shots / f"compare-inspector-{theme}.png")
    print(shots / f"compare-inspector-{theme}.png")
