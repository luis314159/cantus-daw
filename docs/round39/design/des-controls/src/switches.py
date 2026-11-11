"""des-controls — tabs, segmented controls, switches, check boxes, radio buttons, the
strip's text keys and the enable lamp, scrollbars.

Slate's rule holds: colour only for state. A *choice among equals* (a tab, a segment) is
shown by lifting it, neutral; a *thing switched on* (a switch, a check) takes the accent;
solo, arm and play keep their own state colours.
"""
from slate import f, rect, text, mix, tw, frame, well, path, circle, line


def panel_tabs(t, s, x, y, names, active, hover=None, focused=True, closable=True, extra=0):
    """A dock panel's tabs on its head bar (28 px). The active tab is cut from the panel's
    body and merges with it; the focused panel marks it with a 2-px accent top."""
    h = 28 * s
    out = ''
    xx = x + 4 * s
    for nm in names:
        on = nm == active
        hv = nm == hover
        w = tw(nm, 12 * s, 500 if on else 400) + (40 if closable and (on or hv) else 24) * s
        if on:
            out += rect(xx, y + 4 * s, w, h - 4 * s, t['base'], 6 * s if s == 1 else 8)
            out += rect(xx, y + h - 6 * s, w, 6 * s, t['base'])
            if focused:
                out += rect(xx + 4 * s, y + 4 * s, w - 8 * s, 2 * s, t['accent'], 1 * s)
        elif hv:
            out += rect(xx, y + 5 * s, w, h - 9 * s, mix(t['surface'], t['raised'], 0.7), 5 * s)
        col = t['ink1'] if on else (t['ink2'] if hv else t['ink3'])
        out += text(xx + 12 * s, y + h / 2 + 6 * s, nm, 12 * s, col, 500 if on else 400)
        if closable and (on or hv):
            cx, cy = xx + w - 13 * s, y + h / 2 + 2 * s
            k = 3 * s
            out += line(cx - k, cy - k, cx + k, cy + k, t['ink3'], 1.25 * s)
            out += line(cx - k, cy + k, cx + k, cy - k, t['ink3'], 1.25 * s)
        xx += w + 2 * s
    if extra:
        # more tabs than fit: a count and a chevron, one key
        lab = f'+{extra}'
        w = tw(lab, 11 * s) + 24 * s
        out += rect(xx + 2 * s, y + 7 * s, w, h - 12 * s, t['well'], 4 * s)
        out += text(xx + 10 * s, y + h / 2 + 5.5 * s, lab, 11 * s, t['ink2'])
        from fields import chevron
        out += chevron(t, s, xx + w - 7 * s, y + h / 2 + 2 * s, t['ink2'], 'down', True)
    return out


def section_tabs(t, s, x, y, names, active, hover=None):
    """Tabs inside a panel: words; the active one Medium ink 1 over a 2-px ink-1 rule."""
    out = ''
    xx = x
    for nm in names:
        on = nm == active
        col = t['ink1'] if on else (t['ink2'] if nm == hover else t['ink3'])
        w = tw(nm, 12 * s, 500 if on else 400)
        out += text(xx, y + 14 * s, nm, 12 * s, col, 500 if on else 400)
        if on:
            out += rect(xx, y + 22 * s, w, 2 * s, t['ink1'], 1 * s)
        elif nm == hover:
            out += rect(xx, y + 23 * s, w, 1 * s, t['ink4'])
        xx += w + 18 * s
    return out


def segmented(t, s, x, y, labels, sel, h=24, hover=None, disabled=False, widths=None):
    H = h * s
    fs = (12 if h == 24 else 11) * s
    ws = widths or [tw(l, fs, 500) + 22 * s for l in labels]
    W = sum(ws)
    out = well(t, x, y, W, H, 4 * s if s == 1 else 5)
    xx = x
    for i, (lab, w) in enumerate(zip(labels, ws)):
        if i == sel:
            out += rect(xx + 2 * s, y + 2 * s, w - 4 * s, H - 4 * s - 1, t['raised'] if t['name'] == 'Night' else '#ffffff', 3 * s)
            out += rect(xx + 4 * s, y + 2 * s, w - 8 * s, 1, mix(t['raised'], t['ink1'], 0.12) if t['name'] == 'Night' else '#ffffff')
            if t['name'] == 'Paper':
                out += frame(xx + 2 * s, y + 2 * s, w - 4 * s, H - 4 * s - 1, t['edge'], 3 * s)
        elif i == hover:
            out += rect(xx + 2 * s, y + 2 * s, w - 4 * s, H - 4 * s - 1, t['well_hi'], 3 * s)
        elif i > 0 and i - 1 != sel and i != sel:
            out += rect(xx, y + 6 * s, 1, H - 12 * s, mix(t['well'], t['ink4'], 0.6))
        col = t['ink1'] if i == sel else (t['ink2'] if i == hover else t['ink3'])
        if disabled:
            col = t['ink4']
        if isinstance(lab, str):
            out += text(xx + w / 2, y + H / 2 + fs * 0.36, lab, fs, col, 500 if i == sel else 400, 'middle')
        else:
            out += lab(xx + w / 2, y + H / 2, col)
        xx += w
    return out


