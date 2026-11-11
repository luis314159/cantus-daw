"""Slate (dir-a) — the glyphs, drawn for the key they sit in.

Each glyph is drawn per key height (16, 20, 24, 30, 36 physical px) from its own table: the
box, the stroke and the detail change with the size (the gear loses teeth, the magnet its
pole caps, the play its lit facet at 16). Two inks: i1 the body, i2 the edges and inner
lines. Transport primitives sit at about a third of the key's height; line icons at a
little over half — always with air around them.
"""
from base import mix

# key height -> (box, stroke) for line icons
LINE = {16: (9, 1), 20: (11, 1), 24: (13, 1.5), 28: (15, 1.5), 30: (17, 2), 36: (19, 2)}


def _sp(d, ink, w, cap='round', extra=''):
    return (f'<path d="{d}" fill="none" stroke="{ink}" stroke-width="{w}" '
            f'stroke-linecap="{cap}" stroke-linejoin="round"{extra}/>')


def _fp(d, ink, extra=''):
    return f'<path d="{d}" fill="{ink}"{extra}/>'


def _f(v):
    return f'{v:.2f}'.rstrip('0').rstrip('.')


# ------------------------------------------------------------- transport (filled)
PLAY = {16: 6, 20: 7, 24: 9, 28: 10, 30: 11, 36: 13}
STOP = {16: 6, 20: 7, 24: 8, 28: 9, 30: 10, 36: 12}
REC = {16: 7, 20: 8, 24: 10, 28: 11, 30: 12, 36: 14}


def play(cx, cy, H, i1, i2):
    gh = PLAY[H]
    wd = round(gh * 0.88)
    x0 = round(cx - wd / 2 + gh * 0.1)
    y0 = round(cy - gh / 2)
    d = f'M{x0} {y0}L{x0 + wd} {_f(y0 + gh / 2)}L{x0} {y0 + gh}Z'
    out = _fp(d, i1)
    if H >= 20:   # the lower facet a shade darker: light from above
        d2 = f'M{x0} {_f(y0 + gh / 2)}L{x0 + wd} {_f(y0 + gh / 2)}L{x0} {y0 + gh}Z'
        out += _fp(d2, mix(i1, i2, 0.42))
    return out


def stop(cx, cy, H, i1, i2):
    s = STOP[H]
    x0, y0 = round(cx - s / 2), round(cy - s / 2)
    r = 1 if H < 30 else 1.5
    out = f'<rect x="{x0}" y="{y0}" width="{s}" height="{s}" rx="{r}" fill="{i1}"/>'
    if H >= 24:
        out += f'<rect x="{x0}" y="{y0 + s // 2}" width="{s}" height="{s - s // 2}" rx="{r}" fill="{mix(i1, i2, 0.42)}"/>'
        out += f'<rect x="{x0}" y="{y0 + s // 2}" width="{s}" height="1" fill="{mix(i1, i2, 0.42)}"/>'
    return out


def rec(cx, cy, H, col, on=False, glow=None):
    d = REC[H]
    out = ''
    if on and glow:
        out += f'<circle cx="{cx}" cy="{cy}" r="{d / 2 + 3}" fill="{glow}" opacity="0.28"/>'
    out += f'<circle cx="{cx}" cy="{cy}" r="{d / 2}" fill="{col}"/>'
    return out


def to_start(cx, cy, H, i1, i2):
    gh = PLAY[H]
    wd = round(gh * 0.78)
    bw = 1 if H < 24 else 2
    gap = 1 if H < 30 else 2
    tot = bw + gap + wd
    x0 = round(cx - tot / 2)
    y0 = round(cy - gh / 2)
    out = f'<rect x="{x0}" y="{y0}" width="{bw}" height="{gh}" fill="{i2}"/>'
    tx = x0 + bw + gap
    out += _fp(f'M{tx + wd} {y0}L{tx} {_f(y0 + gh / 2)}L{tx + wd} {y0 + gh}Z', i1)
    return out


def pause(cx, cy, H, i1, i2):
    s = STOP[H]
    bw = max(2, round(s * 0.34))
    x0, y0 = round(cx - s / 2), round(cy - s / 2)
    return (f'<rect x="{x0}" y="{y0}" width="{bw}" height="{s}" rx="0.5" fill="{i1}"/>'
            f'<rect x="{x0 + s - bw}" y="{y0}" width="{bw}" height="{s}" rx="0.5" fill="{i1}"/>')


LOOP = {16: (9, 6, 1, 2.5), 20: (11, 7, 1, 3), 24: (13, 8, 1.5, 3.5), 28: (15, 9, 1.5, 4),
        30: (16, 10, 2, 4.5), 36: (19, 12, 2, 5)}


