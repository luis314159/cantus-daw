"""des-controls — faders, their meters, and the horizontal sliders.

A fader is a groove, a scale and a cap; its meter is a pair of slim bars in their own well.
At rest the meter shows its well and the scale, never a black slab and never "−∞".
"""
from slate import f, rect, text, mix, tw, line, circle, frame
from signs import sign, sign_col

# per scale: groove width, cap w x h, the cap's index line, meter bar width, the gap
# between bars, the clip LED's height, the peak-hold line, scale tick lengths
FAD = {1: dict(gw=2, cw=20, ch=18, il=1, mw=3, mg=1, led=3, hold=1, tk=3, tk0=5, gap=5),
       1.5: dict(gw=3, cw=31, ch=28, il=2, mw=4, mg=2, led=4, hold=2, tk=4, tk0=7, gap=7)}
MASTER = {1: dict(mw=5, mg=1), 1.5: dict(mw=7, mg=2)}
DB = [(6, 0.0), (0, 0.20), (-6, 0.36), (-12, 0.51), (-24, 0.70), (-48, 0.90), (-120, 1.0)]


def frac(db):
    """dB to the travel, 0 at the top (+6) and 1 at the bottom (silence)."""
    if db >= DB[0][0]:
        return 0.0
    for (d0, f0), (d1, f1) in zip(DB, DB[1:]):
        if d1 <= db <= d0:
            return f0 + (f1 - f0) * (d0 - db) / (d0 - d1)
    return 1.0


def meter_grad(t, name):
    """The meter's colour by height: green up to −18, through yellow-green to amber at −3."""
    y18, y9, y3 = frac(-18), frac(-9), frac(-3)
    return (f'<linearGradient id="{name}" x1="0" y1="1" x2="0" y2="0" gradientUnits="objectBoundingBox">'
            f'<stop offset="0" stop-color="{t["meter_lo"]}"/>'
            f'<stop offset="{1 - y18:.3f}" stop-color="{t["meter_lo"]}"/>'
            f'<stop offset="{1 - y9:.3f}" stop-color="{t["meter_mid"]}"/>'
            f'<stop offset="{1 - y3:.3f}" stop-color="{t["meter_hi"]}"/>'
            f'<stop offset="1" stop-color="{t["meter_hi"]}"/></linearGradient>')


def meter(t, s, x, top, hgt, levels=None, peaks=None, hold=None, clip=False, mw=None, mg=None,
          gid='mg'):
    """Two bars from x, the travel from `top` over `hgt` px. levels = RMS (dB) per side,
    peaks = the fast peak per side, hold = the held peak per side."""
    g = FAD[s]
    mw = mw or g['mw']
    mg = mg or g['mg']
    W = mw * 2 + mg
    out = ''
    # the clip LED above the pair
    ly = top - g['led'] - (2 if s == 1 else 3)
    led = t['clip'] if clip else mix(t['well'], t['ink4'], 0.25)
    out += rect(x, ly, W, g['led'], led, 1)
    for k in range(2):
        bx = x + k * (mw + mg)
        out += rect(bx, top, mw, hgt, mix(t['well'], t['ink4'], 0.22) if t['name'] == 'Night'
                    else mix(t['well'], t['edge'], 0.35), 1 if s == 1 else 1.5)
        if levels:
            # the bar is cut from the full-height gradient, so a colour belongs to a height
            yp = top + round(hgt * frac(peaks[k]))
            out += (f'<svg x="{f(bx)}" y="{f(yp)}" width="{f(mw)}" height="{f(top + hgt - yp)}" overflow="hidden">'
                    f'<rect x="0" y="{f(top - yp)}" width="{f(mw)}" height="{f(hgt)}" fill="url(#{gid}-{t["name"]})"/></svg>')
        if hold:
            yh = top + round(hgt * frac(hold[k]))
            hc = t['clip'] if hold[k] > 0 else t['ink1']
            out += rect(bx, yh, mw, g['hold'], hc)
    return out


def scale(t, s, x, top, hgt, numbers=False, side='right'):
    """Scale ticks (and on the large strips, their numbers) for the travel."""
    g = FAD[s]
    out = ''
    for d, _ in DB[:-1]:
        y = top + round(hgt * frac(d))
        n = d == 0
        ln = g['tk0'] if n else g['tk']
        col = t['ink3'] if n else mix(t['surface'], t['ink3'], 0.55)
        out += rect(x if side == 'right' else x - ln, y, ln, 1, col)
        if numbers:
            lab = ('+%d' % d) if d > 0 else ('%d' % d if d else '0')
            lab = lab.replace('-', '−')
            nx = x + ln + 3 * s if side == 'right' else x - ln - 3 * s
            out += text(nx, y + 3.3 * s, lab, 9 * s, t['ink2'] if n else t['ink3'], 600 if n else 400,
                        'start' if side == 'right' else 'end')
    return out


