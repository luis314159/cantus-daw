"""Slate (dir-a) — the mixer: master, thirteen strips, the Reverb and Delay returns. No
label column: each strip reads top to bottom (number, name, effects, pan, mute/solo,
the fader with its meter beside it, the level in dB, where it goes)."""
import glyphs as G
from base import lit_edge, rect, text, well, mix, tw, hue_for, HUES, INSP_X, MIX_Y, MIX_HEAD, STRIP_Y
from topbar import segmented

R = INSP_X - 2
SEL = 'Chords'

# number, name, level dB, pan (−1..1), effects, output, meter level (playing)
STRIPS = [
    (1, 'Kick', -4.0, 0.0, 'EQ · Comp', '10', 0.86),
    (2, 'Clap', -6.5, 0.08, 'EQ', '10', 0.70),
    (3, 'Hats', -9.0, -0.22, 'EQ', '10', 0.62),
    (4, 'Percussion', -7.5, 0.28, 'Comp', '10', 0.66),
    (5, 'Bass', -5.0, 0.0, 'Comp · EQ', '', 0.80),
    (6, 'Chords', -3.5, 0.0, 'Comp · EQ', '', 0.74),
    (7, 'Pluck', -8.0, -0.34, 'Delay', '', 0.60),
    (8, 'Lead', -6.0, 0.16, 'EQ · Chorus', '', 0.72),
    (9, 'Choir', -10.0, 0.0, 'EQ · Space', '', 0.55),
    (10, 'Drum bus', -2.0, 0.0, 'Comp · Drive', '', 0.88),
    (11, None, 0.0, 0.0, '', '', 0),
    (12, None, 0.0, 0.0, '', '', 0),
    (13, None, 0.0, 0.0, '', '', 0),
]
RETURNS = [(14, 'Reverb', -8.0, 0.0, 'Space', '', 0.42), (15, 'Delay', -12.0, 0.0, 'Echo', '', 0.36)]

PITCH = 62
SW = 60
SY = STRIP_Y + 4
SH = 288
F_TOP, F_BOT = SY + 122, SY + 248   # the fader's travel


def db_frac(db):
    pts = [(6, 0.0), (0, 0.2), (-6, 0.36), (-12, 0.5), (-24, 0.7), (-48, 0.9), (-120, 1.0)]
    for (a, fa), (b, fb) in zip(pts, pts[1:]):
        if db >= b:
            return fa + (fb - fa) * (a - db) / (a - b)
    return 1.0


def fy(db):
    return F_TOP + (F_BOT - F_TOP) * db_frac(db)


def knob(t, cx, cy, r, val, auto=False, hover=False, drag=False, arc_col=None):
    """A small knob: a dark well, a lighter cap, one pointer; the value as an arc
    outside it, from the centre (pan) or from the minimum."""
    import math
    out = f'<circle cx="{cx}" cy="{cy}" r="{r + 0.5}" fill="{t["edge"]}"/>'
    cap = t['raised'] if not hover else mix(t['raised'], t['ink1'], 0.08)
    if t['name'] == 'Paper':
        cap = '#ffffff' if not hover else '#f6f8fa'
    capf = f'url(#kcap-{t["name"]})' if not hover else cap
    out += f'<circle cx="{cx}" cy="{cy}" r="{r - 0.5}" fill="{capf}"/>'
    out += f'<circle cx="{cx}" cy="{cy - 0.5}" r="{r - 1.5}" fill="none" stroke="{mix(cap, t["ink1"], 0.10)}" stroke-width="1"/>'
    a = val * 135
    ra = math.radians(a)
    px, py = cx + math.sin(ra) * (r - 2.5), cy - math.cos(ra) * (r - 2.5)
    qx, qy = cx + math.sin(ra) * (r * 0.25), cy - math.cos(ra) * (r * 0.25)
    out += f'<path d="M{qx:.2f} {qy:.2f}L{px:.2f} {py:.2f}" stroke="{t["ink1"]}" stroke-width="1.6" stroke-linecap="round"/>'
    ar = r + 3
    col = arc_col or (t['auto'] if auto else (t['accent'] if drag else t['ink3']))
    if abs(val) > 0.01:
        a0, a1 = sorted((0, a))
        x0, y0 = cx + math.sin(math.radians(a0)) * ar, cy - math.cos(math.radians(a0)) * ar
        x1, y1 = cx + math.sin(math.radians(a1)) * ar, cy - math.cos(math.radians(a1)) * ar
        out += f'<path d="M{x0:.2f} {y0:.2f}A{ar} {ar} 0 0 1 {x1:.2f} {y1:.2f}" fill="none" stroke="{col}" stroke-width="1.5" stroke-linecap="round"/>'
    else:
        out += f'<rect x="{cx - 0.5}" y="{cy - ar - 1}" width="1" height="2.5" fill="{t["ink3"]}"/>'
    if auto:
        out += f'<circle cx="{cx + r + 2}" cy="{cy + r + 1}" r="2" fill="{t["auto"]}"/>'
    return out