def loop(cx, cy, H, i1, i2):
    W, h, w, ah = LOOP[H]
    x0 = round(cx - W / 2)
    y0 = round(cy - h / 2)
    off = 0.5 if w % 2 == 1 else 0
    t, b = y0 + off, y0 + h - off
    r = (b - t) / 2
    l_, r_ = x0 + r + off, x0 + W - r - off
    # the top run, left bend: body ink; the bottom run, right bend: second ink
    p1 = f'M{_f(l_ + 1)} {_f(b)}H{_f(l_)}A{_f(r)} {_f(r)} 0 0 1 {_f(l_)} {_f(t)}H{_f(r_ - ah * 0.6)}'
    p2 = f'M{_f(r_ - 1)} {_f(t)}H{_f(r_)}A{_f(r)} {_f(r)} 0 0 1 {_f(r_)} {_f(b)}H{_f(l_ + ah * 0.6)}'
    out = _sp(p1, i1, w) + _sp(p2, i1, w)
    hy = ah * 0.62
    out += _fp(f'M{_f(r_ + ah * 0.35)} {_f(t)}L{_f(r_ - ah * 0.75)} {_f(t - hy)}L{_f(r_ - ah * 0.75)} {_f(t + hy)}Z', i1)
    out += _fp(f'M{_f(l_ - ah * 0.35)} {_f(b)}L{_f(l_ + ah * 0.75)} {_f(b - hy)}L{_f(l_ + ah * 0.75)} {_f(b + hy)}Z', i1)
    return out


def metronome(cx, cy, H, i1, i2):
    B, w = LINE[H]
    x0, y0 = round(cx - B / 2), round(cy - B / 2)
    o = 0.5 if w % 2 == 1 else 0
    tw_ = max(2, round(B * 0.3))
    body = (f'M{_f(x0 + (B - tw_) / 2)} {_f(y0 + o)}H{_f(x0 + (B + tw_) / 2)}'
            f'L{_f(x0 + B - o)} {_f(y0 + B - o)}H{_f(x0 + o)}Z')
    out = _sp(body, i2, w)
    # the pendulum: from the foot's centre, leaning right and past the body's edge
    px, py = x0 + B / 2, y0 + B - B * 0.2
    ex, ey = x0 + B * 0.92, y0 + B * 0.06
    out += _sp(f'M{_f(px)} {_f(py)}L{_f(ex)} {_f(ey)}', i1, w)
    if H >= 20:   # its weight, a short bar across the stem
        wx, wy = px + (ex - px) * 0.5, py + (ey - py) * 0.5
        out += f'<rect x="{_f(wx - 2)}" y="{_f(wy - 1)}" width="4" height="2" rx="0.6" fill="{i1}" transform="rotate(-35 {_f(wx)} {_f(wy)})"/>'
    return out


# ------------------------------------------------------------- line icons
def _box(cx, cy, H, B=None):
    b, w = LINE[H]
    B = B or b
    return round(cx - B / 2), round(cy - B / 2), B, w


def select(cx, cy, H, i1, i2):
    x, y, B, w = _box(cx, cy, H)
    s = B
    pts = [(0.18, 0), (0.18, 0.86), (0.4, 0.66), (0.56, 1.0), (0.7, 0.93), (0.55, 0.6), (0.84, 0.6)]
    d = 'M' + 'L'.join(f'{_f(x + px * s)} {_f(y + py * s)}' for px, py in pts) + 'Z'
    return _fp(d, i1) + _sp(d, i2, 0.75 if H < 24 else 1)


def draw(cx, cy, H, i1, i2):
    x, y, B, w = _box(cx, cy, H)
    L, th = B * 1.18, max(3, round(B * 0.3))
    g = f'<g transform="rotate(-45 {_f(cx)} {_f(cy)})">'
    x0, y0 = cx - L / 2, cy - th / 2
    tip = th * 0.95
    g += _fp(f'M{_f(x0)} {_f(cy)}L{_f(x0 + tip)} {_f(y0)}V{_f(y0 + th)}Z', i2)
    g += f'<rect x="{_f(x0 + tip)}" y="{_f(y0)}" width="{_f(L - tip)}" height="{th}" rx="0.6" fill="{i1}"/>'
    if H >= 24:
        g += f'<rect x="{_f(x0 + L - th * 0.9)}" y="{_f(y0)}" width="{_f(th * 0.32)}" height="{th}" fill="{i2}"/>'
    return g + '</g>'


def slice_(cx, cy, H, i1, i2):
    x, y, B, w = _box(cx, cy, H)
    # a clip cut in two: two halves with air between, the cut line and its two pointers
    bh = max(3, round(B * 0.42))
    by = round(cy - bh / 2)
    gap = 1 if H < 24 else 1.5
    mid = x + B / 2
    out = f'<rect x="{x}" y="{by}" width="{_f(B / 2 - gap - 0.5)}" height="{bh}" rx="0.8" fill="{i2}"/>'
    out += f'<rect x="{_f(mid + gap + 0.5)}" y="{by}" width="{_f(B / 2 - gap - 0.5)}" height="{bh}" rx="0.8" fill="{i2}"/>'
    lx = round(mid - 0.5) + 0.5 if w < 2 else mid
    out += _sp(f'M{_f(lx)} {y + 1}V{y + B - 1}', i1, 1 if H < 30 else 1.5, 'butt')
    t = max(2, round(B * 0.2))
    out += _fp(f'M{_f(lx - t)} {y}H{_f(lx + t)}L{_f(lx)} {_f(y + t)}Z', i1)
    out += _fp(f'M{_f(lx - t)} {y + B}H{_f(lx + t)}L{_f(lx)} {_f(y + B - t)}Z', i1)
    return out


