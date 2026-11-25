"""r42:arcus-upper — pictures side by side at one height, each labelled (the hands' 4-up:
photo 4, our render from photo 4's angle, our picture's camera, photo 3; High beside Light).

    python tools/arcus-figures/nup.py <out.png> <height> <image>[@x0,y0,x1,y1]=<label> ...

``@x0,y0,x1,y1`` crops that image first (its pixels). Images keep their proportions.
"""

import sys

from PIL import Image, ImageDraw, ImageFont


def font(size):
    for name in ("segoeui.ttf", "arial.ttf", "DejaVuSans.ttf"):
        try:
            return ImageFont.truetype(name, size)
        except OSError:
            continue
    return ImageFont.load_default()


def main():
    out, h = sys.argv[1], int(sys.argv[2])
    ims, labels = [], []
    for spec in sys.argv[3:]:
        path, _, label = spec.partition("=")
        path, _, box = path.partition("@")
        im = Image.open(path).convert("RGB")
        if box:
            im = im.crop(tuple(int(v) for v in box.split(",")))
        ims.append(im.resize((max(1, int(im.width * h / im.height)), h), Image.LANCZOS))
        labels.append(label)
    pad, band = 12, 30
    w = sum(i.width for i in ims) + pad * (len(ims) + 1)
    sheet = Image.new("RGB", (w, h + pad * 2 + band), (24, 24, 28))
    d = ImageDraw.Draw(sheet)
    f = font(15)
    x = pad
    for im, label in zip(ims, labels):
        sheet.paste(im, (x, pad + band))
        d.text((x, pad + 4), label, fill=(232, 232, 232), font=f)
        x += im.width + pad
    sheet.save(out)


if __name__ == "__main__":
    main()
