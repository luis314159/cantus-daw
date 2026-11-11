"""des-mixer — beside FL Studio and the other DAWs (private: it shows other products).

1. compare-mixer.png: FL Studio 24.1.2's mixer window (1920 px, as captured) | the Slate
   mixer maximised at 150 % (1920 x 1080, rendered natively) | Cantus beta.12's window at
   150 % (round 37's real-night.png, 1920 x 991, as captured).
2. compare-strips-1to1.png: strips at their physical pixels on a 1920-px screen, then x3
   nearest-neighbour: FL's (100 %), Slate's (150 %), today's Cantus (150 %).
3. compare-daws.png: Ableton, Bitwig, Logic, Studio One mixers as captured (their scale is
   the capture's), beside Slate Large and Wide, each scaled to the same height.

python compare.py <design dir> <private dir>
"""
import os
import sys
from PIL import Image, ImageDraw, ImageFont

FL = 'F:/projects/cantus/research/ui/fl-look-2026-11/screenshots/fl-mixer.png'
SHOTS = 'F:/projects/cantus/research/ui/screenshots/'
FONT = 'F:/projects/daw-r39/fonts/barlow-semi-condensed/BarlowSemiCondensed-Medium.ttf'
FONT_R = 'F:/projects/daw-r39/fonts/barlow-semi-condensed/BarlowSemiCondensed-Regular.ttf'
BG = (24, 28, 33)


def label(img, txt, sub, size=30):
    d = ImageDraw.Draw(img)
    d.text((20, 12), txt, font=ImageFont.truetype(FONT, size), fill=(227, 232, 237))
    d.text((20, 12 + size + 6), sub, font=ImageFont.truetype(FONT_R, 19), fill=(150, 160, 170))


def column(im, title, sub, head=96):
    c = Image.new('RGB', (im.width, head + im.height), BG)
    label(c, title, sub)
    c.paste(im, (0, head))
    return c


def row(cols, gap=24):
    w = sum(c.width for c in cols) + gap * (len(cols) + 1)
    h = max(c.height for c in cols) + 2 * gap
    out = Image.new('RGB', (w, h), BG)
    x = gap
    for c in cols:
        out.paste(c, (x, gap))
        x += c.width + gap
    return out


def stack(rows, gap=0):
    w = max(r.width for r in rows)
    out = Image.new('RGB', (w, sum(r.height for r in rows)), BG)
    y = 0
    for r in rows:
        out.paste(r, (0, y))
        y += r.height
    return out


def x3(im):
    return im.resize((im.width * 3, im.height * 3), Image.NEAREST)


def fit_h(im, h):
    return im.resize((round(im.width * h / im.height), h), Image.LANCZOS)


def main(design, priv):
    os.makedirs(priv, exist_ok=True)
    root = os.path.abspath(os.path.join(design, '..', '..', '..', '..'))
    fl = Image.open(FL).convert('RGB')
    slate = Image.open(os.path.join(design, 'mixer-wide-night-1.5x.png')).convert('RGB')
    paper = Image.open(os.path.join(design, 'mixer-wide-paper-1.5x.png')).convert('RGB')
    today = Image.open(os.path.join(root, 'docs', 'round37', 'finish', 'real-night.png')).convert('RGB')
    sheet = stack([row([column(fl, 'FL Studio 24.1.2 — the mixer', 'fl-mixer.png · 1920 x 1020, as captured'),
                        column(slate, 'Slate — Cantus Night, the mixer maximised', 'des-mixer · 150 % · 1920 x 1080, rendered natively')]),
                   row([column(paper, 'Slate — Cantus Paper', 'des-mixer · 150 % · 1920 x 1080'),
                        column(today, 'Cantus beta.12 today (round 37)', 'docs/round37/finish/real-night.png · 150 % · 1920 x 991, as captured')])])
    sheet.save(os.path.join(priv, 'compare-mixer.png'))
    print(' ', os.path.join(priv, 'compare-mixer.png'), sheet.size)
    half = sheet.resize((sheet.width // 2, sheet.height // 2), Image.LANCZOS)
    half.save(os.path.join(priv, 'compare-mixer-half.png'))
    # strips at their physical pixels, x3
    fl_strips = fl.crop((664, 388, 900, 975))                  # FL: strips 2-5 (Wide layout)
    sl_strips = slate.crop((126, 111, 504, 1047))              # Slate 150 %: Kick … Percussion
    td_strips = today.crop((684, 596, 1066, 960))              # today 150 %: Kick … Percussion
    sheet2 = row([column(fl_strips, 'FL Studio', '4 strips, 1:1 physical'),
                  column(sl_strips, 'Slate (Night)', '4 strips, 150 %, 1:1 physical'),
                  column(td_strips, 'Cantus beta.12', '4 strips, 150 %, 1:1 physical')])
    sheet2.save(os.path.join(priv, 'compare-strips-1to1.png'))
    print(' ', os.path.join(priv, 'compare-strips-1to1.png'), sheet2.size)
    z = row([column(x3(fl.crop((664, 388, 778, 640))), 'FL Studio x3', 'two strips, top'),
             column(x3(slate.crop((126, 111, 312, 520))), 'Slate x3', 'two strips, top (150 %)'),
             column(x3(today.crop((684, 596, 874, 830))), 'Cantus beta.12 x3', 'two strips, top (150 %)')])
    z.save(os.path.join(priv, 'compare-strips-3x.png'))
    print(' ', os.path.join(priv, 'compare-strips-3x.png'), z.size)
    # the other DAWs, as captured, scaled to one height
    H = 900
    refs = [('ableton-live/mixer-tall-meters-wide-strips.png', 'Ableton Live 12'),
            ('bitwig-studio/ug-08-mixer-big-meters.png', 'Bitwig Studio'),
            ('logic-studio-one/logic-mixer-window.png', 'Logic Pro'),
            ('logic-studio-one/s1-console-large-narrow.png', 'Studio One')]
    cols = []
    for p, name in refs:
        im = Image.open(SHOTS + p).convert('RGB')
        cols.append(column(fit_h(im, H), name, f'{os.path.basename(p)} · scaled to {H} px high'))
    large = Image.open(os.path.join(design, 'mixer-large-night-1x.png')).convert('RGB').crop((0, 46, 1032, 700))
    cols.append(column(fit_h(large, H), 'Slate Large', 'des-mixer · 1x, scaled to 900 high'))
    top = row(cols[:3])
    bottom = row(cols[3:])
    sheet3 = stack([top, bottom])
    sheet3.save(os.path.join(priv, 'compare-daws.png'))
    print(' ', os.path.join(priv, 'compare-daws.png'), sheet3.size)


if __name__ == '__main__':
    main(sys.argv[1], sys.argv[2])
