"""des-controls — the driven-value signs, shared by knobs, faders, sliders and fields.

A value can be driven by something other than the hand. The control then carries one of
four signs, each its own shape *and* its own colour, so it never relies on colour alone:

- auto  — an automation clip: a ramp with two nodes, violet (`auto`).
- mod   — a modulator (LFO, envelope): one period of a wave, orchid (`mod`).
- midi  — a controller / MIDI learn: a socket (a ring with its pin), the accent.
- vca   — a VCA fader offsets this level: a small V, ink-2 (it is a group, not a motor).

Each sign is drawn per pixel size (5, 7, 9 and their 150 %) — never one drawing scaled.
"""
from slate import f, path, circle, line


def sign(kind, cx, cy, h, col, bg=None):
    """Draw sign `kind` centred on (cx, cy), `h` px tall (4..14). `bg` knocks it out of
    whatever it sits on with a 1-px halo."""
    w = round(h * 1.8)
    sw = 1 if h <= 5 else (1.25 if h <= 7 else (1.5 if h <= 10 else 2))
    x0, y0 = cx - w / 2, cy - h / 2
    out = ''
    if kind == 'auto':
        # a lane: low, a ramp, high — a node at each bend
        pts = [(x0, y0 + h - sw / 2), (x0 + w * 0.26, y0 + h - sw / 2), (x0 + w * 0.74, y0 + sw / 2),
               (x0 + w, y0 + sw / 2)]
        d = 'M' + 'L'.join(f'{f(px)} {f(py)}' for px, py in pts)
        nr = 1.0 if h <= 5 else (1.3 if h <= 7 else (1.6 if h <= 10 else 2.1))
        if bg:
            out += path(d, bg, sw + 2)
            for px, py in (pts[1], pts[2]):
                out += circle(px, py, nr + 1, bg)
        out += path(d, col, sw)
        for px, py in (pts[1], pts[2]):
            out += circle(px, py, nr, col)
    elif kind == 'mod':
        # one period of a sine, drawn as two arcs
        a = h / 2 - sw / 2
        d = (f'M{f(x0)} {f(cy)}C{f(x0 + w * 0.18)} {f(cy - a * 1.32)} {f(x0 + w * 0.32)} {f(cy - a * 1.32)} '
             f'{f(cx)} {f(cy)}S{f(x0 + w * 0.82)} {f(cy + a * 1.32)} {f(x0 + w)} {f(cy)}')
        if bg:
            out += path(d, bg, sw + 2)
        out += path(d, col, sw)
    elif kind == 'midi':
        r = h / 2 - sw / 2
        if bg:
            out += circle(cx, cy, r + sw / 2 + 1, bg)
        out += f'<circle cx="{f(cx)}" cy="{f(cy)}" r="{f(r)}" fill="none" stroke="{col}" stroke-width="{f(sw)}"/>'
        out += circle(cx, cy, max(0.8, h * 0.14), col)
        out += line(cx, cy - r - sw / 2, cx, cy - r + sw * 1.2, col, sw, 'butt')
    elif kind == 'vca':
        hw = h * 0.55
        d = f'M{f(cx - hw)} {f(y0)}L{f(cx)} {f(y0 + h)}L{f(cx + hw)} {f(y0)}'
        if bg:
            out += path(d, bg, sw + 2)
        out += path(d, col, sw)
    elif kind == 'dot':
        out += circle(cx, cy, h / 2, col)
    return out


def sign_col(t, kind):
    return {'auto': t['auto'], 'mod': t['mod'], 'midi': t['accent'], 'vca': t['ink2']}[kind]
