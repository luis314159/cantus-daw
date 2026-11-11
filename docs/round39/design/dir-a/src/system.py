"""Slate (dir-a) — the system on one page: palette tokens with hex and contrast, the user's
colours as Slate spends them, the type and its scale, spacing, radii, lines, density and a
key in its five states. Both themes side by side."""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import glyphs as G  # noqa: E402
from base import NIGHT, PAPER, HUES, defs, rect, text, mix, contrast, hue_for, note_ink, page, esc  # noqa: E402
from parts import key_state, glyph_in_key  # noqa: E402

W, H = 1260, 1240
SURF = ['gutter', 'base', 'surface', 'raised', 'well', 'edge', 'line']
INKS = ['ink1', 'ink2', 'ink3', 'ink4']
STATE = ['accent', 'play', 'rec', 'solo', 'clip', 'auto']
ROLE = {'gutter': 'between panels', 'base': 'panel body, lanes', 'surface': 'bars, headers, strips',
        'raised': 'popovers, pills', 'well': 'inside keys, fields', 'edge': 'a well’s dark edge',
        'line': 'hairlines (rare)', 'ink1': 'names, values', 'ink2': 'labels, glyphs at rest',
        'ink3': 'units, hints, numbers', 'ink4': 'disabled, idle marks', 'accent': 'on, loop, playhead, focus',
        'play': 'playing, on-LEDs', 'rec': 'record', 'solo': 'solo', 'clip': 'over 0 dBFS',
        'auto': 'automated control'}


