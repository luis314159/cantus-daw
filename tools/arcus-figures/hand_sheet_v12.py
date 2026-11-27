"""r42:arcus-hands — the realistic violinists' hands, before and after, from the app's own
pictures (``shoot_arcus``): for each figure the left hand in first position (``violin-play``)
and high up (``violin-high``) and the bow hand, at 100 % (shown at its own pixels and
magnified 2x by nearest neighbour, so the antialiasing is judged honestly) and at 200 %,
Night and Paper, Photographic and Drawn. With ``--refs=<dir>`` cropped photographs of
professional violinists at about the same scale are added (keep those sheets outside the
repository: the photographs are study material only).

    python tools/arcus-figures/hand_sheet_v12.py <before dir> <after dir> <out dir> [--refs=<dir>]

The shot folders hold ``shoot_arcus <dir> --hold=woman,man --scales=1,2
--only=violin-play+violin-high`` output: ``arcus-<state>-<who>-<look>-<theme>-en-<100|200>.png``.
"""

import os
import sys

from PIL import Image, ImageDraw

# (case, state, who, box as fractions of the shot: x0, y0, x1, y1)
CROPS = [
    ("left hand, first position", "violin-play", "woman", (0.37, 0.19, 0.49, 0.37)),
    ("bow hand", "violin-play", "woman", (0.22, 0.39, 0.33, 0.55)),
    ("left hand, high", "violin-high", "woman", (0.33, 0.17, 0.45, 0.37)),
    ("left hand, first position", "violin-play", "man", (0.37, 0.19, 0.49, 0.37)),
    ("bow hand", "violin-play", "man", (0.22, 0.39, 0.33, 0.55)),
    ("left hand, high", "violin-high", "man", (0.33, 0.17, 0.45, 0.37)),
]
# Reference crops (file in --refs, box as fractions) for each case.
REFS = {
    "left hand, first position": [("crop-08.jpg", (0.35, 0.15, 0.75, 0.65)),
                                  ("crop-11.jpg", (0.0, 0.0, 1.0, 1.0))],
    "left hand, high": [("crop-02.jpg", (0.0, 0.0, 1.0, 1.0))],
    "bow hand": [("bow/right-hand-position-on-bow.jpg", (0.0, 0.0, 1.0, 1.0))],
}
H = 300


def label(im, text):
    d = ImageDraw.Draw(im)
    d.rectangle([0, 0, im.width, 16], fill=(16, 16, 20))
    d.text((5, 2), text, fill=(235, 235, 235))
    return im


def crop(path, box):
    im = Image.open(path).convert("RGB")
    w, h = im.size
    return im.crop((int(box[0] * w), int(box[1] * h), int(box[2] * w), int(box[3] * h)))


def tile(im, h, text, nearest=False):
    im = im.resize((max(1, int(im.width * h / im.height)), h),
                   Image.NEAREST if nearest else Image.LANCZOS)
    out = Image.new("RGB", (im.width, h + 18), (20, 20, 24))
    out.paste(im, (0, 18))
    return label(out, text)


def row(tiles):
    w = sum(t.width for t in tiles) + 6 * (len(tiles) - 1)
    h = max(t.height for t in tiles)
    r = Image.new("RGB", (w, h), (12, 12, 14))
    x = 0
    for t in tiles:
        r.paste(t, (x, 0))
        x += t.width + 6
    return r


def stack(ims, out):
    w = max(i.width for i in ims)
    h = sum(i.height for i in ims) + 4 * len(ims)
    sheet = Image.new("RGB", (w, h), (12, 12, 14))
    y = 0
    for i in ims:
        sheet.paste(i, (0, y))
        y += i.height + 4
    sheet.save(out)
    print(out)


def main():
    pos = [a for a in sys.argv[1:] if not a.startswith("--")]
    before, after, out = pos
    refs = next((a.split("=", 1)[1] for a in sys.argv if a.startswith("--refs=")), None)
    os.makedirs(out, exist_ok=True)
    name = "arcus-{state}-{who}-{look}-{theme}-en-{z}.png"
    for look in ("photo", "drawn"):
        for theme in ("night", "paper"):
            rows = []
            for case, state, who, box in CROPS:
                tiles = []
                for tag, d in (("before", before), ("after", after)):
                    if not d:
                        continue
                    a = os.path.join(d, name.format(state=state, who=who, look=look,
                                                    theme=theme, z=100))
                    b = os.path.join(d, name.format(state=state, who=who, look=look,
                                                    theme=theme, z=200))
                    if not (os.path.exists(a) and os.path.exists(b)):
                        continue
                    c1 = crop(a, box)
                    tiles.append(tile(c1, c1.height, f"{tag} 100 %"))
                    tiles.append(tile(c1, H, f"{tag} 100 %, 2x nearest", nearest=True))
                    tiles.append(tile(crop(b, box), H, f"{tag} 200 %"))
                if refs:
                    for f, rb in REFS.get(case, []):
                        p = os.path.join(refs, f)
                        if os.path.exists(p):
                            tiles.append(tile(crop(p, rb), H, f"photo {os.path.basename(f)} (study)"))
                if tiles:
                    head = Image.new("RGB", (400, 18), (16, 16, 20))
                    label(head, f"{who}, {case}, {look}, {theme}")
                    rows.append(head)
                    rows.append(row(tiles))
            if rows:
                stack(rows, os.path.join(out, f"hands-{look}-{theme}.png"))


if __name__ == "__main__":
    main()
