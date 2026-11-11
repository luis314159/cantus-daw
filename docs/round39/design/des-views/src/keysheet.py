"""des-views — the keyboard sheet: three row heights, every state, and the measures."""
import keys as K
import pianoroll as P
from base import rect, text, mix, tw, lit_edge, HUES

W, H = 1280, 560
TOP, N = 71, 24            # B4 down to C3


def column(t, x, y, rh, kw, title, sub, lit=None, names='c', gw=190, uid='k'):
    out = [text(x, y, title, 13, t['ink1'], 500), text(x, y + 16, sub, 10, t['ink3'])]
    y += 26
    out.append(P.grid(t, x + kw, x + kw + gw, y, rh, TOP, N, 0, 32 if rh >= 12 else 24, snap=0.25,
                      hover_row=next((p for p, k in (lit or {}).items() if k == 'hover'), None), uid=uid + 'g'))
    # a chord sounding (C E G) and a melody, so the rows meet their keys
    hue = HUES['Chords']
    for p in (60, 64, 67):
        out.append(P.note(t, x + kw + 2, y + (TOP - p) * rh, 62, rh, 0.85, 'sel' if (lit or {}).get(p, '').startswith('sound') else '', K.name(p) if rh >= 10 else None, hue))
    for q, p in ((2, 69), (3, 71), (4, 69), (5, 52)):
        out.append(P.note(t, x + kw + 2 + q * 32, y + (TOP - p) * rh, 30, rh, 0.7, '', K.name(p) if rh >= 10 else None, hue))
    out.append(K.keyboard(t, x, y, kw, rh, TOP, N, lit=lit, names=names, uid=uid))
    return ''.join(out)


def measures(t, x, y):
    """One octave, drawn large, with its measures — the rule the kit follows."""
    rh = 14
    kw = 96
    out = [text(x, y, 'The measures', 13, t['ink1'], 500),
           text(x, y + 16, 'one octave, 14-px rows, drawn by the same rule', 10, t['ink3'])]
    y += 26
    out.append(K.keyboard(t, x, y, kw, rh, 71, 12, names='all', uid='km'))
    ink = t['ink2']
    ax = x + kw + 14
    cb = y + 12 * rh
    rows = [(0, 5 / 3, 'C  5/3 row'), (5 / 3, 10 / 3, 'D  5/3'), (10 / 3, 5, 'E  5/3'),
            (5, 6.75, 'F  7/4 row'), (6.75, 8.5, 'G  7/4'), (8.5, 10.25, 'A  7/4'), (10.25, 12, 'B  7/4')]
    for a, b, lab in rows:
        y1, y2 = cb - b * rh, cb - a * rh
        out.append(rect(ax, round(y1) + 1, 1, round(y2 - y1) - 2, t['ink3']))
        out.append(rect(ax - 2, round(y1) + 1, 5, 1, t['ink3']))
        out.append(text(ax + 8, (y1 + y2) / 2 + 4, lab, 10, ink))
    bw = round(kw * 0.6)
    yb = y + 12 * rh + 14
    out.append(rect(x, yb, bw, 1, t['ink3']))
    out.append(rect(x, yb - 3, 1, 7, t['ink3']))
    out.append(rect(x + bw - 1, yb - 3, 1, 7, t['ink3']))
    out.append(text(x, yb + 14, 'black key: 0.60 of the depth, one row high', 10, ink))
    out.append(text(x, yb + 28, '≈ 0.58 of a white key — a real piano’s ratio', 10, t['ink3']))
    notes = [
        'Only E|F and B|C fall on row lines, as on the instrument;',
        'C-D-E share 5 rows, F-G-A-B share 7.',
        'Each white key a piece of its own: a 1-px gap shows the',
        'bed between keys, a lit top edge, a shade at its foot.',
        'Black keys lift: a light edge on the top and the front,',
        'a two-step shadow onto the white keys beside them.',
        'Every C named in its key: middle C (C4) ink 1 SemiBold,',
        'the others ink 3 Medium; from 16-px rows every key.',
        'States: under the pointer (accent 16 %), heard by a click',
        '(accent 55 %, a shade at the front), sounding (the',
        'track’s colour 55 %) — the black keys a little stronger.',
    ]
    for k, s in enumerate(notes):
        out.append(text(x, yb + 52 + k * 15, s, 10, t['ink2'] if k % 2 == 0 else t['ink2']))
    return ''.join(out)


def sheet(t):
    out = [rect(0, 0, W, H, t['gutter']), rect(0, 0, W, 56, t['surface']), lit_edge(t, 0, 0, W)]
    out.append(text(16, 26, f'The piano roll’s keyboard — Slate {t["name"]}', 18, t['ink1'], 500))
    out.append(text(16, 45, 'Three row heights (8, 12, 18 px), each drawn for its size; the states; the measures. '
                    'C3 to B4. Drawn at 1:1 (logical px).', 11, t['ink3']))
    y = 80
    out.append(rect(0, 60, W, H - 60, t['base']))
    out.append(column(t, 16, y, 8, 56, 'Compact · 8-px rows', 'only the Cs named · black-key top inset 1 px',
                      lit={64: 'hover'}, uid='k8'))
    sound = 'sound:' + HUES['Chords']
    out.append(column(t, 290, y, 12, 72, 'Default · 12-px rows', 'pointer on D4 · F♯4 heard by a click · C4 E4 G4 sounding',
                      lit={62: 'hover', 66: 'press', 60: sound, 64: sound, 67: sound}, uid='k12'))
    out.append(column(t, 580, y, 18, 88, 'Tall · 18-px rows', 'every key named, the Cs stronger', names='all',
                      lit={69: 'press'}, uid='k18', gw=170))
    out.append(measures(t, 868, y))
    return ''.join(out)
