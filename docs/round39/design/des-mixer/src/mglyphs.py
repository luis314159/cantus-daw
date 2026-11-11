"""des-mixer — the mixer's own small glyphs, drawn for the keys they sit in (14, 16, 18 px).

Every glyph is drawn per key height, never one drawing scaled: at 14 px a glyph keeps its
silhouette only; at 16 px it gains its second ink; at 18 px its detail. Strokes sit on half
pixels so 1-px lines land on one row of pixels at 100 % (and on whole pixels at 150 %,
since every coordinate is an even number or a half). Two inks: i1 the body, i2 the detail.

These are the mixer's placeholders in Slate's grammar until the `icons` agent's family is
approved; their sizes and places are the spec, their exact drawing may be replaced.
"""
import math


def _sp(d, ink, w=1, cap='round', join='round', extra=''):
    return (f'<path d="{d}" fill="none" stroke="{ink}" stroke-width="{w}" '
            f'stroke-linecap="{cap}" stroke-linejoin="{join}"{extra}/>')


def _f(v):
    return f'{v:.2f}'.rstrip('0').rstrip('.')


# key height -> (radius of a ring, stroke)
RING = {14: (3, 1), 16: (3.5, 1), 18: (4, 1), 20: (4.5, 1.25)}


def phase(cx, cy, H, i1, i2):
    """Ø — polarity: a ring and its stroke through it, the stroke a little past the ring."""
    r, w = RING[H]
    e = r + (1 if H < 18 else 1.5)
    out = f'<circle cx="{_f(cx)}" cy="{_f(cy)}" r="{_f(r)}" fill="none" stroke="{i1}" stroke-width="{w}"/>'
    d = e * 0.70
    out += _sp(f'M{_f(cx - d)} {_f(cy + d)}L{_f(cx + d)} {_f(cy - d)}', i2 if H >= 16 else i1, w)
    return out


def swap(cx, cy, H, i1, i2):
    """⇄ — left and right swapped: two short rails with their heads, one each way."""
    L = {14: 3.5, 16: 4, 18: 4.5, 20: 5}[H]
    g = {14: 1.5, 16: 2, 18: 2, 20: 2.5}[H]
    h = {14: 1.5, 16: 2, 18: 2, 20: 2.5}[H]
    w = 1 if H < 20 else 1.25
    y1, y2 = cy - g, cy + g
    out = _sp(f'M{_f(cx - L)} {_f(y1)}H{_f(cx + L)}M{_f(cx + L - h)} {_f(y1 - h)}L{_f(cx + L)} {_f(y1)}L{_f(cx + L - h)} {_f(y1 + h)}', i1, w)
    out += _sp(f'M{_f(cx + L)} {_f(y2)}H{_f(cx - L)}M{_f(cx - L + h)} {_f(y2 - h)}L{_f(cx - L)} {_f(y2)}L{_f(cx - L + h)} {_f(y2 + h)}',
               i2 if H >= 16 else i1, w)
    return out


def mono(cx, cy, H, i1, i2):
    """Mono: two rings folded into one — one ring, its twin a faint offset ghost at 18 px."""
    r, w = RING[H]
    out = ''
    if H >= 18:
        out += f'<circle cx="{_f(cx + 2)}" cy="{_f(cy)}" r="{_f(r)}" fill="none" stroke="{i2}" stroke-width="{w}" opacity="0.45"/>'
        cx -= 1
    out += f'<circle cx="{_f(cx)}" cy="{_f(cy)}" r="{_f(r)}" fill="none" stroke="{i1}" stroke-width="{w}"/>'
    out += f'<circle cx="{_f(cx)}" cy="{_f(cy)}" r="1" fill="{i1}"/>'
    return out


def arm(cx, cy, H, col):
    """Record arm: a filled dot, a third of the key."""
    r = {14: 2.5, 16: 3, 18: 3.5, 20: 4}[H]
    return f'<circle cx="{_f(cx)}" cy="{_f(cy)}" r="{_f(r)}" fill="{col}"/>'


def power(cx, cy, H, i1, i2):
    """All inserts in/out: an open ring with its stem (drawn, not a bulb)."""
    r = {14: 3, 16: 3.5, 18: 4, 20: 4.5}[H]
    w = 1 if H < 20 else 1.25
    a0, a1 = math.radians(-50), math.radians(230)
    x0, y0 = cx + r * math.sin(a0 + math.pi / 2 - math.pi / 2), cy
    # the ring open at the top, ±40° around the stem
    sx, sy = cx + r * math.sin(math.radians(40)), cy - r * math.cos(math.radians(40))
    ex, ey = cx - r * math.sin(math.radians(40)), cy - r * math.cos(math.radians(40))
    out = _sp(f'M{_f(sx)} {_f(sy)}A{_f(r)} {_f(r)} 0 1 1 {_f(ex)} {_f(ey)}', i1, w)
    out += _sp(f'M{_f(cx)} {_f(cy - r - 1)}V{_f(cy - 0.5)}', i2 if H >= 16 else i1, w)
    return out


