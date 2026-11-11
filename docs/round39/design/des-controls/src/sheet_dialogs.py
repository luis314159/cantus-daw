"""des-controls — buttons (primary, secondary, destructive, quiet) and the dialog frame."""
from slate import Sheet, text, rect, mix, tw, frame, well, f, line, circle
from switches import checkbox
from fields import textfield


def keycap(t, s, x, y, k):
    """A key as a small engraved cap, 15 (22) px: an outline in ink 3 at 50 %, SemiBold 9.5."""
    fs = 9.5 * s
    w = tw(k, fs, 600) + 9 * s
    h = 15 * s if s == 1 else 22
    out = frame(x, y, w, h, mix(t['raised'], t['ink3'], 0.5), 3 * s if s == 1 else 4)
    out += text(x + w / 2, y + h / 2 + fs * 0.36, k, fs, t['ink2'], 600, 'middle')
    return out, w

W, H = 1280, 780
KINDS = ['Primary', 'Secondary', 'Destructive', 'Quiet']
STATES = ['Rest', 'Hover', 'Pressed', 'Focused', 'Disabled', 'Busy']


def button(t, s, x, y, label, kind='secondary', state='rest', h=28, minw=84):
    H_ = h * s
    fs = (12 if h == 28 else 11.5) * s
    w = max(minw * s, tw(label, fs, 500) + (32 if h == 28 else 24) * s)
    if state == 'busy':
        w += 18 * s
    r = 4 * s if s == 1 else 5
    n = t['name'] == 'Night'
    out = ''
    if state == 'disabled':
        out += rect(x, y, w, H_, mix(t['surface'], t['well'], 0.6) if kind != 'quiet' else 'none', r)
        ink = t['ink4']
    elif kind in ('primary', 'destructive'):
        base = t['accent'] if kind == 'primary' else t['danger']
        fill = {'rest': base, 'hover': mix(base, '#ffffff', 0.12), 'pressed': mix(base, '#000000', 0.18),
                'focus': base, 'busy': mix(base, t['surface'], 0.25)}[state]
        out += rect(x, y + 1, w, H_, 'rgba(0,0,0,0.30)' if n else 'rgba(40,55,70,0.16)', r)
        out += rect(x, y, w, H_, fill, r)
        if state != 'pressed':
            out += rect(x + 2 * s, y, w - 4 * s, 1, '#ffffff', 0, ' fill-opacity="0.22"')
        ink = t['primary_ink'] if kind == 'primary' else t['danger_ink']
    elif kind == 'secondary':
        # a lifted neutral: a step lighter than the sheet it sits on, a lit top, a darker foot
        if n:
            fill = {'rest': mix(t['raised'], t['ink1'], 0.08), 'hover': mix(t['raised'], t['ink1'], 0.14),
                    'pressed': t['well'], 'focus': mix(t['raised'], t['ink1'], 0.08),
                    'busy': mix(t['raised'], t['ink1'], 0.08)}[state]
        else:
            fill = {'rest': '#ffffff', 'hover': '#f3f6f9', 'pressed': t['well'], 'focus': '#ffffff', 'busy': '#ffffff'}[state]
        out += rect(x, y + 1, w, H_, 'rgba(0,0,0,0.35)' if n else 'rgba(40,55,70,0.14)', r)
        out += rect(x, y, w, H_, fill, r)
        if not n:
            out += frame(x, y, w, H_, t['edge'], r)
        elif state != 'pressed':
            out += rect(x + 2 * s, y, w - 4 * s, 1, '#ffffff', 0, ' fill-opacity="0.10"')
        ink = t['ink1']
    else:  # quiet: words until the pointer comes
        if state in ('hover', 'pressed'):
            out += rect(x, y, w, H_, t['well_hi'] if state == 'hover' else t['well'], r)
        ink = t['ink1'] if state in ('hover', 'pressed') else t['ink2']
    if state == 'focus':
        g = 2 * s if s == 1 else 3
        out += frame(x - g - 1 * s, y - g - 1 * s, w + 2 * g + 2 * s, H_ + 2 * g + 2 * s, t['accent'], r + g, g,
                     ' stroke-opacity="0.6"')
    tx = x + w / 2
    if state == 'busy':
        # a turning arc before the words
        cx, cy, rr = x + 14 * s, y + H_ / 2, 5 * s
        out += f'<circle cx="{f(cx)}" cy="{f(cy)}" r="{f(rr)}" fill="none" stroke="{ink}" stroke-opacity="0.3" stroke-width="{f(1.5 * s)}"/>'
        out += (f'<path d="M{f(cx)} {f(cy - rr)}A{f(rr)} {f(rr)} 0 0 1 {f(cx + rr)} {f(cy)}" fill="none" stroke="{ink}" '
                f'stroke-width="{f(1.5 * s)}" stroke-linecap="round"/>')
        tx += 9 * s
    out += text(tx, y + H_ / 2 + fs * 0.36, label, fs, ink, 500, 'middle')
    return out, w


