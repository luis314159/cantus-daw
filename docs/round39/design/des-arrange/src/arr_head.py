"""des-arrange — a track's header, an automation lane's header, the "Add track" row.

At rest a header is a name and a quiet line under it: no wells, the keys are letters in ink 3.
Under the pointer the keys gather into one recessed well and the "more" key appears.
Engaged, a key lights in its state colour (mute neutral, solo amber, arm red, lanes violet).
The user's colour is a 3-px stripe; the selected track's header takes a tint of it.
"""
import aglyphs as AG
from arr_geo import (STRIPE, NUM_R, NAME_X, KEY_W, KEY_H, METER_W, INDENT, width_mode)
from base import rect, text, mix, tw, hue_for, HUES, well


def fit(s, size, w, weight=400):
    """Truncate with an ellipsis to w px (layout estimate)."""
    if tw(s, size, weight) <= w:
        return s
    while s and tw(s + '…', size, weight) > w:
        s = s[:-1]
    return s.rstrip() + '…'


def head_bg(t, tr, sel):
    if sel:
        return mix(t['surface'], HUES[tr['hue']], t['head_sel'] if t['name'] == 'Night' else 0.16)
    if tr.get('depth'):
        return mix(t['surface'], t['base'], 0.30)     # a folder's child sits a step back
    return t['surface']


def keys(t, x, cy, tr, which, hover):
    """The joined keys: which ⊂ 'MSRA'. Returns svg. Engaged keys light; at rest, letters only."""
    out = []
    n = len(which)
    y = cy - KEY_H / 2
    if hover:
        out.append(well(t, x, y, n * KEY_W, KEY_H, 3))
        for k in range(1, n):
            out.append(rect(x + k * KEY_W, y + 3, 1, KEY_H - 6, t['edge']))
    dark = t['base'] if t['name'] == 'Night' else '#ffffff'
    for k, ch in enumerate(which):
        kx = x + k * KEY_W
        mx = kx + KEY_W / 2
        on = {'M': tr.get('mute'), 'S': tr.get('solo'), 'R': tr.get('arm'), 'A': tr.get('lanes_open')}[ch]
        if ch in 'MS':
            if on:
                fill = t['ink2'] if ch == 'M' else t['solo']
                out.append(rect(kx + 2, y + 2, KEY_W - 4, KEY_H - 4, fill, 2))
                out.append(text(mx, cy + 3.5, ch, 10, dark, 600, 'middle'))
            elif ch == 'M' and tr.get('implicit'):
                # silenced by another track's solo: an outline, not a lit key
                out.append(rect(kx + 2.5, y + 2.5, KEY_W - 5, KEY_H - 5, 'none', 2,
                                f' stroke="{mix(t["surface"], t["solo"], 0.7)}" stroke-width="1"'))
                out.append(text(mx, cy + 3.5, ch, 10, mix(t['surface'], t['solo'], 0.85), 600, 'middle'))
            else:
                out.append(text(mx, cy + 3.5, ch, 10, t['ink2'] if hover else t['ink3'], 600, 'middle'))
        elif ch == 'R':
            if on:
                out.append(rect(kx + 2, y + 2, KEY_W - 4, KEY_H - 4, mix(t['well'], t['rec'], 0.24), 2))
                out.append(f'<circle cx="{mx}" cy="{cy}" r="3.8" fill="{t["rec"]}"/>')
            else:
                out.append(f'<circle cx="{mx}" cy="{cy}" r="3.2" fill="{t["ink2"] if hover else mix(t["surface"], t["ink3"], 0.7)}"/>')
        elif ch == 'A':
            if on:
                out.append(rect(kx + 2, y + 2, KEY_W - 4, KEY_H - 4, mix(t['well'], t['auto'], 0.24), 2))
                out.append(AG.auto(mx, cy, t['auto']))
            elif tr.get('auto'):
                out.append(AG.auto(mx, cy, mix(t['surface'], t['auto'], 0.75)))   # has lanes, hidden
            else:
                out.append(AG.auto(mx, cy, t['ink2'] if hover else t['ink4']))
    return ''.join(out)


def meter(t, x, y, h, level=None, armed_in=False):
    out = [rect(x, y, METER_W, h, mix(t['surface'], t['edge'], 0.55), 1)]
    if level:
        lh = h * level
        out.append(rect(x, y + h - lh, METER_W, lh, t['meter_lo'], 1))
        if level > 0.72:
            out.append(rect(x, y + h - lh, METER_W, lh - h * 0.72, t['meter_mid'], 1))
    return ''.join(out)


def kind_glyph(t, tr, cx, cy, ink, ink2):
    k = tr['kind']
    if tr.get('frozen'):
        return AG.snow(cx, cy, t['accent'], 4)
    if tr.get('arm') and k == 'audio':
        return AG.mic(cx, cy, ink, ink2)
    if k == 'folder':
        return AG.folder(cx, cy, ink, ink2, tr.get('open', True))
    if k == 'drums':
        return AG.pads_kind(cx, cy, ink, ink2)
    if k in ('audio', 'audio2'):
        return AG.wave_kind(cx, cy, ink, ink2)
    return AG.keys_kind(cx, cy, ink, ink2)


