"""des-controls — the field sheet: numeric fields, text and search, a position field,
drop-downs (closed, hovered, open with their list, disabled, quiet)."""
from slate import Sheet, text, rect, mix
from fields import numfield, textfield, dropdown, timefield
from menus import menu

W, H = 1280, 890
STATES = [('Rest', 'rest'), ('Hover', 'hover'), ('Scrubbing', 'scrub'), ('Typing', 'edit'),
          ('Invalid', 'invalid'), ('Disabled', 'disabled')]


def build(t, s):
    sh = Sheet(t, s, W, H, 'Slate — fields and drop-downs',
               f'des-controls, round 39 · {t["name"]} · {int(s * 100)} % · 24 (36) px and compact 20 (30) px · drawn for this scale')
    P = sh.P
    sh.head(24, 84, 'Numeric fields', 'drag up/down to scrub (Shift fine) · double-click or Enter to type · Esc restores')
    for row, (size, nm) in enumerate(((24, 'Regular, 24 px'), (20, 'Compact, 20 px'))):
        y = 128 + row * 74
        sh.label(24, y + 14, nm, col=t['ink1'], size=12, weight=500)
        sh.label(24, y + 28, 'dialogs, Settings' if size == 24 else 'toolbars, inspector', size=10)
        for i, (lab, st) in enumerate(STATES):
            x = 160 + i * 182
            if row == 0:
                sh.label(x, 114, lab, col=t['ink2'])
            val = {'scrub': '131.50', 'edit': '128.00', 'invalid': '1280'}.get(st, '128.00')
            msg = 'Between 20 and 999 BPM' if st == 'invalid' and size == 24 else ''
            sh.add(numfield(t, s, P(x), P(y), P(120), val, 'BPM', st, size, msg))
    # text, search, position
    y = 296
    sh.head(24, y, 'Text, search and position')
    cases = [('Placeholder', dict(val='', placeholder='Track name')), ('Hover', dict(val='Glass pad', state='hover')),
             ('Typing, a word selected', dict(val='Glass pad', state='edit'))]
    for i, (lab, kw) in enumerate(cases):
        x = 24 + i * 200
        sh.label(x, y + 30, lab, col=t['ink2'])
        sh.add(textfield(t, s, P(x), P(y + 40), P(170), **kw))
    for i, (lab, kw) in enumerate((('Search, empty', dict(val='', placeholder='Search sounds')),
                                   ('Search, typed', dict(val='kick 808', state='edit')))):
        x = 624 + i * 200
        sh.label(x, y + 30, lab, col=t['ink2'])
        sh.add(textfield(t, s, P(x), P(y + 40), P(170), search=True, **kw))
    y2 = y + 92
    for i, (lab, st, hot) in enumerate((('Position, rest', 'rest', None), ('Hover: the beats', 'hover', 1),
                                        ('Typing the beats', 'edit', 1))):
        x = 24 + i * 200
        sh.label(x, y2, lab, col=t['ink2'])
        svg, _ = timefield(t, s, P(x), P(y2 + 10), ['41', '1', '000'], st, hot)
        sh.add(svg)
    sh.label(624, y2 + 4, 'Each segment of a position is its own drag target: bars, beats and ticks scrub', size=11)
    sh.label(624, y2 + 19, 'separately; Tab moves to the next. Bars in ink 1, the rest in ink 2, dots in ink 3.', size=11)

    # drop-downs
    y = 500
    sh.head(24, y, 'Drop-downs', 'the chevron has its own 18 (27) px zone; open, it points up and the list hangs under')
    dd = [('Rest', 'rest'), ('Hover', 'hover'), ('Open', 'open'), ('Disabled', 'disabled')]
    for i, (lab, st) in enumerate(dd):
        x = 24 + i * 200
        sh.label(x, y + 30, lab, col=t['ink2'])
        sh.add(dropdown(t, s, P(x), P(y + 40), P(170), 'Speakers (Realtek)', st))
        if st == 'open':
            items = [dict(label='Speakers (Realtek)', check='check'), dict(label='Headphones (USB)'),
                     dict(label='Focusrite USB ASIO'), dict(k='sep'), dict(label='Refresh the list', short='F5')]
            svg, _ = menu(t, s, P(x), P(y + 68), P(170), items, hot='Headphones (USB)')
            sh.add(svg)
    x = 824
    sh.label(x, y + 30, 'Quiet (a heading), rest · hover', col=t['ink2'])
    sh.add(dropdown(t, s, P(x), P(y + 40), P(120), 'Lyra', 'rest', quiet=True))
    sh.add(dropdown(t, s, P(x + 130), P(y + 40), P(120), 'Lyra', 'hover', quiet=True))
    sh.label(x, y + 92, 'With its label inside, compact', col=t['ink2'])
    sh.add(dropdown(t, s, P(x), P(y + 102), P(180), 'Stereo 1-2', 'rest', size=20, label='Out'))
    spec(sh, t, s, 24, 724)
    return sh


def spec(sh, t, s, x, y):
    P = sh.P
    notes = [('well', 'Slate\'s: 1-px edge on top, 1-px lip under, corners 4 (5); compact 3 (4)'),
             ('number', 'Regular 12 (18) ink 1, right; unit Regular 10 (15) ink 3, 3 (4.5) px after; padding 8 (12)'),
             ('hover', 'the well lifts to well-hi; spin chevrons 5 × 3 (7 × 4) appear in a 12 (18) px column'),
             ('scrubbing', 'the number turns accent; a 2 (3) px accent rule along the bottom'),
             ('typing', 'the well darkens to edge (Paper: white); a 2 (3) px accent ring at 55 %, 2 (3) px out; caret 1 (2) px accent'),
             ('invalid', 'the ring and the text rec; under it, an icon and the allowed range in rec, Regular 10.5'),
             ('drop-down', 'value Regular 12 left; chevron 7 × 4, 1.25 px (10 × 5.5, 1.75), ink 2 → ink 1 on hover')]
    for k, (a, b) in enumerate(notes):
        yy = y + k * 22
        sh.add(text(P(x), P(yy), a, 12 * s, t['ink1'], 500))
        sh.add(text(P(x + 80), P(yy), b, 11 * s, t['ink3']))
