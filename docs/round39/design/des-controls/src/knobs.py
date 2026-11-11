"""des-controls — the knob family: small (strip, inline), medium (panels, inspector, effect
editors), large (an instrument's or effect's hero controls). Unipolar, bipolar, stepped.

Anatomy, outside in: a 270° groove (the scale), the value arc on it, a gap, a cap lit from
above with one pointer. The groove's open quarter at the bottom is where a driven value
shows its sign — the one place every knob has free.
"""
import math
from slate import f, rect, text, circle, mix, tw, path
from signs import sign, sign_col

# per size, per scale: outer diameter, groove width, groove-to-cap gap, pointer width,
# pointer start (fraction of the cap radius), the sign's height, the focus ring's offset
KNOB = {
    'S': {1: dict(D=20, ring=2, gap=2, ptr=1.5, p0=0.20, sg=4, fo=2),
          1.5: dict(D=30, ring=3, gap=3, ptr=2, p0=0.20, sg=6, fo=3)},
    'M': {1: dict(D=32, ring=2, gap=3, ptr=2, p0=0.28, sg=6, fo=2),
          1.5: dict(D=48, ring=3, gap=4, ptr=2.5, p0=0.28, sg=9, fo=3)},
    'L': {1: dict(D=52, ring=3, gap=4, ptr=2.5, p0=0.34, sg=8, fo=3),
          1.5: dict(D=78, ring=4, gap=6, ptr=3.5, p0=0.34, sg=12, fo=4)},
}
SWEEP = 135  # degrees either side of the top


def ang(v, bipolar=False):
    """Value to angle (degrees clockwise from the top)."""
    return SWEEP * v if bipolar else -SWEEP + 2 * SWEEP * v


def pt(cx, cy, r, a):
    ra = math.radians(a)
    return cx + math.sin(ra) * r, cy - math.cos(ra) * r


def arc(cx, cy, r, a0, a1, col, sw, cap='round', op=1.0):
    if abs(a1 - a0) < 0.3:
        return ''
    a0, a1 = sorted((a0, a1))
    x0, y0 = pt(cx, cy, r, a0)
    x1, y1 = pt(cx, cy, r, a1)
    large = 1 if a1 - a0 > 180 else 0
    o = f' stroke-opacity="{op}"' if op < 1 else ''
    return (f'<path d="M{f(x0)} {f(y0)}A{f(r)} {f(r)} 0 {large} 1 {f(x1)} {f(y1)}" fill="none" '
            f'stroke="{col}" stroke-width="{f(sw)}" stroke-linecap="{cap}"{o}/>')


def groove_col(t):
    return t['well'] if t['name'] == 'Night' else mix(t['well'], t['edge'], 0.55)


def cap(t, cx, cy, rc, state):
    """The cap: a 1-px rim, a radial light from the upper left, a lit crescent on top."""
    n = t['name'] == 'Night'
    out = ''
    if state == 'disabled':
        out += circle(cx, cy, rc, t['edge'] if n else t['line'])
        out += circle(cx, cy, rc - 1, mix(t['surface'], t['well'], 0.6))
        return out
    # the cap's own shadow, one pixel down: it stands on the panel
    out += circle(cx, cy + 1, rc, 'rgba(0,0,0,0.35)' if n else 'rgba(40,55,70,0.16)')
    out += circle(cx, cy, rc, t['edge'] if n else mix(t['edge'], t['ink3'], 0.15))
    out += circle(cx, cy, rc - 1, f'url(#cap-{t["name"]})')
    if state in ('hover', 'drag'):
        out += circle(cx, cy, rc - 1, t['ink1'] if n else '#ffffff', ' fill-opacity="0.07"' if n else ' fill-opacity="0.5"')
    hi = mix(t['cap_hi'], '#ffffff', 0.18) if n else '#ffffff'
    out += arc(cx, cy, rc - 1.5, -58, 58, hi, 1, 'round', 0.75 if n else 1.0)
    return out


