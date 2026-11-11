"""Delay's window — the design sheet (round 39, `delay`).

    python -I sheet.py            # writes ../sheet-{night,paper}-{1x,2x}.png and compare-*.png

Drawn as SVG at the window's designed size (1000 x 640 at 100 %), then rendered by Edge
headless at device scale 1 and 2. The echoes are the DSP's own model (`cantus_dsp::delay`):
the first repeat at the time, unfiltered; each later one `feedback` lower and once more
through the one-pole tone filter; ping-pong puts the first on the left and alternates.
"""
import math
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.normpath(os.path.join(HERE, '..'))
BUILD = os.path.join(HERE, 'build')
ROOT = os.path.normpath(os.path.join(HERE, '..', '..', '..', '..'))
FONTS = 'file:///' + os.path.join(ROOT, 'crates', 'cantus-ui', 'assets', 'fonts').replace('\\', '/') + '/'
EDGE = r'C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'

W, H = 1000, 640
PAD = 24
NOTES_H = 300

# The face (kit/skin/tone.rs `DELAY`), resolved per theme for the drawing.
NIGHT = dict(
    name='night', page='#14191e', body='#2a2124', card='#30272a', card_hi='#372d30',
    head='#342a2d', band='#3a2f32', well='#1d1719', edge='#140f11', lip='#43373a',
    ink1='#eee5e7', ink2='#c6b8bb', ink3='#a6979b', ink4='#5c4f53',
    disp='#120c0e', grid='#2b1c21', grid2='#3d2a30', dtext='#e9dde0', ddim='#a8949a',
    plate='#1e1417', acc='#ef6c79', acc2='#bc674e', knob='#47393c', knob_hi='#5a4a4e',
    skirt='#251c1f', ab='#5cafe9', ab_ink='#0d1822', meter='#5dce8a',
)
PAPER = dict(
    name='paper', page='#c5cdd5', body='#ebe3e5', card='#f3eeef', card_hi='#faf7f8',
    head='#e3d8db', band='#dccfd2', well='#d9cfd2', edge='#b9a9ae', lip='#fbf8f9',
    ink1='#1f171a', ink2='#40343a', ink3='#5d5056', ink4='#ad9fa4',
    disp='#1d1418', grid='#33232a', grid2='#47323a', dtext='#f1e6e9', ddim='#b39fa6',
    plate='#281b20', acc='#f07a86', acc2='#c4775f', knob='#d6c9cd', knob_hi='#e8dee1',
    skirt='#a8979c', ab='#1b64b2', ab_ink='#ffffff', meter='#2fa868',
)

# The sheet's state: the "Dotted eighth" preset at 120 BPM.
TIME_MS, FEEDBACK, TONE_HZ, PING_PONG, MIX = 375.0, 0.42, 4500.0, True, 0.20
BPM = 120.0
SR = 48000.0
REF_HZ = 8000.0


def esc(s):
    return s.replace('&', '&amp;').replace('<', '&lt;').replace('>', '&gt;')


