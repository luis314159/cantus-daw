"""r42:arcus-polish — before/after sheets of the realistic violinists, the app's own
pictures only (``shoot_arcus``): the whole picture at 100 % and crops at 200 % (the face,
the ear, the neck and neckline, the bow hand, the left hand, the arms), Night and Paper,
Photographic and Drawn, the woman and the man, each crop before and after side by side
with a label (and, for the whole pictures, the measures given with ``--numbers``).

    python tools/arcus-figures/polish_sheet.py <before dir> <after dir> <out dir> [--numbers=<file>]

The shot folders hold ``shoot_arcus`` output for ``violin-play`` (``--scales=1`` and ``2``):
``arcus-violin-play-<who>-<photo|drawn>-<night|paper>-en-<100|200>.png``. Crops are given
as fractions of the 200 % shot (the picture's place in the editor is fixed).
"""

import os
import sys

from PIL import Image, ImageDraw

# (name, who, box as fractions of the shot: x0, y0, x1, y1)
CROPS = [
    ("face", "woman", (0.17, 0.13, 0.31, 0.33)),
    ("ear and hair", "woman", (0.12, 0.10, 0.24, 0.28)),
    ("neck and neckline", "woman", (0.13, 0.26, 0.33, 0.46)),
    ("bow hand", "woman", (0.20, 0.38, 0.36, 0.52)),
    ("left hand", "woman", (0.38, 0.14, 0.50, 0.32)),
    ("arms", "woman", (0.0, 0.28, 0.48, 0.58)),
    ("face", "man", (0.17, 0.10, 0.31, 0.31)),
    ("hair", "man", (0.14, 0.06, 0.29, 0.22)),
    ("collar and tie", "man", (0.10, 0.26, 0.30, 0.48)),
    ("bow hand", "man", (0.20, 0.38, 0.36, 0.54)),
    ("left hand", "man", (0.38, 0.12, 0.50, 0.32)),
    ("arms", "man", (0.0, 0.28, 0.48, 0.58)),
]
PICTURE = (8 / 1040, 62 / 680, 600 / 1040, 490 / 680)


def label(im, text):
    d = ImageDraw.Draw(im)
    d.rectangle([0, 0, im.width, 16], fill=(16, 16, 20))
    d.text((5, 2), text, fill=(235, 235, 235))
    return im


def crop(path, box, scale=1.0):
    im = Image.open(path).convert("RGB")
    w, h = im.size
    c = im.crop((int(box[0] * w), int(box[1] * h), int(box[2] * w), int(box[3] * h)))
    if scale != 1.0:
        c = c.resize((int(c.width * scale), int(c.height * scale)), Image.LANCZOS)
    return c


def pair(a, b, title):
    gap = 6
    im = Image.new("RGB", (a.width + b.width + gap, a.height + 18), (20, 20, 24))
    im.paste(a, (0, 18))
    im.paste(b, (a.width + gap, 18))
    d = ImageDraw.Draw(im)
    d.text((5, 3), f"{title} - before", fill=(235, 235, 235))
    d.text((a.width + gap + 5, 3), "after", fill=(235, 235, 235))
    return im


def stack(ims, out):
    w = max(i.width for i in ims)
    h = sum(i.height for i in ims) + 4 * len(ims)
    sheet = Image.new("RGB", (w, h), (12, 12, 14))
    y = 0
    for i in ims:
        sheet.paste(i, (0, y))
        y += i.height + 4
    sheet.save(out)


def main():
    pos = [a for a in sys.argv[1:] if not a.startswith("--")]
    before, after, out = pos
    os.makedirs(out, exist_ok=True)
    numbers = next((a.split("=", 1)[1] for a in sys.argv if a.startswith("--numbers=")), None)
    note = open(numbers, encoding="utf-8").read().strip().splitlines() if numbers else []
    name = "arcus-violin-play-{who}-{look}-{theme}-en-{z}.png"
    for look in ("photo", "drawn"):
        for theme in ("night", "paper"):
            tiles = []
            for who in ("woman", "man"):
                f = name.format(who=who, look=look, theme=theme, z=100)
                a, b = os.path.join(before, f), os.path.join(after, f)
                if os.path.exists(a) and os.path.exists(b):
                    tiles.append(pair(crop(a, PICTURE), crop(b, PICTURE), f"{who}, {look}, {theme}, 100 %"))
            if tiles:
                stack(tiles, os.path.join(out, f"full-{look}-{theme}-100.png"))
            tiles = []
            for cname, who, box in CROPS:
                f = name.format(who=who, look=look, theme=theme, z=200)
                a, b = os.path.join(before, f), os.path.join(after, f)
                if os.path.exists(a) and os.path.exists(b):
                    tiles.append(pair(crop(a, box), crop(b, box), f"{who}, {cname}, {look}, {theme}, 200 %"))
            # Two crops a row.
            rows = []
            for i in range(0, len(tiles), 2):
                r = tiles[i:i + 2]
                w = sum(t.width for t in r) + 6 * (len(r) - 1)
                hh = max(t.height for t in r)
                row = Image.new("RGB", (w, hh), (12, 12, 14))
                x = 0
                for t in r:
                    row.paste(t, (x, 0))
                    x += t.width + 6
                rows.append(row)
            if rows:
                stack(rows, os.path.join(out, f"crops-{look}-{theme}-200.png"))
    if note:
        im = Image.new("RGB", (900, 18 * len(note) + 12), (16, 16, 20))
        d = ImageDraw.Draw(im)
        for i, line in enumerate(note):
            d.text((8, 6 + 18 * i), line, fill=(235, 235, 235))
        im.save(os.path.join(out, "numbers.png"))


if __name__ == "__main__":
    main()
