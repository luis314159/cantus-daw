"""des-mixer — the options sheet with its 4x crops under it (nearest-neighbour, the real
pixels at 125 %), and the private comparison of the key options beside FL, Ableton and
Bitwig as captured.

python options_compose.py <render dir> <design dir> <private dir>
"""
import os
import sys
from PIL import Image, ImageDraw, ImageFont

FONT = 'F:/projects/daw-r39/fonts/barlow-semi-condensed/BarlowSemiCondensed-Medium.ttf'
FONT_R = 'F:/projects/daw-r39/fonts/barlow-semi-condensed/BarlowSemiCondensed-Regular.ttf'
S = 1.25
ROW0, ROW_H, CAB_Y, H_DESK = 92, 226, 800, 360
BG = {'night': (20, 25, 30), 'paper': (197, 205, 213)}
INK = {'night': ((227, 232, 237), (150, 160, 170)), 'paper': ((23, 32, 42), (74, 88, 103))}


def box(lx0, ly0, lx1, ly1):
    return tuple(round(v * S) for v in (lx0, ly0, lx1, ly1))


def cuts():
    out = []
    for r, opt in enumerate('ABC'):
        ry = ROW0 + r * ROW_H
        out.append((f'Keys {opt} · Wide, strips 1–2', box(394, ry + 34, 522, ry + 206)))
    for r, opt in enumerate('ABC'):
        ry = ROW0 + r * ROW_H
        out.append((f'Keys {opt} · Compact', box(268, ry + 34, 374, ry + 206)))
    for k, opt in ((0, 'A'), (1, 'B'), (3, 'C')):
        px = 24 + (k % 2) * 496
        dy = CAB_Y + 8 + (k // 2) * 480 + 40
        out.append((f'Cables {opt} · Kick to Bass', box(px + 70, dy + 300, px + 200, dy + H_DESK + 46)))
    return out


def label(d, x, y, s, th, big=True):
    d.text((x, y), s, font=ImageFont.truetype(FONT if big else FONT_R, 22 if big else 18), fill=INK[th][0 if big else 1])


def compose(render, design, th):
    sheet = Image.open(os.path.join(render, f'opt-{th}.png')).convert('RGB')
    tiles = []
    for name, b in cuts():
        c = sheet.crop(b)
        tiles.append((name, c.resize((c.width * 4, c.height * 4), Image.NEAREST)))
    gap = 24
    rows = [tiles[0:3], tiles[3:6], tiles[6:9]]
    width = max(sheet.width, max(sum(t.width for _, t in r) + gap * (len(r) + 1) for r in rows))
    heights = [max(t.height for _, t in r) + 40 for r in rows]
    out = Image.new('RGB', (width, sheet.height + 70 + sum(heights) + gap * 3), BG[th])
    out.paste(sheet, (0, 0))
    d = ImageDraw.Draw(out)
    y = sheet.height + 16
    label(d, gap, y, '4× crops of the sheet above (nearest-neighbour: the real pixels at 125 %)', th)
    y += 54
    for r in rows:
        x = gap
        for name, t in r:
            label(d, x, y, name, th, False)
            out.paste(t, (x, y + 30))
            x += t.width + gap
        y += max(t.height for _, t in r) + 40 + gap
    p = os.path.join(design, f'options-keys-cables-{th}.png')
    out.save(p)
    print(' ', p, out.size)
    return sheet


def private(sheet, priv):
    """Keys beside other DAWs' keys, each as captured (their scale is the capture's)."""
    os.makedirs(priv, exist_ok=True)
    shots = 'F:/projects/cantus/research/ui/'
    refs = [(shots + 'fl-look-2026-11/screenshots/fl-mixer.png', (664, 556, 900, 700), 'FL Studio 24 · mute dots, pan, as captured'),
            (shots + 'screenshots/bitwig-studio/crop-05-bws6-mixer-strip.png', (0, 222, 450, 282), 'Bitwig 6 · ● S M, as captured'),
            (shots + 'screenshots/ableton-live/mixer-tall-meters-wide-strips.png', (660, 1460, 1000, 1590), 'Ableton Live 12 · number, S, arm, as captured')]
    cols = [(n, Image.open(p).convert('RGB').crop(b)) for p, b, n in refs]
    for name, b in cuts()[:3]:
        cols.append((name + ' · 125 %', sheet.crop(b)))
    gap = 24
    w = max(c.width for _, c in cols) + 2 * gap
    h = sum(c.height + 40 for _, c in cols) + gap * (len(cols) + 1)
    out = Image.new('RGB', (w, h), BG['night'])
    d = ImageDraw.Draw(out)
    y = gap
    for n, c in cols:
        label(d, gap, y, n, 'night', False)
        out.paste(c, (gap, y + 30))
        y += c.height + 40 + gap
    p = os.path.join(priv, 'options-keys-beside-daws.png')
    out.save(p)
    print(' ', p, out.size)


if __name__ == '__main__':
    render, design, priv = sys.argv[1:4]
    s = None
    for th in ('night', 'paper'):
        sh = compose(render, design, th)
        if th == 'night':
            s = sh
    private(s, priv)