def swatch_col(t, x, y):
    out = [rect(x - 12, y - 30, 600, 446, t['base'], 8)]
    out.append(text(x, y - 8, f'Cantus {t["name"]}', 15, t['ink1'], 500))
    yy = y + 6
    for group, names in (('Surfaces', SURF), ('Ink', INKS), ('State', STATE)):
        out.append(text(x, yy + 10, group, 11, t['ink3'], 600, track=0.2))
        yy += 18
        for k, n in enumerate(names):
            cx = x + (k % 2) * 290
            cy = yy + (k // 2) * 30
            c = t[n]
            out.append(rect(cx, cy, 24, 22, c, 4))
            out.append(f'<rect x="{cx + 0.5}" y="{cy + 0.5}" width="23" height="21" rx="3.5" fill="none" stroke="{mix(t["base"], t["ink1"], 0.15)}"/>')
            out.append(text(cx + 32, cy + 10, n, 11, t['ink1'], 500))
            out.append(text(cx + 32, cy + 21, ROLE[n], 10, t['ink3']))
            out.append(text(cx + 150, cy + 10, c.upper(), 11, t['ink2']))
            if n in INKS or n in STATE:
                ref = 'surface'
                cr = contrast(c, t[ref])
                ok = cr >= (4.5 if n in ('ink1', 'ink2') else 3.0)
                cb = contrast(c, t['base'])
                out.append(text(cx + 150, cy + 21, f'{cr:.1f}:1 surface · {cb:.1f}:1 base', 10, t['ink2'] if ok else t['ink3']))
        yy += ((len(names) + 1) // 2) * 30 + 8
    return ''.join(out)


def user_colours(t, x, y):
    out = [text(x, y, f'The user’s colours, as Slate spends them — {t["name"]}', 13, t['ink1'], 500)]
    out.append(text(x, y + 16, 'rest: a stripe and a tint, notes a step lighter · selected or playing: lit', 10, t['ink3']))
    y += 28
    for k, nm in enumerate(['Kick', 'Clap', 'Hats', 'Percussion', 'Bass', 'Chords', 'Pluck', 'Lead', 'Choir']):
        cx = x + k * 64
        for row, sel in enumerate((False, True)):
            cy = y + row * 34
            body = hue_for(t, nm, t['tint_sel'] if sel else t['tint_rest'])
            out.append(rect(cx, cy, 58, 28, t['base'], 0))
            out.append(rect(cx, cy, 3, 28, HUES[nm] if sel else hue_for(t, nm, t['stripe_rest'])))
            out.append(rect(cx + 6, cy + 1, 52, 26, body, 3))
            ink = note_ink(t, nm, sel)
            for j, (a, b, yy) in enumerate(((0.05, 0.4, 8), (0.42, 0.7, 14), (0.72, 0.95, 19))):
                out.append(rect(cx + 6 + 52 * a, cy + yy, 52 * (b - a) - 1, 3, ink, 1))
        out.append(text(cx + 32, y + 82, nm, 10, t['ink3'], 400, 'middle'))
    return ''.join(out)


def type_block(t, x, y):
    out = [text(x, y, 'Type — Barlow Semi Condensed', 15, t['ink1'], 500)]
    out.append(text(x, y + 17, 'SIL Open Font License 1.1 · version 1.408 · github.com/jpt/barlow (via github.com/google/fonts, ofl/barlowsemicondensed) · tabular figures on', 10, t['ink3']))
    rows = [(30, 200, '41.1', 'Clock — ExtraLight 30, ticks Light 15 in ink-3'),
            (15, 400, '124.000  412 MB', 'Values that matter — Regular 15'),
            (13, 500, 'Arrangement  Chords', 'Panel titles, the selection — Medium 13'),
            (12, 400, 'Kick  Comprimo  1/16', 'Names, menus, fields — Regular 12'),
            (11, 400, 'Count-in  −16.0  Post', 'Labels, readouts, hints — Regular 11'),
            (11, 600, 'Inserts  Windows', 'Section titles — SemiBold 11, ink-3, Title case'),
            (10, 400, '→ 10  Comp · EQ  2 of 10', 'Units, secondary — Regular 10, ink-3'),
            (9, 600, 'BPM  Alt F8', 'Unit tags, key caps — SemiBold 9')]
    yy = y + 44
    for size, wt, sample, note in rows:
        out.append(text(x, yy + (size * 0.35 if size > 20 else 0), sample, size, t['ink1'], wt))
        out.append(text(x + 230, yy, note, 10, t['ink3']))
        yy += max(22, size + 10)
    return ''.join(out), yy


def metrics(t, x, y):
    out = [text(x, y, 'Spacing, radii, lines, density', 15, t['ink1'], 500)]
    yy = y + 24
    lines = [
        ('Grid', '2-px unit; layout on even numbers so 150 % lands on whole pixels; steps 4 · 8 · 12 · 16 · 24'),
        ('Keys', '16 (strip, inline) · 20 (tool rows) · 24 (top bar) high; width 1.25× for glyph keys'),
        ('Rows', 'track 30 · inspector 24–26 · menu 24 · status 20 · top bar 44 · panel head 28'),
        ('Mixer', 'strip 60 wide, pitch 62 (2 px of base between: value, not borders); master 68'),
        ('Radii', '2 marks · 3 clips, small keys · 4 keys, fields · 6 menus · 8 popovers · round toggles'),
        ('Lines', 'hairline 1 px only where value cannot separate; glyph strokes 1 · 1 · 1.5 · 2 · 2 by size'),
        ('Relief', 'one lit top edge per bar; knob and fader caps lit from above; wells recessed (dark edge, light lip)'),
        ('Glyph', 'transport ≈ ⅓ of the key; line icons ≈ 55 %; two inks: ink-2 body at rest, ink-3 detail'),
    ]
    for a, b in lines:
        out.append(text(x, yy, a, 11, t['ink2'], 600))
        out.append(text(x + 60, yy, b, 11, t['ink1']))
        yy += 20
    # radii, drawn
    yy += 6
    for k, r in enumerate((2, 3, 4, 6, 8)):
        out.append(f'<rect x="{x + k * 56 + 0.5}" y="{yy + 0.5}" width="44" height="28" rx="{r}" fill="{t["well"]}" stroke="{t["edge"]}"/>')
        out.append(text(x + k * 56 + 22, yy + 42, f'r {r}', 10, t['ink3'], 400, 'middle'))
    return ''.join(out), yy + 50


def key_states(t, x, y):
    out = [text(x, y, 'A key, five states', 15, t['ink1'], 500)]
    names = ['rest', 'hover', 'pressed', 'on', 'disabled']
    for k, s in enumerate(names):
        cx = x + 20 + k * 74
        for row, (nm, g) in enumerate((('Loop', G.loop_hinted), ('Play', G.play_hinted))):
            ky = y + 18 + row * 36
            out.append(key_state(t, cx, ky, 34, 24, s, nm))
            out.append(glyph_in_key(t, nm, g, cx + 17, ky + 11.5, 24, s))
        out.append(text(cx + 17, y + 104, s.capitalize(), 10, t['ink3'], 400, 'middle'))
    out.append(text(x, y + 124, 'Rest: a recessed well, a shade darker than its bar; glyph ink-2.  Hover: the well lifts a step; glyph ink-1.', 10, t['ink3']))
    out.append(text(x, y + 138, 'Pressed: the well darkens to its edge.  On: an accent tint and an accent glyph (play: green).  Disabled: 55 %.', 10, t['ink3']))
    return ''.join(out)


def sheet():
    n, p = NIGHT, PAPER
    out = [defs(n), defs(p), rect(0, 0, W, H, '#e9edf1')]
    out.append(rect(0, 0, W, 70, '#1d2329'))
    out.append(text(24, 36, 'Slate — the system', 22, '#e3e8ed', 500))
    out.append(text(24, 56, 'dir-a, round 39 · a cool, quiet grey-blue; panels separated by value; one accent; the user’s colours a stripe and a tint until lit', 12, '#a6b0ba'))
    out.append(swatch_col(n, 36, 116))
    out.append(swatch_col(p, 660, 116))
    # lower half: type and metrics on Night, user colours both themes, key states both
    out.append(rect(24, 600, 1212, 420, n['base'], 8))
    tb, ty = type_block(n, 40, 630)
    out.append(tb)
    mb, my = metrics(n, 650, 630)
    out.append(mb)
    out.append(user_colours(n, 40, 880))
    out.append(key_states(n, 650, 880))
    out.append(rect(24, 1030, 1212, 200, p['base'], 8))
    out.append(user_colours(p, 40, 1056))
    out.append(key_states(p, 650, 1056))
    body = ''.join(out)
    return f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="0 0 {W} {H}">{body}</svg>'


if __name__ == '__main__':
    outdir = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'build')
    with open(os.path.join(outdir, 'system.html'), 'w', encoding='utf-8') as f:
        f.write(page(sheet(), W, H, '#e9edf1'))
    print('ok')
