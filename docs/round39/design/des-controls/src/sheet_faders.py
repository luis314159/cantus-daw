"""des-controls — the fader sheet: strip faders and meters in every state, the master,
the anatomy, and the horizontal sliders and value bars."""
from slate import Sheet, text, rect, mix, tw
from faders import fader, meter, scale, cap, meter_grad, FAD, MASTER, frac
from sliders import slider, valuebar
from signs import sign

W, H = 1280, 790
TRAVEL = 126
CASES = [
    ('Silent', dict(db=-3.5), '−3.5', ''),
    ('Playing', dict(db=-3.5, levels=(-14, -15.5), peaks=(-8.5, -9.2), hold=(-6.8, -7.4)), '−3.5', '−6.8'),
    ('Peak hold', dict(db=-3.5, levels=(-31, -33), peaks=(-27, -29), hold=(-6.8, -7.4)), '−3.5', '−6.8'),
    ('Clip', dict(db=-3.5, levels=(-3, -4), peaks=(0.6, -0.4), hold=(0.6, -0.2), clip=True), '−3.5', '+0.6'),
    ('Hover', dict(db=-3.5, state='hover', levels=(-14, -15.5), peaks=(-8.5, -9.2), hold=(-6.8, -7.4)), '−3.5', '−6.8'),
    ('Dragged', dict(db=-1.2, state='drag', levels=(-14, -15.5), peaks=(-8.5, -9.2), hold=(-6.8, -7.4)), '−1.2', '−6.8'),
    ('Automated', dict(db=-8.0, drive='auto', levels=(-19, -20), peaks=(-13, -14), hold=(-11.2, -12)), '−8.0', '−11.2'),
    ('VCA', dict(db=-3.5, vca=-4.0, levels=(-18, -19.5), peaks=(-12, -13), hold=(-10.5, -11)), '−3.5', '−10.5'),
    ('Frozen', dict(db=-3.5, state='disabled'), '−3.5', ''),
]


def build(t, s):
    sh = Sheet(t, s, W, H, 'Slate — faders, meters and sliders',
               f'des-controls, round 39 · {t["name"]} · {int(s * 100)} % · drawn for this scale, 1 px = 1 screen pixel')
    P = sh.P
    sh.add('<defs>' + meter_grad(t, f'mg-{t["name"]}') + '</defs>')
    sh.head(24, 84, 'Strip fader and meter', 'travel 126 px (189 at 150 %); the meter shares the fader\'s scale')
    top = 128
    for i, (nm, kw, val, pk) in enumerate(CASES):
        x0 = 24 + i * 70
        # the strip under it, so the colours are judged where they live
        sh.add(rect(P(x0), P(top - 22), P(62), P(TRAVEL + 96), t['surface'], 0))
        kw = dict(kw)
        db = kw.pop('db')
        st = kw.pop('state', 'rest')
        drv = kw.pop('drive', None)
        vca = kw.pop('vca', None)
        svg, mx = fader(t, s, P(x0 + 22), P(top), P(TRAVEL), db, st, drv, vca, **kw)
        sh.add(svg)
        cx = P(x0 + 31)
        vy = P(top + TRAVEL + 24)
        vcol = t['accent'] if st == 'drag' else (t['auto'] if drv == 'auto' else (t['ink4'] if st == 'disabled' else t['ink1']))
        sh.add(text(cx, vy, val, 12 * s, vcol, 400, 'middle'))
        if pk:
            clip = kw.get('clip')
            sh.add(text(cx, vy + P(14), pk, 10 * s, t['clip'] if clip else t['ink3'], 500 if clip else 400, 'middle'))
        if vca is not None:
            sh.add(text(cx, vy + P(28), 'VCA 1 −4.0', 9 * s, t['ink2'], 600, 'middle'))
        sh.add(text(cx, P(top + TRAVEL + 92), nm, 11 * s, t['ink2'], 400, 'middle'))
    master(sh, t, s, 670, 106)
    anatomy(sh, t, s, 900, 84)
    sliders(sh, t, s, 24, 420)
    return sh


def master(sh, t, s, x, top):
    """The master: a longer travel, numbers on the scale, broad bars."""
    P = sh.P
    sh.add(text(P(x), P(84 - 0), 'Master', 15 * s, t['ink1'], 500))
    tr = 196
    for k, (nm, lv) in enumerate((('Silent', None), ('Playing', ((-11, -12.4), (-5.5, -6.2), (-3.1, -3.6))))):
        x0 = x + k * 104
        sh.add(rect(P(x0), P(top), P(96), P(tr + 66), t['surface']))
        gx = P(x0 + 34)
        sh.add(scale(t, s, gx - P(4), P(top + 22), P(tr), numbers=True, side='left'))
        g = FAD[s]
        sh.add(rect(gx, P(top + 22) - 2 * s, g['gw'], P(tr) + 4 * s, t['edge'] if t['name'] == 'Night' else mix(t['edge'], t['well'], 0.3), g['gw'] / 2))
        m = MASTER[s]
        kw = dict(levels=lv[0], peaks=lv[1], hold=lv[2]) if lv else {}
        sh.add(meter(t, s, gx + g['gw'] + P(14), P(top + 22), P(tr), mw=m['mw'], mg=m['mg'], **kw))
        cy = P(top + 22) + round(P(tr) * frac(0.0))
        sh.add(cap(t, s, gx + g['gw'] / 2, cy))
        sh.add(text(P(x0 + 48), P(top + tr + 44), '0.0', 12 * s, t['ink1'], 400, 'middle'))
        if lv:
            sh.add(text(P(x0 + 48), P(top + tr + 58), '−3.1', 10 * s, t['ink3'], 400, 'middle'))
        sh.add(text(P(x0 + 48), P(top + tr + 92), nm, 11 * s, t['ink2'], 400, 'middle'))


