"""Slate (dir-a), phase B — the build sheets: the top bar at every width it folds through,
each group in its states with its measures, the overflow panel at each fold, the
arrangement's toolbar at each width.

python -I sheets_b.py  →  ../build/b-*.html
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import glyphs as G  # noqa: E402
import topbar2 as TB  # noqa: E402
from base import NIGHT, PAPER, defs, rect, text, mix, tw, page  # noqa: E402

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'build')
WIDTHS = [1920, 1600, 1440, 1280, 1152, 1024, 900, 800]
IDLE = dict(loop=True, metronome=True, snap=True, pos=('41', '1', '000'), time='1:17.42', armed=0)


def svg_doc(body, W, H, t):
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="0 0 {W} {H}">'
            f'{defs(t)}{body}</svg>')


def title(t, x, y, s, sub):
    return text(x, y, s, 20, t['ink1'], 500) + text(x, y + 18, sub, 11, t['ink3'])


def widths_sheet(t):
    L = 24
    W = L + 1920 + 24
    out = [rect(0, 0, W, 10, t['base'])]
    out.append(title(t, L, 34, f'Top bar — every width it folds through · {t["name"]}',
                     'logical px (1× = physical); groups fold in a fixed order; the badge on ••• counts what is folded'))
    y = 78
    rows = []
    for Wd in WIDTHS:
        svg, forms, _ = TB.bar(t, Wd, IDLE)
        folded = [TB.NAMES[g] for g in TB.LEFT + TB.RIGHT if forms[g] is None]
        compact = [f'{TB.NAMES[g]} ({forms[g]})' for g in TB.LEFT + TB.RIGHT if forms[g] not in ('full', None)]
        out.append(text(L, y + 12, f'{Wd} px', 13, t['ink1'], 500))
        line = ('Folded: ' + ', '.join(folded)) if folded else 'Everything on the bar'
        out.append(text(L + 70, y + 12, line, 11, t['ink2']))
        if compact:
            out.append(text(L + 70 + tw(line, 11) + 16, y + 12, 'Compact: ' + ', '.join(compact), 11, t['ink3']))
        out.append(f'<g transform="translate({L} {y + 20})">{svg}</g>')
        out.append(rect(L + Wd, y + 20, 1, 44, t['accent']))
        y += 20 + 44 + 26
        rows.append((Wd, folded))
    H = y + 10
    out[0] = rect(0, 0, W, H, t['base'])
    return svg_doc(''.join(out), W, H, t), W, H


GROUPS = [
    ('brand', 'Menus and project', [('full', {}, 'Full'), ('compact', {}, 'Compact'), ('minimal', {}, 'Minimal')],
     ['mark 24×24 (logo.py, drawn per size); menus 12 Regular ink-2, 13 apart, baseline 19',
      'name 13 Medium ink-1, baseline 36; summary 11 Regular ink-3, 8 after the name',
      'minimal: menu key 28×24 (≡), the name on the bar’s centre line']),
    ('mode', 'Pattern / song', [('full', {}, 'Song'), ('full', {'pattern': True}, 'Pattern')],
     ['segmented well 74×24 (34 + 40); the chosen segment a raised pill, inset 2, radius 3',
      'label 11: chosen Medium ink-1, other Regular ink-3 — never coloured']),
    ('transport', 'Transport and loop', [('full', {}, 'Stopped'), ('full', {'transport': 'playing'}, 'Playing'),
                                          ('full', {'transport': 'countin'}, 'Count-in'),
                                          ('full', {'transport': 'recording'}, 'Recording'),
                                          ('full', {'loop': False}, 'Loop off')],
     ['one well 122×24: keys 30 · 32 · 30 · 30, divided by the well’s dark edge (inset 4)',
      'loop key 30×24, 6 after; glyphs hinted (play 9×8, loop 15×11 at 24 px)',
      'playing: play-green tint 18 % in the play key; recording: rec tint 22 % and a lit dot',
      'count-in: the beats left in a 13×12 pill over the record key']),
    ('metronome', 'Metronome', [('full', {}, 'On'), ('full', {'metronome': False}, 'Off')],
     ['one well 46×24: key 30 + options 16 (chevron 5×2.5)', 'on: accent tint 20 % and accent glyph']),
    ('tempo', 'Tempo and metre', [('full', {}, 'Full'), ('compact', {}, 'Compact'),
                                  ('full', {'tempo_edit': True}, 'Typing'), ('full', {'tapping': True}, 'Tapping')],
     ['tempo well 78×24: integer 15 Regular ink-1, decimals 11 ink-3, unit 9 SemiBold ink-3 tracked 0.6',
      'metre well 34×24, 4 after: 12 Regular ink-2 centred; compact drops the metre (into the overflow)',
      'typing: a 2-px accent ring outside the well (45 % accent), caret 1×14 accent']),
    ('clock', 'Position', [('full', {}, 'Full'), ('compact', {}, 'Compact'),
                           ('full', {'transport': 'recording'}, 'Recording')],
     ['bar.beat 30 ExtraLight ink-1, baseline 34; ticks 15 Light ink-3; no well',
      'time 12 Regular ink-2 (baseline 20) over “bar · beat” 9 Medium ink-3 (baseline 33), 12 after',
      'recording: the numbers turn rec red']),
    ('snap', 'Grid and snap', [('full', {}, 'On'), ('full', {'snap': False}, 'Off'), ('compact', {}, 'Compact')],
     ['one well 80×24: magnet key 28 + grid 52 (value 12 Regular ink-1, chevron at 70)',
      'compact 40×24: magnet + a chevron; the grid moves into the overflow']),
    ('undo', 'Undo and redo', [('full', {}, 'Redo unavailable')],
     ['one well 56×24, two keys 28; unavailable = glyph ink-4 (no tint, no box)']),
    ('windows', 'Windows', [('full', {}, 'Default'), ('full', {'open': set()}, 'All closed')],
     ['one well 196×24, seven keys 28; glyph box 11 at 20 px', 'open: accent tint 16 % and accent glyph']),
    ('perf', 'CPU, RAM, voices, dropouts', [('full', {}, 'Calm'), ('full', {'cpu': 78}, 'Busy'),
                                            ('full', {'drops': 3}, 'Dropouts')],
     ['well 142×24: CPU graph 34×12 + value 12 Regular; RAM 12 Regular, divider 1×12 edge',
      'busy (≥ 70 %): graph and value in solo amber; dropouts: a rec-red 14×12 pill with the count']),
    ('inputs', 'Inputs', [('full', {}, 'MIDI in, none armed'), ('full', {'midi_activity': False, 'armed': 2}, 'Silent, 2 armed')],
     ['well 112×24: lamp r 2.5 (play green, halo r 4.5 at 20 %) + “MIDI” 11 ink-2',
      'arm ring r 3.5 + “Armed” 11 ink-2 + count 11 (rec red Medium when > 0)']),
    ('meter', 'Master meter', [('full', {}, 'Idle'), ('full', {'master': (0.82, 0.79)}, 'Playing'),
                               ('full', {'master': (0.98, 0.97), 'clipping': True}, 'Clipping'),
                               ('full', {'lufs': True, 'master': (0.7, 0.68)}, 'LUFS'),
                               ('compact', {}, 'Compact')],
     ['well 132×24 (compact 92, no label): mode 10 Medium ink-3; bars 3 high, 2 apart; ticks at −48 −24 −12 −6 0',
      'idle: only the grooves and the ticks — no “−∞”; clip: a 3×9 clip-red lamp at the right']),
    ('device', 'Audio device', [('full', {}, 'Running'), ('full', {'no_device': True}, 'No device'),
                                ('compact', {}, 'Compact'), ('compact', {'no_device': True}, 'Compact, none')],
     ['well 104×24: speaker glyph at 20 px + name 11 Regular ink-2 + chevron; compact a 30×24 key',
      'no device: a solo-amber warning and the words — the one place the bar raises its voice']),
    ('tools', 'Tools and zoom', [('full', {}, 'Rest'), ('full', {'picking': True}, 'Picking')],
     ['picker key 28×24 + zoom well 52×24 (11 Regular ink-2, centred)']),
    ('more', 'More', [('full', {}, 'Nothing folded'), ('full', {'folded_n': 4}, '4 folded'),
                      ('full', {'overflow_open': True, 'folded_n': 4}, 'Open')],
     ['key 30×24; the count in a 13×11 raised pill, 8.5 SemiBold ink-2, at the key’s top right']),
]


def groups_sheet(t):
    L = 24
    W = 1600
    out = []
    out.append(title(t, L, 34, f'Top bar — each group, its forms and states, its measures · {t["name"]}',
                     'drawn at 1:1 on the bar’s own surface; widths and heights in logical px; type roles and tokens as built'))
    y = 74
    for g, nm, states, spec in GROUPS:
        out.append(text(L, y + 16, nm, 13, t['ink1'], 500))
        for k, line in enumerate(spec):
            out.append(text(L, y + 34 + k * 15, line, 10.5, t['ink2'] if k == 0 else t['ink3']))
        x = 560
        rowh = max(78, 30 + 15 * len(spec))
        for form, st, cap in states:
            s2 = dict(IDLE, **st)
            w = TB.width(g, form, s2)
            out.append(rect(x - 8, y + 2, w + 16, 44, t['surface'], 4))
            out.append(f'<g transform="translate({x} {y + 2})">{TB.draw_group(t, g, form, 0, s2)}</g>')
            out.append(text(x, y + 60, cap, 10, t['ink3']))
            out.append(text(x, y + 73, f'{w:.0f} × 24', 10, t['ink3']))
            x += max(w, tw(cap, 10) + 24) + 28
        y += rowh + 18
        out.append(rect(L, y - 10, W - 2 * L, 1, mix(t['base'], t['line'], 0.6)))
    H = y + 10
    body = rect(0, 0, W, H, t['base']) + ''.join(out)
    return svg_doc(body, W, H, t), W, H


def overflow_sheet(t):
    import overflow2 as OV
    L = 24
    W = 1700
    out = [title(t, L, 34, f'Overflow panel — what each width folds into it, and its states · {t["name"]}',
                 'built from the bar’s folds: big groups are sections (one column up to three, two from four), small folds are rows under “Also here”')]
    y = 74
    cases = [(1600, {}, 'At 1600: three groups, one column'),
             (1280, {}, 'At 1280 (the main window): four groups, two columns'),
             (1024, {}, 'At 1024: six groups and the compact forms’ pieces'),
             (800, {}, 'At 800: seven groups; the panel never outgrows the window'),
             (1280, {'cpu': 82, 'drops': 3, 'focus_key': 5, 'hover_key': None, 'hint': ('Mixer', '— F9 · Enter opens it, arrows move between keys')},
              'States: CPU busy, three dropouts, a key with the keyboard focus'),
             (1024, {'metronome': False, 'armed': 2, 'midi_activity': False},
              'States: metronome off (its rows quiet), two tracks armed, MIDI silent')]
    for Wd, st, cap in cases:
        s2 = dict(IDLE, overflow_open=True, **st)
        forms = TB.layout(Wd, s2)
        svg, forms, more_x = TB.bar(t, Wd, s2, forms=forms)
        out.append(text(L, y + 14, cap, 13, t['ink1'], 500))
        out.append(f'<g transform="translate({L} {y + 24})">{svg}</g>')
        psvg, pw, ph = OV.panel(t, forms, more_x + 30, s2, top=40)
        out.append(f'<g transform="translate({L} {y + 24})">{psvg}</g>')
        y += 24 + 40 + ph + 40
    H = y
    body = rect(0, 0, W, H, t['base']) + ''.join(out)
    return svg_doc(body, W, H, t), W, H


def toolbar_sheet(t):
    import toolbar2 as TO
    L = 24
    W = 1140
    out = [title(t, L, 34, f'Toolbar — the arrangement’s, at every width the panel takes · {t["name"]}',
                 'row 28 high; keys 20 high at y 4; gaps 12; edges 12; the tools are a radio set — the chosen one a lifted pill, never coloured')]
    y = 78
    for Wd in (1032, 860, 700, 560, 420, 340):
        svg, forms = TO.row(t, Wd, {})
        changed = [f'{g} → {forms[g] or "folded"}' for g in forms if forms[g] != 'full']
        out.append(text(L, y + 12, f'{Wd} px', 13, t['ink1'], 500))
        out.append(text(L + 70, y + 12, ('; '.join(changed)) if changed else 'everything shown', 11, t['ink3']))
        out.append(f'<g transform="translate({L} {y + 20})">{svg}</g>')
        out.append(rect(L + Wd, y + 20, 1, 28, t['accent']))
        y += 20 + 28 + 22
    y += 10
    out.append(text(L, y, 'States', 15, t['ink1'], 500))
    y += 16
    x = L
    for n, _, lab, sc in TO.TOOLS:
        out.append(rect(x, y, 172, 28, t['surface'], 4))
        out.append(f'<g transform="translate({x + 8} {y})">{TO.draw_group(t, "tools", "full", 0, {"tool": n})}</g>')
        out.append(text(x, y + 44, f'{lab} chosen', 10, t['ink2']))
        out.append(text(x + 172, y + 44, sc, 10, t['ink3'], 500, 'end'))
        x += 172 + 8
    y += 66
    x = L
    variants = [('tools', 'full', {'tool': 'draw', 'tool_hover': 'erase'}, 'Hover on Erase', 172),
                ('tools', 'chosen', {'tool': 'draw'}, 'Folded to the chosen', 104),
                ('follow', 'full', {'follow': True}, 'Follow on', 80), ('follow', 'full', {'follow': False}, 'Follow off', 80),
                ('grid', 'full', {}, 'Grid', 104), ('grid', 'full', {'grid_open': True}, 'Grid open', 104),
                ('menu', 'full', {}, 'Panel menu', 40)]
    for g, f, st, lab, w in variants:
        out.append(rect(x, y, w, 28, t['surface'], 4))
        out.append(f'<g transform="translate({x + 8} {y})">{TO.draw_group(t, g, f, 0, st)}</g>')
        out.append(text(x, y + 44, lab, 10, t['ink2']))
        if st.get('grid_open'):
            out.append(TO.grid_menu(t, x + 30 + 8, y + 30))
        x += w + 16
    out.append(text(L, y + 72, 'Hints (the help line): “Draw — click to add a clip, drag to stretch it · P”; '
                                '“Grid — what clips snap to · Bar”; “Follow — the view keeps the playhead in sight”.', 10.5, t['ink3']))
    H = y + 290
    body = rect(0, 0, W, H, t['base']) + ''.join(out)
    return svg_doc(body, W, H, t), W, H


def logo_sheet(t):
    import logo as LG
    W, L = 760, 24
    out = [title(t, L, 34, f'The mark, drawn for each place · {t["name"]}',
                 'the idea of logo #2: a C of wave spokes open toward a playhead with its crossbar; every straight edge on whole pixels')]
    places = {16: 'title bar, small taskbar', 20: 'title bar at 125 %', 24: 'top bar at 100 %',
              30: 'taskbar at 125 % (of 24)', 32: 'taskbar', 36: 'top bar at 150 %', 48: 'taskbar large, 150 % of 32'}
    y = 78
    out.append(rect(L, y, W - 2 * L, 70, t['surface'], 6))
    x = L + 16
    for n in LG.SPEC:
        out.append(LG.mark(n, x, y + 35 - n // 2, uid=f'ls{n}{t["name"]}'))
        out.append(text(x, y + 86, f'{n} px', 11, t['ink1'], 500))
        x += n + 52
    y += 104
    for n in LG.SPEC:
        out.append(text(L, y, f'{n} px', 11, t['ink2'], 500))
        out.append(text(L + 52, y, places[n], 11, t['ink3']))
        y += 17
    y += 8
    out.append(text(L, y, 'The original at 48 px for reference (its glow blurs below 48):', 11, t['ink3']))
    out.append(f'<image href="{LOGO_SRC}" x="{L + 380}" y="{y - 30}" width="48" height="48"/>')
    H = y + 30
    body = rect(0, 0, W, H, t['base']) + ''.join(out)
    return svg_doc(body, W, H, t), W, H


LOGO_SRC = 'file:///F:/projects/daw-r39/dir-a/crates/cantus-ui/assets/logos/02.svg'


def write(name, html):
    os.makedirs(OUT, exist_ok=True)
    with open(os.path.join(OUT, name), 'w', encoding='utf-8') as f:
        f.write(html)


if __name__ == '__main__':
    sheets = [('topbar-widths', widths_sheet), ('topbar-groups', groups_sheet), ('overflow', overflow_sheet),
              ('toolbar', toolbar_sheet), ('mark', logo_sheet)]
    for scale, tag in ((1.0, ''), (1.5, '@15')):
        G.SCALE[0] = scale
        for t in (NIGHT, PAPER):
            k = t['name'].lower()
            for nm, fn in sheets:
                if tag and nm not in ('topbar-groups', 'toolbar'):
                    continue
                s, W, H = fn(t)
                write(f'b-{nm}-{k}{tag}.html', page(s, W, H, t['base']))
                print(f'b-{nm}-{k}{tag} {W} {H}')
    G.SCALE[0] = 1.0
