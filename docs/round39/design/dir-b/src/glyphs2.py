"""dir-b glyphs, part 2: tools, windows, track kinds, the strip, and the registry."""
import math
from glyphs import (Pen, play, pause, stop, record, to_start, loop, metronome, follow, magnet,
                    undo, redo, select, draw, slice_, erase, mute_tool, zoom)


def gear(p):
    # drawn for its size: 6 teeth under 14 px, 8 from 14; teeth are squared lugs
    n = 6 if p.P < 14 else 8
    cx, cy = p.x0 + p.P / 2, p.y0 + p.P / 2
    ro, ri = p.P * 0.50, p.P * (0.36 if n == 6 else 0.38)
    pts = []
    for i in range(n):
        a0 = 2 * math.pi * i / n
        hw = math.pi / n * 0.48
        for a, r in ((a0 - hw * 1.25, ri), (a0 - hw, ro), (a0 + hw, ro), (a0 + hw * 1.25, ri)):
            pts.append((cx + r * math.cos(a), cy + r * math.sin(a)))
    d = "M" + " L".join("%.2f %.2f" % q for q in pts) + "Z"
    hole = p.P * (0.15 if n == 6 else 0.16)
    p.raw('<path d="%s M%.2f %.2f m%.2f 0 a%.2f %.2f 0 1 0 %.2f 0 a%.2f %.2f 0 1 0 %.2f 0Z" '
          'fill="%s" fill-rule="evenodd"/>' % (d, cx, cy, -hole, hole, hole, 2 * hole, hole,
                                               hole, -2 * hole, p.i2))
    if p.edge:
        p.raw('<path d="M%.2f %.2f A%.2f %.2f 0 0 1 %.2f %.2f" fill="none" stroke="%s" '
              'stroke-width="1" stroke-linecap="round"/>' % (
                  cx - ri * 0.92, cy - ri * 0.30, ri * 0.96, ri * 0.96, cx + ri * 0.30,
                  cy - ri * 0.92, p.i1))


def pick(p):
    p.line([(0.10, 0.90), (0.56, 0.44)], sw=max(1.0, p.sw))
    p.curve("M0.48 0.30 L0.70 0.52", sw=max(1.5, p.P * 0.12))
    p.dot(0.76, 0.24, 0.17, fill=p.i1 if p.edge else p.i2)
    p.dot(0.10, 0.90, 0.07, fill=p.i1)


def more(p):
    for x in (0.14, 0.5, 0.86):
        p.dot(x, 0.5, 0.11 if p.P >= 12 else 0.13, fill=p.i1)


def plus(p):
    p.line([(0.5, 0.06), (0.5, 0.94)])
    p.line([(0.06, 0.5), (0.94, 0.5)])


def chev_down(p):
    p.line([(0.14, 0.34), (0.5, 0.70), (0.86, 0.34)])


def chev_right(p):
    p.line([(0.34, 0.14), (0.70, 0.5), (0.34, 0.86)])


def caret_down(p):
    p.poly([(0.10, 0.30), (0.90, 0.30), (0.50, 0.76)])


# ---- windows ------------------------------------------------------------------------

def win_browser(p):
    # a tree: a folder tab, then three entries stepping in
    p.box(0.0, 0.04, 0.42, 0.26)
    p.box(0.0, 0.26, 0.04 if p.P < 14 else 0.06, 0.92, fill=p.i2)
    for y, x in ((0.40, 0.22), (0.64, 0.22), (0.88, 0.22)):
        p.box(x, y - 0.08, 1.0 if y != 0.64 else 0.80, y + (0.04 if p.P < 12 else 0.06),
              fill=p.i1 if y == 0.40 else p.i2)


def win_arr(p):
    # three lanes; clips as blocks of differing length, the first lit
    rows = (0.02, 0.38, 0.74)
    h = 0.24
    p.box(0.0, rows[0], 0.46, rows[0] + h, fill=p.i1)
    p.box(0.54, rows[0], 1.0, rows[0] + h)
    p.box(0.22, rows[1], 0.80, rows[1] + h)
    p.box(0.0, rows[2], 0.30, rows[2] + h)
    p.box(0.38, rows[2], 1.0, rows[2] + h)