def header(t, x, y, w, h, tr, num, hover=False, level=None, sel_override=None, folder_span=None):
    """One track's header in the box (x, y, w, h)."""
    sel = tr.get('sel') if sel_override is None else sel_override
    mode = width_mode(w)
    hue = HUES[tr['hue']]
    depth = tr.get('depth', 0)
    ind = depth * INDENT
    muted = tr.get('mute')
    dimmed = muted or tr.get('implicit')
    bg = head_bg(t, tr, sel)
    if hover and not sel:
        bg = mix(bg, t['ink1'], 0.035)
    out = [rect(x, y, w, h - 1, bg), rect(x, y + h - 1, w, 1, mix(t['surface'], t['gutter'], 0.75))]
    # the stripe: the user's colour, quiet at rest, whole when selected, grey when muted
    if depth and tr.get('parent'):
        out.append(rect(x, y, STRIPE, h, hue_for(t, tr.get('parent_hue', tr['parent']), t['stripe_rest'] * 0.8)))
        sc = HUES[tr['hue']] if sel else hue_for(t, tr['hue'], t['stripe_rest'])
        if muted:
            sc = mix(bg, t['ink4'], 0.8)
        out.append(rect(x + ind - 2, y + 4, 2, h - 9, sc, 1))
    else:
        sc = hue if sel else hue_for(t, tr['hue'], t['stripe_rest'])
        if muted:
            sc = mix(bg, t['ink4'], 0.8)
        out.append(rect(x, y, STRIPE, h - (0 if tr['kind'] == 'folder' and tr.get('open', True) else 1), sc))
    if tr.get('arm'):
        out.append(rect(x + STRIPE, y, 1, h - 1, mix(bg, t['rec'], 0.7)))
    one_line = h < 40
    base1 = y + (h / 2 + 4 if one_line else 18)
    cy1 = base1 - 4
    name_ink = t['ink3'] if dimmed else t['ink1']
    # keys at the right
    if mode == 'narrow':
        which = 'MS'
    elif tr['kind'] == 'folder':
        which = 'MS'
    else:
        which = 'MSRA'
    kx = x + w - 8 - METER_W - 4 - len(which) * KEY_W
    nx = x + NAME_X + ind
    # the number (a folder shows its chevron in the number's place)
    if tr['kind'] == 'folder':
        import glyphs as G
        ch = G.chevron_down if tr.get('open', True) else G.chevron_right
        out.append(ch(x + NUM_R - 6, cy1, 3 if tr.get('open', True) else 3, t['ink2'], 1.3))
    elif mode != 'narrow' or True:
        out.append(text(x + NUM_R + ind, base1, str(num), 10, t['ink2'] if sel else t['ink3'], 400, 'end'))
    name_w = kx - nx - (26 if hover else 8)
    weight = 500 if (sel or tr['kind'] == 'folder') else 400
    out.append(text(nx, base1, fit(tr['name'], 12, name_w, weight), 12, name_ink, weight))
    if hover:
        out.append(AG.more(kx - 13, cy1, t['ink2']))
    out.append(keys(t, kx, cy1, tr, which, hover))
    out.append(meter(t, x + w - 4 - METER_W, y + 5, h - 11, level))
    if one_line:
        return ''.join(out)
    # line 2: the kind and the instrument (or the input, or "Frozen")
    base2 = y + 33
    cy2 = base2 - 4
    i2, i3 = (t['ink2'], t['ink3']) if not dimmed else (t['ink3'], t['ink4'])
    out.append(kind_glyph(t, tr, nx + 5, cy2, i2, i3))
    lx = nx + 15
    if tr.get('frozen'):
        lw = tw('Frozen', 10, 600) + 10
        out.append(rect(lx - 1, cy2 - 6, lw, 13, mix(bg, t['accent'], 0.18), 3))
        out.append(text(lx + 4, base2 - 0.5, 'Frozen', 10, t['accent'], 600))
        lx += lw + 6
        out.append(text(lx, base2, fit(tr['inst'].split('·')[-1].strip(), 11, x + w - 12 - lx), 11, i3))
    elif tr.get('arm') and tr.get('input'):
        out.append(text(lx, base2, tr['input'], 11, i2))
        mx_ = lx + tw(tr['input'], 11) + 8
        mw = min(48, x + w - 14 - mx_)
        if mw > 16:
            out.append(rect(mx_, cy2 - 1, mw, 3, mix(bg, t['edge'], 0.55), 1.5))
            out.append(rect(mx_, cy2 - 1, mw * 0.46, 3, t['meter_lo'], 1.5))
    else:
        room = x + w - 12 - METER_W - lx
        if mode == 'wide' and h < 60:     # the wide header gives the level its number on line 2
            out.append(text(x + w - 12 - METER_W, base2, tr.get('db', '−3.5 dB'), 11, i3, 400, 'end'))
            room -= 56
        out.append(text(lx, base2, fit(tr['inst'], 11, room), 11, i3 if not sel else t['ink2']))
    # line 3 (64 and taller): the level, as a slider with its number; pan in the wide header
    if h >= 60:
        cy3 = y + 50
        sx = nx
        vw = 52 if mode != 'narrow' else 40
        sw = x + w - 14 - METER_W - vw - sx - (44 if mode == 'wide' else 0)
        frac = tr.get('vol', 0.72)
        out.append(rect(sx, cy3 - 1, sw, 2, mix(bg, t['edge'], 0.6), 1))
        out.append(rect(sx, cy3 - 1, sw * frac, 2, mix(bg, t['ink3'], 0.75), 1))
        out.append(f'<circle cx="{sx + sw * frac:.1f}" cy="{cy3}" r="4" fill="{t["raised"] if t["name"] == "Night" else "#ffffff"}" '
                   f'stroke="{t["edge"]}" stroke-width="1"/>')
        out.append(text(x + w - 12 - METER_W - (44 if mode == 'wide' else 0), cy3 + 4, tr.get('db', '−3.5 dB'), 11, i2, 400, 'end'))
        if mode == 'wide':
            px = x + w - 12 - METER_W - 22
            out.append(f'<circle cx="{px}" cy="{cy3}" r="6.5" fill="url(#kcap-{t["name"]})" stroke="{t["edge"]}" stroke-width="1"/>')
            out.append(f'<path d="M{px} {cy3 - 2}V{cy3 - 5.5}" stroke="{t["ink1"]}" stroke-width="1.4" stroke-linecap="round"/>')
            out.append(text(px + 12, cy3 + 4, 'C', 10, i3))
    # line 4 (88 and taller): the inserts, in words
    if h >= 88 and tr.get('fx'):
        out.append(text(nx, y + 74, ' · '.join(tr['fx']), 10, i3))
    return ''.join(out)