def snow(cx, cy, H, ink):
    """Frozen: three strokes through the centre, with their barbs from 16 px."""
    L = {12: 3.5, 14: 4, 16: 4.5, 18: 5.5, 20: 6}[H]
    w = 1
    out = ''
    for k in range(3):
        a = math.radians(90 + 60 * k)
        dx, dy = math.cos(a) * L, math.sin(a) * L
        out += _sp(f'M{_f(cx - dx)} {_f(cy - dy)}L{_f(cx + dx)} {_f(cy + dy)}', ink, w)
        if H >= 16:
            for s in (1, -1):
                bx, by = cx + s * dx * 0.62, cy + s * dy * 0.62
                for t in (1, -1):
                    b = a + s * 0 + t * math.radians(45)
                    ux, uy = math.cos(b) * 1.6 * s, math.sin(b) * 1.6 * s
                    out += _sp(f'M{_f(bx)} {_f(by)}L{_f(bx + ux)} {_f(by + uy)}', ink, w)
    return out


def key(cx, cy, H, ink):
    """Sidechain key: a ring bow and a shaft with one bit."""
    r = {12: 2, 14: 2.25, 16: 2.5, 18: 3}[H]
    L = {12: 4, 14: 4.5, 16: 5, 18: 6}[H]
    bx = cx - L / 2 - r / 2
    out = f'<circle cx="{_f(bx)}" cy="{_f(cy)}" r="{_f(r)}" fill="none" stroke="{ink}" stroke-width="1"/>'
    x1 = bx + r
    out += _sp(f'M{_f(x1)} {_f(cy)}H{_f(x1 + L)}V{_f(cy + 2)}M{_f(x1 + L - 2)} {_f(cy)}V{_f(cy + 1.5)}', ink, 1, 'butt', 'miter')
    return out


def link(cx, cy, H, i1, i2):
    """Link: two rounded links overlapping."""
    w = {14: 4, 16: 4.5, 18: 5}[H]
    h = {14: 2.5, 16: 3, 18: 3.5}[H]
    out = f'<rect x="{_f(cx - w - 0.5)}" y="{_f(cy - h / 2)}" width="{_f(w + 1)}" height="{_f(h)}" rx="{_f(h / 2)}" fill="none" stroke="{i1}" stroke-width="1"/>'
    out += f'<rect x="{_f(cx - 0.5)}" y="{_f(cy - h / 2)}" width="{_f(w + 1)}" height="{_f(h)}" rx="{_f(h / 2)}" fill="none" stroke="{i2}" stroke-width="1"/>'
    return out


def into(cx, cy, H, ink):
    """Input: an arrow into a bracket (the strip's input chip)."""
    s = {12: 3, 14: 3.5, 16: 4}[H]
    out = _sp(f'M{_f(cx - s - 1)} {_f(cy)}H{_f(cx + 0.5)}M{_f(cx - 1.5)} {_f(cy - 2)}L{_f(cx + 0.5)} {_f(cy)}L{_f(cx - 1.5)} {_f(cy + 2)}', ink)
    out += _sp(f'M{_f(cx + 1.5)} {_f(cy - s)}H{_f(cx + s)}V{_f(cy + s)}H{_f(cx + 1.5)}', ink, 1, 'butt', 'miter')
    return out


def out_arrow(cx, cy, H, ink):
    """Output: a short arrow out of the strip."""
    s = {12: 3.5, 14: 4, 16: 4.5}[H]
    return _sp(f'M{_f(cx - s)} {_f(cy)}H{_f(cx + s)}M{_f(cx + s - 2.5)} {_f(cy - 2.5)}L{_f(cx + s)} {_f(cy)}L{_f(cx + s - 2.5)} {_f(cy + 2.5)}', ink)


def ret(cx, cy, H, ink):
    """A return: an arrow coming back down into a bar (the return strip's badge)."""
    s = {12: 3, 14: 3.5}[H]
    out = _sp(f'M{_f(cx - s)} {_f(cy - s)}V{_f(cy + 0.5)}H{_f(cx + s)}M{_f(cx + s - 2)} {_f(cy - 1.5)}L{_f(cx + s)} {_f(cy + 0.5)}L{_f(cx + s - 2)} {_f(cy + 2.5)}', ink)
    return out


def grip(cx, cy, ink):
    """A drag grip: two columns of three dots (shown on hover only)."""
    return ''.join(f'<rect x="{_f(cx - 2 + 3 * i)}" y="{_f(cy - 3.5 + 3 * j)}" width="1" height="1" fill="{ink}"/>'
                   for i in range(2) for j in range(3))