class Svg:
    def __init__(self, w, h, scale):
        self.w, self.h, self.s = w, h, scale
        self.parts = []

    def add(self, x):
        self.parts.append(x)

    def rect(self, x, y, w, h, fill, r=0, stroke=None, sw=1):
        st = f' stroke="{stroke}" stroke-width="{sw}"' if stroke else ''
        self.add(f'<rect x="{x:.2f}" y="{y:.2f}" width="{w:.2f}" height="{h:.2f}" rx="{r}" fill="{fill}"{st}/>')

    def line(self, x1, y1, x2, y2, c, w=1, dash=None, op=1):
        d = f' stroke-dasharray="{dash}"' if dash else ''
        self.add(f'<line x1="{x1:.2f}" y1="{y1:.2f}" x2="{x2:.2f}" y2="{y2:.2f}" stroke="{c}" '
                 f'stroke-width="{w}" stroke-opacity="{op}"{d} stroke-linecap="round"/>')

    def circle(self, x, y, r, fill='none', stroke=None, sw=1, op=1):
        st = f' stroke="{stroke}" stroke-width="{sw}"' if stroke else ''
        self.add(f'<circle cx="{x:.2f}" cy="{y:.2f}" r="{r:.2f}" fill="{fill}" fill-opacity="{op}"{st}/>')

    def path(self, d, c, w=1, fill='none', op=1, dash=None):
        da = f' stroke-dasharray="{dash}"' if dash else ''
        self.add(f'<path d="{d}" stroke="{c}" stroke-width="{w}" fill="{fill}" stroke-opacity="{op}"{da}/>')

    def text(self, x, y, s, c, size=12, weight=400, anchor='start', family='Plex', track=0, caps=False):
        if caps:
            s = s.upper()
        ls = f' letter-spacing="{track}em"' if track else ''
        self.add(f'<text x="{x:.2f}" y="{y:.2f}" fill="{c}" font-family="{family}" font-size="{size}" '
                 f'font-weight="{weight}" text-anchor="{anchor}"{ls}>{esc(s)}</text>')

    def html(self):
        fonts = f"""
@font-face {{ font-family: Plex; src: url('{FONTS}IBMPlexSans-Regular.ttf'); font-weight: 400; }}
@font-face {{ font-family: Plex; src: url('{FONTS}IBMPlexSans-SemiBold.ttf'); font-weight: 600; }}
@font-face {{ font-family: Mono; src: url('{FONTS}IBMPlexMono-Regular.ttf'); font-weight: 400; }}
@font-face {{ font-family: Mono; src: url('{FONTS}IBMPlexMono-Medium.ttf'); font-weight: 500; }}
html, body {{ margin: 0; padding: 0; background: #000; }}
"""
        body = '\n'.join(self.parts)
        return (f'<!doctype html><html><head><meta charset="utf-8"><style>{fonts}</style></head><body>'
                f'<svg xmlns="http://www.w3.org/2000/svg" width="{self.w * self.s}" height="{self.h * self.s}" '
                f'viewBox="0 0 {self.w} {self.h}" shape-rendering="geometricPrecision">{body}</svg></body></html>')


def taps(time_ms, fb, tone_hz, ping, span_s):
    """(seconds, level relative to the first repeat, brightness at REF_HZ, side): the DSP's
    echoes. Side: 0 both, 1 left, 2 right."""
    a = 1.0 - math.exp(-2 * math.pi * min(tone_hz, SR * 0.45) / SR)
    w = 2 * math.pi * REF_HZ / SR
    one = a / math.sqrt(1 - 2 * (1 - a) * math.cos(w) + (1 - a) ** 2)
    out, n = [], 1
    while n * time_ms / 1000 <= span_s and n < 64:
        lvl = fb ** (n - 1)
        if lvl < 0.001:
            break
        side = 0 if not ping else (1 if n % 2 == 1 else 2)
        out.append((n * time_ms / 1000, lvl, one ** (n - 1), side))
        n += 1
    return out, one


def mixc(a, b, t):
    A = [int(a[i:i + 2], 16) for i in (1, 3, 5)]
    B = [int(b[i:i + 2], 16) for i in (1, 3, 5)]
    return '#%02x%02x%02x' % tuple(round(A[i] + (B[i] - A[i]) * t) for i in range(3))


def nice_span(t_s):
    """The display's span: a round time that holds about six and a half repeats."""
    want = t_s * 6.5
    for s in (0.05, 0.1, 0.2, 0.25, 0.5, 0.75, 1, 1.5, 2, 3, 4, 6, 8, 12, 16):
        if s >= want:
            return s
    return 16


def card(g, t, x, y, w, h, title):
    g.rect(x, y, w, h, t['card'], r=5)
    g.rect(x, y, w, 24, t['band'], r=5)
    g.rect(x, y + 18, w, 6, t['band'])
    g.rect(x + 8, y + 7, 3, 11, t['acc'])
    g.text(x + 17, y + 16.5, title, t['ink2'], size=11, weight=600, track=0.12, caps=True)
    g.line(x + 17 + len(title) * 8.2 + 8, y + 12.5, x + w - 10, y + 12.5, t['ink4'], 1)


