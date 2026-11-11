"""The arrangement: the ruler, the track headers and the clips with their content."""
import math
import random
from core import mix, HUES, clip_tones, UI, NARROW, CLOCK
from kit import key, label
from glyphs2 import glyph

INSP_X = 1040
RUL_Y, RUL_H = 80, 26
LANE_Y, ROW_H = 106, 34
HEAD_W = 168
B0, NB = 37, 16                         # bars 37..52 on screen
BW = (INSP_X - HEAD_W) / NB             # 54.5 px a bar


def bx(bar):
    return HEAD_W + (bar - B0) * BW


TRACKS = [  # key, name, instrument, kind, clips (bar from, bar to, name, content)
    ("kick", "Kick", "Pulsus · kit", "k_drum", [(40, 41, "Fill", "kfill"), (41, 49, "Kick A", "kick"),
                                               (49, 53, "Kick A", "kick")]),
    ("clap", "Clap", "Pulsus · kit", "k_drum", [(41, 49, "Clap", "clap"), (49, 53, "Clap", "clap")]),
    ("hats", "Hats", "Pulsus · kit", "k_drum", [(39, 41, "Hat roll", "roll"), (41, 49, "Hats 16", "hats"),
                                               (49, 53, "Hats 16", "hats")]),
    ("perc", "Percussion", "audio · 124 BPM", "k_audio", [(41, 53, "Perc loop 124", "aperc")]),
    ("bass", "Bass", "Gravis", "k_synth", [(41, 49, "Bass drop", "bass"), (49, 53, "Bass drop", "bass")]),
    ("chords", "Chords", "Chorda", "k_synth", [(37, 41, "Chords break", "pads"),
                                               (41, 49, "Chords drop", "chords"),
                                               (49, 53, "Chords drop", "chords")]),
    ("pluck", "Pluck", "Modulus", "k_synth", [(41, 53, "Pluck arp", "arp")]),
    ("lead", "Lead", "Calor", "k_synth", [(43, 49, "Lead hook", "lead"), (49, 53, "Lead answer", "lead2")]),
    ("choir", "Choir", "audio · vocal", "k_voice", [(37, 41, "Choir aah", "achoir"),
                                                   (45, 53, "Choir aah", "achoir")]),
]
SELECTED = 5   # Chords


