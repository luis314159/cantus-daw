"""r42:arcus-upper — the realistic figures' design sheet: a side-by-side of a shot's picture
(cropped from the editor's shot) with a reference photograph at the same height, labelled.

    python tools/arcus-figures/sheet.py <shot.png> <reference.png> <out.png> [label left]
        [label right] [--crop=x0,y0,x1,y1] [--ref-crop=x0,y0,x1,y1] [--height=N]

``--crop`` takes a part of the shot (its pixels; a hand at 200 %), ``--ref-crop`` a part of
the photograph; ``--height`` the sheet's picture height (default: the crop's).

The reference is shown for the posture, the dress and the light only (the faces are
original; see docs/round42/arcus-upper-next.md).
"""

import sys

from PIL import Image, ImageDraw

# The picture's box in the Arcus editor's shot (fractions of its size).
BOX = (8 / 1040, 62 / 680, 600 / 1040, 490 / 680)


def box_of(arg):
    return tuple(int(v) for v in arg.split(","))


def crop(path, box=None):
    im = Image.open(path).convert("RGB")
    w, h = im.size
    if box:
        return im.crop(box)
    return im.crop((int(BOX[0] * w), int(BOX[1] * h), int(BOX[2] * w), int(BOX[3] * h)))


def main():
    pos = [a for a in sys.argv[1:] if not a.startswith("--")]
    opt = dict(a[2:].split("=", 1) for a in sys.argv[1:] if a.startswith("--"))
    shot, ref, out = pos[:3]
    left = pos[3] if len(pos) > 3 else ""
    right = pos[4] if len(pos) > 4 else ""
    a = crop(shot, box_of(opt["crop"]) if "crop" in opt else None)
    b = Image.open(ref).convert("RGB")
    if "ref-crop" in opt:
        b = b.crop(box_of(opt["ref-crop"]))
    h = int(opt.get("height", a.height))
    a = a.resize((int(a.width * h / a.height), h), Image.LANCZOS)
    b = b.resize((int(b.width * h / b.height), h), Image.LANCZOS)
    pad = 12
    band = 28
    im = Image.new("RGB", (a.width + b.width + pad * 3, h + pad * 2 + band), (24, 24, 28))
    im.paste(a, (pad, pad + band))
    im.paste(b, (a.width + pad * 2, pad + band))
    d = ImageDraw.Draw(im)
    d.text((pad, pad), left, fill=(230, 230, 230))
    d.text((a.width + pad * 2, pad), right, fill=(230, 230, 230))
    im.save(out)


if __name__ == "__main__":
    main()