def erase(cx, cy, H, i1, i2):
    x, y, B, w = _box(cx, cy, H)
    L, th = B * 1.0, max(4, round(B * 0.44))
    g = f'<g transform="rotate(-40 {_f(cx)} {_f(cy - B * 0.06)})">'
    x0, y0 = cx - L / 2, cy - B * 0.06 - th / 2
    g += f'<rect x="{_f(x0)}" y="{_f(y0)}" width="{_f(L * 0.4)}" height="{th}" rx="1" fill="{i2}"/>'
    g += f'<rect x="{_f(x0 + L * 0.4 + 1)}" y="{_f(y0)}" width="{_f(L * 0.6 - 1)}" height="{th}" rx="1" fill="{i1}"/>'
    g += '</g>'
    return g + _sp(f'M{_f(x + B * 0.1)} {_f(y + B - w / 2)}H{_f(x + B * 0.9)}', i2, w)


def mute_tool(cx, cy, H, i1, i2):
    x, y, B, w = _box(cx, cy, H)
    sp = (f'M{_f(x)} {_f(y + B * 0.34)}H{_f(x + B * 0.24)}L{_f(x + B * 0.52)} {_f(y + B * 0.08)}'
          f'V{_f(y + B * 0.92)}L{_f(x + B * 0.24)} {_f(y + B * 0.66)}H{_f(x)}Z')
    out = _fp(sp, i1)
    a, b = x + B * 0.68, x + B * 0.98
    out += _sp(f'M{_f(a)} {_f(y + B * 0.35)}L{_f(b)} {_f(y + B * 0.65)}M{_f(b)} {_f(y + B * 0.35)}L{_f(a)} {_f(y + B * 0.65)}', i2, w)
    return out


def zoom(cx, cy, H, i1, i2):
    x, y, B, w = _box(cx, cy, H)
    r = B * 0.33
    c = (x + B * 0.42, y + B * 0.42)
    out = f'<circle cx="{_f(c[0])}" cy="{_f(c[1])}" r="{_f(r)}" fill="none" stroke="{i1}" stroke-width="{w}"/>'
    out += _sp(f'M{_f(c[0] + r * 0.74)} {_f(c[1] + r * 0.74)}L{_f(x + B - w)} {_f(y + B - w)}', i1, w * 1.7)
    if H >= 24:
        k = r * 0.48
        out += _sp(f'M{_f(c[0] - k)} {_f(c[1])}H{_f(c[0] + k)}M{_f(c[0])} {_f(c[1] - k)}V{_f(c[1] + k)}', i2, w * 0.75)
    return out


def magnet(cx, cy, H, i1, i2):
    x, y, B, w = _box(cx, cy, H)
    sw = max(2, round(B * 0.24))
    r = (B - sw) / 2
    l_, r_ = x + sw / 2, x + B - sw / 2
    top, bend = y + 0.5, y + B - sw / 2 - r
    d = f'M{_f(l_)} {_f(top)}V{_f(bend)}A{_f(r)} {_f(r)} 0 0 0 {_f(r_)} {_f(bend)}V{_f(top)}'
    out = _sp(d, i1, sw, 'butt')
    if H >= 20:   # the pole caps, second ink
        ph = max(2, round(B * 0.22))
        out += f'<rect x="{_f(x)}" y="{_f(top - 0.5)}" width="{sw}" height="{ph}" fill="{i2}"/>'
        out += f'<rect x="{_f(x + B - sw)}" y="{_f(top - 0.5)}" width="{sw}" height="{ph}" fill="{i2}"/>'
    return out


def undo(cx, cy, H, i1, i2, mirror=False):
    x, y, B, w = _box(cx, cy, H)
    ah = B * 0.26
    a0, ay = x + B * 0.14, y + B * 0.36
    rr = B * 0.29
    d = (f'M{_f(a0)} {_f(ay)}H{_f(x + B * 0.6)}A{_f(rr)} {_f(rr)} 0 0 1 {_f(x + B * 0.6)} {_f(ay + 2 * rr)}'
         f'H{_f(x + B * 0.34)}')
    head = f'M{_f(a0 + ah)} {_f(ay - ah)}L{_f(a0)} {_f(ay)}L{_f(a0 + ah)} {_f(ay + ah)}'
    g = _sp(d, i1, w) + _sp(head, i1, w)
    if mirror:
        return f'<g transform="translate({_f(2 * cx)} 0) scale(-1 1)">{g}</g>'
    return g