def build(t, s):
    sh = Sheet(t, s, W, H, 'Slate — buttons and dialogs',
               f'des-controls, round 39 · {t["name"]} · {int(s * 100)} % · drawn for this scale')
    P = sh.P
    sh.head(24, 84, 'Buttons', '28 (42) px in dialogs, 24 (36) px compact · label Medium 12 · min width 84 (126)')
    labels = {'primary': 'Export', 'secondary': 'Cancel', 'destructive': 'Delete', 'quiet': 'Details'}
    for i, st in enumerate(STATES):
        sh.label(150 + i * 150, 112, st, col=t['ink2'])
    for r_, kind in enumerate(KINDS):
        y = 124 + r_ * 44
        sh.label(24, y + 18, kind, col=t['ink1'], size=12, weight=500)
        for i, st in enumerate(STATES):
            lab = labels[kind.lower()]
            if st == 'Busy':
                lab = {'primary': 'Exporting', 'secondary': 'Waiting', 'destructive': 'Deleting', 'quiet': 'Loading'}[kind.lower()]
            svg, _ = button(t, s, P(150 + i * 150), P(y), lab, kind.lower(), st.lower().replace('focused', 'focus'))
            sh.add(svg)
    y = 310
    sh.label(24, y + 14, 'Compact', col=t['ink1'], size=12, weight=500)
    xx = 150
    for kind in ('primary', 'secondary', 'destructive', 'quiet'):
        svg, w = button(t, s, P(xx), P(y), labels[kind], kind, 'rest', 24, 64)
        sh.add(svg)
        xx += w / s + 12
    sh.label(xx + 20, y + 15, 'One primary per dialog, at the right end; Enter presses it, Esc presses Cancel.', size=11)
    sh.label(xx + 20, y + 30, 'A destructive action is never the default: the focus starts on Cancel.', size=11)

    # two dialogs on a scrim
    y0 = 370
    sh.head(24, y0, 'Dialogs', 'a scrim over the window; the frame raised, corners 8 (12); the footer a step darker')
    wy = y0 + 18
    sh.add(rect(P(24), P(wy), P(1232), P(360), t['gutter']))
    for k in range(6):
        sh.add(rect(P(24), P(wy + k * 31), P(180), P(30), t['surface']))
        sh.add(rect(P(206), P(wy + k * 31), P(1050), P(30), t['base']))
        sh.add(rect(P(300 + k * 90), P(wy + 6 + k * 31), P(160), P(18), mix(t['base'], ['#e8794f', '#e6a54a', '#d2c25c', '#9a80ee', '#3fc0c8', '#e06aa8'][k], 0.3), 3 * s))
    sh.add(rect(P(24), P(wy + 190), P(1232), P(170), t['surface']))
    sh.add(rect(P(24), P(wy), P(1232), P(360), t['scrim']))
    confirm(sh, t, s, 110, wy + 40)
    rename(sh, t, s, 690, wy + 30)
    return sh


def frame_(sh, t, s, x, y, w, h, title, foot_h=56):
    P = sh.P
    X, Y, Wd, Hd = P(x), P(y), P(w), P(h)
    r = 8 if s == 1 else 12
    edge = mix(t['raised'], t['ink1'], 0.10) if t['name'] == 'Night' else t['line']
    out = (f'<rect x="{X}" y="{Y}" width="{Wd}" height="{Hd}" rx="{r}" fill="{t["raised"]}" filter="url(#dlg-{t["name"]})"/>')
    fy = Y + Hd - P(foot_h)
    out += (f'<clipPath id="dc-{x}"><rect x="{X}" y="{Y}" width="{Wd}" height="{Hd}" rx="{r}"/></clipPath>'
            f'<rect x="{X}" y="{fy}" width="{Wd}" height="{P(foot_h)}" fill="{mix(t["raised"], t["surface"], 0.65)}" clip-path="url(#dc-{x})"/>')
    out += rect(X, fy, Wd, 1, mix(t['raised'], t['edge'], 0.4))
    out += frame(X, Y, Wd, Hd, edge, r)
    out += text(X + P(20), Y + P(31), title, 15 * s, t['ink1'], 500)
    # the close key: an x in a 24 (36) px key, quiet until hovered
    cx, cy = X + Wd - P(24), Y + P(26)
    k = 4 * s
    out += line(cx - k, cy - k, cx + k, cy + k, t['ink2'], 1.5 * s if s == 1 else 2)
    out += line(cx - k, cy + k, cx + k, cy - k, t['ink2'], 1.5 * s if s == 1 else 2)
    sh.add(out)
    return fy


