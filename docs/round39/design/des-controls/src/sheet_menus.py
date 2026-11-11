"""des-controls — the menu sheet: a menu-bar menu with submenus, two context menus, an
overflowing menu with its filter."""
from slate import Sheet, text, rect, mix, tw, popover, frame
from menus import menu, menubar, height, ROW, HEAD
from fields import textfield
from knobs import knob

W, H = 1280, 880
HUES = ['#e8794f', '#e6a54a', '#d2c25c', '#5fc584', '#3fc0c8', '#6c93ea',
        '#9a80ee', '#e06aa8', '#e0606f', '#9aa6b2', '#86a9c8', '#93b5a3']


def build(t, s):
    sh = Sheet(t, s, W, H, 'Slate — menus and context menus',
               f'des-controls, round 39 · {t["name"]} · {int(s * 100)} % · rows 24 (36) px · drawn for this scale')
    P = sh.P
    # 1 — the menu bar with Edit open, Quantise's submenu open
    sh.head(24, 84, 'A menu with submenus', 'checks, radio choices, shortcuts, a disabled row')
    sh.add(rect(P(24), P(98), P(520), P(28), t['surface']))
    bar, pos = menubar(t, s, P(30), P(101), ['File', 'Edit', 'Add', 'View', 'Tools', 'Help'], 'Edit')
    sh.add(bar)
    edit = [dict(label='Undo', short='Ctrl+Z', note='Move clip'), dict(label='Redo', short='Ctrl+Y', dis=True),
            dict(k='sep'),
            dict(label='Cut', short='Ctrl+X'), dict(label='Copy', short='Ctrl+C'), dict(label='Paste', short='Ctrl+V'),
            dict(label='Duplicate', short='Ctrl+D'), dict(label='Delete', short='Del'),
            dict(k='sep'), dict(label='Select all', short='Ctrl+A'),
            dict(k='head', label='Grid'),
            dict(label='Quantise', sub=True), dict(label='Snap to grid', check='check', short='Ctrl+G'),
            dict(label='Snap to clips', check='check'), dict(label='Show triplet lines')]
    mx, my = pos['Edit'], P(126)
    svg, rows = menu(t, s, mx, my, P(236), edit, hot='Quantise')
    sh.add(svg)
    sub = [dict(k='head', label='Grid value'),
           dict(label='1/4', check='radio-off'), dict(label='1/8', check='radio-off'),
           dict(label='1/16', check='radio', bold=True), dict(label='1/32', check='radio-off'),
           dict(k='sep'), dict(label='Triplets', check='check', short='T'), dict(label='Swing…', short='Ctrl+Alt+Q')]
    svg, _ = menu(t, s, mx + P(236) - P(4), rows['Quantise'] - 5 * s - HEAD[s], P(176), sub, hot='1/16')
    sh.add(svg)

    # 2 — the context menu of a clip, its colour submenu
    cx0 = 570
    sh.head(cx0, 84, 'Context menu: a clip', 'the object\'s name heads it')
    clip_items = [dict(k='head', label='Clip · Lead 2', swatch=HUES[7]),
                  dict(label='Open in piano roll', short='Enter'), dict(label='Rename', short='F2'),
                  dict(label='Colour', sub=True), dict(k='sep'),
                  dict(label='Mute clip', short='Ctrl+M'), dict(label='Split at playhead', short='S'),
                  dict(label='Make unique'), dict(label='Glue to next', dis=True),
                  dict(k='sep'), dict(label='Delete', short='Del', danger=True)]
    # the clip it was opened on
    sh.add(rect(P(cx0), P(102), P(150), P(26), mix(t['base'], HUES[7], 0.30), 3 * s))
    sh.add(rect(P(cx0), P(102), P(150), P(3), mix(t['base'], HUES[7], 0.8), 0))
    sh.add(text(P(cx0 + 6), P(119), 'Lead 2', 11 * s, t['ink1']))
    svg, rows = menu(t, s, P(cx0 + 60), P(124), P(204), clip_items, hot='Colour')
    sh.add(svg)
    sh.add(swatches(t, s, P(cx0 + 60 + 204 - 4), rows['Colour'] - P(6)))

    # 3 — the context menu of an automated knob
    kx = 1000
    sh.head(kx, 84, 'Context menu: a knob')
    sh.add(knob(t, s, 'M', P(kx + 20), P(124), 0.5, drive='auto'))
    sh.add(text(P(kx + 46), P(122), 'Cutoff', 11 * s, t['ink2']))
    sh.add(text(P(kx + 46), P(136), '1.85 kHz', 12 * s, t['auto']))
    kitems = [dict(k='head', label='Cutoff · Lead'),
              dict(label='Edit its automation clip'), dict(label='Clear automation'),
              dict(k='sep'), dict(label='Learn a MIDI controller…'), dict(label='Modulate', sub=True),
              dict(k='sep'), dict(label='Type a value…', short='Enter'), dict(label='Reset to default', short='Double-click'),
              dict(label='Copy value', short='Ctrl+C'), dict(label='Paste value', short='Ctrl+V', dis=True)]
    svg, _ = menu(t, s, P(kx + 10), P(150), P(246), kitems, hot='Clear automation')
    sh.add(svg)

    # 4 — an overflowing menu: a filter on top, a fade and a chevron at the bottom
    ox, oy = 24, 572
    sh.head(ox, oy, 'An overflowing menu', 'a filter on top; a fade, a chevron, a thumb')
    presets = [dict(k='head', label='Presets · Lyra')] + [dict(label=n, check='radio' if n == 'Glass pad' else None)
                                                          for n in ('Init', 'Airy bells', 'Analog brass', 'Bright pluck', 'Deep sub',
                                                                    'Glass pad', 'Hollow lead', 'Nightfall', 'Organ drawbars',
                                                                    'Pulse arp', 'Soft keys', 'Warm strings', 'Wide saw')]
    px, py = P(ox), P(oy + 18)
    pw = P(220)
    fh = P(40)
    sh.add(popover(t, px, py, pw, fh + P(200), 6 if s == 1 else 8))
    sh.add(textfield(t, s, px + P(8), py + P(8), pw - P(16), '', 'edit', 'Filter presets', search=True))
    svg, _ = menu(t, s, px, py + fh, pw, presets, hot='Glass pad', scroll=True, max_h=P(200))
    sh.add(svg)

    spec(sh, t, s, 560, 572)
    return sh