def redo(cx, cy, H, i1, i2):
    return undo(cx, cy, H, i1, i2, mirror=True)


GEAR = {16: 6, 20: 6, 24: 8, 28: 8, 30: 8, 36: 9}


def gear(cx, cy, H, i1, i2, bg=None):
    import math
    x, y, B, w = _box(cx, cy, H)
    n = GEAR[H]
    R, Ri = B / 2, B / 2 * 0.74
    pts = []
    for k in range(n):
        a0 = 2 * math.pi * k / n
        span = 2 * math.pi / n
        for f, rad in ((-0.27, Ri), (-0.17, R), (0.17, R), (0.27, Ri)):
            a = a0 + f * span
            pts.append((cx + rad * math.sin(a), cy - rad * math.cos(a)))
    d = 'M' + 'L'.join(f'{_f(px)} {_f(py)}' for px, py in pts) + 'Z'
    hr = R * (0.34 if H < 24 else 0.3)
    d += f'M{_f(cx + hr)} {_f(cy)}A{_f(hr)} {_f(hr)} 0 1 0 {_f(cx - hr)} {_f(cy)}A{_f(hr)} {_f(hr)} 0 1 0 {_f(cx + hr)} {_f(cy)}Z'
    out = f'<path d="{d}" fill="{i1}" fill-rule="evenodd"/>'
    if H >= 24:
        out += f'<circle cx="{_f(cx)}" cy="{_f(cy)}" r="{_f(Ri * 0.78)}" fill="none" stroke="{i2}" stroke-width="0.9" opacity="0.9"/>'
    return out


def more(cx, cy, H, i1, i2):
    d = {16: 2, 20: 2, 24: 2.6, 28: 3, 30: 3, 36: 3.6}[H]
    gap = d * 2.1
    return ''.join(f'<circle cx="{_f(cx + k * gap)}" cy="{_f(cy)}" r="{_f(d / 2)}" fill="{i1}"/>' for k in (-1, 0, 1))


def sliders(cx, cy, H, i1, i2):
    x, y, B, w = _box(cx, cy, H)
    out = ''
    for k, kp in enumerate((0.68, 0.3, 0.55)):
        yy = round(y + B * (0.18 + 0.32 * k)) + (0.5 if w % 2 else 0)
        out += _sp(f'M{x} {_f(yy)}H{x + B}', i2, w, 'butt')
        kx = x + B * kp
        out += f'<rect x="{_f(kx - 1.5)}" y="{_f(yy - 2)}" width="3" height="4" rx="0.8" fill="{i1}"/>'
    return out


def speaker(cx, cy, H, i1, i2):
    x, y, B, w = _box(cx, cy, H)
    sp = (f'M{_f(x)} {_f(y + B * 0.34)}H{_f(x + B * 0.22)}L{_f(x + B * 0.5)} {_f(y + B * 0.08)}'
          f'V{_f(y + B * 0.92)}L{_f(x + B * 0.22)} {_f(y + B * 0.66)}H{_f(x)}Z')
    out = _fp(sp, i1)
    for k, rr in enumerate((B * 0.2, B * 0.38)):
        cx0 = x + B * 0.56
        out += _sp(f'M{_f(cx0 + rr * 0.6)} {_f(cy - rr * 0.8)}A{_f(rr)} {_f(rr)} 0 0 1 {_f(cx0 + rr * 0.6)} {_f(cy + rr * 0.8)}', i2, w)
    return out


# ------------------------------------------------------------- the window keys
def w_browser(cx, cy, H, i1, i2):
    x, y, B, w = _box(cx, cy, H)
    o = 0.5 if w % 2 else 0
    t = y + B * 0.16
    d = (f'M{_f(x + o)} {_f(y + B * 0.9)}V{_f(t)}H{_f(x + B * 0.38)}L{_f(x + B * 0.5)} {_f(t + B * 0.14)}'
         f'H{_f(x + B - o)}V{_f(y + B * 0.9)}Z')
    out = _sp(d, i1, w)
    out += _sp(f'M{_f(x + B * 0.22)} {_f(y + B * 0.52)}H{_f(x + B * 0.78)}M{_f(x + B * 0.22)} {_f(y + B * 0.72)}H{_f(x + B * 0.62)}', i2, w)
    return out


def w_arrange(cx, cy, H, i1, i2):
    x, y, B, w = _box(cx, cy, H)
    th = max(2, round(B * 0.18))
    rows = [(0.0, 0.55), (0.3, 1.0), (0.12, 0.72)]
    out = ''
    for k, (a, b) in enumerate(rows):
        yy = round(y + B * 0.08 + k * (B * 0.84 - th) / 2)
        out += f'<rect x="{_f(x + a * B)}" y="{yy}" width="{_f((b - a) * B)}" height="{th}" rx="0.8" fill="{i1}"/>'
    px = round(x + B * 0.86) + 0.5
    return _sp(f'M{_f(px - 2)} {y - 1}V{_f(y + B + 1)}', i2, 1, 'butt') + out