def knob(g, t, cx, cy, r, frac, label, value, unit=''):
    """A skirted knob (Delay's face): a fluted skirt under a small cap, the value arc."""
    g.text(cx, cy - r - 12, label, t['ink2'], size=11, anchor='middle')
    a0, a1 = math.radians(135), math.radians(405)
    a = a0 + (a1 - a0) * frac
    def arc(r1, b0, b1):
        x0, y0 = cx + r1 * math.cos(b0), cy + r1 * math.sin(b0)
        x1, y1 = cx + r1 * math.cos(b1), cy + r1 * math.sin(b1)
        large = 1 if b1 - b0 > math.pi else 0
        return f'M{x0:.2f},{y0:.2f} A{r1},{r1} 0 {large} 1 {x1:.2f},{y1:.2f}'
    g.path(arc(r + 4, a0, a1), t['well'], 3.5)
    g.path(arc(r + 4, a0, a), t['acc'], 3.5)
    g.circle(cx, cy, r, fill=t['skirt'])
    for i in range(24):
        b = i * math.pi / 12
        g.line(cx + (r - 4) * math.cos(b), cy + (r - 4) * math.sin(b),
               cx + (r - 1) * math.cos(b), cy + (r - 1) * math.sin(b), t['knob_hi'], 1)
    g.circle(cx, cy, r * 0.58, fill=t['knob'], stroke=t['edge'], sw=0.8)
    g.line(cx + r * 0.62 * math.cos(a), cy + r * 0.62 * math.sin(a),
           cx + (r - 2) * math.cos(a), cy + (r - 2) * math.sin(a), t['ink1'], 2)
    g.text(cx, cy + r + 20, value, t['ink1'], size=13, anchor='middle', family='Mono')
    if unit:
        g.text(cx + len(value) * 4.2 + 3, cy + r + 20, unit, t['ink3'], size=10, family='Mono')


def header(g, t, x, y, w):
    g.rect(x, y, w, 34, t['head'])
    g.rect(x, y + 33, w, 1, t['edge'])
    # engraved plate: the name cut into the face
    g.rect(x + 28, y + 7, 74, 20, t['well'], r=3, stroke=t['edge'])
    g.text(x + 65, y + 21.5, 'DELAY', t['ink1'], size=12, weight=600, anchor='middle', track=0.14)
    g.text(x + 110, y + 21.5, '(Insert 1 · Strip 1 · Keys)', t['ink3'], size=12)
    g.rect(x + 380, y + 6, 250, 22, t['well'], r=3)
    g.text(x + 505, y + 21, 'Dotted eighth', t['ink1'], size=12, anchor='middle')
    g.text(x + 368, y + 21, '‹', t['ink2'], size=14, anchor='middle')
    g.text(x + 642, y + 21, '›', t['ink2'], size=14, anchor='middle')
    g.rect(x + 656, y + 6, 46, 22, t['well'], r=4)
    g.rect(x + 658, y + 8, 21, 18, t['ab'], r=3)
    g.text(x + 668.5, y + 21, 'A', t['ab_ink'], size=11, weight=600, anchor='middle')
    g.text(x + 690.5, y + 21, 'B', t['ink2'], size=11, weight=600, anchor='middle')
    for i, s in enumerate(['↶', '↷', '⏻', '⋯', '⤢']):
        g.text(x + 722 + i * 24, y + 22, s, t['acc'] if s == '⏻' else t['ink2'], size=14, anchor='middle')
    g.rect(x + 840, y + 6, 70, 22, t['well'], r=3)
    g.text(x + 875, y + 21, 'Swap', t['ink1'], size=11, anchor='middle')
    for i, s in enumerate(['–', '□', '×']):
        g.text(x + 935 + i * 24, y + 22, s, t['ink2'], size=13, anchor='middle')


def footer(g, t, x, y, w):
    g.rect(x, y, w, 24, t['head'])
    g.text(x + 10, y + 16, 'Drag the echoes: across for the time, up and down for the feedback', t['ink2'], size=11)
    g.rect(x + w - 190, y + 4, 62, 16, t['well'], r=3)
    g.text(x + w - 159, y + 15.5, 'No latency', t['ink2'], size=10, anchor='middle')
    g.text(x + w - 118, y + 15.5, 'OUT', t['ink3'], size=10, weight=600)
    g.rect(x + w - 90, y + 9, 80, 6, t['well'], r=2)
    g.rect(x + w - 90, y + 9, 48, 6, t['meter'], r=2)