def lane_header(t, x, y, w, h, tr, ln, hover=False):
    """An automation lane under its track: the parameter, its owner, the value, the mode."""
    bg = mix(t['surface'], t['base'], 0.45)
    out = [rect(x, y, w, h - 1, bg), rect(x, y + h - 1, w, 1, mix(t['surface'], t['gutter'], 0.75))]
    out.append(rect(x, y, STRIPE, h, hue_for(t, tr['hue'], t['stripe_rest'] * (1.25 if tr.get('sel') else 0.8))))
    ind = tr.get('depth', 0) * INDENT
    gx = x + NAME_X + ind - 10
    out.append(rect(gx, y, 1, h - 6, mix(bg, t['auto'], 0.35)))          # the lane hangs from its track
    out.append(rect(gx, y + h - 7, 6, 1, mix(bg, t['auto'], 0.35)))
    cy = y + (h / 2 if h < 40 else 14)
    nx = x + NAME_X + ind
    out.append(AG.auto(nx + 5, cy, t['auto']))
    out.append(text(nx + 15, cy + 4, ln['param'], 11, t['ink1'], 500))
    ox = nx + 15 + tw(ln['param'], 11, 500) + 5
    mode_w = 34
    rx = x + w - 8 - METER_W - 4
    vx = rx - mode_w - 8
    if width_mode(w) != 'narrow':
        out.append(text(ox, cy + 4, fit(ln['owner'], 10, vx - ox - tw(ln['value'], 11) - 10 - (18 if hover else 0)), 10, t['ink3']))
    out.append(text(vx, cy + 4, ln['value'], 11, t['ink2'], 400, 'end'))
    mcol = {'Read': t['ink2'], 'Touch': t['auto'], 'Latch': t['auto'], 'Write': t['rec']}[ln['mode']]
    out.append(well(t, rx - mode_w, cy - 7, mode_w, 14, 3))
    out.append(text(rx - mode_w / 2, cy + 3.5, ln['mode'], 9, mcol, 600, 'middle'))
    if hover:
        out.append(AG.close(rx - mode_w - 8 - tw(ln['value'], 11) - 12, cy, t['ink2']))
    return ''.join(out)


def add_row(t, x, y, w, h=28, hover=False):
    out = [rect(x, y, w, h, t['surface'] if hover else mix(t['surface'], t['base'], 0.55))]
    import glyphs as G
    if hover:
        out.append(well(t, x + 8, y + 5, w - 16, h - 10, 4, hi=True))
    out.append(G.plus(x + NAME_X - 10, y + h / 2, 4, t['ink2'] if hover else t['ink3'], 1.2))
    out.append(text(x + NAME_X, y + h / 2 + 4, 'Add track', 11, t['ink1'] if hover else t['ink3']))
    if hover:
        out.append(text(x + w - 16, y + h / 2 + 3.5, 'Ctrl T', 9, t['ink3'], 500, 'end'))
    return ''.join(out)