def w_rack(cx, cy, H, i1, i2):
    x, y, B, w = _box(cx, cy, H)
    n = 3
    s = max(2, round(B * 0.2))
    gy = (B - n * s) / (n - 1)
    out = ''
    for r in range(n):
        yy = round(y + r * (s + gy))
        out += f'<rect x="{x}" y="{yy}" width="{s + 1}" height="{s}" rx="0.6" fill="{i2}"/>'
        for c in range(3):
            xx = x + s + 3 + c * ((B - s - 3) / 3)
            on = (r + c) % 2 == 0
            out += f'<rect x="{_f(xx)}" y="{yy}" width="{_f((B - s - 3) / 3 - 1)}" height="{s}" rx="0.6" fill="{i1 if on else "none"}" stroke="{i1}" stroke-width="{0.8 if not on else 0}"/>'
    return out


def w_piano(cx, cy, H, i1, i2):
    x, y, B, w = _box(cx, cy, H)
    kw = max(4, round(B * 0.34))
    # the keyboard: an outlined strip, three black keys reaching from its left edge
    out = f'<rect x="{x + 0.5}" y="{y + 0.5}" width="{kw}" height="{B - 1}" rx="1" fill="none" stroke="{i2}" stroke-width="1"/>'
    bh = max(1.5, round(B * 0.12))
    for k in (0.22, 0.5, 0.78):
        out += f'<rect x="{x}" y="{_f(round(y + k * B - bh / 2))}" width="{_f(kw * 0.6)}" height="{_f(bh)}" fill="{i2}"/>'
    th = max(2, round(B * 0.16))
    gx = x + kw + 2.5
    gw = B - kw - 2.5
    for a, b, ry in ((0.0, 0.45, 0.22), (0.38, 1.0, 0.5), (0.2, 0.7, 0.78)):
        out += f'<rect x="{_f(gx + a * gw)}" y="{_f(round(y + ry * B - th / 2))}" width="{_f((b - a) * gw)}" height="{th}" rx="0.6" fill="{i1}"/>'
    return out


def w_plugin(cx, cy, H, i1, i2):
    x, y, B, w = _box(cx, cy, H)
    o = 0.5 if w % 2 else 0
    out = f'<rect x="{_f(x + o)}" y="{_f(y + B * 0.1 + o)}" width="{_f(B - 2 * o)}" height="{_f(B * 0.8 - 2 * o)}" rx="1.5" fill="none" stroke="{i1}" stroke-width="{w}"/>'
    for k in (0.32, 0.68):
        out += f'<circle cx="{_f(x + B * k)}" cy="{_f(y + B * 0.4)}" r="{_f(B * 0.11)}" fill="{i2}"/>'
    out += _sp(f'M{_f(x + B * 0.2)} {_f(y + B * 0.68)}H{_f(x + B * 0.8)}', i2, w, 'butt')
    return out


def w_mixer(cx, cy, H, i1, i2):
    x, y, B, w = _box(cx, cy, H)
    out = ''
    caps = (0.62, 0.3, 0.5)
    for k, cp in enumerate(caps):
        lx = round(x + B * (0.17 + 0.33 * k)) + 0.5
        out += _sp(f'M{_f(lx)} {y}V{y + B}', i2, 1, 'butt')
        ch = max(2, round(B * 0.18))
        out += f'<rect x="{_f(lx - 2.5)}" y="{_f(round(y + cp * B - ch / 2))}" width="5" height="{ch}" rx="0.8" fill="{i1}"/>'
    return out


def w_inspector(cx, cy, H, i1, i2):
    x, y, B, w = _box(cx, cy, H)
    o = 0.5 if w % 2 else 0
    out = f'<rect x="{_f(x + o)}" y="{_f(y + B * 0.08 + o)}" width="{_f(B - 2 * o)}" height="{_f(B * 0.84 - 2 * o)}" rx="1.5" fill="none" stroke="{i2}" stroke-width="{w}"/>'
    out += f'<rect x="{_f(x + B * 0.58)}" y="{_f(y + B * 0.08)}" width="{_f(B * 0.42)}" height="{_f(B * 0.84)}" rx="1.5" fill="{i1}"/>'
    return out


# ------------------------------------------------------------- small marks
def chevron_down(cx, cy, s, ink, w=1):
    return _sp(f'M{_f(cx - s)} {_f(cy - s / 2)}L{_f(cx)} {_f(cy + s / 2)}L{_f(cx + s)} {_f(cy - s / 2)}', ink, w)


def chevron_right(cx, cy, s, ink, w=1):
    return _sp(f'M{_f(cx - s / 2)} {_f(cy - s)}L{_f(cx + s / 2)} {_f(cy)}L{_f(cx - s / 2)} {_f(cy + s)}', ink, w)


