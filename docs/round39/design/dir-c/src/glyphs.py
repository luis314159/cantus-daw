"""Studio ink glyphs, drawn by hand for each physical key size (16, 20, 24, 30, 36 px).

A glyph is designed in the physical pixels of its key, never one drawing scaled: every
size has its own table row (extent, stroke, arrowhead), and every edge that can sit on a
pixel does. The glyph is about a third of its key with air around it. Two inks: `a` is the
body (ink-2 at rest, the accent when on), `b` the second tone (edges, inner lines, poles).
"""

SIZES = (16, 20, 24, 30, 36)


def near(P):
    return min(SIZES, key=lambda s: abs(s - P))


def _p(d, a, sw, fill="none", cap="round", join="round"):
    if fill != "none":
        return f'<path d="{d}" fill="{fill}"/>'
    return (f'<path d="{d}" fill="none" stroke="{a}" stroke-width="{sw}" '
            f'stroke-linecap="{cap}" stroke-linejoin="{join}"/>')


def _r(x, y, w, h, f, rx=0):
    return f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{rx}" fill="{f}"/>'


def _c(cx, cy, r, f="none", st=None, sw=1):
    s = f' stroke="{st}" stroke-width="{sw}"' if st else ""
    return f'<circle cx="{cx}" cy="{cy}" r="{r}" fill="{f}"{s}/>'


# --- transport -------------------------------------------------------------------

def play(P, a, b):
    h = {16: 6, 20: 8, 24: 9, 30: 11, 36: 13}[P]
    w = {16: 5, 20: 7, 24: 8, 30: 10, 36: 11}[P]
    x0 = (P - w) // 2 + (1 if P >= 24 else 0)
    y0 = (P - h) / 2
    d = f"M{x0} {y0} L{x0 + w} {P / 2} L{x0} {y0 + h} Z"
    out = [f'<path d="{d}" fill="{a}" stroke="{a}" stroke-width="{0.6 if P < 24 else 1}" stroke-linejoin="round"/>']
    if P >= 24:  # the lit upper face: a hairline along the top edge, the second ink
        out.append(_p(f"M{x0 + 0.5} {y0 + 0.9} L{x0 + w - 1.2} {P / 2 - 0.2}", b, 1 if P < 30 else 1.25))
    return out


def stop(P, a, b):
    s = {16: 5, 20: 6, 24: 7, 30: 9, 36: 10}[P]
    o = (P - s) / 2
    return [_r(o, o, s, s, a, 0.75 if P < 24 else 1.25)]


def record(P, a, b):
    d = {16: 6, 20: 7, 24: 8, 30: 10, 36: 12}[P]
    out = [_c(P / 2, P / 2, d / 2, a)]
    if P >= 24:
        out.append(_c(P / 2, P / 2, d / 2 - 0.5, "none", b, 1))
    return out


def to_start(P, a, b):
    h = {16: 6, 20: 8, 24: 9, 30: 11, 36: 13}[P]
    bw = {16: 1, 20: 1, 24: 2, 30: 2, 36: 2}[P]
    w = {16: 5, 20: 6, 24: 7, 30: 9, 36: 10}[P]
    gap = 1 if P < 30 else 1.5
    tw = bw + gap + w
    x0 = (P - tw) // 2
    y0 = (P - h) / 2
    xt = x0 + bw + gap
    return [_r(x0, y0, bw, h, a),
            f'<path d="M{xt + w} {y0} L{xt} {P / 2} L{xt + w} {y0 + h} Z" fill="{a}" '
            f'stroke="{a}" stroke-width="{0.6 if P < 24 else 1}" stroke-linejoin="round"/>']


def loop(P, a, b):
    W, H, t, ah = {16: (10, 6, 1, 2), 20: (12, 7, 1.25, 2.25), 24: (14, 8, 1.5, 2.75),
                   30: (17, 10, 1.75, 3.25), 36: (21, 12, 2, 4)}[P]
    x0, y0 = (P - W) / 2, (P - H) / 2
    x1, y1 = x0 + W, y0 + H
    r = H / 2 - 0.5
    ym = P / 2
    ea = x1 - ah * 0.9
    eb = x0 + ah * 0.9
    A = f"M{x0} {ym + 0.5} V{y0 + r} Q{x0} {y0} {x0 + r} {y0} H{ea}"
    B = f"M{x1} {ym - 0.5} V{y1 - r} Q{x1} {y1} {x1 - r} {y1} H{eb}"
    ha = f"M{ea - ah} {y0 - ah} L{ea} {y0} L{ea - ah} {y0 + ah}"
    hb = f"M{eb + ah} {y1 - ah} L{eb} {y1} L{eb + ah} {y1 + ah}"
    return [_p(A, a, t), _p(B, a, t), _p(ha, a, t), _p(hb, a, t)]


