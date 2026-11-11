"""des-mixer — the cables sheet with its 4x crops (nearest-neighbour, the pixels at 125 %).

python optcab_compose.py <render dir> <design dir>
"""
import os
import sys
from PIL import Image, ImageDraw, ImageFont

FONT = 'F:/projects/daw-r39/fonts/barlow-semi-condensed/BarlowSemiCondensed-Medium.ttf'
FONT_R = 'F:/projects/daw-r39/fonts/barlow-semi-condensed/BarlowSemiCondensed-Regular.ttf'
S = 1.25
CW, CH = 470, 206
BG = {'night': (20, 25, 30), 'paper': (197, 205, 213)}
INK = {'night': ((227, 232, 237), (150, 160, 170)), 'paper': ((23, 32, 42), (74, 88, 103))}


def cell_xy(row, k):
    y = 74 + row * (28 + 2 * (CH + 12) + 14) + 28
    return 16 + (k % 3) * (CW + 16), y + (k // 3) * (CH + 12)


def crops():
    out = []
    for row, name in ((0, 'A'), (1, 'B'), (2, 'C')):
        px, py = cell_xy(row, 0)
        out.append((f'{name} · Kick selected: key, output, send leaving the Kick', (px + 96, py + 124, px + 256, py + 200)))
    for row, k, name in ((0, 2, 'A · hover: the route lit, the rest dimmed'), (1, 1, 'B · the gleams while playing'),
                         (0, 4, 'A · drag refused: the loop')):
        px, py = cell_xy(row, k)
        x0 = {2: px + 150, 1: px + 230, 4: px + 80}[k]
        out.append((name, (x0, py + 124, x0 + 160, py + 200)))
    return [(n, tuple(round(v * S) for v in b)) for n, b in out]


def main(render, design):
    for th in ('night', 'paper'):
        sheet = Image.open(os.path.join(render, f'cab-{th}.png')).convert('RGB')
        tiles = [(n, sheet.crop(b)) for n, b in crops()]
        tiles = [(n, c.resize((c.width * 4, c.height * 4), Image.NEAREST)) for n, c in tiles]
        gap = 24
        rows = [tiles[:3], tiles[3:]]
        width = max(sheet.width, max(sum(t.width for _, t in r) + gap * 4 for r in rows))
        hh = [max(t.height for _, t in r) + 34 for r in rows]
        out = Image.new('RGB', (width, sheet.height + 70 + sum(hh) + 2 * gap), BG[th])
        out.paste(sheet, (0, 0))
        d = ImageDraw.Draw(out)
        y = sheet.height + 16
        d.text((gap, y), '4× crops (nearest-neighbour: the real pixels at 125 %)', font=ImageFont.truetype(FONT, 22), fill=INK[th][0])
        y += 54
        for r, h in zip(rows, hh):
            x = gap
            for n, t in r:
                d.text((x, y), n, font=ImageFont.truetype(FONT_R, 18), fill=INK[th][1])
                out.paste(t, (x, y + 28))
                x += t.width + gap
            y += h + gap
        p = os.path.join(design, f'options-cables-{th}.png')
        out.save(p)
        print(' ', p, out.size)


if __name__ == '__main__':
    main(sys.argv[1], sys.argv[2])