def swatches(t, s, x, y):
    """The colour submenu: the user's colours as 18-px tiles, six to a row, the chosen ringed."""
    tile, gap, pad = 18 * s, 6 * s, 10 * s
    w = pad * 2 + 6 * tile + 5 * gap
    h = pad * 2 + 2 * tile + gap + 34 * s
    out = popover(t, x, y, w, h, 6 if s == 1 else 8)
    for i, c in enumerate(HUES):
        cx = x + pad + (i % 6) * (tile + gap)
        cy = y + pad + (i // 6) * (tile + gap)
        out += rect(cx, cy, tile, tile, c, 3 * s)
        if i == 7:
            out += frame(cx - 3 * s, cy - 3 * s, tile + 6 * s, tile + 6 * s, t['ink1'], 5 * s, 1.5 * s if s == 1 else 2)
    yy = y + pad + 2 * tile + gap + 10 * s
    out += rect(x + 10 * s, yy, w - 20 * s, 1, mix(t['raised'], t['ink4'], 0.5))
    out += text(x + pad, yy + 18 * s, "Use the track's colour", 12 * s, t['ink1'])
    return out


def spec(sh, t, s, x, y):
    P = sh.P
    sh.head(x + 0, y, 'Measured')
    notes = [('row 24 (36) px', 'label Regular 12 ink 1 at 26 (39) px; shortcut Regular 11 ink 3, 12 (18) px from the right'),
             ('heading 24 (35) px', 'SemiBold 10.5 ink 3, Title case, no bar; it may carry the object\'s colour'),
             ('separator 9 (13) px', 'a 1-px line, raised → ink 4 at 50 %, inset 10 (15) px'),
             ('hover', 'an inset pill: raised → accent 24 % (Paper 13 %), corners 4 (5); a destructive row: rec 20 % and its label rec'),
             ('check · radio', 'tick 9 × 6 (13 × 9), 1.6 (2.2) px accent · dot r 3 (4.5) accent; off: a 1-px ring'),
             ('submenu', 'opens beside its row, 4 (6) px overlap, its first row level with the parent\'s; chevron 4 × 7'),
             ('sheet', 'raised, corners 6 (8), 1-px edge, shadow 0 6 18 at 50 % (Paper 18 %); 5 (7.5) px top and bottom'),
             ('disabled', 'label and shortcut ink 4; it still shows, so the user learns the shortcut')]
    for k, (a, b) in enumerate(notes):
        yy = y + 28 + k * 34
        sh.add(text(P(x), P(yy), a, 12 * s, t['ink1'], 500))
        sh.add(text(P(x), P(yy + 15), b, 11 * s, t['ink3']))
