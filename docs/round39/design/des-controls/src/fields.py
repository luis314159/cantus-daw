"""des-controls — fields and drop-downs.

A field is Slate's recessed well. The number sits right, its unit after it in ink 3; text
sits left. Typing lifts the well into an accent ring; a wrong value turns the ring red and
says, under the field, what is allowed. A drop-down is a field with a chevron; it opens a
list that is a menu (menus.py) attached under it.
"""
from slate import f, rect, text, mix, tw, frame, well, path, circle, line

FLD = {1: {24: dict(h=24, fs=12, us=10, pad=8, ring=2), 20: dict(h=20, fs=11, us=10, pad=6, ring=2)},
       1.5: {24: dict(h=36, fs=18, us=15, pad=12, ring=3), 20: dict(h=30, fs=16.5, us=15, pad=9, ring=3)}}


def chevron(t, s, cx, cy, col, d='down', small=False):
    """A chevron drawn for its size: 7 x 4 (1.25) at 100 %, 10 x 6 (1.75) at 150 %."""
    w, h, sw = (7, 4, 1.25) if s == 1 else (10, 5.5, 1.75)
    if small:
        w, h, sw = (5, 3, 1.1) if s == 1 else (7, 4, 1.5)
    if d == 'down':
        dd = f'M{f(cx - w / 2)} {f(cy - h / 2)}L{f(cx)} {f(cy + h / 2)}L{f(cx + w / 2)} {f(cy - h / 2)}'
    elif d == 'up':
        dd = f'M{f(cx - w / 2)} {f(cy + h / 2)}L{f(cx)} {f(cy - h / 2)}L{f(cx + w / 2)} {f(cy + h / 2)}'
    else:  # right
        dd = f'M{f(cx - h / 2)} {f(cy - w / 2)}L{f(cx + h / 2)} {f(cy)}L{f(cx - h / 2)} {f(cy + w / 2)}'
    return path(dd, col, sw)


def ring(t, s, x, y, w, h, col, r):
    """The focus ring: 2 (3) px outside the well, the colour at 55 %."""
    g = 2 if s == 1 else 3
    return frame(x - g, y - g, w + 2 * g, h + 2 * g, col, r + g, g, ' stroke-opacity="0.55"')


def numfield(t, s, x, y, w, val, unit='', state='rest', size=24, msg=''):
    g = FLD[s][size]
    h = g['h']
    r = 3 if size == 20 else 4
    r = r if s == 1 else r + 1
    hov = state in ('hover', 'scrub')
    out = ''
    if state in ('edit', 'invalid'):
        out += ring(t, s, x, y, w, h, t['rec'] if state == 'invalid' else t['accent'], r)
    fill = t['well_hi'] if hov else (t['edge'] if state in ('edit', 'invalid') and t['name'] == 'Night' else
                                      ('#ffffff' if state in ('edit', 'invalid') else t['well']))
    out += well(t, x, y, w, h, r, fill)
    ty = y + h / 2 + g['fs'] * 0.36
    dis = state == 'disabled'
    vx = x + w - g['pad']
    if hov:
        # the spin arrows appear on hover, a column of their own
        ax = x + w - g['pad'] - 2 * s
        out += chevron(t, s, ax, y + h * 0.32, t['ink2'], 'up', True)
        out += chevron(t, s, ax, y + h * 0.68, t['ink2'], 'down', True)
        vx -= 12 * s
    if unit and state not in ('edit', 'invalid'):
        out += text(vx, ty, unit, g['us'], t['ink4'] if dis else t['ink3'], 400, 'end')
        vx -= tw(unit, g['us']) + 3 * s
    vcol = {'rest': t['ink1'], 'hover': t['ink1'], 'scrub': t['accent'], 'edit': t['ink1'],
            'invalid': t['rec'], 'disabled': t['ink4']}[state]
    if state in ('edit', 'invalid'):
        # typing: the text sits left like any text, the caret after it, the part selected
        tx = x + g['pad']
        if state == 'edit':
            sel = val[:3]
            sw_ = tw(sel, g['fs'])
            out += rect(tx - 1, y + 4 * s, sw_ + 2, h - 8 * s, t['accent'], 2, ' fill-opacity="0.35"')
        out += text(tx, ty, val, g['fs'], vcol)
        cx = tx + tw(val, g['fs']) + 1.5 * s
        out += rect(round(cx), y + 4 * s, 1 if s == 1 else 2, h - 8 * s, t['accent'] if state == 'edit' else t['rec'])
        if unit:
            out += text(x + w - g['pad'], ty, unit, g['us'], t['ink3'], 400, 'end')
    else:
        out += text(vx, ty, val, g['fs'], vcol, 400, 'end')
    if state == 'scrub':
        out += rect(x + 1, y + h - 1 - (2 if s == 1 else 3), w - 2, 2 if s == 1 else 3, t['accent'])
    if msg:
        mx = x + 1
        my = y + h + 14 * s
        out += circle(mx + 3 * s, my - 3.5 * s, 3 * s, t['rec'])
        out += rect(mx + 3 * s - 0.5 * s, my - 5.5 * s, 1 * s, 2.5 * s, '#ffffff')
        out += rect(mx + 3 * s - 0.5 * s, my - 2.2 * s, 1 * s, 1 * s, '#ffffff')
        out += text(mx + 10 * s, my, msg, 10.5 * s, t['rec'])
    return out