def win_rack(p):
    for i, y in enumerate((0.02, 0.38, 0.74)):
        p.box(0.0, y, 0.30, y + 0.24, fill=p.i1 if i == 0 else p.i2)
        for j, x in enumerate((0.42, 0.62, 0.82)):
            on = (i + j) % 2 == 0
            if on:
                p.box(x, y + 0.02, x + 0.16, y + 0.22)
            else:
                X0, Y0 = p.X(x), p.Y(y + 0.02)
                X1, Y1 = p.X(x + 0.16), p.Y(y + 0.22)
                p.raw('<rect x="%.1f" y="%.1f" width="%.1f" height="%.1f" fill="none" '
                      'stroke="%s" stroke-width="1"/>' % (X0 + 0.5, Y0 + 0.5, max(1, X1 - X0 - 1),
                                                          max(1, Y1 - Y0 - 1), p.i2))


def win_piano(p):
    # keys at the left, notes in the grid
    p.box(0.0, 0.0, 0.22, 1.0)
    for y in (0.18, 0.46, 0.74):
        p.box(0.0, y, 0.12, y + 0.10, fill=p.c.t["key"])
    p.box(0.32, 0.10, 0.64, 0.28, fill=p.i1)
    p.box(0.50, 0.42, 1.0, 0.60)
    p.box(0.36, 0.74, 0.76, 0.92)


def win_mixer(p):
    for x, k in ((0.14, 0.62), (0.5, 0.26), (0.86, 0.48)):
        p.box(x - 0.04, 0.0, x + 0.04 if p.P >= 14 else x + 0.05, 1.0)
        p.box(x - 0.14, k - 0.08, x + 0.14, k + 0.08, fill=p.i1)


def win_session(p):
    if p.P < 12:  # small: a 2x2 grid, one cell lit — 3x3 would close up
        for i, y in enumerate((0.0, 0.58)):
            for j, x in enumerate((0.0, 0.58)):
                p.box(x, y, x + 0.42, y + 0.42, fill=p.i1 if (i, j) == (0, 1) else p.i2)
        return
    for i, y in enumerate((0.0, 0.36, 0.72)):
        for j, x in enumerate((0.0, 0.36, 0.72)):
            p.box(x, y, x + 0.28, y + 0.28, fill=p.i1 if (i, j) == (1, 1) else p.i2)


def win_inspector(p):
    X0, Y0, X1, Y1 = p.X(0.0), p.Y(0.04), p.X(1.0), p.Y(0.96)
    p.raw('<rect x="%.1f" y="%.1f" width="%.1f" height="%.1f" rx="1" fill="none" '
          'stroke="%s" stroke-width="1"/>' % (X0 + 0.5, Y0 + 0.5, X1 - X0 - 1, Y1 - Y0 - 1, p.i2))
    p.box(0.62, 0.04, 1.0, 0.96, fill=p.i2)
    p.box(0.70, 0.22, 0.92, 0.30, fill=p.i1)
    p.box(0.70, 0.44, 0.92, 0.52, fill=p.c.t["key"])


def win_plugins(p):
    # a socket: two pins over a rounded body
    p.box(0.26, 0.0, 0.38, 0.30, fill=p.i1)
    p.box(0.62, 0.0, 0.74, 0.30, fill=p.i1)
    p.curve("M0.10 0.30 L0.90 0.30 L0.90 0.52 C0.90 0.74 0.70 0.84 0.50 0.84 "
            "C0.30 0.84 0.10 0.74 0.10 0.52 Z", fill=p.i2, color="none")
    p.box(0.44, 0.84, 0.56, 1.0)


# ---- track kinds and the strip ------------------------------------------------------

def k_drum(p):
    p.curve("M0.06 0.34 C0.06 0.18 0.94 0.18 0.94 0.34 L0.94 0.74 C0.94 0.92 0.06 0.92 "
            "0.06 0.74 Z", fill=p.i2, color="none")
    p.curve("M0.06 0.34 C0.06 0.50 0.94 0.50 0.94 0.34 C0.94 0.18 0.06 0.18 0.06 0.34 Z",
            fill=p.i1, color="none")