def metronome(P, a, b):
    h, bw, tw, t = {16: (8, 8, 4, 1), 20: (10, 9, 4, 1.1), 24: (11, 10, 4, 1.25),
                    30: (14, 12, 5, 1.5), 36: (16, 14, 6, 1.75)}[P]
    cx = P / 2
    y0 = (P - h) / 2
    y1 = y0 + h
    body = (f"M{cx - bw / 2} {y1} L{cx - tw / 2} {y0} L{cx + tw / 2} {y0} L{cx + bw / 2} {y1} Z")
    arm = f"M{cx - bw * 0.05} {y1 - t * 1.2} L{cx + bw * 0.46} {y0 - h * 0.08}"
    out = [_p(body, a, t), _p(f"M{cx - bw / 2 + t} {y1 - h * 0.28} H{cx + bw / 2 - t}", a, t * 0.8), _p(arm, b, t)]
    return out


# --- tools -------------------------------------------------------------------------

def select(P, a, b):
    k = {16: 0.62, 20: 0.75, 24: 0.9, 30: 1.1, 36: 1.3}[P]
    pts = [(0, 0), (0, 10), (2.6, 7.7), (4.3, 11.3), (5.9, 10.6), (4.3, 7.1), (7.4, 7.1)]
    w, h = 7.4 * k, 11.3 * k
    ox, oy = round((P - w) / 2), round((P - h) / 2)
    d = "M" + " L".join(f"{ox + x * k:.2f} {oy + y * k:.2f}" for x, y in pts) + " Z"
    return [f'<path d="{d}" fill="{a}" stroke="{a}" stroke-width="0.5" stroke-linejoin="round"/>']


def draw(P, a, b):
    L, T = {16: (10, 3), 20: (12, 3.5), 24: (14, 4), 30: (17, 5), 36: (20, 6)}[P]
    c = P / 2
    x0 = c - L / 2
    tip = T * 1.1
    body = _r(x0 + tip, c - T / 2, L - tip - T * 0.7, T, a)
    cap = _r(x0 + L - T * 0.55, c - T / 2, T * 0.55, T, b)
    nib = f'<path d="M{x0} {c} L{x0 + tip - 0.6} {c - T / 2} L{x0 + tip - 0.6} {c + T / 2} Z" fill="{b}"/>'
    return [f'<g transform="rotate(-45 {c} {c})">{body}{cap}{nib}</g>']


def slice_(P, a, b):
    w, h, gap = {16: (4, 6, 2), 20: (5, 7, 2), 24: (6, 8, 3), 30: (7, 10, 3), 36: (9, 12, 4)}[P]
    tw = w * 2 + gap
    x0, y0 = (P - tw) // 2, (P - h) / 2
    out = [_r(x0, y0, w, h, a, 0.75), _r(x0 + w + gap, y0, w, h, a, 0.75)]
    lx = x0 + w + gap / 2
    ov = {16: 2, 20: 2, 24: 3, 30: 3, 36: 4}[P]
    sw = 1 if P < 30 else 1.5
    out.append(_p(f"M{lx} {y0 - ov} V{y0 + h + ov}", b, sw, cap="butt"))
    return out


def erase(P, a, b):
    L, T = {16: (9, 5), 20: (11, 6), 24: (13, 7), 30: (16, 8), 36: (19, 10)}[P]
    c = P / 2
    x0 = c - L / 2
    sw = 1 if P < 30 else 1.25
    front = _r(x0, c - T / 2, L * 0.42, T, a, 1)
    back = (f'<rect x="{x0 + L * 0.42 + sw / 2}" y="{c - T / 2 + sw / 2}" width="{L * 0.58 - sw}" '
            f'height="{T - sw}" rx="1" fill="none" stroke="{b}" stroke-width="{sw}"/>')
    return [f'<g transform="rotate(-40 {c} {c})">{front}{back}</g>']


