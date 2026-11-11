"""Slate (dir-a), phase B — beside FL Studio at the same scale (private: it shows FL).

1. The top bar: FL's (1920 px window, as captured) above Slate's at 1920 logical px (1×)
   and Slate's 1280 window at 150 % (1920 physical).
2. Settings: FL's Settings (as published, 1:1) beside Slate's Appearance and Audio pages
   cut from the 1× render.

python compare_b.py <dir of b-*.png> <private dir>
"""
import os
import sys
from PIL import Image, ImageDraw, ImageFont

FL = 'F:/projects/cantus/research/ui/'
FONT = 'F:/projects/daw-r39/fonts/barlow-semi-condensed/BarlowSemiCondensed-Medium.ttf'
BG = (24, 28, 33)


def stack(items, gap=20):
    f = ImageFont.truetype(FONT, 24)
    W = max(im.width for _, im in items) + 2 * gap
    H = sum(im.height + 44 for _, im in items) + gap
    out = Image.new('RGB', (W, H), BG)
    d = ImageDraw.Draw(out)
    y = gap
    for lab, im in items:
        d.text((gap, y), lab, font=f, fill=(227, 232, 237))
        out.paste(im, (gap, y + 34))
        y += im.height + 44
    return out


def side(items, gap=24):
    f = ImageFont.truetype(FONT, 24)
    W = sum(im.width for _, im in items) + gap * (len(items) + 1)
    H = max(im.height for _, im in items) + 70
    out = Image.new('RGB', (W, H), BG)
    d = ImageDraw.Draw(out)
    x = gap
    for lab, im in items:
        d.text((x, 16), lab, font=f, fill=(227, 232, 237))
        out.paste(im, (x, 54))
        x += im.width + gap
    return out


def main(b, priv):
    design = os.path.abspath(os.path.join(b, '..'))
    fl = Image.open(FL + 'fl-look-2026-11/screenshots/fl-playlist.png').convert('RGB').crop((0, 0, 1920, 100))
    sl = Image.open(os.path.join(b, 'b-topbar-widths-night-1x.png')).convert('RGB').crop((24, 98, 1944, 142))
    s15 = Image.open(os.path.join(design, 'window-night-1.5x.png')).convert('RGB').crop((0, 0, 1920, 66))
    stack([('FL Studio 24.1.2 — top rows, 1920 px window, as captured', fl),
           ('Slate Night — the bar at 1920 logical px, 100 %', sl),
           ('Slate Night — the 1280 window at 150 % (1920 physical)', s15)]).save(os.path.join(priv, 'compare-b-topbar.png'))
    fls = Image.open(FL + 'screenshots/fl-studio/19-settings-theme-hue-saturation-brightness.png').convert('RGB')
    look = Image.open(os.path.join(b, 'dlg-settings-look-night-1x.png')).convert('RGB').crop((186, 54, 1094, 666))
    aud = Image.open(os.path.join(b, 'dlg-settings-audio-night-1x.png')).convert('RGB').crop((186, 54, 1094, 666))
    side([('FL Studio — Settings, Theme (published, 1:1)', fls), ('Slate — Settings, Appearance (1×)', look),
          ('Slate — Settings, Audio (1×)', aud)]).save(os.path.join(priv, 'compare-b-settings.png'))
    print('compared')


if __name__ == '__main__':
    main(sys.argv[1], sys.argv[2])