def display(g, t, x, y, w, h):
    """The echoes in time: L above the centre, R below; the first repeat at the time."""
    g.rect(x, y, w, h, t['disp'], r=2)
    plot_l, plot_r = x + 34, x + w - 16
    top, bot = y + 30, y + h - 26
    mid = (top + bot) / 2
    half = (bot - top) / 2 - 4
    span = nice_span(TIME_MS / 1000)
    X = lambda s: plot_l + (plot_r - plot_l) * s / span
    # the grid: the centre, a quarter and half of each lane, the time ticks
    g.line(plot_l, mid, plot_r, mid, t['grid2'], 1)
    for f in (0.5, 1.0):
        for sgn in (-1, 1):
            g.line(plot_l, mid + sgn * half * f, plot_r, mid + sgn * half * f, t['grid'], 1)
    for q in range(0, 7):
        s = span * q / 6
        g.line(X(s), bot, X(s), bot + 3, t['grid2'], 1)
        lab = f'{s * 1000:.0f} ms' if span < 1.5 else f'{s:.2f} s'
        g.text(X(s), bot + 15, lab, t['ddim'], size=10, anchor='start' if q == 0 else ('end' if q == 6 else 'middle'))
    g.text(x + 12, mid - half / 2 + 4, 'L', t['ddim'], size=11, weight=600, anchor='middle')
    g.text(x + 12, mid + half / 2 + 4, 'R', t['ddim'], size=11, weight=600, anchor='middle')
    # the dry hit at 0, in the text's dim ink
    g.line(X(0), mid - half, X(0), mid + half, t['ddim'], 1.2, op=0.55)
    # the decay envelope, through the heads of the repeats
    ts, one = taps(TIME_MS, FEEDBACK, TONE_HZ, PING_PONG, span)
    pts = []
    for i in range(0, 121):
        s = TIME_MS / 1000 + (span - TIME_MS / 1000) * i / 120
        lvl = FEEDBACK ** (s / (TIME_MS / 1000) - 1)
        pts.append((X(s), math.sqrt(lvl)))
    for sgn in (-1, 1):
        d = 'M' + ' L'.join(f'{px:.2f},{mid + sgn * half * lv:.2f}' for px, lv in pts)
        g.path(d, t['acc'], 1, op=0.35, dash='3 3')
    # the feedback: a level line in each lane from the first repeat, its handle on it
    fy = half * math.sqrt(FEEDBACK)
    for sgn in (-1, 1):
        g.line(X(TIME_MS / 1000), mid + sgn * fy, plot_r, mid + sgn * fy, t['acc'], 1, dash='6 4', op=0.75)
    # the repeats: a stem from the centre, its head, its colour darkening with the tone
    for (s, lvl, bright, side) in ts:
        c = mixc(t['acc2'], t['acc'], bright)
        px = X(s)
        for sgn, on in ((-1, side in (0, 1)), (1, side in (0, 2))):
            if not on:
                continue
            hy = mid + sgn * half * math.sqrt(lvl)
            g.circle(px, hy, 7, fill=c, op=0.18 * bright + 0.05)
            g.line(px, mid, px, hy, c, 3)
            g.circle(px, hy, 3.2, fill=c)
    # the handle: the first repeat's time at the feedback's level, in both lanes
    for sgn in (-1, 1):
        g.circle(X(TIME_MS / 1000), mid + sgn * fy, 6.5, stroke=t['dtext'], sw=1.6)
    # words on plates
    def plate(px, py, s, anchor='start', c=None, size=12, family='Mono'):
        wpx = len(s) * size * 0.6 + 8
        x0 = px - 4 if anchor == 'start' else (px - wpx + 4 if anchor == 'end' else px - wpx / 2)
        g.rect(x0, py - size + 1, wpx, size + 5, t['plate'], r=2)
        g.text(px, py + 1, s, c or t['dtext'], size=size, anchor=anchor, family=family)
    plate(x + 10, y + 18, '375 ms · 1/8 dotted', size=12)
    plate(x + w - 10, y + 18, 'PING-PONG', anchor='end', size=10, family='Plex', c=t['ddim'])
    plate(plot_r - 2, mid - fy - 6, 'Feedback 42 %', anchor='end', size=10)
    db4 = 20 * math.log10(one ** 3)
    plate(X(ts[3][0]) + 8, mid + half * math.sqrt(ts[3][1]) + 22, f'4th repeat {db4:.1f} dB at 8 kHz'.replace('-', '−'), size=10, c=t['ddim'])
    return ts, one