def fader(t, x, db, sel=False, hue=None, quiet=False, levels=None, peak=None, clip=False, mw=3):
    """The fader's groove, its cap at the level, the 0 dB and scale ticks, the meter."""
    out = ''
    gx = x
    groove = mix(t['well'], t['edge'], 0.5)
    out += rect(gx - 2, F_TOP - 4, 4, F_BOT - F_TOP + 8, groove, 2)
    for d in (6, 0, -6, -12, -24, -48):
        yy = round(fy(d))
        out += rect(gx + 10, yy, 2 if d else 3, 1, t['ink3'] if d == 0 else mix(t['ink4'], t['base'], 0.3))
    # the meter: two slim bars; idle shows only their grooves
    mx = gx + 15
    mg = mix(t['well'], t['ink4'], 0.22)
    for k in range(2):
        out += rect(mx + k * (mw + 1), F_TOP - 4, mw, F_BOT - F_TOP + 8, mg, 1)
    if levels:
        for k, lv in enumerate(levels):
            top = round(fy(lv))
            ylo = round(fy(-12))
            yhi = round(fy(-3))
            xk = mx + k * (mw + 1)
            bot = F_BOT + 4
            out += rect(xk, max(top, ylo), mw, bot - max(top, ylo), t['meter_lo'])
            if top < ylo:
                out += rect(xk, max(top, yhi), mw, ylo - max(top, yhi), t['meter_mid'])
            if top < yhi:
                out += rect(xk, top, mw, yhi - top, t['meter_hi'])
            if peak is not None:
                out += rect(xk, round(fy(peak[k])), mw, 1, t['ink1'])
        if clip:
            out += rect(mx, F_TOP - 9, 2 * mw + 1, 3, t['clip'], 1)
    # the cap
    cy = round(fy(db))
    if quiet:
        out += rect(gx - 9, cy - 3, 18, 6, mix(t['surface'], t['ink4'], 0.45), 2)
        return out
    capc = '#3b444e' if t['name'] == 'Night' else '#ffffff'
    out += rect(gx - 11, cy - 6, 22, 13, t['edge'], 2.5)
    out += rect(gx - 10, cy - 5, 20, 11, f'url(#fcap-{t["name"]})', 2)
    grip = mix(capc, t['edge'], 0.55)
    out += rect(gx - 6, cy - 3, 12, 1, grip) + rect(gx - 6, cy + 3, 12, 1, grip)
    out += rect(gx - 8, cy, 16, 1, hue if (sel and hue) else t['ink1'])
    return out


def ms_keys(t, cx, y, mute=False, solo=False, sel=False):
    out = ''
    for k, (lab, on, col) in enumerate((('M', mute, t['ink2']), ('S', solo, t['solo']))):
        x = cx - 23 + k * 24
        if on:
            out += rect(x, y, 22, 16, col, 3)
            out += text(x + 11, y + 12, lab, 10, '#10151a' if t['name'] == 'Night' else '#fff', 600, 'middle')
        else:
            out += well(t, x, y, 22, 16, 3)
            out += text(x + 11, y + 12, lab, 10, t['ink2'] if sel else t['ink3'], 600, 'middle')
    return out