def plus(cx, cy, s, ink, w=1):
    return _sp(f'M{_f(cx - s)} {_f(cy)}H{_f(cx + s)}M{_f(cx)} {_f(cy - s)}V{_f(cy + s)}', ink, w, 'butt')


def kind_drum(cx, cy, ink, ink2):
    # a drum seen a little from above: the head an ellipse, the shell two sides
    return (f'<path d="M{cx - 4.5} {cy - 1.5}V{cy + 2}A4.5 1.6 0 0 0 {cx + 4.5} {cy + 2}V{cy - 1.5}" '
            f'fill="none" stroke="{ink2}" stroke-width="1.1"/>'
            f'<ellipse cx="{cx}" cy="{cy - 1.5}" rx="4.5" ry="1.6" fill="{ink}"/>')


def kind_synth(cx, cy, ink, ink2):
    return _sp(f'M{cx - 4} {cy + 0.5}C{cx - 2.5} {cy - 4} {cx - 1} {cy - 4} {cx} {cy + 0.5}S{cx + 2.5} {cy + 5} {cx + 4} {cy + 0.5}', ink, 1.1)


def kind_audio(cx, cy, ink, ink2):
    hs = [1.5, 3, 4.5, 2.5, 3.5, 1.5]
    return ''.join(f'<rect x="{cx - 5.5 + 2 * k}" y="{_f(cy - h)}" width="1" height="{_f(2 * h)}" fill="{ink}"/>' for k, h in enumerate(hs))


# ------------------------------------------------------------- hinted on the pixel grid
# Placeholders until the icons agent's family is approved: play and loop at 16, 20 and
# 24 px, set pixel by pixel. Every edge is whole or a half-covered pixel in a regular
# rhythm; nothing is left to the rasteriser's guess.
def pixmap(ox, oy, cells, ink):
    """cells: {(x, y): alpha}. Horizontal runs of one alpha become one rect."""
    out = []
    rows = {}
    for (x, y), a in cells.items():
        rows.setdefault(y, []).append((x, a))
    for y, xs in rows.items():
        xs.sort()
        run = None
        for x, a in xs + [(None, None)]:
            if run and x == run[1] + 1 and a == run[2]:
                run[1] = x
                continue
            if run:
                op = '' if run[2] >= 1 else f' opacity="{run[2]}"'
                out.append(f'<rect x="{ox + run[0]}" y="{oy + y}" width="{run[1] - run[0] + 1}" height="1" fill="{ink}"{op}/>')
            run = [x, x, a] if x is not None else None
    return ''.join(out)


PLAY_H = {16: (5, 4), 20: (7, 6), 24: (9, 8)}


def play_hinted(cx, cy, H, i1, i2):
    if H not in PLAY_H:
        return play(cx, cy, H, i1, i2)
    h, W = PLAY_H[H]
    ox = round(cx - W / 2 + 0.5)
    oy = round(cy - h / 2 + 0.5)
    top, bot = {}, {}
    for r in range(h):
        d = abs(r + 0.5 - h / 2)
        w = W * (1 - d / (h / 2))
        w = round(w * 2) / 2            # whole or half pixels only
        full = int(w)
        tgt = top if (r + 0.5) <= h / 2 or H < 20 else bot
        for x in range(full):
            tgt[(x, r)] = 1.0
        if w - full >= 0.5:
            tgt[(full, r)] = 0.5
    out = pixmap(ox, oy, top, i1)
    if bot:
        out += pixmap(ox, oy, bot, mix(i1, i2, 0.42))
    return out


LOOP_PX = {16: (11, 1, 5, 1, 2), 20: (13, 2, 6, 2, 3), 24: (15, 2, 8, 2, 3)}


def loop_hinted(cx, cy, H, i1, i2):
    if H not in LOOP_PX:
        return loop_rect(cx, cy, H, i1, i2)
    W, t, b, ah, al = LOOP_PX[H]
    hh = b + ah + 1 if ah > 1 else b + 2
    c = {}
    ax = W - 3 - al
    bx = al + 2
    for x in range(2, ax):
        c[(x, t)] = 1.0
    for x in range(bx, W - 2):
        c[(x, b)] = 1.0
    for y in range(t + 1, b):
        c[(0, y)] = 1.0
        c[(W - 1, y)] = 1.0
    for x, y in ((1, t), (W - 2, t), (1, b), (W - 2, b)):
        c[(x, y)] = 0.6
    for j in range(al):
        span = ah - j if ah > 1 else (1 if j == 0 else 0)
        for dy in range(-span, span + 1):
            a = 0.5 if (ah > 1 and abs(dy) == ah) else 1.0
            c[(ax + j, t + dy)] = a
            c[(bx - j, b + dy)] = a
    ox = round(cx - W / 2 + 0.5)
    oy = round(cy - (b + t) / 2 - 0.5 + 0.5)
    return pixmap(ox, oy, c, i1)


