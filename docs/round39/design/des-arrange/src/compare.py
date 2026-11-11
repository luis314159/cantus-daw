"""des-arrange — beside FL Studio and the others at the same scale (private: it shows them).

python compare.py <design dir> <private dir>
  compare-arrangement.png       FL's playlist (1920 px window, as captured), Slate Night and Paper at
                                125 % on a 1920 × 1080 screen, and today's Cantus (the r39 kit build,
                                125 %): all four 1920 px wide, one above the other
  compare-arrangement-half.png  the same and Ableton Live and Bitwig Studio, at half size, 2 × 3
  compare-headers.png           the track headers cut at their physical pixels, then × 2
"""
import os
import sys
from PIL import Image, ImageDraw, ImageFont

R = 'F:/projects/cantus/research/ui/'
KIT = 'F:/projects/daw-r39/kit/docs/round39/kit/'
FONT = 'F:/projects/daw-r39/fonts/barlow-semi-condensed/BarlowSemiCondensed-Medium.ttf'
BG = (20, 25, 30)
INK = (227, 232, 237)


def stack(items, gap=24, size=26):
    f = ImageFont.truetype(FONT, size)
    W = max(im.width for _, im in items) + 2 * gap
    H = sum(im.height + size + 22 for _, im in items) + gap
    out = Image.new('RGB', (W, H), BG)
    d = ImageDraw.Draw(out)
    y = gap
    for lab, im in items:
        d.text((gap, y), lab, font=f, fill=INK)
        out.paste(im, (gap, y + size + 12))
        y += im.height + size + 22
    return out


def grid(items, cols, cell_w, gap=20, size=20):
    f = ImageFont.truetype(FONT, size)
    ims = []
    for lab, im in items:
        k = cell_w / im.width
        ims.append((lab, im.resize((cell_w, round(im.height * k)), Image.LANCZOS)))
    rows = [ims[i:i + cols] for i in range(0, len(ims), cols)]
    rh = [max(im.height for _, im in r) + size + 16 for r in rows]
    out = Image.new('RGB', (cols * cell_w + (cols + 1) * gap, sum(rh) + (len(rows) + 1) * gap), BG)
    d = ImageDraw.Draw(out)
    y = gap
    for r, h in zip(rows, rh):
        x = gap
        for lab, im in r:
            d.text((x, y), lab, font=f, fill=INK)
            out.paste(im, (x, y + size + 8))
            x += cell_w + gap
        y += h + gap
    return out


def side(items, gap=24, size=22):
    f = ImageFont.truetype(FONT, size)
    W = sum(im.width for _, im in items) + gap * (len(items) + 1)
    H = max(im.height for _, im in items) + size + 40
    out = Image.new('RGB', (W, H), BG)
    d = ImageDraw.Draw(out)
    x = gap
    for lab, im in items:
        d.text((x, 14), lab, font=f, fill=INK)
        out.paste(im, (x, size + 26))
        x += im.width + gap
    return out


def main(design, priv):
    os.makedirs(priv, exist_ok=True)
    fl = Image.open(R + 'fl-look-2026-11/screenshots/fl-playlist.png').convert('RGB')
    sn = Image.open(os.path.join(design, 'window-night-1.25x.png')).convert('RGB')
    sp = Image.open(os.path.join(design, 'window-paper-1.25x.png')).convert('RGB')
    mx = Image.open(os.path.join(design, 'max-night-1.25x.png')).convert('RGB')
    today = Image.open(KIT + 'window-song125-slate_night-1920x1080.png').convert('RGB')
    stack([('FL Studio 24.1.2 — the playlist, 1920 px window, as captured', fl),
           ('Cantus, des-arrange — Slate Night, 125 % on a 1920 × 1080 screen (the default scale there)', sn),
           ('Cantus, des-arrange — Slate Paper, 125 %', sp),
           ('Cantus, des-arrange — the arrangement maximised, Slate Night, 125 %', mx),
           ('Cantus today — the round 39 kit build (Slate tokens on the beta.13 layout), 125 %', today)]
          ).save(os.path.join(priv, 'compare-arrangement.png'))
    ab = Image.open(R + 'screenshots/ableton-live/arrangement-view-full-demo-set.png').convert('RGB')
    bw = Image.open(R + 'screenshots/bitwig-studio/ug-01-blank-project-full-window.png').convert('RGB')
    grid([('FL Studio 24 — playlist', fl), ('Cantus des-arrange — Slate Night 125 %', sn),
          ('Cantus today — r39 kit build 125 %', today), ('Cantus des-arrange — Slate Paper 125 %', sp),
          ('Ableton Live 12 — arrangement (published, Retina)', ab), ('Bitwig Studio — a blank project (published)', bw)],
         2, 960).save(os.path.join(priv, 'compare-arrangement-half.png'))
    # the headers at their physical pixels: FL's column of track names + its playlist's headers,
    # Slate's at 125 %, today's at 125 %
    a = fl.crop((1096, 160, 1440, 740))
    b = sn.crop((0, 90, 560, 670))
    c = today.crop((0, 150, 560, 730))
    one = side([('FL Studio — 1:1', a), ('Slate — 125 %, 1:1 physical', b), ('Cantus today — 125 %, 1:1 physical', c)])
    one.save(os.path.join(priv, 'compare-headers.png'))
    two = side([('FL Studio — × 2', a.crop((180, 40, 344, 340)).resize((328, 600), Image.NEAREST)),
                ('Slate — × 2', b.crop((0, 70, 312, 370)).resize((624, 600), Image.NEAREST)),
                ('Cantus today — × 2', c.crop((0, 50, 280, 350)).resize((560, 600), Image.NEAREST))])
    two.save(os.path.join(priv, 'compare-headers-2x.png'))
    print('compared')


if __name__ == '__main__':
    main(sys.argv[1], sys.argv[2])
