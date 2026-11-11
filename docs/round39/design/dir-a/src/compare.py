"""Slate (dir-a) — the comparison beside FL Studio at the same scale (private: it shows FL).

FL Studio 24.1.2's screenshots are 1920 px wide windows; Slate is shown at 150 % (1920 x
1080, rendered natively); Cantus beta.13's startup.png is 1280 x 720 and is enlarged by
1.5 (Lanczos) — said so on the sheet — so all three windows are 1920 px wide.

python compare.py <design dir> <private dir>
"""
import os
import sys
from PIL import Image, ImageDraw, ImageFont

FL = 'F:/projects/cantus/research/ui/fl-look-2026-11/screenshots/'
FONT = 'F:/projects/daw-r39/fonts/barlow-semi-condensed/BarlowSemiCondensed-Medium.ttf'
FONT_R = 'F:/projects/daw-r39/fonts/barlow-semi-condensed/BarlowSemiCondensed-Regular.ttf'
BG = (24, 28, 33)


def label(img, txt, sub, size=34):
    d = ImageDraw.Draw(img)
    d.text((24, 14), txt, font=ImageFont.truetype(FONT, size), fill=(227, 232, 237))
    d.text((24, 14 + size + 8), sub, font=ImageFont.truetype(FONT_R, 22), fill=(150, 160, 170))


def column(im, title, sub, h=1080):
    c = Image.new('RGB', (1920, 110 + h), BG)
    label(c, title, sub)
    c.paste(im, (0, 110))
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


def main(design, priv):
    os.makedirs(priv, exist_ok=True)
    root = os.path.abspath(os.path.join(design, '..', '..', '..', '..'))
    today = Image.open(os.path.join(root, 'docs', 'manual', 'img', 'startup.png')).convert('RGB')
    today = today.resize((1920, 1080), Image.LANCZOS)
    slate = {k: Image.open(os.path.join(design, f'window-{k}-1.5x.png')).convert('RGB') for k in ('night', 'paper')}
    for name, fl in (('playlist', 'fl-playlist.png'), ('mixer', 'fl-mixer.png')):
        f = Image.open(FL + fl).convert('RGB')
        sheet = row([column(f, f'FL Studio 24.1.2 — {name}', f'{fl} · 1920 x {f.height}, as captured', f.height),
                     column(slate['night'], 'Slate (dir-a) — Cantus Night', 'the design at 150 % · 1920 x 1080, rendered natively'),
                     column(today, 'Cantus beta.13 today', 'docs/manual/img/startup.png · 1280 x 720 enlarged x1.5 (Lanczos)')])
        sheet.save(os.path.join(priv, f'compare-{name}.png'))
        print('  ', os.path.join(priv, f'compare-{name}.png'), sheet.size)
    # an overview at half size: the two FL views, Slate Night and Paper, today
    f1 = Image.open(FL + 'fl-playlist.png').convert('RGB')
    f2 = Image.open(FL + 'fl-mixer.png').convert('RGB')
    top = row([column(f1, 'FL Studio — playlist', '1920 wide', f1.height), column(slate['night'], 'Slate — Night', '150 %'),
               column(today, 'Cantus beta.13', 'x1.5')])
    bot = row([column(f2, 'FL Studio — mixer', '1920 wide', f2.height), column(slate['paper'], 'Slate — Paper', '150 %'),
               column(Image.open(os.path.join(root, 'docs', 'round37', 'finish', 'real-paper.png')).convert('RGB').resize((1920, 991), Image.LANCZOS),
                      'Cantus beta.12 — Paper', 'docs/round37/finish/real-paper.png', 991)])
    ov = Image.new('RGB', (max(top.width, bot.width), top.height + bot.height), BG)
    ov.paste(top, (0, 0))
    ov.paste(bot, (0, top.height))
    ov = ov.resize((ov.width // 2, ov.height // 2), Image.LANCZOS)
    ov.save(os.path.join(priv, 'compare-overview.png'))
    print('  ', os.path.join(priv, 'compare-overview.png'), ov.size)
    # details at 1:1 physical: the top bars, then the mixers
    s = slate['night']
    tb = [('FL Studio', f1.crop((0, 0, 1920, 100))), ('Slate, Night, 150 %', s.crop((0, 0, 1920, 100))),
          ('Cantus beta.13, x1.5', today.crop((0, 0, 1920, 100)))]
    mx = [('FL Studio', f2.crop((490, 360, 1600, 980))), ('Slate, Night, 150 %', s.crop((0, 567, 1110, 1050))),
          ('Cantus beta.13, x1.5', today.crop((0, 530, 1110, 1060)))]
    H = sum(c.height + 56 for _, c in tb) + 40
    det = Image.new('RGB', (1920 + 48, H), BG)
    d = ImageDraw.Draw(det)
    y = 20
    for lab, c in tb:
        d.text((24, y), lab, font=ImageFont.truetype(FONT, 26), fill=(227, 232, 237))
        det.paste(c, (24, y + 36))
        y += c.height + 56
    det.save(os.path.join(priv, 'compare-topbar.png'))
    W = sum(c.width for _, c in mx) + 24 * 4
    det = Image.new('RGB', (W, 660 + 60), BG)
    d = ImageDraw.Draw(det)
    x = 24
    for lab, c in mx:
        d.text((x, 16), lab, font=ImageFont.truetype(FONT, 26), fill=(227, 232, 237))
        det.paste(c, (x, 60))
        x += c.width + 24
    det.save(os.path.join(priv, 'compare-mixer-detail.png'))
    print('  details written')


if __name__ == '__main__':
    main(sys.argv[1], sys.argv[2])
