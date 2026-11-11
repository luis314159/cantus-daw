"""des-controls — horizontal sliders and the inline value bar.

A slider speaks the fader's language turned on its side: a groove, a fill for the value and
a small lit cap with one index line. The value bar is the dense form for inspector rows and
editors: a well whose fill *is* the value, the name at the left and the number at the right.
"""
from slate import f, rect, text, mix, tw, frame, well
from signs import sign, sign_col

SL = {1: dict(tr=3, tw=9, th=15, il=1, vb=20, edge=1),
      1.5: dict(tr=4, tw=13, th=22, il=2, vb=30, edge=2)}


def _fill_col(t, state, drive):
    if state == 'disabled':
        return t['ink4']
    if drive == 'auto':
        return t['auto']
    return {'rest': mix(t['ink2'], t['ink3'], 0.25), 'hover': mix(t['ink2'], t['ink1'], 0.6),
            'drag': t['accent'], 'focus': t['ink1']}[state]


def thumb(t, s, cx, cy, state='rest', drive=None):
    g = SL[s]
    w, h = g['tw'], g['th']
    x, y = cx - w / 2, cy - h / 2
    n = t['name'] == 'Night'
    r = 2 if s == 1 else 3
    if state == 'disabled':
        return (rect(x, y, w, h, mix(t['surface'], t['well'], 0.5), r) + frame(x, y, w, h, t['ink4'], r))
    rim = t['auto'] if drive == 'auto' else (t['accent'] if state == 'drag' else (t['edge'] if n else mix(t['edge'], t['ink3'], 0.25)))
    out = rect(x, y + 1, w, h, 'rgba(0,0,0,0.45)' if n else 'rgba(40,55,70,0.20)', r)
    out += rect(x, y, w, h, rim, r)
    out += rect(x + 1, y + 1, w - 2, h - 2, f'url(#fcap-{t["name"]})', max(1, r - 1))
    if state == 'hover':
        out += rect(x + 1, y + 1, w - 2, h - 2, t['ink1'] if n else '#ffffff', max(1, r - 1),
                    ' fill-opacity="0.08"' if n else ' fill-opacity="0.6"')
    ic = t['auto'] if drive == 'auto' else (t['accent'] if state == 'drag' else (t['ink1'] if n else t['ink2']))
    out += rect(round(cx - g['il'] / 2), y + 3 * s, g['il'], h - 6 * s, ic)
    return out


def slider(t, s, x, cy, w, v, state='rest', bipolar=False, drive=None, rng=None):
    """A slider from x over w px, centred on the line cy. v in 0..1 (or −1..1 if bipolar).
    rng = (lo, hi) draws a range slider with two thumbs."""
    g = SL[s]
    out = ''
    tr = g['tr']
    ty = round(cy - tr / 2)
    grv = t['edge'] if t['name'] == 'Night' else mix(t['edge'], t['well'], 0.3)
    out += rect(x, ty, w, tr, grv, tr / 2)
    col = _fill_col(t, state, drive)
    if rng:
        a, b = x + w * rng[0], x + w * rng[1]
        out += rect(a, ty, b - a, tr, col, tr / 2)
        out += thumb(t, s, a, cy, state, drive) + thumb(t, s, b, cy, state, drive)
        return out
    if bipolar:
        c = x + w / 2
        px = c + v * w / 2
        out += rect(round(c) - (0 if s == 1 else 1), ty - 3 * s, 1 if s == 1 else 2, tr + 6 * s, t['ink3'])
        a, b = sorted((c, px))
        out += rect(a, ty, b - a, tr, col)
    else:
        px = x + w * v
        out += rect(x, ty, px - x, tr, col, tr / 2)
    out += thumb(t, s, px, cy, state, drive)
    if drive == 'auto':
        out += sign('auto', x - 9 * s, cy, 5 if s == 1 else 8, t['auto'])
    return out


def valuebar(t, s, x, y, w, name, val, unit, v, state='rest', bipolar=False, drive=None,
             mod=None):
    """The inline value bar: the fill is the value; name left, number right."""
    g = SL[s]
    h = g['vb']
    r = 3 if s == 1 else 4
    hov = state in ('hover', 'drag')
    out = well(t, x, y, w, h, r, t['well_hi'] if hov else t['well'])
    ix, iw = x + 1, w - 2
    dis = state == 'disabled'
    if drive == 'auto':
        fc, ec = mix(t['well'], t['auto'], 0.30 if t['name'] == 'Night' else 0.22), t['auto']
    elif state == 'drag':
        fc, ec = mix(t['well'], t['accent'], 0.30 if t['name'] == 'Night' else 0.22), t['accent']
    else:
        base = t['well_hi'] if hov else t['well']
        fc = mix(base, t['ink3'], 0.30 if t['name'] == 'Night' else 0.26)
        ec = t['ink1'] if hov else t['ink2']
    if dis:
        fc, ec = mix(t['well'], t['ink4'], 0.25), t['ink4']
    if bipolar:
        c = ix + iw / 2
        px = c + v * iw / 2
        a, b = sorted((c, px))
        out += rect(a, y + 1, b - a, h - 2, fc)
        out += rect(round(c), y + 1, 1, h - 2, mix(fc, t['ink3'], 0.5))
    else:
        px = ix + iw * v
        out += rect(ix, y + 1, px - ix, h - 2, fc, max(0, r - 1))
    if mod:
        lo, hi, live = mod
        out += rect(ix + iw * lo, y + h - 1 - 3 * s, iw * (hi - lo), 3 * s, t['mod'], 0, ' fill-opacity="0.55"')
        out += rect(round(ix + iw * live), y + 1, g['edge'], h - 2, t['mod'])
    out += rect(round(px) - (g['edge'] // 2), y + 1, g['edge'], h - 2, ec)      # the value's edge
    ty = y + h / 2 + 4 * s
    pad = 8 * s
    out += text(x + pad, ty, name, 11 * s, t['ink4'] if dis else (t['ink1'] if hov else t['ink2']))
    us = 10 * s
    vx = x + w - pad
    if unit:
        out += text(vx, ty, unit, us, t['ink4'] if dis else t['ink3'], 400, 'end')
        vx -= tw(unit, us) + 3 * s
    vcol = t['ink4'] if dis else (t['auto'] if drive == 'auto' else (t['accent'] if state == 'drag' else t['ink1']))
    out += text(vx, ty, val, 12 * s, vcol, 400, 'end')
    if drive:
        sx = vx - tw(val, 12 * s) - 8 * s
        out += sign(drive, sx, y + h / 2, 5 if s == 1 else 8, sign_col(t, drive), None)
    if state == 'focus':
        out += frame(x - 2 * s, y - 2 * s, w + 4 * s, h + 4 * s, t['accent'], r + 2 * s, 1)
    return out