def window(g, t, ox, oy):
    g.rect(ox, oy, W, H, t['body'], r=6)
    header(g, t, ox, oy, W)
    footer(g, t, ox, oy + H - 24, W)
    bx, by, bw, bh = ox + 8, oy + 42, W - 16, H - 42 - 32
    row2 = 150
    gap = 6
    row1 = bh - row2 - gap
    side = 262
    card(g, t, bx, by, bw - side - gap, row1, 'Echoes')
    ts, one = display(g, t, bx + 8, by + 30, bw - side - gap - 16, row1 - 38)
    # what is heard
    sx = bx + bw - side
    card(g, t, sx, by, side, row1, 'What is heard')
    n = 1 + int(math.floor(math.log(0.001) / math.log(FEEDBACK)))
    rows = [('REPEATS TO −60 dB', f'{n}'), ('TAIL', f'{n * TIME_MS / 1000:.2f} s'),
            ('EACH REPEAT AT 8 kHz', f'{20 * math.log10(one):.1f} dB'.replace('-', '−')),
            ('FIRST REPEAT', 'Left')]
    yy = by + 46
    for lab, val in rows:
        g.text(sx + 14, yy, lab, t['ink3'], size=10, weight=600, track=0.08)
        g.rect(sx + 12, yy + 6, side - 24, 24, t['well'], r=3)
        g.text(sx + side - 20, yy + 23, val, t['ink1'], size=12, anchor='end', family='Mono')
        yy += 54
    g.text(sx + 14, yy + 4, 'Ping-pong sends the sum to the left;', t['ink3'], size=11)
    g.text(sx + 14, yy + 20, 'the repeats cross from side to side.', t['ink3'], size=11)
    # the row of cards
    ry = by + row1 + gap
    cw = [310, 280, 200, bw - 310 - 280 - 200 - 3 * gap]
    cx = bx
    titles = ['Time', 'Repeats', 'Stereo', 'Output']
    for i, wdt in enumerate(cw):
        card(g, t, cx, ry, wdt, row2, titles[i])
        cw[i] = (cx, wdt)
        cx += wdt + gap
    (x0, w0), (x1, w1), (x2, w2), (x3, w3) = cw
    knob(g, t, x0 + 70, ry + 82, 30, math.log(TIME_MS) / math.log(2000), 'Time', '375', 'ms')
    g.text(x0 + 140, ry + 48, 'AT THE TEMPO', t['ink3'], size=10, weight=600, track=0.08)
    g.rect(x0 + 140, ry + 56, 156, 24, t['well'], r=3)
    g.text(x0 + 148, ry + 72, '1/8 dotted', t['ink1'], size=12)
    g.text(x0 + 286, ry + 72, '▾', t['ink2'], size=11, anchor='middle')
    g.text(x0 + 140, ry + 102, '120 BPM: sets the time once;', t['ink3'], size=10.5)
    g.text(x0 + 140, ry + 116, 'it does not follow the tempo.', t['ink3'], size=10.5)
    knob(g, t, x1 + 76, ry + 82, 30, FEEDBACK / 0.95, 'Feedback', '42', '%')
    knob(g, t, x1 + 204, ry + 86, 22, math.log(TONE_HZ / 200) / math.log(100), 'Tone', '4.5', 'kHz')
    # the stereo card: a rocker, the two choices as words
    g.text(x2 + w2 / 2, ry + 48, 'Ping-pong', t['ink2'], size=11, anchor='middle')
    g.rect(x2 + w2 / 2 - 22, ry + 58, 44, 56, t['well'], r=5, stroke=t['edge'])
    g.rect(x2 + w2 / 2 - 18, ry + 62, 36, 26, t['acc'], r=4)
    g.text(x2 + w2 / 2, ry + 80, 'ON', t['ab_ink'] if t['name'] == 'paper' else '#2a0b10', size=10, weight=600, anchor='middle')
    g.text(x2 + w2 / 2, ry + 106, 'OFF', t['ink3'], size=10, weight=600, anchor='middle')
    knob(g, t, x3 + w3 / 2, ry + 82, 26, MIX, 'Mix', '20', '%')
    return ts, one


