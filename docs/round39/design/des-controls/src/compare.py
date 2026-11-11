"""des-controls — the comparison sheets (private: they show FL Studio).

FL Studio 24.1.2 | Slate (this sheet set) | Cantus beta.13, at the same scale, then x4.
Writes to research/ui/round39/des-controls/ (git add -f; stripped from the public repo).

python compare.py
"""
import os
from PIL import Image, ImageDraw, ImageFont

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.normpath(os.path.join(HERE, '..'))
WT = os.path.normpath(os.path.join(OUT, '..', '..', '..', '..'))
PRIV = os.path.join(WT, 'research', 'ui', 'round39', 'des-controls')
FL = r'F:\projects\cantus\research\ui\fl-look-2026-11\screenshots'
FLS = r'F:\projects\cantus\research\ui\screenshots\fl-studio'
FONT = r'F:\projects\daw-r39\fonts\barlow-semi-condensed\BarlowSemiCondensed-Medium.ttf'
FONT_R = r'F:\projects\daw-r39\fonts\barlow-semi-condensed\BarlowSemiCondensed-Regular.ttf'
BG = (16, 20, 24)


def img(p, box=None, k=1):
    im = Image.open(p).convert('RGB')
    if box:
        im = im.crop(box)
    if k != 1:
        im = im.resize((round(im.width * k), round(im.height * k)), Image.NEAREST)
    return im


def column(title, sub, parts, gap=16):
    """Stack images under a caption."""
    f1 = ImageFont.truetype(FONT, 22)
    f2 = ImageFont.truetype(FONT_R, 15)
    w = max(p.width for p in parts)
    h = 64 + sum(p.height for p in parts) + gap * (len(parts) - 1)
    col = Image.new('RGB', (w, h), BG)
    d = ImageDraw.Draw(col)
    d.text((0, 6), title, font=f1, fill=(226, 231, 236))
    d.text((0, 36), sub, font=f2, fill=(140, 150, 160))
    y = 64
    for p in parts:
        col.paste(p, (0, y))
        y += p.height + gap
    return col


def row(cols, title, gap=40, pad=32):
    f = ImageFont.truetype(FONT, 26)
    w = sum(c.width for c in cols) + gap * (len(cols) - 1) + 2 * pad
    h = max(c.height for c in cols) + 2 * pad + 50
    im = Image.new('RGB', (w, h), BG)
    ImageDraw.Draw(im).text((pad, pad - 6), title, font=f, fill=(240, 244, 248))
    x = pad
    for c in cols:
        im.paste(c, (x, pad + 50))
        x += c.width + gap
    return im


def main():
    os.makedirs(PRIV, exist_ok=True)
    mixer = os.path.join(FL, 'fl-mixer.png')
    eq = os.path.join(FL, 'fl-parametric-eq.png')
    cur = os.path.join(WT, 'docs', 'round37', 'finish')
    # 1 — knobs, faders, meters, slots at 1:1
    fl = column('FL Studio 24.1.2', '1:1 physical pixels, 1920 × 1020 window',
                [img(mixer, (670, 560, 1010, 900)), img(mixer, (1615, 395, 1895, 560)), img(eq, (1225, 700, 1450, 790))])
    sl = column('Slate — des-controls', '100 %, Night, drawn for this scale',
                [img(os.path.join(OUT, 'faders-night-1x.png'), (20, 100, 372, 372)),
                 img(os.path.join(OUT, 'knobs-night-1x.png'), (100, 190, 560, 300)),
                 img(os.path.join(OUT, 'faders-night-1x.png'), (20, 580, 500, 700))])
    cb = column('Cantus beta.13 (today)', '100 %, Night', [img(os.path.join(cur, 'controls-night-100.png')),
                                                            img(os.path.join(cur, 'strip-buttons-night.png'), (0, 0, 486, 208))])
    row([fl, sl, cb], 'Knobs, faders, meters and slots — the same scale').save(os.path.join(PRIV, 'compare-controls-1x.png'), optimize=True)
    # 2 — x4, nearest-neighbour: one knob, one fader cap with its meter
    fl4 = column('FL Studio', 'x4 nearest', [img(mixer, (678, 583, 728, 633), 4), img(mixer, (690, 660, 740, 720), 4)])
    sl4 = column('Slate', 'x4 nearest', [img(os.path.join(OUT, 'knobs-night-1x.png'), (125, 205, 175, 255), 4),
                                         img(os.path.join(OUT, 'faders-night-1x.png'), (28, 138, 78, 198), 4)])
    cb4 = column('Cantus beta.13', 'x4 nearest', [img(os.path.join(cur, 'controls-night-100.png'), (85, 35, 135, 85), 4),
                                                  img(os.path.join(cur, 'controls-night-100.png'), (19, 203, 69, 263), 4)])
    row([fl4, sl4, cb4], 'One knob, one fader cap and meter — x4').save(os.path.join(PRIV, 'compare-controls-4x.png'), optimize=True)
    # 3 — menus and dialogs
    flm = column('FL Studio', 'its manual\'s picture (about 125 %)', [img(os.path.join(FLS, '42-playlist-track-headers-context-menu.png'), (380, 150, 640, 416))])
    slm = column('Slate', '150 %, Night and Paper', [img(os.path.join(OUT, 'menus-night-1.5x.png'), (840, 140, 1500, 600)),
                                                     img(os.path.join(OUT, 'dialogs-paper-1x.png'), (100, 420, 560, 670))])
    cbm = column('Cantus beta.13', 'a dialog today, 100 %', [img(os.path.join(WT, 'docs', 'manual', 'img', 'transcribe-dialog.png'), (300, 40, 900, 680))])
    row([flm, slm, cbm], 'Menus and dialogs').save(os.path.join(PRIV, 'compare-menus-dialogs.png'), optimize=True)
    for n in ('compare-controls-1x.png', 'compare-controls-4x.png', 'compare-menus-dialogs.png'):
        print(os.path.join(PRIV, n))


if __name__ == '__main__':
    main()
