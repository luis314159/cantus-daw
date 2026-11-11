"""Round 39 (el-mixer): des-mixer's sheet beside the built window, one above the other at
1:1, for looking at what differs. Cantus pictures only (the design and the build).

    python -I tools/look/mixer39_compare.py <design.png> <built.png> <out.png> [label…]
"""
import sys
from PIL import Image, ImageDraw


def main():
    design, built, out = sys.argv[1:4]
    a = Image.open(design).convert('RGB')
    b = Image.open(built).convert('RGB')
    if b.size != a.size:
        b = b.resize(a.size, Image.LANCZOS)
    w, h = a.size
    pad = 22
    sheet = Image.new('RGB', (w, 2 * h + 2 * pad), (20, 25, 30))
    d = ImageDraw.Draw(sheet)
    d.text((8, 5), 'des-mixer (approved sheet)', fill=(200, 210, 220))
    sheet.paste(a, (0, pad))
    d.text((8, h + pad + 5), 'built (shoot_mixer39)', fill=(200, 210, 220))
    sheet.paste(b, (0, h + 2 * pad))
    sheet.save(out)


if __name__ == '__main__':
    main()
