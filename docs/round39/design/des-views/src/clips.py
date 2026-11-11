"""des-views (round 39, Slate) — the arrangement's clip content in detail: MIDI (a melody,
chords, drums), audio (mono and stereo) and automation, at four zooms, plus the states of a
clip. What each zoom draws is a rule the kit can follow (README, "Clip content")."""
import math
import song_full as S
from base import rect, text, mix, tw, hue_for, note_ink, HUES, lit_edge

W, H = 1280, 1350
LX = 176                     # the lanes' label column
X0, X1 = LX + 8, 1280 - 16   # the time area

ZOOMS = [
    # title, rule, first bar, px a bar
    ('Far', 'the whole song in view · 17 px a bar · MIDI as its pitch contour, drums as their pulse, audio as loudness, automation as a line', 1, 17),
    ('Default', '44 px a bar (today’s arrangement) · notes in rows ranked by pitch, drums one row a piece, audio as its peak envelope, automation with its points', 37, 44),
    ('Near', '120 px a bar · notes at their true length, audio as peak and loudness in two tones, automation points as handles', 40, 120),
    ('Close', '360 px a bar · every hit and attack, audio in detail with its two channels apart, automation with curve handles and values', 41, 360),
]


def band(ppb):
    return 'far' if ppb < 6 else 'default' if ppb < 20 else 'near' if ppb < 60 else 'close'


def auto_ink(t):
    return t['auto']


def ruler(t, y, first, ppbar, title, rule):
    out = [text(16, y + 14, title, 13, t['ink1'], 500), text(16 + tw(title, 13, 500) + 10, y + 14, rule, 11, t['ink3'])]
    y += 22
    out.append(rect(0, y, W, 20, t['surface']))
    out.append(f'<clipPath id="r{first}{ppbar}"><rect x="{X0}" y="{y}" width="{X1 - X0}" height="20"/></clipPath><g clip-path="url(#r{first}{ppbar})">')
    bar = first
    while X0 + (bar - first) * ppbar < X1:
        x = round(X0 + (bar - first) * ppbar)
        every = 4 if ppbar < 30 else 1
        sec = next((nm for s, nm in S.SECTIONS if s == bar), None)
        if (bar - 1) % every == 0:
            out.append(rect(x, y + 10, 1, 10, t['ink3']))
            out.append(text(x + 4, y + 18, str(bar), 10, t['ink1'] if sec else t['ink2'], 600 if sec else 400))
        if sec:
            out.append(rect(x, y + 1, 1, 9, t['ink2']))
            out.append(text(x + 4, y + 9, sec, 9, t['ink2'], 600))
        if ppbar >= 100:
            for q in range(1, 4):
                xq = round(x + q * ppbar / 4)
                out.append(rect(xq, y + 14, 1, 6, t['ink4']))
                out.append(text(xq + 3, y + 18, f'{bar}.{q + 1}', 9, t['ink3']))
            if ppbar >= 300:
                for s in range(16):
                    if s % 4:
                        out.append(rect(round(x + s * ppbar / 16), y + 17, 1, 3, mix(t['surface'], t['ink4'], 0.6)))
        bar += 1
    out.append('</g>')
    return ''.join(out), y + 20