def anatomy(sh, t, s, x, y):
    P = sh.P
    sh.head(x, y, 'Anatomy', '100 % geometry ×5')
    z = 5
    g = FAD[1]
    inner, _ = fader(t, 1, 0, 0, 40, 0.0, levels=(-9, -10), peaks=(-4, -5), hold=(-2, -2.5))
    sh.add(f'<g transform="translate({P(x + 40)} {P(y + 46)}) scale({z * s})">{inner}</g>')
    notes = [('groove 2 (3) px', 'edge colour, round ends'),
             ('cap 20 × 18 (31 × 28)', "the strip's brightest grey; index 1 (2) px, engraved"),
             ('grips ±4 (±6) px', 'fcap-lo → edge, 1 px, pressed in'),
             ('0 dB detent', 'a 5 (7) px tick both sides, ink 3'),
             ('ticks +6 0 −6 −12 −24 −48', '3 (4) px; 0 dB 5 (7) px'),
             ('meter 2 × 3 (4) px, gap 1 (2)', 'its own well; the peak, solid, 30 ms fall'),
             ('peak hold 1 (2) px', 'ink 1; clip red above 0 dBFS'),
             ('clip LED 3 (4) px', 'above the pair; click resets'),
             ('colour by height', 'green to −18, yellow-green −9, amber −3')]
    for k, (a, b) in enumerate(notes):
        yy = y + 30 + k * 30
        sh.add(text(P(x + 160), P(yy), a, 12 * s, t['ink1'], 500))
        sh.add(text(P(x + 160), P(yy + 14), b, 11 * s, t['ink3']))


def sliders(sh, t, s, x, y):
    P = sh.P
    sh.head(x, y, 'Sliders', 'the fader on its side: groove 3 (4) px, cap 9 × 15 (13 × 22)')
    cols = [('Rest', {}), ('Hover', {'state': 'hover'}), ('Dragged', {'state': 'drag'}),
            ('Automated', {'drive': 'auto'}), ('Disabled', {'state': 'disabled'})]
    for i, (nm, kw) in enumerate(cols):
        xx = x + 20 + i * 180
        sh.add(text(P(xx), P(y + 32), nm, 11 * s, t['ink3']))
        sh.add(slider(t, s, P(xx), P(y + 54), P(140), 0.58, **kw))
    yb = y + 84
    sh.add(text(P(x + 20), P(yb), 'Bipolar, centre', 11 * s, t['ink3']))
    sh.add(slider(t, s, P(x + 20), P(yb + 22), P(140), 0.0, bipolar=True))
    sh.add(text(P(x + 200), P(yb), 'Bipolar, L 30', 11 * s, t['ink3']))
    sh.add(slider(t, s, P(x + 200), P(yb + 22), P(140), -0.3, bipolar=True))
    sh.add(text(P(x + 380), P(yb), 'Range, velocity 40–108', 11 * s, t['ink3']))
    sh.add(slider(t, s, P(x + 380), P(yb + 22), P(140), 0, rng=(0.31, 0.85)))
    sh.add(text(P(x + 560), P(yb), 'Range, dragged', 11 * s, t['ink3']))
    sh.add(slider(t, s, P(x + 560), P(yb + 22), P(140), 0, rng=(0.31, 0.85), state='drag'))

    yv = y + 150
    sh.head(x, yv, 'Value bars', 'the dense form for inspector rows and editors: the fill is the value; 20 (30) px high')
    bars = [('Rest', 'Cutoff', '2.40', 'kHz', 0.62, {}),
            ('Hover', 'Cutoff', '2.40', 'kHz', 0.62, {'state': 'hover'}),
            ('Dragged', 'Cutoff', '3.10', 'kHz', 0.74, {'state': 'drag'}),
            ('Focused (keyboard)', 'Cutoff', '2.40', 'kHz', 0.62, {'state': 'focus'}),
            ('Automated', 'Cutoff', '1.85', 'kHz', 0.48, {'drive': 'auto'}),
            ('Modulated', 'Cutoff', '2.40', 'kHz', 0.62, {'drive': 'mod', 'mod': (0.46, 0.78, 0.71)}),
            ('MIDI learned', 'Resonance', '0.35', '', 0.35, {'drive': 'midi'}),
            ('Bipolar', 'Pan', 'L 30', '', -0.3, {'bipolar': True}),
            ('Disabled', 'Cutoff', '2.40', 'kHz', 0.62, {'state': 'disabled'}),
            ('Stepped', 'Voices', '4', 'of 8', 0.5, {})]
    for i, (lab, nm, val, unit, v, kw) in enumerate(bars):
        col, row = i % 5, i // 5
        xx = x + 20 + col * 230
        yy = yv + 32 + row * 56
        sh.add(text(P(xx), P(yy), lab, 11 * s, t['ink3']))
        sh.add(valuebar(t, s, P(xx), P(yy + 8), P(200), nm, val, unit, v, **kw))
    yn = yv + 160
    notes = ['Drag sideways or up/down to change; Shift for fine; double-click to type; right-click for automation and MIDI learn.',
             'The value\'s edge is a 1 (2) px line in ink 2 (hover ink 1, held accent, automated violet); the fill is ink 3 at 30 % of the well.',
             'A driven bar shows its sign before the number and the number in the sign\'s colour; a modulated bar adds the range as a 3 (4.5) px band and the live value as a line.']
    for k, n in enumerate(notes):
        sh.add(text(P(x + 20), P(yn + k * 16), n, 11 * s, t['ink3']))