NOTES = [
    ('The face', 'Tape echo at night: an oxblood tint (350°, vinyl amp covering), the echoes on a '
     'phosphor display in a coral accent (354°) that cools to rust (14°) as the tone darkens them; '
     'skirted knobs (a tape echo’s), a rocker for ping-pong, an engraved name, screws, printed rules.'),
    ('The display', 'The control. L above the centre line, R below. The dry hit at 0; each repeat a '
     'stem from the centre, its level relative to the first (drawn as its square root, so a quiet repeat still shows), its colour from coral (bright) to rust '
     '(the tone has taken its highs: the one-pole filter’s loss at 8 kHz, once per pass). Dashed: '
     'the decay through the heads, and the feedback’s level in each lane, from the first repeat.'),
    ('The drag', 'Anywhere on the display: across moves the time (the first repeat follows the '
     'pointer), up and down the feedback (the level lines follow it, in the lane it began in). One '
     'step of undo per drag. Shift: fine. Double-click: back to the defaults (375 ms, 40 %). The '
     'span is a round time holding about six and a half repeats, kept still while dragging.'),
    ('Tempo', 'The engine’s time is milliseconds; it has no sync. *At the tempo* sets the time once '
     'from a note value at the song’s tempo, and names the note when the time is one; it does not '
     'follow a tempo change, and says so.'),
    ('At rest', 'Nothing moves while no sound plays: the window asks for no frame. While sound comes, '
     'each repeat glows with what the input was one, two, three… times ago, at its level: the '
     'echoes leave in turn. Through the ticker, 60 a second at most.'),
    ('Measured', 'Designed size 1000 × 640, like Concentus, Aula and Vibrans; display card 72 %, '
     '262 wide side card; the card row at the XL knob card height (150); 8 px grid gaps; type: '
     'card titles SemiBold 11 caps +0.12, values IBM Plex Mono 13, units Mono 10 ink 3.'),
]


def wrap(s, n):
    out, line = [], ''
    for w in s.split():
        if len(line) + len(w) + 1 > n:
            out.append(line)
            line = w
        else:
            line = (line + ' ' + w).strip()
    out.append(line)
    return out


def sheet(t, scale):
    tw, th = W + 2 * PAD, H + 2 * PAD + NOTES_H
    g = Svg(tw, th, scale)
    g.rect(0, 0, tw, th, t['page'])
    window(g, t, PAD, PAD)
    y = PAD + H + 30
    col = (W - 24) / 2
    for i, (head, body) in enumerate(NOTES):
        cx = PAD + (i % 2) * (col + 24)
        cy = y + (i // 2) * 92
        g.text(cx, cy, head, t['ink1'] if t['name'] == 'paper' else '#e3e8ed', size=13, weight=600)
        for j, ln in enumerate(wrap(body, 92)):
            g.text(cx, cy + 18 + j * 15, ln, t['ink2'] if t['name'] == 'paper' else '#b4bdc6', size=11.5)
    return g


def render(html, w, h, png):
    url = 'file:///' + html.replace(chr(92), '/')
    subprocess.run([EDGE, '--headless=new', '--disable-gpu', '--disable-lcd-text', '--hide-scrollbars',
                    '--force-device-scale-factor=1', f'--window-size={w},{h}', f'--screenshot={png}', url],
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False, timeout=120)


def compare():
    """The window beside three of the family's, at 100 %: Concentus, Aula, Spatium."""
    from PIL import Image
    units = os.path.join(ROOT, 'docs', 'images', 'units')
    for th, tag in (('night', 'dark'), ('paper', 'light')):
        mine = Image.open(os.path.join(OUT, f'sheet-{th}-1x.png')).convert('RGB')
        mine = mine.crop((PAD, PAD, PAD + W, PAD + H))
        others = [Image.open(os.path.join(units, f'{n}-{tag}.png')).convert('RGB')
                  for n in ('concentus', 'aula', 'spatium')]
        cw = max(W, *(o.width for o in others))
        page = Image.new('RGB', (2 * cw + 3 * 16, 2 * H + 3 * 16), (20, 25, 30) if th == 'night' else (197, 205, 213))
        for i, im in enumerate([mine] + others):
            page.paste(im, (16 + (i % 2) * (cw + 16), 16 + (i // 2) * (H + 16)))
        page.save(os.path.join(OUT, f'compare-{th}.png'), optimize=True)
        print(os.path.join(OUT, f'compare-{th}.png'))


def main():
    os.makedirs(BUILD, exist_ok=True)
    for t in (NIGHT, PAPER):
        for scale, tag in ((1, '1x'), (2, '2x')):
            g = sheet(t, scale)
            hp = os.path.join(BUILD, f'sheet-{t["name"]}-{tag}.html')
            with open(hp, 'w', encoding='utf-8') as fh:
                fh.write(g.html())
            png = os.path.join(OUT, f'sheet-{t["name"]}-{tag}.png')
            render(hp, g.w * scale, g.h * scale, png)
            print(png)
    compare()


if __name__ == '__main__':
    sys.exit(main())
