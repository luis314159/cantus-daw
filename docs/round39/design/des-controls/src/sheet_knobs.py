"""des-controls — the knob sheet: three sizes x every state, the signs, the anatomy."""
from slate import Sheet, text, rect, mix, tw, f
from knobs import knob, bubble, valtext, KNOB, pt, ang
from signs import sign, sign_col

W, H = 1280, 800
COLS = [  # (header, group, kwargs)
    ('Rest', 'Unipolar', {}), ('Hover', 'Unipolar', {'state': 'hover'}),
    ('Dragged', 'Unipolar', {'state': 'drag'}), ('Focused', 'Unipolar', {'state': 'focus'}),
    ('Disabled', 'Unipolar', {'state': 'disabled'}),
    ('Automated', 'Driven', {'drive': 'auto'}), ('Automated, held', 'Driven', {'drive': 'auto', 'state': 'drag', 'held': 0.38}),
    ('Modulated', 'Driven', {'drive': 'mod', 'mod': (0.14, 0.09)}), ('MIDI learned', 'Driven', {'drive': 'midi'}),
    ('Centre', 'Bipolar', {'bipolar': True, 'v': 0.0}), ('Right 45', 'Bipolar', {'bipolar': True, 'v': 0.45}),
    ('Detents', 'Stepped', {'steps': 5, 'v': 0.5}),
]
X0, PITCH = 150, 92
# per size: the row's top, its height, the name and the values shown per column
ROWS = [('S', 'Small', 'strips, inline rows', 128, 70),
        ('M', 'Medium', 'panels, inspector, effects', 198, 104),
        ('L', 'Large', "an editor's main controls", 302, 138)]
VALS = {'S': ['', '', 'L 30', '', '', '', 'L 30', '', '', 'C', 'R 45', ''],
        'M': ['2.40', '2.40', '3.10', '2.40', '2.40', '1.85', '3.10', '2.40', '2.40', 'C', 'R 45', '4'],
        'L': ['6.0', '6.0', '8.5', '6.0', '6.0', '4.2', '8.5', '6.0', '6.0', '0.0', '+5.4', '4×']}
UNIT = {'M': 'kHz', 'L': 'dB'}
NAME = {'M': 'Cutoff', 'L': 'Drive'}


def build(t, s):
    sh = Sheet(t, s, W, H, 'Slate — knobs',
               f'des-controls, round 39 · {t["name"]} · {int(s * 100)} % · drawn for each size at this scale, 1 px = 1 screen pixel')
    P = sh.P
    # group bands
    groups = {}
    for i, (_, g, _) in enumerate(COLS):
        groups.setdefault(g, []).append(i)
    for g, idx in groups.items():
        x0 = X0 + idx[0] * PITCH - PITCH / 2 + 6
        x1 = X0 + idx[-1] * PITCH + PITCH / 2 - 6
        sh.add(rect(P(x0), P(74), P(x1 - x0), P(1), mix(t['base'], t['ink4'], 0.8)))
        sh.add(text(P(x0), P(90), g, 11 * s, t['ink3'], 600))
    for i, (hd, _, _) in enumerate(COLS):
        sh.label(X0 + i * PITCH, 110, hd, 'middle', t['ink2'])
    for size, nm, use, y, hgt in ROWS:
        sh.label(24, y + hgt / 2 - 2, nm, col=t['ink1'], size=12, weight=500)
        sh.label(24, y + hgt / 2 + 13, use, size=10)
        g = KNOB[size][s]
        D = g['D'] / s  # logical
        for i, (hd, grp, kw) in enumerate(COLS):
            kw = dict(kw)
            kw.setdefault('v', 0.62 if i != 2 else 0.74)
            if i == 5:
                kw['v'] = 0.5
            if i == 6:
                kw['v'] = 0.74
            if size == 'L' and grp == 'Stepped':
                kw['steps'] = 4
                kw['v'] = 2 / 3
            if size == 'S' and grp == 'Stepped':
                kw['steps'] = 4
                kw['v'] = 1 / 3
            if size == 'S' and grp in ('Unipolar', 'Driven'):
                kw['bipolar'] = True
                kw['v'] = {0: -0.3, 1: -0.3, 2: -0.3, 3: -0.3, 4: -0.3, 5: 0.25, 6: -0.3, 7: 0.1, 8: -0.3}[i]
                if i == 6:
                    kw['held'] = 0.6
            cx = P(X0 + i * PITCH)
            ky = y + 16 + D / 2 if size != 'S' else y + hgt / 2 - 6
            cy = P(ky)
            st = kw.get('state', 'rest')
            sh.add(knob(t, s, size, cx, cy, **kw))
            val = VALS[size][i]
            dis = st == 'disabled'
            if size == 'S':
                if st == 'drag' and val:
                    sh.add(bubble(t, s, cx + P(D / 2 + 6), cy, val))
                continue
            # name above, value below
            ncol = t['ink4'] if dis else (t['ink1'] if st in ('hover', 'drag', 'focus') else t['ink2'])
            nsz = 11 if size == 'M' else 12
            nm_ = NAME[size] if grp != 'Stepped' else {'M': 'Voices', 'L': 'Quality'}[size]
            sh.add(text(cx, P(y + 9), nm_, nsz * s, ncol, 400 if size == 'M' else 500, 'middle'))
            vy = P(ky + D / 2 + (17 if size == 'M' else 22))
            vcol = None
            if dis:
                vcol = t['ink4']
            elif st == 'drag':
                vcol = t['accent']
            elif kw.get('drive') == 'auto':
                vcol = t['auto']
            unit = '' if grp in ('Bipolar', 'Stepped') else UNIT[size]
            if grp == 'Bipolar' and size == 'L':
                unit = 'st'
            sh.add(valtext(t, s, cx, vy, val, unit, vcol, 12 if size == 'M' else 15))
        if size == 'L':
            # the stepped hero knob names its detents around the groove
            i = 11
            cx, cy = P(X0 + i * PITCH), P(y + 16 + D / 2)
            for k, lab in enumerate(('Off', '2×', '4×', '8×')):
                a = ang(k / 3)
                lx, ly = pt(cx, cy, g['D'] / 2 + 9 * s, a)
                anc = 'end' if a < 0 else 'start'
                col = t['ink1'] if k == 2 else t['ink3']
                sh.add(text(lx, ly + 3.5 * s, lab, 9.5 * s, col, 600 if k == 2 else 400, anc))
    signs_legend(sh, t, s, 24, 470)
    anatomy(sh, t, s, 600, 470)
    return sh


