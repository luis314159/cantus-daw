"""des-views — beside FL Studio and the others at the same scale (private: it shows other
products). FL Studio 24.1.2's screenshots are 1920-px windows (150 % displays); Slate is shown
at 150 %, rendered natively; Cantus beta.13's pictures are 1:1 and are enlarged x1.5
(Lanczos) — each sheet says so.

python compare.py <design dir> <private dir>
"""
import os
import sys
from PIL import Image, ImageDraw, ImageFont

FL = 'F:/projects/cantus/research/ui/fl-look-2026-11/screenshots/'
OTHER = 'F:/projects/cantus/research/ui/screenshots/'
FONT = 'F:/projects/daw-r39/fonts/barlow-semi-condensed/BarlowSemiCondensed-Medium.ttf'
FONT_R = 'F:/projects/daw-r39/fonts/barlow-semi-condensed/BarlowSemiCondensed-Regular.ttf'
BG = (24, 28, 33)


def column(im, title, sub):
    c = Image.new('RGB', (im.width, 110 + im.height), BG)
    d = ImageDraw.Draw(c)
    d.text((16, 14), title, font=ImageFont.truetype(FONT, 34), fill=(227, 232, 237))
    d.text((16, 56), sub, font=ImageFont.truetype(FONT_R, 22), fill=(150, 160, 170))
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


def x15(im):
    return im.resize((round(im.width * 1.5), round(im.height * 1.5)), Image.LANCZOS)


def nn(im, k):
    return im.resize((im.width * k, im.height * k), Image.NEAREST)


def save(img, path):
    img.save(path)
    print('  ', path, img.size)


def main(design, priv):
    os.makedirs(priv, exist_ok=True)
    root = os.path.abspath(os.path.join(design, '..', '..', '..', '..'))
    img = lambda *p: Image.open(os.path.join(root, *p)).convert('RGB')  # noqa: E731
    mine = lambda n: Image.open(os.path.join(design, n)).convert('RGB')  # noqa: E731
    fl_pr = Image.open(FL + 'fl-piano-roll.png').convert('RGB')
    fl_cr = Image.open(FL + 'fl-channel-rack.png').convert('RGB')

    # 1. piano roll, whole windows
    save(row([column(fl_pr, 'FL Studio 24.1.2 — piano roll', 'fl-piano-roll.png · 1920 x 1020, as captured'),
              column(mine('pianoroll-night-1.5x.png'), 'Slate (des-views) — Night', 'the design at 150 % · 1920 x 1080, rendered natively'),
              column(x15(img('docs', 'images', 'pianoroll.png')), 'Cantus beta.13 today', 'docs/images/pianoroll.png · 1240 x 640 enlarged x1.5 (Lanczos)')]),
         os.path.join(priv, 'compare-pianoroll.png'))

    # 2. the keyboards, physical pixels at 150 %, then x3 nearest: the same scale for all
    fk = fl_pr.crop((963, 280, 1048, 780))
    sk = mine('pianoroll-night-1.5x.png').crop((0, 147, 108, 647))
    sp = mine('pianoroll-paper-1.5x.png').crop((0, 147, 108, 647))
    tk = x15(img('docs', 'images', 'pianoroll.png')).crop((0, 99, 96, 599))
    ab = Image.open(OTHER + 'ableton-live/clip-view-midi.png').convert('RGB').crop((760, 160, 820, 500))
    save(row([column(nn(fk, 3), 'FL Studio', 'x3 nearest'), column(nn(sk, 3), 'Slate Night', '150 %, x3'),
              column(nn(sp, 3), 'Slate Paper', '150 %, x3'), column(nn(tk, 3), 'beta.13', 'x1.5 Lanczos, x3'),
              column(nn(ab, 3), 'Ableton Live', 'clip view, x3 (its own scale)')]),
         os.path.join(priv, 'compare-keyboard-3x.png'))

    # 3. channel rack
    save(row([column(fl_cr, 'FL Studio 24.1.2 — channel rack', 'fl-channel-rack.png · 1920 x 1020, as captured'),
              column(mine('rack-browser-night-1.5x.png'), 'Slate (des-views) — Night', 'browser and rack at 150 % · rendered natively'),
              column(x15(img('docs', 'images', 'rack.png')), 'Cantus beta.13 today', 'docs/images/rack.png · 1240 x 640 enlarged x1.5 (Lanczos)')]),
         os.path.join(priv, 'compare-rack.png'))

    # 4. browsers, the panels alone at 150 %
    fb = fl_pr.crop((0, 100, 460, 1020))
    sb = mine('rack-browser-night-1.5x.png').crop((0, 69, 432, 1047))
    ss = mine('browser-pages-night-1.5x.png').crop((18, 69, 450, 1047))
    tb = x15(img('docs', 'manual', 'img', 'browser.png'))
    bw = Image.open(OTHER + 'bitwig-studio/ug-13-browser-panel.png').convert('RGB')
    bw = bw.resize((round(bw.width * 978 / bw.height), 978), Image.LANCZOS)
    save(row([column(fb, 'FL Studio — browser', 'from fl-piano-roll.png, 1:1'), column(sb, 'Slate — tree', '150 %'),
              column(ss, 'Slate — search', '150 %'), column(tb, 'beta.13', 'manual/img/browser.png x1.5'),
              column(bw, 'Bitwig Studio', 'user guide, scaled to height')]),
         os.path.join(priv, 'compare-browser.png'))

    # 5. clip content: FL's playlist clips | Slate at 44 px a bar, 150 % | beta.13's arrangement
    fp = Image.open(FL + 'fl-playlist.png').convert('RGB').crop((1300, 200, 1890, 1010))
    sc = mine('clips-night-1.5x.png').crop((0, 560, 1920, 980))
    st = x15(img('docs', 'manual', 'img', 'startup.png')).crop((0, 140, 1300, 700))
    top = row([column(fp, 'FL Studio — playlist clips', 'fl-playlist.png, 1:1'), column(st, 'Cantus beta.13', 'startup.png x1.5 (Lanczos)')])
    save(row([column(top, 'Clip content', 'FL and beta.13 above; Slate at 44 px a bar below')]), os.path.join(priv, 'compare-clips-a.png'))
    save(column(sc, 'Slate — clip content at 44 px a bar', 'clips-night-1.5x.png, the Default section, 150 %'), os.path.join(priv, 'compare-clips-b.png'))


if __name__ == '__main__':
    main(sys.argv[1], sys.argv[2])