def lane_bg(t, y, h, first, ppbar):
    out = [rect(X0, y, X1 - X0, h, t['base'])]
    bar = first
    while X0 + (bar - first) * ppbar < X1:
        x = round(X0 + (bar - first) * ppbar)
        if ((bar - 1) // 4) % 2 == 1:
            out.append(rect(x, y, math.ceil(ppbar), h, t['lane2']))
        if ppbar >= 30:
            for q in range(1, 4):
                out.append(rect(round(x + q * ppbar / 4), y, 1, h, t['grid_beat']))
        if ppbar >= 12 or (bar - 1) % 4 == 0:
            out.append(rect(x, y, 1, h, t['grid_bar']))
        bar += 1
    out.append(rect(X0, y + h - 1, X1 - X0, 1, t['grid_bar']))
    return ''.join(out)


def label_col(t, y, h, name, hue, kind):
    sub = {'melody': 'MIDI · a melody', 'chords': 'MIDI · chords', 'drums': 'MIDI · a drum kit',
           'audio': 'Audio · mono', 'audio2': 'Audio · stereo', 'auto': 'Automation · Chords cutoff'}[kind]
    out = [rect(0, y, LX, h - 1, t['surface']), rect(0, y, 3, h - 1, hue_for(t, hue, t['stripe_rest']))]
    out.append(text(14, y + min(h / 2, 17) + (0 if h > 34 else 4), name, 12, t['ink1']))
    if h > 34:
        out.append(text(14, y + 31, sub, 10, t['ink3']))
    else:
        out.append(text(LX - 10, y + h / 2 + 4, sub, 10, t['ink3'], 400, 'end'))
    out.append(text(LX - 10, y + h - 6 if h > 34 else y + 12, f'{h} px' if h > 34 else '', 9, t['ink4'], 400, 'end'))
    return ''.join(out)


_UID = [0]


def section(t, y, title, rule, first, ppbar):
    import clipdraw as D
    r, y = ruler(t, y, first, ppbar, title, rule)
    out = [r]
    ppb = ppbar / 4
    bd = band(ppb)
    for name, hue, kind, h, clips in S.LANES:
        out.append(label_col(t, y, h, name, hue, kind))
        out.append(lane_bg(t, y, h, first, ppbar))
        out.append(f'<clipPath id="ln{_UID[0]}"><rect x="{X0}" y="{y}" width="{X1 - X0}" height="{h}"/></clipPath><g clip-path="url(#ln{_UID[0]})">')
        _UID[0] += 1
        for c in clips:
            x = X0 + (c[0] - first) * ppbar
            w = c[1] * ppbar - 1
            if x > X1 or x + w < X0:
                continue
            st = 'sel' if (name == 'Chords' and c[0] == 41 and bd == 'default') else ''
            _UID[0] += 1
            out.append(D.clip(t, (name, hue, kind), c, round(x), y + 1, round(w), h - 3, ppb, bd, st, uid=f'c{_UID[0]}'))
        out.append('</g>')
        y += h
    return ''.join(out), y


def states(t, y):
    import clipdraw as D
    out = [text(16, y + 14, 'States', 13, t['ink1'], 500),
           text(16 + tw('States', 13, 500) + 10, y + 14, 'one clip at 44 px a bar, 30-px lane — the colour says the state; nothing else moves', 11, t['ink3'])]
    y += 28
    caps = [('At rest', 'a tint and its notes', ''), ('Selected', 'lit, an edge, its name', 'sel'),
            ('Playing', 'under the playhead: a step up', 'play'), ('Muted', 'grey notes, a fine hatch', 'mute'),
            ('Linked', 'a linked copy: the mark by its name', 'link')]
    x = X0
    for k, (a, b, st) in enumerate(caps):
        out.append(text(x, y + 12, a, 12, t['ink1'], 500))
        out.append(text(x + tw(a, 12, 500) + 6, y + 12, b, 10, t['ink3']))
        out.append(rect(x, y + 20, 176, 30, t['base']))
        for q in range(5):
            out.append(rect(x + q * 44, y + 20, 1, 30, t['grid_bar']))
        _UID[0] += 1
        s2 = '' if st == 'link' else st
        out.append(D.clip(t, ('Chords', 'Chords', 'chords'), (41, 4, 'Stabs'), x, y + 21, 175, 27, 11, 'default', s2, uid=f's{_UID[0]}'))
        if st == 'link':
            # the link: two rings, drawn at 10 px, beside a name tag
            lx, ly = x + 6, y + 23
            out.append(rect(lx - 2, ly, 52, 12, mix(hue_for(t, 'Chords', t['tint_rest']), t['base'], 0.45), 2))
            for dx in (0, 5):
                out.append(f'<rect x="{lx + 1 + dx}" y="{ly + 3.5}" width="7" height="5" rx="2.5" fill="none" stroke="{t["ink2"]}" stroke-width="1.1"/>')
            out.append(text(lx + 16, ly + 9, 'Stabs', 10, mix(HUES['Chords'], t['ink1'], 0.55) if t['name'] == 'Night' else t['ink1'], 500))
        if st == 'play':
            out.append(rect(x + 96, y + 20, 1, 30, t['accent']))
        x += 176 + 26
    return ''.join(out), y + 56


def sheet(t):
    out = [rect(0, 0, W, H, t['gutter']), rect(0, 0, W, 56, t['surface']), lit_edge(t, 0, 0, W)]
    out.append(text(16, 26, f'Clip content — Slate {t["name"]}', 18, t['ink1'], 500))
    out.append(text(16, 45, 'Neon Tide, the arrangement at four zooms: a melody, chords, a drum kit, mono and stereo audio, '
                    'automation. Lanes of 30, 44 and 60 px. Drawn at 1:1 (logical px).', 11, t['ink3']))
    y = 68
    for title, rule, first, ppbar in ZOOMS:
        s, y = section(t, y, title, rule, first, ppbar)
        out.append(s)
        y += 16
    s, y = states(t, y)
    out.append(s)
    return ''.join(out)