def arrangement(c, playing=False):
    t = c.t
    night = t["name"] == "Night"
    # lanes ground: four-bar groups alternate, very quietly
    c.rect(HEAD_W, RUL_Y, INSP_X - HEAD_W, LANE_Y + ROW_H * 9 - RUL_Y, t["canvas"])
    for b in range(B0, B0 + NB, 4):
        if ((b - 1) // 4) % 2 == 0:
            c.rect(bx(b), LANE_Y, BW * 4, ROW_H * 9, t["lane_alt"])
    for i in range(9):
        if i == SELECTED:
            c.rect(HEAD_W, LANE_Y + i * ROW_H, INSP_X - HEAD_W, ROW_H,
                   mix(t["canvas"], t["gold"], 0.035))
    for b in range(B0, B0 + NB + 1):
        for q in range(4):
            x = bx(b) + q * BW / 4
            if x >= INSP_X:
                break
            c.vline(x, LANE_Y, LANE_Y + ROW_H * 9, t["line"] if q == 0 else t["line_soft"],
                    op=1 if q == 0 else (0.7 if night else 0.8))
    for i in range(10):
        c.hline(HEAD_W, INSP_X, LANE_Y + i * ROW_H - (1 if i else 0), t["line_soft"])
    ruler(c)
    for i, tr in enumerate(TRACKS):
        header(c, i, tr)
        for (a, b, name, kind) in tr[4]:
            clip(c, i, tr[0], a, b, name, kind, sel=(i == SELECTED and a == 41))
    # the playhead, parked at the drop
    px = bx(41) + (BW * 0.22 if playing else 0)
    c.vline(px, RUL_Y + 12, LANE_Y + ROW_H * 9, t["gold"], op=0.9)
    c.path("M%.1f %.1f L%.1f %.1f L%.1f %.1f Z" % (px - 4.5, RUL_Y + 13, px + 5.5, RUL_Y + 13,
                                                    px + 0.5, RUL_Y + 19), fill=t["gold"])
    c.hline(0, INSP_X, LANE_Y + ROW_H * 9, t["key_edge"] if night else t["line"])
    # the horizontal scroll: a thin thumb resting on the lanes' bottom edge
    c.rect(HEAD_W + 300, LANE_Y + ROW_H * 9 - 5, 180, 3, t["ink4"], rx=1.5, op=0.8)


def ruler(c):
    t = c.t
    night = t["name"] == "Night"
    c.rect(0, RUL_Y, INSP_X, RUL_H, t["panel"])
    # header corner: the track count and the add key
    c.text(14, RUL_Y + 17, "9 tracks", size=10.5, fill=t["ink3"], wt=500)
    key(c, HEAD_W - 28, RUL_Y + 4, 20, 18, g="plus", gsize=8)
    # sections: a word and a hairline flag; the band itself stays quiet
    for bar, name in ((37, "Break"), (41, "Drop 2")):
        x = bx(bar)
        c.vline(x, RUL_Y + 1, RUL_Y + 12, t["ink3"])
        c.text(x + 5, RUL_Y + 10, name, size=10, fill=t["ink2"], wt=650)
    # loop 41..49: gold, because it is on
    c.rect(bx(41), RUL_Y + 13, BW * 8, 12, t["gold"], op=0.16 if night else 0.18)
    c.rect(bx(41), RUL_Y + 13, 2, 12, t["gold"])
    c.rect(bx(49) - 2, RUL_Y + 13, 2, 12, t["gold"])
    for b in range(B0, B0 + NB):
        x = bx(b)
        c.vline(x, RUL_Y + 16 if b % 4 != 1 else RUL_Y + 13, RUL_Y + RUL_H, t["line_strong"] if
                b % 4 == 1 else t["line"])
        c.text(x + 4, RUL_Y + 23, str(b), size=10.5 if b % 4 == 1 else 9.5,
               fill=t["ink1"] if b % 4 == 1 else t["ink3"], wt=600 if b % 4 == 1 else 500, tnum=True)
    c.hline(0, INSP_X, RUL_Y + RUL_H - 1, t["line"])


def header(c, i, tr):
    t = c.t
    night = t["name"] == "Night"
    k, name, inst, kind, _ = tr
    y = LANE_Y + i * ROW_H
    sel = i == SELECTED
    base = t["raised"] if sel else t["panel"]
    c.rect(0, y, HEAD_W, ROW_H - 1, mix(base, HUES[k], 0.05 if night else 0.06))
    c.rect(0, y, 3, ROW_H - 1, HUES[k])
    glyph(c, kind, 16, y + 12, 10, ink1=t["ink2"], ink2=t["ink3"])
    c.text(28, y + 15.5, name, size=12.5, fill=t["ink1"], wt=600 if sel else 550)
    c.text(28, y + 28, inst, size=10, fill=t["ink3"], wt=500)
    for j, s in enumerate(("M", "S")):
        xk = HEAD_W - 44 + j * 19
        c.rect(xk, y + 8, 17, 15, t["key"], rx=2.5, stroke=t["key_edge"])
        c.text(xk + 8.5, y + 19.3, s, size=9.5, fill=t["ink3"], wt=700, anchor="middle")
    if sel:
        c.rect(HEAD_W - 2, y, 2, ROW_H - 1, t["gold"])
    c.hline(0, HEAD_W, y + ROW_H - 1, t["line_soft"] if night else t["line"])


def clip(c, i, k, a, b, name, kind, sel=False):
    t = c.t
    night = t["name"] == "Night"
    body, band, ink, edge = clip_tones(t, HUES[k])
    x0, x1 = bx(a) + 1, min(bx(b), INSP_X) - 1
    y0 = LANE_Y + i * ROW_H + 2
    h = ROW_H - 5
    c.rect(x0, y0, x1 - x0, h, body, rx=2.5)
    c.rect(x0, y0, x1 - x0, 11, band, rx=2.5)
    c.rect(x0, y0 + 8, x1 - x0, 3, band)
    c.text(x0 + 5, y0 + 8.6, name, size=9, fill=ink, wt=650, fam=NARROW)
    c.rect(x0, y0, x1 - x0, h, "none", rx=2.5, stroke=t["gold"] if sel else edge,
           extra=' stroke-opacity="%s"' % ("1" if sel else ("0.55" if night else "0.6")))
    content(c, kind, x0 + 2, y0 + 13, x1 - x0 - 4, h - 15, ink, a, b, k)


def content(c, kind, x, y, w, h, ink, a, b, seed):
    rnd = random.Random(seed + kind + str(a))
    beats = (b - a) * 4
    bw = w / beats
    notes = []  # (beat, length, row 0..1 from top, vel)
    if kind == "kick":
        notes = [(q, 0.5, 0.75, 1) for q in range(beats)]
    elif kind == "kfill":
        notes = [(q * 0.5, 0.25, 0.75, 0.7 + 0.3 * (q % 2)) for q in range(8)] + \
                [(q * 0.25 + 2, 0.2, 0.35, 0.8) for q in range(8)]
    elif kind == "clap":
        notes = [(q, 0.5, 0.5, 1) for q in range(beats) if q % 2 == 1]
    elif kind == "hats":
        notes = [(q * 0.25, 0.15, 0.30 if q % 4 == 2 else 0.55, 0.45 + 0.55 * (q % 2 == 0))
                 for q in range(beats * 4)]
    elif kind == "roll":
        n = beats * 4
        notes = [(q * 0.25 + (0.125 if q > n / 2 else 0) * (q % 2), 0.12, 0.5, 0.3 + 0.7 * q / n)
                 for q in range(n)] + [(q * 0.25 + 0.125, 0.1, 0.5, 0.3 + 0.7 * q / n)
                                       for q in range(n // 2, n)]
    elif kind == "bass":
        pat = [0, 0, 7, 0, 5, 0, 3, 5]
        roots = [0, 0, 3, 5]
        for bar in range(b - a):
            r = roots[bar % 4]
            for j, s in enumerate(pat):
                if j in (2, 6) and bar % 2:
                    continue
                notes.append((bar * 4 + j * 0.5, 0.42, 1 - (r + s) / 14, 1))
    elif kind in ("chords", "pads"):
        prog = [(0, 3, 7, 10), (3, 7, 10, 14), (5, 8, 12, 15), (2, 5, 9, 12)]
        step = 4 if kind == "pads" else 2
        for q in range(0, beats, step):
            ch = prog[(q // 4) % 4]
            for p in ch:
                ln = step - 0.15 if kind == "pads" else (1.2 if q % 4 == 0 else 1.6)
                notes.append((q + (0 if kind == "pads" or q % 4 == 0 else 0.5), ln, 1 - p / 16, 1))
    elif kind == "arp":
        seq = [0, 7, 12, 15, 12, 7, 3, 7]
        for q in range(beats * 2):
            notes.append((q * 0.5, 0.35, 1 - (seq[q % 8] + (3 if (q // 16) % 2 else 0)) / 19, 1))
    elif kind in ("lead", "lead2"):
        mel = [(0, 1.5, 12), (1.5, 0.5, 10), (2, 1, 7), (3, 1, 10), (4, 2.5, 15), (6.5, 0.5, 14),
               (7, 1, 12), (8, 1.5, 10), (9.5, 0.5, 12), (10, 2, 7), (12, 3.5, 5)]
        if kind == "lead2":
            mel = [(0, 1, 7), (1, 1, 10), (2, 2, 12), (4, 1.5, 15), (5.5, 0.5, 14), (6, 2, 12),
                   (8, 1.5, 10), (9.5, 0.5, 7), (10, 1.5, 10), (12, 3.5, 12)]
        span = 16
        for k in range(0, beats, span):
            for (s, ln, p) in mel:
                if k + s < beats:
                    notes.append((k + s, ln - 0.1, 1 - (p - 3) / 14, 1))
    elif kind in ("aperc", "achoir"):
        wave(c, kind, x, y, w, h, ink, beats, rnd)
        return
    # drum hits sit on the same note drawing, short; pitched notes get rounded ends
    nh = max(2.0, min(3.0, h / 9))
    for (s, ln, row, vel) in notes:
        nx = x + s * bw
        nw = max(1.5, ln * bw - 0.6)
        ny = y + row * (h - nh)
        c.rect(nx, ny, nw, nh, ink, rx=0.8, op=0.55 + 0.45 * vel)


def wave(c, kind, x, y, w, h, ink, beats, rnd):
    """A filled, mirrored waveform: transients for the loop, a breathing pad for the choir."""
    s = c.s
    n = max(40, int(w * s / 1.5))
    mid = y + h / 2
    top, bot = [], []
    for j in range(n + 1):
        u = j / n
        beat = u * beats
        if kind == "aperc":
            ph = beat * 2 % 1
            env = math.exp(-ph * 7) * (0.95 if int(beat * 2) % 2 == 0 else 0.6)
            amp = env * (0.65 + 0.35 * rnd.random()) + 0.04
        else:
            ph = (beat % 8) / 8
            env = math.sin(min(1, ph * 1.4) * math.pi / 2) * (1 - max(0, ph - 0.85) * 5)
            amp = 0.18 + 0.62 * env * (0.82 + 0.18 * math.sin(beat * 7.1) * rnd.random())
        amp = max(0.03, min(1, amp))
        px = x + u * w
        top.append((px, mid - amp * h / 2))
        bot.append((px, mid + amp * h / 2 * 0.92))
    d = "M" + " L".join("%.2f %.2f" % (px * s, py * s) for px, py in top + bot[::-1]) + "Z"
    c.add('<path d="%s" fill="%s" fill-opacity="0.82"/>' % (d, ink))
    c.hline(x, x + w, mid, ink, op=0.35)