def signs_legend(sh, t, s, x, y):
    P = sh.P
    sh.head(x, y, 'The driven signs', 'shape and colour both — never colour alone')
    rows = [('auto', 'Automated', 'an automation clip drives it'),
            ('mod', 'Modulated', 'a modulator moves it around your value'),
            ('midi', 'MIDI learned', 'a controller sets it'),
            ('vca', 'VCA', 'a VCA fader offsets this level')]
    for k, (kind, nm, what) in enumerate(rows):
        yy = y + 30 + k * 38
        col = sign_col(t, kind)
        for j, h in enumerate((5, 7, 9, 12)):
            hh = h * s if s == 1 else round(h * 1.5)
            sh.add(sign(kind, P(x + 10 + j * 26), P(yy), hh, col))
        sh.add(text(P(x + 120), P(yy - 2), nm, 12 * s, t['ink1'], 500))
        sh.add(text(P(x + 120), P(yy + 12), what, 11 * s, t['ink3']))
    sh.label(x, y + 30 + 4 * 38 + 2, 'Heights 5 · 7 · 9 · 12 px, each drawn for its size (150 %: 8 · 11 · 14 · 18).', size=10)
    sh.label(x, y + 30 + 4 * 38 + 16, 'On a knob the sign sits in the groove\'s open quarter; on a fader, beside the cap; in a field, before the value.', size=10)
    sh.label(x, y + 30 + 4 * 38 + 30, 'Automated also turns the value arc, the pointer and the value violet: the owner must never take it for broken.', size=10)


def anatomy(sh, t, s, x, y):
    """The medium knob at 6x as a measured drawing (vector, for the kit builder)."""
    P = sh.P
    sh.head(x, y, 'Anatomy', 'medium knob, 100 % geometry drawn ×6')
    z = 6
    g = KNOB['M'][1]
    cx, cy = P(x + 110), P(y + 30 + g['D'] * z / 2 + 8)
    inner = knob(t, 1, 'M', 0, 0, 0.62, drive='auto')
    sh.add(f'<g transform="translate({cx} {cy}) scale({z * s})">{inner}</g>')
    R = g['D'] / 2
    notes = [('D 32 (48 at 150 %)', 'the whole control, square'),
             ('groove 2 (3) px, 270°', 'track: well (Night) · well→edge (Paper)'),
             ('gap 3 (4) px', 'between the groove and the cap'),
             ('cap Ø 22 (34)', 'rim 1 px edge, radial light 40 % / 28 %'),
             ('crescent ±58°, 1 px', 'cap-hi lightened, the light from above'),
             ('pointer 2 (2.5) px', 'from 0.28 r to the rim − 2.5 px, round'),
             ('value arc', 'rest ink2→3 · hover ink1 · held accent · auto violet'),
             ('sign 6 (9) px high', 'centred in the open quarter, 1-px halo')]
    for k, (a, b) in enumerate(notes):
        yy = y + 40 + k * 30
        sh.add(text(P(x + 240), P(yy), a, 12 * s, t['ink1'], 500))
        sh.add(text(P(x + 240), P(yy + 14), b, 11 * s, t['ink3']))
