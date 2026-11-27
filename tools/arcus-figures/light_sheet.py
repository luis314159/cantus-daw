"""r42:arcus-polish3 — the concert light's before/after sheet beside the photographs the
owner chose (ref-04, ref-06, ref-10: study only, never committed): the app's own pictures
(``shoot_arcus --hold=woman,man --only=violin-play --scales=1``), the picture cropped from
the editor, both figures, Photographic and Drawn, Night; Paper as the whole editor (the
picture in a light window). With ``--numbers`` each tile carries light_measure's numbers.

    python tools/arcus-figures/light_sheet.py <before dir> <after dir> <refs dir> <out.png>
"""

import os
import sys

from PIL import Image, ImageDraw

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import light_measure as lm  # noqa: E402

W = 560
SHOT = "arcus-violin-play-{who}-{look}-{theme}-en-100.png"


def tile(path, crop=True, width=W, note=None):
    if not path or not os.path.exists(path):
        im = Image.new("RGB", (width, int(width * 428 / 592)), (40, 40, 40))
        ImageDraw.Draw(im).text((10, 30), "missing", fill=(200, 200, 200))
        return im
    im = Image.open(path).convert("RGB")
    if crop:
        w, h = im.size
        p = lm.PICTURE
        im = im.crop((int(p[0] * w), int(p[1] * h), int(p[2] * w), int(p[3] * h)))
    im = im.resize((width, int(im.height * width / im.width)), Image.LANCZOS)
    return im


def ref(path, width=W, height=None):
    im = Image.open(path).convert("RGB")
    im = im.resize((width, int(im.height * width / im.width)), Image.LANCZOS)
    if height and im.height > height:
        t = (im.height - height) // 2
        im = im.crop((0, t, width, t + height))
    return im


def label(im, text):
    out = Image.new("RGB", (im.width, im.height + 18), (16, 16, 20))
    out.paste(im, (0, 18))
    ImageDraw.Draw(out).text((5, 3), text, fill=(235, 235, 235))
    return out


def numbers(path, who):
    try:
        m = lm.measure(path, lm.SCENE[who], crop=lm.PICTURE)
    except Exception:  # noqa: BLE001
        return ""
    return (
        f"face {m['face_y']:.2f} R/B {m['face_r_b']:.1f} sides {m.get('face_sides', 0):.1f}"
        f" face/hall {m['face_over_bg']:.0f} hall {m['bg_y']:.4f}"
    )


def main():
    before, after, refs, out = sys.argv[1:5]
    rows = [
        ("woman", "photo", "night", "ref-04-front-soloist-hahn.jpg", True),
        ("man", "photo", "night", "ref-06-front-soloist-khoury.jpg", True),
        ("woman", "drawn", "night", "ref-10-front-soloist-raychen-orch.jpg", True),
        ("man", "drawn", "night", None, True),
        ("woman", "photo", "paper", None, False),
        ("man", "drawn", "paper", None, False),
    ]
    tiles = []
    for who, look, theme, r, crop in rows:
        name = SHOT.format(who=who, look=look, theme=theme)
        b = tile(os.path.join(before, name), crop)
        a = tile(os.path.join(after, name), crop)
        tb = f"{who}, {look}, {theme}, 100 % - before"
        ta = f"after"
        if look == "photo" and theme == "night":
            tb += "   " + numbers(os.path.join(before, name), who)
            ta += "   " + numbers(os.path.join(after, name), who)
        row = [label(b, tb), label(a, ta)]
        if r:
            row.append(label(ref(os.path.join(refs, r), W, b.height), f"{r[:6]} (photograph, study only)"))
        tiles.append(row)
    width = sum(t.width for t in max(tiles, key=len)) + 6 * 2
    height = sum(max(t.height for t in row) for row in tiles) + 6 * len(tiles)
    sheet = Image.new("RGB", (width, height), (10, 10, 12))
    y = 0
    for row in tiles:
        x = 0
        for t in row:
            sheet.paste(t, (x, y))
            x += t.width + 6
        y += max(t.height for t in row) + 6
    sheet.save(out)
    print(out)


if __name__ == "__main__":
    main()