# ------------------------------------------------------------- native drawing at 150 %
SCALE = [1.0]
NATIVE = {16: 24, 20: 30, 24: 36, 28: 36}


def nat(fn, cx, cy, H, *ink, **kw):
    """Draw a glyph for the physical size it ends up at: at 150 % a 24-px key's glyph is
    the 36-px drawing, placed in a group scaled back by 1/1.5 (the page's device scale
    then makes it whole again)."""
    s = SCALE[0]
    if s == 1.0:
        return fn(cx, cy, H, *ink, **kw)
    return f'<g transform="scale({1 / s:.6f})">{fn(cx * s, cy * s, NATIVE[H], *ink, **kw)}</g>'


LOOP_R = {28: (17, 10, 2, 3, 4), 30: (19, 11, 2, 3.5, 4), 36: (23, 13, 2, 4, 5)}


def loop_rect(cx, cy, H, i1, i2):
    """The hinted loop's shape at 30 and 36 px: straight runs on whole pixels, small round
    corners, two arrowheads — the same drawing as at 16-24, with a 2-px line."""
    W, h, sw, ah, al = LOOP_R[H]
    x0 = round(cx - W / 2)
    y0 = round(cy - h / 2)
    o = sw / 2
    l, r_, t, b = x0 + o, x0 + W - o, y0 + o, y0 + h - o
    rc = sw + 0.5
    ax = r_ - rc - 2 - al          # the top arrow's base
    bx = l + rc + 2 + al           # the bottom arrow's base
    p1 = f'M{_f(bx - al - 1)} {_f(b)}H{_f(l + rc)}A{rc} {rc} 0 0 1 {_f(l)} {_f(b - rc)}V{_f(t + rc)}A{rc} {rc} 0 0 1 {_f(l + rc)} {_f(t)}H{_f(ax)}'
    p2 = f'M{_f(ax + al + 1)} {_f(t)}H{_f(r_ - rc)}A{rc} {rc} 0 0 1 {_f(r_)} {_f(t + rc)}V{_f(b - rc)}A{rc} {rc} 0 0 1 {_f(r_ - rc)} {_f(b)}H{_f(bx)}'
    out = _sp(p1, i1, sw, 'butt') + _sp(p2, i1, sw, 'butt')
    out += _fp(f'M{_f(ax)} {_f(t - ah)}L{_f(ax + al)} {_f(t)}L{_f(ax)} {_f(t + ah)}Z', i1)
    out += _fp(f'M{_f(bx)} {_f(b - ah)}L{_f(bx - al)} {_f(b)}L{_f(bx)} {_f(b + ah)}Z', i1)
    return out


def picker(cx, cy, H, i1, i2):
    """The eyedropper: a bulb, a collar, a slim glass reaching down-left."""
    x, y, B, w = _box(cx, cy, H)
    g = f'<g transform="rotate(45 {_f(cx)} {_f(cy)})">'
    bw = max(3, round(B * 0.36))
    g += f'<rect x="{_f(cx - bw / 2)}" y="{_f(y - B * 0.05)}" width="{bw}" height="{_f(B * 0.3)}" rx="{_f(bw / 2)}" fill="{i1}"/>'
    g += f'<rect x="{_f(cx - bw / 2 - 1)}" y="{_f(y + B * 0.26)}" width="{bw + 2}" height="{_f(max(1.5, B * 0.1))}" rx="0.5" fill="{i1}"/>'
    gw = max(1.5, round(B * 0.16))
    g += f'<rect x="{_f(cx - gw / 2)}" y="{_f(y + B * 0.4)}" width="{_f(gw)}" height="{_f(B * 0.52)}" rx="{_f(gw / 2)}" fill="{i2}"/>'
    return g + '</g>'


def menu_lines(cx, cy, H, i1, i2):
    x, y, B, w = _box(cx, cy, H)
    out = ''
    for k, fr in enumerate((0.2, 0.5, 0.8)):
        yy = round(y + B * fr) + (0.5 if w % 2 else 0)
        out += _sp(f'M{x + 1} {_f(yy)}H{x + B - 1}', i1 if k != 1 else i2, w, 'butt')
    return out


def follow(cx, cy, H, i1, i2):
    """Follow the playhead: the playhead line and a chevron carrying the view along."""
    x, y, B, w = _box(cx, cy, H)
    lx = round(x + B * 0.3) + 0.5
    out = _sp(f'M{_f(lx)} {y}V{y + B}', i2, 1, 'butt')
    out += _sp(f'M{_f(x + B * 0.52)} {_f(y + B * 0.22)}L{_f(x + B * 0.86)} {_f(y + B * 0.5)}L{_f(x + B * 0.52)} {_f(y + B * 0.78)}', i1, w)
    return out