def snap(P, a, b):
    W, H, t, tip = {16: (8, 8, 2, 2), 20: (9, 9, 2, 2), 24: (10, 11, 2.5, 3),
                    30: (13, 13, 3, 3), 36: (15, 16, 3.5, 4)}[P]
    x0, y0 = (P - W) / 2, (P - H) / 2
    r = W / 2 - t / 2
    xl, xr = x0 + t / 2, x0 + W - t / 2
    yb = y0 + H - t / 2 - r
    body = f"M{xl} {y0 + tip + 1} V{yb} A{r} {r} 0 0 0 {xr} {yb} V{y0 + tip + 1}"
    return [_p(body, a, t, cap="butt"),
            _r(x0, y0, t, tip, b), _r(x0 + W - t, y0, t, tip, b)]


def undo(P, a, b, mirror=False):
    W, H, t, ah = {16: (9, 6, 1.1, 2.2), 20: (11, 7, 1.25, 2.5), 24: (12, 8, 1.5, 3),
                   30: (15, 10, 1.75, 3.5), 36: (18, 12, 2, 4)}[P]
    x0, y0 = (P - W) / 2, (P - H) / 2
    R = H / 2
    xr = x0 + W
    d = f"M{x0 + 0.5} {y0} H{xr - R} A{R} {R} 0 0 1 {xr - R} {y0 + H} H{x0 + 2}"
    hd = f"M{x0 + 0.5 + ah} {y0 - ah} L{x0 + 0.5} {y0} L{x0 + 0.5 + ah} {y0 + ah}"
    g = _p(d, a, t) + _p(hd, a, t)
    if mirror:
        return [f'<g transform="translate({P} 0) scale(-1 1)">{g}</g>']
    return [g]


def redo(P, a, b):
    return undo(P, a, b, mirror=True)


def more(P, a, b):
    r = {16: 1, 20: 1.1, 24: 1.3, 30: 1.6, 36: 1.9}[P]
    g = {16: 4, 20: 4.5, 24: 5, 30: 6, 36: 7}[P]
    return [_c(P / 2 + k * g, P / 2, r, a) for k in (-1, 0, 1)]


# --- windows -------------------------------------------------------------------------

def w_browser(P, a, b):
    W, H, t = {16: (9, 7, 1), 20: (11, 8, 1), 24: (12, 9, 1.25), 30: (15, 11, 1.5), 36: (18, 13, 1.75)}[P]
    x0, y0 = (P - W) / 2, (P - H) / 2
    tab = W * 0.4
    d = (f"M{x0} {y0 + H} V{y0 + 0.5} H{x0 + tab - 1} L{x0 + tab + 1} {y0 + 2} H{x0 + W} V{y0 + H} Z")
    return [_p(d, a, t), _p(f"M{x0} {y0 + 3.5} H{x0 + W}", b, t)]


def w_arrange(P, a, b):
    W, bh, g = {16: (10, 2, 1), 20: (12, 2, 1.5), 24: (13, 2, 2), 30: (17, 3, 2), 36: (20, 3, 3)}[P]
    H = bh * 3 + g * 2
    x0, y0 = (P - W) // 2, (P - H) // 2
    return [_r(x0, y0, W * 0.55, bh, a, 0.5), _r(x0 + W * 0.3, y0 + bh + g, W * 0.7, bh, b, 0.5),
            _r(x0 + W * 0.12, y0 + 2 * (bh + g), W * 0.5, bh, a, 0.5)]


def w_rack(P, a, b):
    s, g = {16: (2, 1), 20: (2, 1.5), 24: (2.5, 1.5), 30: (3, 2), 36: (3.5, 2.5)}[P]
    n = 4
    W = n * s + (n - 1) * g
    H = 3 * s + 2 * g
    x0, y0 = (P - W) / 2, (P - H) / 2
    on = {(0, 0), (2, 0), (1, 1), (3, 1), (0, 2), (2, 2), (3, 2)}
    return [f'<rect x="{x0 + i * (s + g)}" y="{y0 + j * (s + g)}" width="{s}" height="{s}" rx="0.4" '
            f'fill="{a}" opacity="{1 if (i, j) in on else 0.32}"/>'
            for j in range(3) for i in range(n)]


