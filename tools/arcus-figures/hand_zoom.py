"""r42:arcus-hands — the left hand's finger-by-finger look at 400 %: crops of ``shoot_arcus``
shots taken at ``--scales=4`` (the app's own pixels, its own camera) round the left hand,
found as the skin-coloured blob nearest the neck's hand box, side by side as rows (first,
middle and high position; before and after), with the photographs beside when asked.
The photographs are study material only: keep sheets made with ``--refs`` outside the
repository.

    python tools/arcus-figures/hand_zoom.py <out.png> <label>=<shot.png>[@x0,y0,x1,y1] ... [--refs=<dir>] [--h=<px>]

A box (fractions of the shot) overrides the search.
"""
import os
import sys

from PIL import Image, ImageDraw

H = 520


def label(im, text):
    d = ImageDraw.Draw(im)
    d.rectangle([0, 0, im.width, 18], fill=(16, 16, 20))
    d.text((5, 3), text, fill=(235, 235, 235))
    return im


def tile(im, h, text):
    im = im.resize((max(1, int(im.width * h / im.height)), h), Image.LANCZOS)
    out = Image.new("RGB", (im.width, h + 20), (20, 20, 24))
    out.paste(im, (0, 20))
    return label(out, text)


def main():
    out = sys.argv[1]
    h = int(next((a.split("=", 1)[1] for a in sys.argv if a.startswith("--h=")), H))
    refs = next((a.split("=", 1)[1] for a in sys.argv if a.startswith("--refs=")), None)
    tiles = []
    for a in sys.argv[2:]:
        if a.startswith("--"):
            continue
        name, path = a.split("=", 1)
        box = None
        if "@" in path:
            path, b = path.split("@")
            box = [float(x) for x in b.split(",")]
        im = Image.open(path).convert("RGB")
        if box:
            w, hh = im.size
            im = im.crop((int(box[0] * w), int(box[1] * hh), int(box[2] * w), int(box[3] * hh)))
        tiles.append(tile(im, h, name))
    if refs:
        for f, box in (("crop-08.jpg", (0.35, 0.15, 0.75, 0.65)), ("crop-11.jpg", (0.25, 0.1, 0.65, 0.6))):
            im = Image.open(os.path.join(refs, f)).convert("RGB")
            w, hh = im.size
            im = im.crop((int(box[0] * w), int(box[1] * hh), int(box[2] * w), int(box[3] * hh)))
            tiles.append(tile(im, h, f"photograph {f}"))
    W = sum(t.width for t in tiles) + 4 * (len(tiles) - 1)
    sheet = Image.new("RGB", (W, h + 20), (8, 8, 10))
    x = 0
    for t in tiles:
        sheet.paste(t, (x, 0))
        x += t.width + 4
    sheet.save(out)
    print("wrote", out, sheet.size)


if __name__ == "__main__":
    main()