def knob(t, s, size, cx, cy, v=0.6, bipolar=False, state='rest', drive=None, steps=None,
         mod=(0.0, 0.0), held=None, bg=None):
    """One knob centred on (cx, cy), drawn for `size` at scale `s`.

    state: rest | hover | drag | focus | disabled.  drive: None | auto | mod | midi.
    steps: a number of detents (stepped knob).  mod: (depth, live offset) for a modulator.
    held: the automation clip's value while the hand holds an automated knob."""
    g = KNOB[size][s]
    R = g['D'] / 2 - g['ring'] / 2                 # the groove's centre line
    rc = g['D'] / 2 - g['ring'] - g['gap']         # the cap
    out = ''
    dis = state == 'disabled'
    a = ang(v, bipolar)
    if steps:
        # detents instead of a groove: a dot per step, the chosen one lit
        for k in range(steps):
            ak = ang(k / (steps - 1))
            px, py = pt(cx, cy, R, ak)
            on = abs(ak - a) < 0.5
            rr = g['ring'] * (0.75 if on else 0.5)
            col = (t['ink1'] if state in ('hover', 'drag') else t['ink2']) if on else mix(t['surface'], t['ink3'], 0.7)
            if state == 'drag' and on:
                col = t['accent']
            out += circle(px, py, rr, t['ink4'] if dis else col)
    else:
        out += arc(cx, cy, R, -SWEEP, SWEEP, groove_col(t), g['ring'], 'round', 0.5 if dis else 1)
        if bipolar:
            # the centre detent: a notch above the groove
            nx, ny = pt(cx, cy, R + g['ring'] / 2 + 0.5, 0)
            out += rect(cx - 0.5 * (1 if s == 1 else 1.5), cy - R - g['ring'] / 2 - (2 if s == 1 else 3),
                        1 if s == 1 else 1.5, 2 if s == 1 else 3, t['ink3'] if not dis else t['ink4'])
        val_col = {'rest': mix(t['ink2'], t['ink3'], 0.25), 'hover': mix(t['ink2'], t['ink1'], 0.6), 'drag': t['accent'],
                   'focus': t['ink1'], 'disabled': t['ink4']}[state]
        if drive == 'auto':
            val_col = t['auto'] if held is None else t['accent']
        a0 = 0 if bipolar else -SWEEP
        if drive == 'mod':
            depth, live = mod
            lo, hi = max(-SWEEP, a - depth * 2 * SWEEP), min(SWEEP, a + depth * 2 * SWEEP)
            out += arc(cx, cy, R, lo, hi, t['mod'], g['ring'] + (1 if s == 1 else 2), 'butt', 0.38)
        out += arc(cx, cy, R, a0, a, val_col, g['ring'])
        if drive == 'mod':
            lx, ly = pt(cx, cy, R, a + mod[1] * 2 * SWEEP)
            out += circle(lx, ly, g['ring'] * 0.5 + (1 if s == 1 else 1.5), t['base'])
            out += circle(lx, ly, g['ring'] * 0.5 + (0.5 if s == 1 else 1), t['mod'])
        if held is not None:
            # where the clip would put it: a violet tick across the groove
            ah = ang(held, bipolar)
            x0, y0 = pt(cx, cy, R - g['ring'], ah)
            x1, y1 = pt(cx, cy, R + g['ring'], ah)
            out += f'<path d="M{f(x0)} {f(y0)}L{f(x1)} {f(y1)}" stroke="{t["auto"]}" stroke-width="{1.5 if s == 1 else 2}" stroke-linecap="round"/>'
    out += cap(t, cx, cy, rc, state)
    # the pointer: from near the centre to just inside the rim
    pc = t['ink1']
    if dis:
        pc = t['ink4']
    elif drive == 'auto' and held is None:
        pc = t['auto'] if t['name'] == 'Night' else mix(t['auto'], '#000000', 0.1)
    x0, y0 = pt(cx, cy, rc * g['p0'], a)
    x1, y1 = pt(cx, cy, rc - g['ptr'] * 0.5 - 1.5, a)
    out += (f'<path d="M{f(x0)} {f(y0)}L{f(x1)} {f(y1)}" stroke="{pc}" stroke-width="{f(g["ptr"])}" '
            f'stroke-linecap="round"/>')
    if state == 'focus':
        out += (f'<circle cx="{f(cx)}" cy="{f(cy)}" r="{f(g["D"] / 2 + g["fo"] + 0.5)}" fill="none" '
                f'stroke="{t["accent"]}" stroke-width="1" stroke-opacity="0.85"/>')
    if drive:
        # the sign, in the groove's open quarter
        sy = cy + rc + (1.5 if s == 1 else 2) + g['sg'] / 2
        if size == 'S':
            out += circle(cx, cy + R + 0.5, g['sg'] / 2 + 0.5, bg or t['base'])
            out += sign('dot', cx, cy + R + 0.5, g['sg'], sign_col(t, drive))
        else:
            out += sign(drive, cx, sy, g['sg'], sign_col(t, drive), bg or t['base'])
    return out


def bubble(t, s, x, cy, val):
    """The value while a small knob is held: a raised pill beside it."""
    h = 18 if s == 1 else 26
    sz = 11 * s
    w = tw(val, sz) + (12 if s == 1 else 18)
    return (rect(x, cy - h / 2, w, h, t['raised'], 4 * s if s == 1 else 5)
            + text(x + w / 2, cy + sz * 0.36, val, sz, t['ink1'], 400, 'middle'))


def valtext(t, s, cx, y, val, unit, col=None, size=12):
    """A value and its unit: the number in ink 1, the unit a size down in ink 3."""
    vs, us = size * s, (size - 2) * s
    w = tw(val, vs) + (tw(' ' + unit, us) if unit else 0)
    x = cx - w / 2
    out = text(x, y, val, vs, col or t['ink1'])
    if unit:
        out += text(x + tw(val, vs) + 2.5 * s, y, unit, us, t['ink3'])
    return out