def strip(t, x, s, state, w=SW):
    num, name, db, pan, fx, outp, lv = s
    cx = x + w // 2
    sel = name == SEL
    playing = state.get('playing')
    if name is None:
        out = rect(x, SY, w, SH, mix(t['surface'], t['base'], 0.55), 3)
        out += text(cx, SY + 16, str(num), 10, t['ink3'], 400, 'middle')
        out += fader(t, x + 20, 0.0, quiet=True)
        return out
    hue = HUES[name]
    bg = mix(t['surface'], hue, t['head_sel']) if sel else f'url(#strip-{t["name"]})'
    out = rect(x, SY, w, SH, bg, 3)
    out += rect(x, SY, w, 3, hue if sel else hue_for(t, name, t['stripe_rest']), 0)
    out += text(cx, SY + 16, str(num), 10, t['ink3'], 400, 'middle')
    fs = 12 if tw(name, 12) <= w - 6 else 11
    out += text(cx, SY + 31, name, fs, t['ink1'], 500 if sel else 400, 'middle')
    if fx:
        out += text(cx, SY + 46, fx, 10, t['ink3'], 400, 'middle')
    out += knob(t, cx, SY + 68, 9, pan)
    st = (state.get('tracks') or {}).get(name, '')
    out += ms_keys(t, cx, SY + 88, 'm' in st, 's' in st, sel)
    levels = (lv * 0.92, lv * 0.88) if playing and lv else None
    to_db = (lambda f: -48 + 54 * f) if levels else None
    lvdb = (to_db(levels[0]) + 0, to_db(levels[1])) if levels else None
    out += fader(t, x + 20, db, sel, hue, levels=lvdb, peak=(lvdb[0] + 2.5, lvdb[1] + 3) if lvdb else None)
    out += text(cx, SY + 268, ('%.1f' % db).replace('-', '−'), 11, t['ink1'] if sel else t['ink2'], 400, 'middle')
    if outp:
        out += text(cx, SY + 283, f'→ {outp}', 10, t['ink3'], 400, 'middle')
    return out


def master(t, x, state, w=68):
    cx = x + w // 2
    out = rect(x, SY, w, SH, f'url(#strip-{t["name"]})', 3)
    out += rect(x, SY, w, 3, t['ink3'], 0)
    out += text(cx, SY + 16, 'M', 10, t['ink3'], 600, 'middle')
    out += text(cx, SY + 31, 'Master', 12, t['ink1'], 500, 'middle')
    out += text(cx, SY + 46, 'EQ · Limit', 10, t['ink3'], 400, 'middle')
    out += knob(t, cx, SY + 68, 9, 0)
    out += ms_keys(t, cx, SY + 88)
    # the scale's numbers to the left of the fader: the only labels the mixer keeps
    for d in (6, 0, -6, -12, -24, -48):
        out += text(x + 19, round(fy(d)) + 3, str(d).replace('-', '−'), 9, t['ink3'] if d else t['ink2'], 400, 'end')
    lv = state.get('master')
    lvdb = (-48 + 54 * lv[0], -48 + 54 * lv[1]) if lv else None
    out += fader(t, x + 31, 0.0, levels=lvdb, peak=(lvdb[0] + 1.5, lvdb[1] + 2) if lvdb else None, mw=4)
    out += text(cx, SY + 268, '0.0', 11, t['ink2'], 400, 'middle')
    out += text(cx, SY + 283, 'Out 1–2', 10, t['ink3'], 400, 'middle')
    return out


def header(t):
    y = MIX_Y
    out = rect(0, y, R, MIX_HEAD, t['surface']) + lit_edge(t, 0, y, R)
    x = 12
    for k, lab in enumerate(('Mixer', 'Piano roll', 'Channel rack')):
        on = k == 0
        out += text(x, y + 18, lab, 13 if on else 12, t['ink1'] if on else t['ink3'], 500 if on else 400)
        if on:
            out += rect(x, y + MIX_HEAD - 2, tw(lab, 13, 500), 2, t['ink2'])
        x += tw(lab, 13 if on else 12, 500 if on else 400) + 20
    out += segmented(t, x + 8, y + 4, ['Compact', 'Wide', 'Large'], [58, 44, 46], 20, 1, 11)
    out += text(R - 12, y + 18, 'Snapshots', 11, t['ink3'], 400, 'end')
    out += G.chevron_down(R - 12 - tw('Snapshots', 11) - 9, y + 14, 2.5, t['ink3'], 1.2)
    return out


def mixer(t, state):
    out = header(t)
    out += rect(0, STRIP_Y, R, 698 - STRIP_Y, t['base'])
    out += master(t, 8, state)
    x = 84
    for s in STRIPS:
        out += strip(t, x, s, state)
        x += PITCH
    x += 8
    for s in RETURNS:
        out += strip(t, x, s, state)
        x += PITCH
    return out