def cap(t, s, cx, cy, state='rest', drive=None):
    """The fader's cap, centred on (cx, cy): flat-lit, a 1-px rim, an index line across."""
    g = FAD[s]
    w, h = g['cw'], g['ch']
    x, y = cx - w / 2, cy - h / 2
    n = t['name'] == 'Night'
    r = 2 if s == 1 else 3
    out = ''
    if state == 'disabled':
        out += rect(x, y, w, h, mix(t['surface'], t['well'], 0.5), r) + frame(x, y, w, h, t['ink4'], r)
        out += rect(x + 3 * s, cy - g['il'] / 2, w - 6 * s, g['il'], t['ink4'])
        return out
    out += rect(x, y + 1, w, h, 'rgba(0,0,0,0.45)' if n else 'rgba(40,55,70,0.20)', r)    # stands on the strip
    rim = t['edge'] if n else mix(t['edge'], t['ink3'], 0.25)
    if drive == 'auto':
        rim = t['auto']
    elif state == 'drag':
        rim = t['accent']
    out += rect(x, y, w, h, rim, r)
    out += rect(x + 1, y + 1, w - 2, h - 2, f'url(#fcap-{t["name"]})', max(1, r - 1))
    if state == 'hover':
        out += rect(x + 1, y + 1, w - 2, h - 2, t['ink1'] if n else '#ffffff', max(1, r - 1),
                    ' fill-opacity="0.07"' if n else ' fill-opacity="0.6"')
    out += rect(x + 2, y + 1, w - 4, 1, mix(t['fcap_hi'], '#ffffff', 0.35 if n else 1.0))          # lit top
    # the grips: two dark lines either side of the index, pressed into the cap
    gc = mix(t['fcap_lo'], t['edge'], 0.55) if n else mix(t['fcap_lo'], t['ink4'], 0.7)
    for dy in (-4, 4) if s == 1 else (-6, 6):
        out += rect(x + 5 * s, round(cy + dy - 0.5), w - 10 * s, 1, gc)
    # the index: the exact level, a light line on a dark one (engraved)
    ic = t['ink1'] if n else t['ink1']
    if drive == 'auto':
        ic = t['auto']
    elif state == 'drag':
        ic = t['accent']
    iy = round(cy - g['il'] / 2)
    if n:
        out += rect(x + 2 * s, iy - 1, w - 4 * s, 1, mix(t['fcap_mid'], t['edge'], 0.6))
    out += rect(x + 2 * s, iy, w - 4 * s, g['il'], '#f4f8fb' if (n and ic == t['ink1']) else (ic if ic != t['ink1'] else t['ink1']))
    return out


def fader(t, s, x, top, hgt, db, state='rest', drive=None, vca=None, **mtr):
    """A strip's fader at x (the groove's left edge): groove, scale, meter, cap."""
    g = FAD[s]
    out = ''
    gx = x
    dis = state == 'disabled'
    out += rect(gx, top - 2 * s, g['gw'], hgt + 4 * s, t['edge'] if t['name'] == 'Night' else mix(t['edge'], t['well'], 0.3), g['gw'] / 2)
    # the 0 dB detent: a tick either side of the groove
    y0 = top + round(hgt * frac(0))
    out += rect(gx - g['tk0'] - 2 * s, y0, g['tk0'], 1, t['ink3'])
    out += scale(t, s, gx + g['gw'] + 2 * s, top, hgt)
    mx = gx + g['gw'] + 2 * s + g['tk0'] + g['gap'] - 2 * s
    out += meter(t, s, mx, top, hgt, **mtr)
    cx = gx + g['gw'] / 2
    cy = top + round(hgt * frac(db))
    if vca is not None:
        # where the VCA puts it: a hollow ghost cap and the sign beside it
        vy = top + round(hgt * frac(db + vca))
        out += frame(cx - g['cw'] / 2, vy - g['ch'] / 2, g['cw'], g['ch'], t['ink3'], 2 if s == 1 else 3,
                     extra=f' stroke-dasharray="{2 * s} {2 * s}"')
        out += sign('vca', cx - g['cw'] / 2 - 6 * s, vy, 5 if s == 1 else 8, sign_col(t, 'vca'))
    out += cap(t, s, cx, cy, state, drive)
    if drive == 'auto':
        out += sign('auto', cx - g['cw'] / 2 - 7 * s, cy, 5 if s == 1 else 8, t['auto'], t['base'])
    if dis:
        out = f'<g opacity="0.55">{out}</g>'
    return out, mx