def k_synth(p):
    p.curve("M0.0 0.5 C0.12 0.0 0.26 0.0 0.36 0.5 C0.46 1.0 0.58 1.0 0.66 0.5 L0.66 0.12 "
            "L1.0 0.88", color=p.i1)


def k_voice(p):
    p.curve("M0.34 0.10 C0.34 -0.02 0.66 -0.02 0.66 0.10 L0.66 0.48 C0.66 0.60 0.34 0.60 "
            "0.34 0.48 Z", fill=p.i1, color="none")
    p.curve("M0.18 0.42 C0.18 0.80 0.82 0.80 0.82 0.42")
    p.line([(0.5, 0.76), (0.5, 0.98)])


def k_audio(p):
    for x, h in ((0.06, 0.30), (0.22, 0.72), (0.38, 0.96), (0.54, 0.58), (0.70, 0.80), (0.86, 0.36)):
        p.box(x, 0.5 - h / 2, x + 0.10, 0.5 + h / 2, fill=p.i1 if h > 0.9 else p.i2)


def phase(p):
    p.dot(0.5, 0.5, 0.36, fill="none", stroke=p.i2)
    p.line([(0.10, 0.94), (0.90, 0.06)], color=p.i1)


def swap(p):
    p.line([(0.06, 0.30), (0.80, 0.30)])
    p.poly([(0.70, 0.08), (0.98, 0.30), (0.70, 0.52)], fill=p.i1 if p.edge else p.i2)
    p.line([(0.20, 0.72), (0.94, 0.72)])
    p.poly([(0.30, 0.50), (0.02, 0.72), (0.30, 0.94)])


def link(p):
    # two links of a chain, at 45°, drawn as rounded slots
    p.curve("M0.46 0.30 L0.62 0.14 C0.72 0.04 0.86 0.04 0.92 0.12 C0.98 0.20 0.96 0.32 "
            "0.86 0.40 L0.70 0.56", color=p.i1)
    p.curve("M0.54 0.70 L0.38 0.86 C0.28 0.96 0.14 0.96 0.08 0.88 C0.02 0.80 0.04 0.68 "
            "0.14 0.60 L0.30 0.44")
    p.line([(0.38, 0.62), (0.62, 0.38)], color=p.i2)


def power(p):
    p.curve("M0.28 0.24 C0.06 0.40 0.08 0.92 0.50 0.94 C0.92 0.92 0.94 0.40 0.72 0.24")
    p.line([(0.5, 0.04), (0.5, 0.50)], color=p.i1)


def grip(p):
    for y in (0.2, 0.5, 0.8):
        for x in (0.32, 0.68):
            p.dot(x, y, 0.08, fill=p.i2)


REG = dict(play=play, pause=pause, stop=stop, to_start=to_start, loop=loop,
           metronome=metronome, follow=follow, magnet=magnet, undo=undo, redo=redo,
           select=select, draw=draw, slice=slice_, erase=erase, mute_tool=mute_tool, zoom=zoom,
           gear=gear, pick=pick, more=more, plus=plus, chev_down=chev_down,
           chev_right=chev_right, caret_down=caret_down, win_browser=win_browser,
           win_arr=win_arr, win_rack=win_rack, win_piano=win_piano, win_mixer=win_mixer,
           win_session=win_session, win_inspector=win_inspector, win_plugins=win_plugins,
           k_drum=k_drum, k_synth=k_synth, k_voice=k_voice, k_audio=k_audio, phase=phase,
           swap=swap, link=link, power=power, grip=grip)


def glyph(c, name, cx, cy, size, on=False, off=False, ink1=None, ink2=None, lit=False):
    t = c.t
    p = Pen(c, cx, cy, size, ink1 or t["ink1"], ink2 or t["ink2"], on=on, off=off)
    if name == "record":
        record(p, lit=lit)
    else:
        REG[name](p)
    return p
