"""Slate (dir-a) — the parts sheet, second half: knobs, fader and meter, strips and the
small controls (tabs, field, segmented, toggle, scrollbars, a menu with its submenu)."""
import glyphs as G
import mixer as M
from base import rect, text, well, mix, tw
from topbar import segmented
from overflow import toggle, keycap


def knobs(t, x0, y0):
    out = [text(x0, y0, 'Knobs', 14, t['ink1'], 500)]
    out.append(text(x0 + 52, y0, 'a dark well, a lit cap, one pointer; the value as an arc outside — from the centre for pan', 11, t['ink3']))
    names = [('Rest', {}), ('Hover', {'hover': True}), ('Dragged', {'drag': True}), ('Automated', {'auto': True})]
    pitch = 170
    for row, (r, lab) in enumerate(((9, 'strip · r 9'), (14, 'panel · r 14'))):
        y = y0 + 34 + row * 66
        out.append(text(x0, y + 4, lab, 10, t['ink3']))
        for k, (nm, kw) in enumerate(names):
            cx = x0 + 100 + k * pitch
            val = -0.3 if r == 9 else 0.55
            if r == 14:
                # a panel knob reads from its minimum: draw its track ring too
                out.append(f'<circle cx="{cx}" cy="{y}" r="{r + 3}" fill="none" stroke="{mix(t["base"], t["ink4"], 0.6)}" stroke-width="1.5" stroke-dasharray="{(r + 3) * 3.1416 * 1.5:.1f} 999" transform="rotate(135 {cx} {y})" stroke-linecap="round"/>')
            out.append(M.knob(t, cx, y, r, val, **kw, arc_col=None))
            if r == 14 and not kw.get('auto') and not kw.get('drag'):
                pass
            out.append(text(cx, y + r + 18, nm, 10, t['ink2'], 400, 'middle'))
            if nm == 'Dragged':
                v = 'L 30' if r == 9 else '−4.2 dB'
                bw = tw(v, 11) + 12
                out.append(rect(cx + r + 8, y - 9, bw, 18, t['raised'], 4))
                out.append(text(cx + r + 8 + bw / 2, y + 4, v, 11, t['ink1'], 400, 'middle'))
            if nm == 'Automated':
                out.append(text(cx + r + 8, y + 4, 'lane', 10, t['auto'], 500))
    return ''.join(out)


def _fader_at(t, x, y_top, db, **kw):
    dy = y_top - (M.F_TOP - 10)
    return f'<g transform="translate(0 {dy})">{M.fader(t, x, db, **kw)}</g>'


def lower(t, x0, y0):
    out = [text(x0, y0, 'Fader and meter', 14, t['ink1'], 500)]
    yt = y0 + 18
    cases = [('Silent', None, None, False, '−3.5', ''),
             ('Playing', (-9.5, -11.0), (-6.8, -8.1), False, '−3.5', '−6.8'),
             ('Clipping', (0.6, -0.4), (0.6, 0.2), True, '−3.5', '+0.6')]
    for k, (nm, lv, pk, clip, val, peak) in enumerate(cases):
        x = x0 + 26 + k * 76
        out.append(_fader_at(t, x, yt, -3.5, levels=lv, peak=pk, clip=clip))
        by = yt + (M.F_BOT - M.F_TOP) + 30
        out.append(text(x + 8, by, val, 11, t['ink2'], 400, 'middle'))
        if peak:
            out.append(text(x + 8, by + 14, peak, 10, t['clip'] if clip else t['ink3'], 500 if clip else 400, 'middle'))
        out.append(text(x + 8, by + 30, nm, 10, t['ink3'], 400, 'middle'))
    # two strips: at rest, and selected while playing
    sx = x0 + 250
    out.append(text(sx, y0, 'Strip', 14, t['ink1'], 500))
    dy = y0 + 14 - M.SY
    s1 = (4, 'Percussion', -7.5, 0.28, 'Comp', '10', 0.66)
    s2 = (6, 'Chords', -3.5, 0.0, 'Comp · EQ', '', 0.74)
    out.append(f'<g transform="translate(0 {dy})">{M.strip(t, sx, s1, {})}'
               f'{M.strip(t, sx + 66, s2, {"playing": True, "tracks": {"Chords": "s"}})}</g>')
    out.append(controls(t, x0 + 420, y0))
    return ''.join(out)


def field(t, x, y, w, s, state='rest'):
    out = ''
    if state == 'focus':
        out += rect(x - 2, y - 2, w + 4, 28, mix(t['base'], t['accent'], 0.45), 6)
    out += well(t, x, y, w, 24, 4)
    col = t['ink1'] if state != 'disabled' else t['ink4']
    out += text(x + 8, y + 16, s, 12, col)
    if state == 'focus':
        cx = x + 8 + tw(s, 12) + 2
        out += rect(round(cx), y + 6, 1, 13, t['accent'])
    return out


def scrollbars(t, x, y):
    out = text(x, y + 10, 'Scrollbars', 11, t['ink2'])
    # rest: a thin thumb, no track
    out += rect(x + 80, y + 4, 160, 8, t['base'])
    out += rect(x + 120, y + 6, 70, 4, mix(t['base'], t['ink3'], 0.55), 2)
    out += text(x + 250, y + 12, 'rest', 10, t['ink3'])
    # hover: the track appears, the thumb grows
    out += well(t, x + 80, y + 22, 160, 10, 5)
    out += rect(x + 120, y + 23, 70, 8, mix(t['well'], t['ink2'], 0.6), 4)
    out += text(x + 250, y + 31, 'hover', 10, t['ink3'])
    # vertical pair
    out += rect(x + 300, y - 2, 4, 50, t['base'])
    out += rect(x + 300, y + 4, 4, 22, mix(t['base'], t['ink3'], 0.55), 2)
    out += well(t, x + 312, y - 2, 10, 50, 5)
    out += rect(x + 313, y + 4, 8, 22, mix(t['well'], t['ink2'], 0.6), 4)
    return out