def warn(cx, cy, s, col, ink):
    """A small warning triangle with its mark."""
    h = s * 0.9
    return (f'<path d="M{_f(cx)} {_f(cy - h / 2)}L{_f(cx + s / 2)} {_f(cy + h / 2)}H{_f(cx - s / 2)}Z" fill="{col}" stroke="{col}" stroke-width="1.2" stroke-linejoin="round"/>'
            f'<rect x="{_f(cx - 0.6)}" y="{_f(cy - h * 0.18)}" width="1.2" height="{_f(h * 0.36)}" fill="{ink}"/>'
            f'<rect x="{_f(cx - 0.6)}" y="{_f(cy + h * 0.28)}" width="1.2" height="1.2" fill="{ink}"/>')


def g_midi(cx, cy, H, i1, i2):
    x, y, B, w = _box(cx, cy, H)
    r = B / 2 - 0.5
    out = f'<circle cx="{_f(cx)}" cy="{_f(cy)}" r="{_f(r)}" fill="none" stroke="{i1}" stroke-width="{w}"/>'
    import math
    for a in (-150, -120, -90, -60, -30):
        px, py = cx + r * 0.55 * math.cos(math.radians(a)), cy + r * 0.55 * math.sin(math.radians(a)) + r * 0.2
        out += f'<circle cx="{_f(px)}" cy="{_f(py)}" r="{_f(max(0.8, B * 0.07))}" fill="{i2}"/>'
    return out


def g_look(cx, cy, H, i1, i2):
    x, y, B, w = _box(cx, cy, H)
    r = B / 2 - 0.5
    out = f'<circle cx="{_f(cx)}" cy="{_f(cy)}" r="{_f(r)}" fill="none" stroke="{i1}" stroke-width="{w}"/>'
    out += f'<path d="M{_f(cx)} {_f(cy - r)}A{_f(r)} {_f(r)} 0 0 1 {_f(cx)} {_f(cy + r)}Z" fill="{i1}"/>'
    return out


def g_keys(cx, cy, H, i1, i2):
    x, y, B, w = _box(cx, cy, H)
    o = 0.5 if w % 2 else 0
    out = f'<rect x="{_f(x + o)}" y="{_f(y + B * 0.2 + o)}" width="{_f(B - 2 * o)}" height="{_f(B * 0.62)}" rx="1.5" fill="none" stroke="{i1}" stroke-width="{w}"/>'
    for k in range(4):
        out += f'<rect x="{_f(x + B * (0.18 + 0.18 * k))}" y="{_f(y + B * 0.36)}" width="{_f(B * 0.1)}" height="{_f(B * 0.1)}" fill="{i2}"/>'
    out += f'<rect x="{_f(x + B * 0.28)}" y="{_f(y + B * 0.58)}" width="{_f(B * 0.44)}" height="{_f(max(1, B * 0.09))}" fill="{i2}"/>'
    return out


def g_about(cx, cy, H, i1, i2):
    x, y, B, w = _box(cx, cy, H)
    r = B / 2 - 0.5
    return (f'<circle cx="{_f(cx)}" cy="{_f(cy)}" r="{_f(r)}" fill="none" stroke="{i1}" stroke-width="{w}"/>'
            f'<rect x="{_f(cx - 0.75)}" y="{_f(cy - r * 0.15)}" width="1.5" height="{_f(r * 0.75)}" fill="{i2}"/>'
            f'<rect x="{_f(cx - 0.75)}" y="{_f(cy - r * 0.55)}" width="1.5" height="1.5" fill="{i2}"/>')


def headphones(cx, cy, H, i1, i2):
    x, y, B, w = _box(cx, cy, H)
    r = B / 2 - 1
    out = _sp(f'M{_f(x + 1)} {_f(cy + 1)}A{_f(r)} {_f(r)} 0 0 1 {_f(x + B - 1)} {_f(cy + 1)}', i2, w)
    out += f'<rect x="{_f(x)}" y="{_f(cy)}" width="{_f(B * 0.24)}" height="{_f(B * 0.42)}" rx="1" fill="{i1}"/>'
    out += f'<rect x="{_f(x + B * 0.76)}" y="{_f(cy)}" width="{_f(B * 0.24)}" height="{_f(B * 0.42)}" rx="1" fill="{i1}"/>'
    return out


def interface(cx, cy, H, i1, i2):
    x, y, B, w = _box(cx, cy, H)
    o = 0.5 if w % 2 else 0
    out = f'<rect x="{_f(x + o)}" y="{_f(y + B * 0.25 + o)}" width="{_f(B - 2 * o)}" height="{_f(B * 0.5)}" rx="1.5" fill="none" stroke="{i1}" stroke-width="{w}"/>'
    for k in (0.3, 0.62):
        out += f'<circle cx="{_f(x + B * k)}" cy="{_f(cy)}" r="{_f(B * 0.1)}" fill="{i2}"/>'
    out += f'<rect x="{_f(x + B * 0.78)}" y="{_f(cy - B * 0.08)}" width="{_f(B * 0.08)}" height="{_f(B * 0.16)}" fill="{i2}"/>'
    return out
