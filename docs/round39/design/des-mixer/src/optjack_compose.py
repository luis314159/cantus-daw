"""des-mixer — the Jack sheet with its 4x crops (nearest-neighbour, the pixels at 125 %).

python optjack_compose.py <render dir> <design dir>
"""
import os
import sys
from PIL import Image, ImageDraw, ImageFont

FONT = 'F:/projects/daw-r39/fonts/barlow-semi-condensed/BarlowSemiCondensed-Medium.ttf'
FONT_R = 'F:/projects/daw-r39/fonts/barlow-semi-condensed/BarlowSemiCondensed-Regular.ttf'
S = 1.25
CW, CH = 470, 240
BG = {'night': (20, 25, 30), 'paper': (197, 205, 213)}
INK = {'night': ((227, 232, 237), (150, 160, 170)), 'paper': ((23, 32, 42), (74, 88, 103))}


def cell(k):
    return 16 + (k % 3) * (CW + 16), 70 + (k // 3) * (CH + 12)


def crops():
    c = []
    px, py = cell(0)
    c.append(('Seated plugs: output, key, send leaving the Kick', (px + 90, py + 136, px + 250, py + 216)))
    px, py = cell(4)
    c.append(('Dragged plug, refused: the red boot', (px + 90, py + 136, px + 250, py + 216)))
    px, py = cell(2)
    c.append(('Hover: the route lifted (shadow farther, highlight up)', (px + 110, py + 136, px + 270, py + 216)))
    vx, vy = 16 + CW + 16, 70 + 2 * (CH + 12)
    c.append(('The vintage look: cloth cords, nickel, bakelite', (vx + 30, vy + 120, vx + 190, vy + 200)))
    return [(n, tuple(round(v * S) for v in b)) for n, b in c]


def main(render, design):
    for th in ('night', 'paper'):
        sheet = Image.open(os.path.join(render, f'jk-{th}.png')).convert('RGB')
        tiles = [(n, sheet.crop(b)) for n, b in crops()]
        tiles = [(n, t.resize((t.width * 4, t.height * 4), Image.NEAREST)) for n, t in tiles]
        gap = 24
        rows = [tiles[:2], tiles[2:]]
        width = max(sheet.width, max(sum(t.width for _, t in r) + gap * 3 for r in rows))
        out = Image.new('RGB', (width, sheet.height + 70 + sum(max(t.height for _, t in r) + 34 + gap for r in rows)), BG[th])
        out.paste(sheet, (0, 0))
        d = ImageDraw.Draw(out)
        y = sheet.height + 16
        d.text((gap, y), '4× crops (nearest-neighbour: the real pixels at 125 %)', font=ImageFont.truetype(FONT, 22), fill=INK[th][0])
        y += 54
        for r in rows:
            x = gap
            for n, t in r:
                d.text((x, y), n, font=ImageFont.truetype(FONT_R, 18), fill=INK[th][1])
                out.paste(t, (x, y + 28))
                x += t.width + gap
            y += max(t.height for _, t in r) + 34 + gap
        p = os.path.join(design, f'options-cables-jack-{th}.png')
        out.save(p)
        print(' ', p, out.size)


if __name__ == '__main__':
    main(sys.argv[1], sys.argv[2])