def menu(t, x, y):
    W, rh = 200, 24
    items = [('Cut', 'Ctrl X', ''), ('Copy', 'Ctrl C', ''), ('Paste', 'Ctrl V', ''),
             ('Paste here', '', 'disabled'), ('-', '', ''), ('Quantise', '', 'sub'),
             ('Colour', '', 'sub2'), ('-', '', ''), ('Rename', 'F2', ''), ('Delete', 'Del', '')]
    hh = sum(9 if i[0] == '-' else rh for i in items) + 12
    flt = f'<filter id="mfs-{t["name"]}" x="-20%" y="-20%" width="140%" height="140%"><feDropShadow dx="0" dy="6" stdDeviation="9" flood-color="#000" flood-opacity="{0.45 if t["name"] == "Night" else 0.16}"/></filter>'
    edge = mix(t['raised'], t['ink1'], 0.08) if t['name'] == 'Night' else t['line']

    def box(bx, by, bw, bh):
        return (f'<rect x="{bx}" y="{by}" width="{bw}" height="{bh}" rx="6" fill="{t["raised"]}" filter="url(#mfs-{t["name"]})"/>'
                f'<rect x="{bx + 0.5}" y="{by + 0.5}" width="{bw - 1}" height="{bh - 1}" rx="5.5" fill="none" stroke="{edge}"/>')
    out = flt + box(x, y, W, hh)
    yy = y + 6
    sub_y = None
    for nm, sc, kind in items:
        if nm == '-':
            out += rect(x + 10, yy + 4, W - 20, 1, mix(t['raised'], t['ink4'], 0.45))
            yy += 9
            continue
        if kind == 'sub':
            out += rect(x + 4, yy, W - 8, rh, mix(t['raised'], t['accent'], 0.22), 4)
            sub_y = yy
        col = t['ink4'] if kind == 'disabled' else t['ink1']
        out += text(x + 14, yy + 16, nm, 12, col)
        if sc:
            out += text(x + W - 14, yy + 16, sc, 11, t['ink3'], 400, 'end')
        if kind.startswith('sub'):
            out += G.chevron_right(x + W - 16, yy + 12, 3, t['ink1'] if kind == 'sub' else t['ink3'], 1.2)
        yy += rh
    # the submenu, aligned to its row
    sx, sw = x + W - 4, 150
    subs = [('1/4', ''), ('1/8', ''), ('1/16', 'check'), ('1/32', ''), ('-', ''), ('Triplets', ''), ('Swing…', '')]
    sh = sum(9 if s[0] == '-' else rh for s in subs) + 12
    out += box(sx, sub_y - 6, sw, sh)
    yy = sub_y
    for nm, k in subs:
        if nm == '-':
            out += rect(sx + 10, yy + 4, sw - 20, 1, mix(t['raised'], t['ink4'], 0.45))
            yy += 9
            continue
        if k == 'check':
            out += f'<path d="M{sx + 12} {yy + 12}L{sx + 15.5} {yy + 15.5}L{sx + 21} {yy + 8.5}" fill="none" stroke="{t["accent"]}" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>'
        out += text(sx + 28, yy + 16, nm, 12, t['ink1'], 500 if k == 'check' else 400)
        yy += rh
    return out


def controls(t, x0, y0):
    out = [text(x0, y0, 'Controls', 14, t['ink1'], 500)]
    y = y0 + 24
    # tabs
    out.append(text(x0, y + 12, 'Tabs', 11, t['ink2']))
    tx = x0 + 80
    for k, (lab, st) in enumerate((('Mixer', 'on'), ('Piano roll', 'hover'), ('Channel rack', 'rest'))):
        on = st == 'on'
        col = t['ink1'] if on else (t['ink2'] if st == 'hover' else t['ink3'])
        out.append(text(tx, y + 12, lab, 13 if on else 12, col, 500 if on else 400))
        if on:
            out.append(rect(tx, y + 20, tw(lab, 13, 500), 2, t['ink2']))
        tx += tw(lab, 13 if on else 12) + 20
    y += 40
    out.append(text(x0, y + 16, 'Field', 11, t['ink2']))
    out.append(field(t, x0 + 80, y, 110, 'Glass pad'))
    out.append(field(t, x0 + 204, y, 110, 'Glass pad', 'focus'))
    y += 40
    out.append(text(x0, y + 14, 'Segmented', 11, t['ink2']))
    out.append(segmented(t, x0 + 80, y, ['Compact', 'Wide', 'Large'], [58, 44, 46], 20, 1, 11))
    y += 34
    out.append(text(x0, y + 12, 'Toggle', 11, t['ink2']))
    out.append(toggle(t, x0 + 80, y + 1, False))
    out.append(text(x0 + 112, y + 12, 'Off', 10, t['ink3']))
    out.append(toggle(t, x0 + 150, y + 1, True))
    out.append(text(x0 + 182, y + 12, 'On', 10, t['ink3']))
    out.append(keycap(t, x0 + 250, y + 1, 'Ctrl M'))
    out.append(text(x0 + 274, y + 12, 'a shortcut', 10, t['ink3']))
    y += 34
    out.append(scrollbars(t, x0, y))
    out.append(text(x0 + 420, y0, 'Menu and submenu', 14, t['ink1'], 500))
    out.append(menu(t, x0 + 420, y0 + 18))
    return ''.join(out)