def w_piano(P, a, b):
    H, kw = {16: (9, 3), 20: (10, 3), 24: (12, 4), 30: (15, 5), 36: (18, 6)}[P]
    W = H + 2
    x0, y0 = (P - W) // 2, (P - H) // 2
    nb = 2 if P < 30 else 3
    kh = (H - 2) / 3  # three white keys seen end-on, a pixel apart
    out = [_r(x0, y0 + i * (kh + 1), kw, kh, a, 0.5) for i in range(3)]
    out.append(_r(x0 + kw + 2, y0 + 1, W * 0.34, nb, b, 0.5))
    out.append(_r(x0 + kw + 2 + W * 0.2, y0 + H / 2 - nb / 2, W * 0.42, nb, b, 0.5))
    out.append(_r(x0 + kw + 2 + W * 0.06, y0 + H - nb - 1, W * 0.3, nb, b, 0.5))
    return out


def w_mixer(P, a, b):
    H, g, t, cw = {16: (9, 3, 1, 3), 20: (10, 3.5, 1, 3), 24: (12, 4, 1.25, 4),
                   30: (15, 5, 1.5, 5), 36: (18, 6, 1.75, 6)}[P]
    x0, y0 = P / 2 - g, (P - H) / 2
    out = []
    for i, k in enumerate((0.62, 0.3, 0.5)):
        x = x0 + i * g
        out.append(_p(f"M{x} {y0} V{y0 + H}", b, t * 0.8, cap="butt"))
        out.append(_r(x - cw / 2, y0 + H * k - 1, cw, 2 if P < 30 else 2.5, a, 0.4))
    return out


def w_inspector(P, a, b):
    W, H, t = {16: (10, 8, 1), 20: (12, 9, 1), 24: (13, 11, 1.25), 30: (16, 13, 1.5), 36: (19, 16, 1.75)}[P]
    x0, y0 = (P - W) / 2, (P - H) / 2
    out = [f'<rect x="{x0 + t / 2}" y="{y0 + t / 2}" width="{W - t}" height="{H - t}" rx="1" fill="none" stroke="{a}" stroke-width="{t}"/>']
    xs = x0 + W * 0.58
    out.append(_r(xs, y0, W - (xs - x0), H, a, 1))
    for k in (0.32, 0.55, 0.78):
        out.append(_r(x0 + 2.2, y0 + H * k - 0.5, W * 0.3, 1, b))
    return out


def w_instrument(P, a, b):
    W, H, t = {16: (10, 7, 1), 20: (12, 8, 1), 24: (14, 9, 1.25), 30: (17, 11, 1.5), 36: (20, 13, 1.75)}[P]
    x0, y0 = (P - W) / 2, (P - H) / 2
    out = [f'<rect x="{x0 + t / 2}" y="{y0 + t / 2}" width="{W - t}" height="{H - t}" rx="1" fill="none" stroke="{a}" stroke-width="{t}"/>']
    for k in (0.25, 0.5, 0.75):
        out.append(_r(x0 + W * k - 0.9, y0 + t / 2, 1.8, H * 0.58, b))
    return out


GLYPHS = dict(play=play, stop=stop, record=record, to_start=to_start, loop=loop,
              metronome=metronome, select=select, draw=draw, slice=slice_, erase=erase,
              snap=snap, undo=undo, redo=redo, more=more, browser=w_browser,
              arrange=w_arrange, rack=w_rack, piano=w_piano, mixer=w_mixer,
              inspector=w_inspector, instrument=w_instrument)


def glyph(svg, name, x, y, key, a, b):
    """Draw glyph `name` centred in a `key`-px (logical) key whose top-left is (x, y)."""
    P = near(round(key * svg.s))
    els = GLYPHS[name](P, a, b)
    # centre the physical drawing in the physical key, then go back to logical units
    off = (round(key * svg.s) - P) / 2
    tx, ty = svg.sn(x) + off / svg.s, svg.sn(y) + off / svg.s
    svg.raw(f'<g transform="translate({tx:.3f} {ty:.3f}) scale({1 / svg.s:.5f})">' + "".join(els) + "</g>")