def confirm(sh, t, s, x, y):
    P = sh.P
    w, h = 440, 212
    fy = frame_(sh, t, s, x, y, w, h, 'Delete 3 tracks?')
    sh.add(text(P(x + 20), P(y + 62), 'Kick, Clap and Hats go, with their 14 clips and their mixer strips.', 12 * s, t['ink2']))
    sh.add(text(P(x + 20), P(y + 80), 'Ctrl+Z brings them back.', 12 * s, t['ink2']))
    sh.add(checkbox(t, s, P(x + 20), P(y + 104), False))
    sh.add(text(P(x + 42), P(y + 116), 'Don\'t ask again for tracks', 12 * s, t['ink1']))
    b2, w2 = button(t, s, 0, 0, 'Delete tracks', 'destructive')
    b1, w1 = button(t, s, 0, 0, 'Cancel', 'secondary', 'focus')
    bx2 = P(x + w - 20) - w2
    bx1 = bx2 - P(10) - w1
    by = fy + P(14)
    sh.add(button(t, s, bx1, by, 'Cancel', 'secondary', 'focus')[0])
    sh.add(button(t, s, bx2, by, 'Delete tracks', 'destructive')[0])
    kc, _ = keycap(t, s, P(x + 20), by + P(7), 'Esc')
    sh.add(kc)
    sh.add(text(P(x + 52), by + P(18), 'cancels', 11 * s, t['ink3']))
    sh.label(x, y + h + 22, 'Destructive: the focus starts on Cancel; the red button says what it deletes.', size=11, col=t['ink2'])


def rename(sh, t, s, x, y):
    P = sh.P
    w, h = 420, 218
    fy = frame_(sh, t, s, x, y, w, h, 'Track settings')
    sh.add(text(P(x + 20), P(y + 66), 'Name', 11 * s, t['ink3']))
    sh.add(textfield(t, s, P(x + 90), P(y + 50), P(310), 'Lead synth', 'edit'))
    sh.add(text(P(x + 20), P(y + 106), 'Colour', 11 * s, t['ink3']))
    hues = ['#e8794f', '#e6a54a', '#d2c25c', '#5fc584', '#3fc0c8', '#6c93ea', '#9a80ee', '#e06aa8', '#e0606f', '#9aa6b2']
    for i, c in enumerate(hues):
        cx = P(x + 90 + i * 26)
        sh.add(rect(cx, P(y + 94), P(18), P(18), c, 3 * s))
        if i == 7:
            sh.add(frame(cx - 3 * s, P(y + 94) - 3 * s, P(18) + 6 * s, P(18) + 6 * s, t['ink1'], 5 * s, 1.5 * s if s == 1 else 2))
    sh.add(text(P(x + 20), P(y + 146), 'Options', 11 * s, t['ink3']))
    sh.add(checkbox(t, s, P(x + 90), P(y + 135), True))
    sh.add(text(P(x + 112), P(y + 146), 'Show in the mixer', 12 * s, t['ink1']))
    sh.add(checkbox(t, s, P(x + 250), P(y + 135), False))
    sh.add(text(P(x + 272), P(y + 146), 'Freeze on save', 12 * s, t['ink1']))
    b2, w2 = button(t, s, 0, 0, 'Save', 'primary')
    b1, w1 = button(t, s, 0, 0, 'Cancel', 'secondary')
    bx2 = P(x + w - 20) - w2
    bx1 = bx2 - P(10) - w1
    by = fy + P(14)
    sh.add(button(t, s, bx1, by, 'Cancel', 'secondary')[0])
    sh.add(button(t, s, bx2, by, 'Save', 'primary')[0])
    sh.add(button(t, s, P(x + 14), by, 'Reset', 'quiet', 'rest', 28, 60)[0])
    sh.label(x, y + h + 22, 'A form: labels in a 70 (105) px column, ink 3; fields 24 (36) px; rows 40 (60) px apart.', size=11, col=t['ink2'])