def switch(t, s, x, y, on, state='rest'):
    w, h = 28 * s, 16 * s
    r = h / 2
    out = ''
    dis = state == 'disabled'
    if on:
        out += rect(x, y, w, h, t['accent'], r)
        if state == 'hover':
            out += rect(x, y, w, h, '#ffffff', r, ' fill-opacity="0.12"')
    else:
        out += well(t, x, y, w, h, r, t['well_hi'] if state == 'hover' else t['well'])
    kr = h / 2 - 2 * s
    kx = x + w - h / 2 if on else x + h / 2
    out += circle(kx, y + h / 2 + 0.75, kr, 'rgba(0,0,0,0.30)')
    kc = ('#f4f9fd' if t['name'] == 'Night' else '#ffffff') if on else (mix(t['ink3'], t['ink2'], 0.5 if state == 'hover' else 0))
    if not on and t['name'] == 'Paper':
        kc = '#ffffff'
    out += circle(kx, y + h / 2, kr, kc)
    if not on and t['name'] == 'Paper':
        out += f'<circle cx="{f(kx)}" cy="{f(y + h / 2)}" r="{f(kr - 0.5)}" fill="none" stroke="{t["edge"]}"/>'
    if state == 'focus':
        out += frame(x - 3 * s, y - 3 * s, w + 6 * s, h + 6 * s, t['accent'], r + 3 * s, 1, ' stroke-opacity="0.85"')
    if dis:
        out = f'<g opacity="0.45">{out}</g>'
    return out


def checkbox(t, s, x, y, val, state='rest'):
    """14 (20) px. val: False, True, 'mixed'."""
    z = 14 * s if s == 1 else 20
    r = 3 * s if s == 1 else 4
    out = ''
    if val:
        out += rect(x, y, z, z, t['accent'], r)
        if state == 'hover':
            out += rect(x, y, z, z, '#ffffff', r, ' fill-opacity="0.14"')
        ink = t['accent_ink']
        if val == 'mixed':
            out += rect(x + z * 0.26, y + z / 2 - (0.75 if s == 1 else 1), z * 0.48, 1.5 if s == 1 else 2, ink, 1)
        else:
            sw = 1.75 if s == 1 else 2.5
            out += path(f'M{f(x + z * 0.24)} {f(y + z * 0.52)}L{f(x + z * 0.43)} {f(y + z * 0.70)}L{f(x + z * 0.77)} {f(y + z * 0.32)}', ink, sw)
    else:
        out += well(t, x, y, z, z, r, t['well_hi'] if state == 'hover' else t['well'])
        if state == 'hover':
            out += frame(x, y, z, z, t['ink3'], r)
    if state == 'focus':
        out += frame(x - 3 * s, y - 3 * s, z + 6 * s, z + 6 * s, t['accent'], r + 3 * s, 1, ' stroke-opacity="0.85"')
    if state == 'disabled':
        out = f'<g opacity="0.45">{out}</g>'
    return out


def radio(t, s, x, y, on, state='rest'):
    z = 14 * s if s == 1 else 20
    cx, cy = x + z / 2, y + z / 2
    out = ''
    if on:
        out += circle(cx, cy, z / 2, t['accent'])
        out += circle(cx, cy, z * 0.2, t['accent_ink'])
    else:
        out += circle(cx, cy, z / 2, t['lip'])
        out += circle(cx, cy - 0.5, z / 2 - 0.5, t['edge'])
        out += circle(cx, cy, z / 2 - 1, t['well_hi'] if state == 'hover' else t['well'])
        if state == 'hover':
            out += f'<circle cx="{f(cx)}" cy="{f(cy)}" r="{f(z / 2 - 0.5)}" fill="none" stroke="{t["ink3"]}"/>'
    if state == 'disabled':
        out = f'<g opacity="0.45">{out}</g>'
    return out


def textkey(t, s, x, y, letter, on=False, kind='mute', h=18, hover=False):
    """The strip's keys: Mute, Solo, Arm. Off: a well, the letter ink 2 SemiBold.
    On: mute greys out to a lit neutral, solo amber, arm red — each with its own ink."""
    H = h * s
    W = round(H * 1.2)
    r = 3 * s if s == 1 else 4
    if on:
        fill = {'mute': t['ink2'] if t['name'] == 'Night' else t['ink3'], 'solo': t['solo'], 'arm': t['rec']}[kind]
        ink = {'mute': t['surface'] if t['name'] == 'Night' else '#ffffff', 'solo': '#1b1404' if t['name'] == 'Night' else '#ffffff',
               'arm': '#ffffff'}[kind]
        out = rect(x, y, W, H, fill, r)
        out += rect(x + 2 * s, y, W - 4 * s, 1, '#ffffff', 0, ' fill-opacity="0.18"')
    else:
        out = well(t, x, y, W, H, r, t['well_hi'] if hover else t['well'])
        ink = t['ink1'] if hover else t['ink2']
    fs = (10.5 if h <= 18 else 12) * s
    out += text(x + W / 2, y + H / 2 + fs * 0.36, letter, fs, ink, 600, 'middle')
    return out, W


def lamp(t, s, cx, cy, on, hover=False):
    """The enable lamp of an insert or a send: 8 (12) px. On, it is lit green with a soft
    halo; off, a hollow ring — the effect is bypassed."""
    r = 4 * s
    if on:
        return (circle(cx, cy, r + 2.5 * s, t['play'], ' fill-opacity="0.18"') + circle(cx, cy, r, t['play'])
                + circle(cx - r * 0.3, cy - r * 0.35, r * 0.35, '#ffffff', ' fill-opacity="0.45"'))
    col = t['ink2'] if hover else t['ink4']
    return f'<circle cx="{f(cx)}" cy="{f(cy)}" r="{f(r - 0.5 * s)}" fill="none" stroke="{col}" stroke-width="{f(1.5 * s if s == 1 else 2)}"/>'