def textfield(t, s, x, y, w, val, state='rest', placeholder='', search=False, size=24):
    g = FLD[s][size]
    h = g['h']
    r = 4 if s == 1 else 5
    out = ''
    if state == 'edit':
        out += ring(t, s, x, y, w, h, t['accent'], r)
    fill = t['well_hi'] if state == 'hover' else (t['edge'] if state == 'edit' and t['name'] == 'Night'
                                                   else ('#ffffff' if state == 'edit' else t['well']))
    out += well(t, x, y, w, h, r, fill)
    tx = x + g['pad']
    ty = y + h / 2 + g['fs'] * 0.36
    if search:
        # a lens drawn for its size: ring 4.5 (7) px, handle at 45°
        rr = 4 if s == 1 else 6
        cx, cy = x + g['pad'] + rr, y + h / 2 - 1 * s
        sw = 1.25 if s == 1 else 1.75
        out += f'<circle cx="{f(cx)}" cy="{f(cy)}" r="{f(rr)}" fill="none" stroke="{t["ink3"]}" stroke-width="{sw}"/>'
        out += line(cx + rr * 0.72, cy + rr * 0.72, cx + rr * 1.55, cy + rr * 1.55, t['ink3'], sw + 0.25)
        tx += rr * 2 + 7 * s
    if val:
        if state == 'edit':
            sel = val.split(' ')[-1]
            sx = tx + tw(val[:len(val) - len(sel)], g['fs'])
            out += rect(sx - 1, y + 4 * s, tw(sel, g['fs']) + 2, h - 8 * s, t['accent'], 2, ' fill-opacity="0.35"')
        out += text(tx, ty, val, g['fs'], t['ink1'])
        if state == 'edit':
            cx = tx + tw(val, g['fs']) + 1.5 * s
            out += rect(round(cx), y + 4 * s, 1 if s == 1 else 2, h - 8 * s, t['accent'])
        if search:
            # the clear key: a small ring with an x, ink 3
            kx, ky = x + w - g['pad'] - 5 * s, y + h / 2
            out += circle(kx, ky, 6 * s, mix(t['well'], t['ink4'], 0.6))
            k = 2.2 * s
            out += line(kx - k, ky - k, kx + k, ky + k, t['ink1'], 1.25 * s)
            out += line(kx - k, ky + k, kx + k, ky - k, t['ink1'], 1.25 * s)
    else:
        out += text(tx, ty, placeholder, g['fs'], t['ink3'])
        if state == 'edit':
            out += rect(round(tx), y + 4 * s, 1 if s == 1 else 2, h - 8 * s, t['accent'])
    return out


def dropdown(t, s, x, y, w, val, state='rest', size=24, quiet=False, label=''):
    """A drop-down: the value, the chevron in its own 18 (27) px zone at the right."""
    g = FLD[s][size]
    h = g['h']
    r = 4 if s == 1 else 5
    out = ''
    dis = state == 'disabled'
    if quiet:
        # a heading's drop-down: no well until hovered
        if state in ('hover', 'open'):
            out += rect(x, y, w, h, t['raised'] if t['name'] == 'Night' else t['well'], r)
    else:
        if state == 'open':
            out += ring(t, s, x, y, w, h, t['accent'], r)
        out += well(t, x, y, w, h, r, t['well_hi'] if state in ('hover', 'open') else t['well'])
    ty = y + h / 2 + g['fs'] * 0.36
    tx = x + g['pad']
    if label:
        out += text(tx, ty, label, g['us'] + 1 * s, t['ink3'])
        tx += tw(label, g['us'] + 1 * s) + 6 * s
    out += text(tx, ty, val, g['fs'], t['ink4'] if dis else t['ink1'], 500 if quiet else 400)
    cz = 18 * s
    if quiet:
        cx = tx + tw(val, g['fs'], 500) + 9 * s
    else:
        cx = x + w - cz / 2 - 1 * s
    col = t['ink4'] if dis else (t['ink1'] if state in ('hover', 'open') else t['ink2'])
    out += chevron(t, s, cx, y + h / 2, col, 'up' if state == 'open' else 'down')
    return out


def timefield(t, s, x, y, segs, state='rest', hot=None):
    """A position field: bars . beats . ticks, each segment its own drag target."""
    g = FLD[s][24]
    h = g['h']
    r = 4 if s == 1 else 5
    out = ''
    w = sum(tw(sg, g['fs']) for sg in segs) + (len(segs) - 1) * 10 * s + 2 * g['pad']
    if state == 'edit':
        out += ring(t, s, x, y, w, h, t['accent'], r)
    out += well(t, x, y, w, h, r, t['well_hi'] if state == 'hover' else t['well'])
    tx = x + g['pad']
    ty = y + h / 2 + g['fs'] * 0.36
    for i, sg in enumerate(segs):
        sw_ = tw(sg, g['fs'])
        if hot == i:
            out += rect(tx - 3 * s, y + 3 * s, sw_ + 6 * s, h - 6 * s,
                        t['accent'] if state == 'edit' else mix(t['well'], t['ink3'], 0.35), 2,
                        ' fill-opacity="0.35"' if state == 'edit' else '')
        out += text(tx, ty, sg, g['fs'], t['ink1'] if i == 0 else t['ink2'])
        tx += sw_
        if i < len(segs) - 1:
            out += text(tx + 5 * s, ty, '.', g['fs'], t['ink3'], 400, 'middle')
            tx += 10 * s
    return out, w
