"""r42:arcus-upper — the realistic figures' design sheet: a side-by-side of a shot's picture
(cropped from the editor's shot) with a reference photograph at the same height, labelled.

    python tools/arcus-figures/sheet.py <shot.png> <reference.png> <out.png> [label left] [label right]

The reference is shown for the posture, the dress and the light only (the faces are
original; see docs/round42/arcus-upper-next.md).
"""

import sys

from PIL import Image, ImageDraw

# The picture's box in the Arcus editor's shot (fractions of its size).
BOX = (8 / 1040, 62 / 680, 600 / 1040, 490 / 680)


def crop(path):
    im = Image.open(path).convert("RGB")
    w, h = im.size
    return im.crop((int(BOX[0] * w), int(BOX[1] * h), int(BOX[2] * w), int(BOX[3] * h)))


def main():
    shot, ref, out = sys.argv[1:4]
    left = sys.argv[4] if len(sys.argv) > 4 else ""
    right = sys.argv[5] if len(sys.argv) > 5 else ""
    a = crop(shot)
    b = Image.open(ref).convert("RGB")
    b = b.resize((int(b.width * a.height / b.height), a.height), Image.LANCZOS)
    pad = 12
    band = 28
    im = Image.new("RGB", (a.width + b.width + pad * 3, a.height + pad * 2 + band), (24, 24, 28))
    im.paste(a, (pad, pad + band))
    im.paste(b, (a.width + pad * 2, pad + band))
    d = ImageDraw.Draw(im)
    d.text((pad, pad), left, fill=(230, 230, 230))
    d.text((a.width + pad * 2, pad), right, fill=(230, 230, 230))
    im.save(out)


if __name__ == "__main__":
    main()
