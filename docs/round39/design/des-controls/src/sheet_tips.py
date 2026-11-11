"""des-controls — tooltips and the help line."""
from slate import Sheet, text, rect, mix, tw, frame, path, f, esc
from signs import sign, sign_col
from knobs import knob

W, H = 1280, 760


FACE = "font-family:'Slate Sans';font-variant-numeric:tabular-nums;"


def _sign_svg(t, s, kind):
    h = 7 if s == 1 else 10
    w = round(h * 1.8) + 2
    inner = sign(kind, w / 2, (h + 2) / 2, h, sign_col(t, kind))
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h + 2}" '
            f'style="display:inline-block;vertical-align:-1px;margin-right:{f(5 * s)}px">{inner}</svg>')


def html_run(t, s, parts):
    """A line of runs laid out by the browser, so every gap is exact:
    (text, size, colour, weight) · ('key', label) · ('sign', kind) · ('dot', colour)."""
    out = []
    for p in parts:
        if p[0] == 'key':
            h = 15 * s if s == 1 else 22
            out.append(f'<span style="display:inline-block;box-sizing:border-box;height:{h}px;line-height:{h - 2}px;'
                       f'padding:0 {f(4 * s)}px;border:1px solid {mix(t["raised"], t["ink3"], 0.5)};border-radius:{f(3 * s)}px;'
                       f'font-size:{f(9.5 * s)}px;font-weight:600;color:{t["ink2"]};vertical-align:1px;margin:0 {f(4 * s)}px 0 0">'
                       f'{esc(p[1])}</span>')
        elif p[0] == 'sign':
            out.append(_sign_svg(t, s, p[1]))
        elif p[0] == 'dot':
            out.append(f'<span style="display:inline-block;width:{f(7 * s)}px;height:{f(7 * s)}px;border-radius:50%;'
                       f'background:{p[1]};margin-right:{f(6 * s)}px;vertical-align:0"></span>')
        else:
            s_, sz, col, wt = p
            out.append(f'<span style="font-size:{f(sz * s)}px;color:{col};font-weight:{wt};margin-right:{f(4 * s)}px">{esc(s_)}</span>')
    return ''.join(out)


def fo(x, y, w, h, html):
    return (f'<foreignObject x="{f(x)}" y="{f(y)}" width="{f(w)}" height="{f(h)}">'
            f'<div xmlns="http://www.w3.org/1999/xhtml" style="{FACE}">{html}</div></foreignObject>')


def pointer(t, s, x, y):
    """The arrow cursor, to show where a tip appears (drawn here, not the system's)."""
    k = s
    d = (f'M{f(x)} {f(y)}L{f(x)} {f(y + 15 * k)}L{f(x + 3.8 * k)} {f(y + 11.5 * k)}L{f(x + 6.5 * k)} {f(y + 17 * k)}'
         f'L{f(x + 8.6 * k)} {f(y + 16 * k)}L{f(x + 6 * k)} {f(y + 10.6 * k)}L{f(x + 10.8 * k)} {f(y + 10.6 * k)}Z')
    return f'<path d="{d}" fill="#ffffff" stroke="#000000" stroke-width="{f(1 * k)}" stroke-linejoin="round"/>'


def tip(t, s, x, y, lines, w=None):
    """A tooltip: raised, corners 5 (7), its own small shadow, padding 8 (12) x 10 (15)."""
    edge = mix(t['raised'], t['ink1'], 0.09) if t['name'] == 'Night' else t['line']
    sh_ = '0 3px 10px rgba(0,0,0,%.2f)' % (t['shadow_op'] * 0.9)
    rows = ''.join(f'<div style="white-space:nowrap;line-height:{f((17 if i == 0 else 15) * s)}px">{html_run(t, s, ln)}</div>'
                   for i, ln in enumerate(lines))
    box = (f'<div style="display:inline-block;background:{t["raised"]};border:1px solid {edge};border-radius:{f(5 * s)}px;'
           f'box-shadow:{sh_};padding:{f(7 * s)}px {f(10 * s)}px {f(8 * s)}px">{rows}</div>')
    return fo(x, y, 600 * s, 120 * s, box)


def helpline(t, s, x, y, w, parts, right=None):
    """The help line: the status line's left part, 22 (33) px, never empty."""
    h = 22 * s
    out = rect(x, y, w, h, t['surface'])
    out += rect(x, y, w, 1, mix(t['surface'], t['gutter'], 0.7))
    out += fo(x + 10 * s, y, w - 20 * s, h, f'<div style="white-space:nowrap;line-height:{f(h)}px">{html_run(t, s, parts)}</div>')
    if right:
        out += text(x + w - 10 * s, y + h / 2 + 4.3 * s, right, 11 * s, t['ink3'], 400, 'end')
    return out


def build(t, s):
    sh = Sheet(t, s, W, H, 'Slate — tooltips and the help line',
               f'des-controls, round 39 · {t["name"]} · {int(s * 100)} % · drawn for this scale')
    P = sh.P
    i1, i2, i3 = t['ink1'], t['ink2'], t['ink3']
    sh.head(24, 84, 'Tooltips', 'after 500 ms at rest, 12 (18) px below and right of the pointer; the next one at once')
    # 1 a key
    x, y = 24, 112
    sh.label(x, y, 'A key: its name and shortcut', col=i2)
    sh.add(pointer(t, s, P(x + 30), P(y + 14)))
    sh.add(tip(t, s, P(x + 42), P(y + 36), [[('Loop', 12, i1, 500), ('key', 'L')]]))
    # 2 a control, rich
    x = 270
    sh.label(x, y, 'A control: name, value, what it does, how', col=i2)
    sh.add(knob(t, s, 'M', P(x + 20), P(y + 32), 0.62))
    sh.add(pointer(t, s, P(x + 26), P(y + 36)))
    sh.add(tip(t, s, P(x + 38), P(y + 58), [
        [('Cutoff', 12, i1, 500), ('2.40 kHz', 12, i1, 400)],
        [('Where the filter starts to take the highs away.', 11, i2, 400)],
        [('Drag', 10.5, i3, 400), ('·', 10.5, i3, 400), ('key', 'Shift'), ('fine', 10.5, i3, 400), ('·', 10.5, i3, 400),
         ('Double-click to type', 10.5, i3, 400)]]))
    # 3 automated
    x = 640
    sh.label(x, y, 'Automated: what drives it, how to take it back', col=i2)
    sh.add(knob(t, s, 'M', P(x + 20), P(y + 32), 0.48, drive='auto'))
    sh.add(pointer(t, s, P(x + 26), P(y + 36)))
    sh.add(tip(t, s, P(x + 38), P(y + 58), [
        [('sign', 'auto'), ('Automated by the clip ‘Cutoff’ on Lead', 12, i1, 500)],
        [('While the song plays, the clip moves this knob.', 11, i2, 400)],
        [('Right-click ▸ Clear automation, or edit the clip.', 11, i2, 400)]]))
    # 4 modulated, 5 a name cut short
    y2 = 270
    x = 24
    sh.label(x, y2, 'Modulated', col=i2)
    sh.add(tip(t, s, P(x), P(y2 + 12), [
        [('sign', 'mod'), ('Modulated by LFO 1', 12, i1, 500), ('± 18 %', 12, i1, 400)],
        [('Your value stays 2.40 kHz; the ring shows the swing.', 11, i2, 400)]]))
    x = 400
    sh.label(x, y2, 'A name cut short: the whole of it', col=i2)
    sh.add(tip(t, s, P(x), P(y2 + 12), [[('Choir — Ahh pad, stereo, long release', 12, i1, 400)]]))
    x = 700
    sh.label(x, y2, 'A disabled control says why', col=i2)
    sh.add(tip(t, s, P(x), P(y2 + 12), [
        [('Input monitoring', 12, i1, 500)],
        [('Arm the track to monitor its input.', 11, i2, 400)]]))
    spec_tips(sh, t, s, 24, 360)

    # the help line
    y = 470
    sh.head(24, y, 'The help line', 'the status line\'s left part: what is under the pointer, never empty; 22 (33) px')
    rows = [
        ('Pointer over a knob', [('Cutoff', 11.5, i1, 500), ('2.40 kHz', 11.5, i1, 400), ('—', 11.5, i3, 400),
                                 ('drag to change', 11.5, i2, 400), ('·', 11.5, i3, 400), ('key', 'Shift'), ('fine', 11.5, i2, 400),
                                 ('·', 11.5, i3, 400), ('double-click to type', 11.5, i2, 400), ('·', 11.5, i3, 400),
                                 ('right-click: automation, MIDI learn', 11.5, i2, 400)], '48 kHz · 128 samples · CPU 12 %'),
        ('Dragging', [('Cutoff', 11.5, i1, 500), ('3.10 kHz', 11.5, t['accent'], 400), ('was 2.40', 11.5, i3, 400),
                      ('—', 11.5, i3, 400), ('key', 'Shift'), ('fine', 11.5, i2, 400), ('·', 11.5, i3, 400), ('key', 'Esc'),
                      ('puts it back', 11.5, i2, 400)], None),
        ('An automated knob', [('sign', 'auto'), ('Cutoff is automated by the clip ‘Cutoff’ on Lead', 11.5, i1, 500), ('—', 11.5, i3, 400),
                               ('right-click ▸ Clear automation, or edit the clip', 11.5, i2, 400)], None),
        ('Over nothing', [('Lead', 11.5, i1, 500), ('selected · 4 clips · 2 effects', 11.5, i2, 400), ('—', 11.5, i3, 400),
                          ('key', 'Space'), ('plays from 41.1', 11.5, i2, 400)], 'Saved 2 min ago'),
        ('A warning', [('dot', t['solo']), ('The audio device dropped 2 buffers', 11.5, i1, 500), ('—', 11.5, i3, 400),
                       ('Settings ▸ Audio: a larger buffer stops it', 11.5, i2, 400), ('Open', 11.5, t['accent'], 500)], None),
        ('Spanish, the longest', [('Corte', 11.5, i1, 500), ('2,40 kHz', 11.5, i1, 400), ('—', 11.5, i3, 400),
                                  ('arrastra para cambiar', 11.5, i2, 400), ('·', 11.5, i3, 400), ('key', 'Mayús'), ('fino', 11.5, i2, 400),
                                  ('·', 11.5, i3, 400), ('doble clic para escribir', 11.5, i2, 400), ('·', 11.5, i3, 400),
                                  ('clic derecho: automatización, MIDI', 11.5, i2, 400)], '48 kHz · 128 muestras · CPU 12 %'),
    ]
    for k, (lab, parts, right) in enumerate(rows):
        yy = y + 22 + k * 40
        sh.label(24, yy + 15, lab, col=i2)
        sh.add(helpline(t, s, P(170), P(yy), P(1086), parts, right))
    return sh


def spec_tips(sh, t, s, x, y):
    P = sh.P
    notes = [('sheet', 'raised, corners 5 (7), 1-px edge, shadow 0 3 10; padding 9 (13.5); max width 320 (480), then it wraps'),
             ('lines', 'the name Medium 12 ink 1 and the value Regular 12; what it does Regular 11 ink 2; how, Regular 10.5 ink 3 with keycaps'),
             ('keycap', '15 (22) px, an outline in ink 3 at 50 %, SemiBold 9.5 ink 2, padding 4 (6)'),
             ('driven', 'the sign leads the first line; the words say what drives it and how to take it back — never just "Automated"')]
    for k, (a, b) in enumerate(notes):
        yy = y + k * 22
        sh.add(text(P(x), P(yy), a, 12 * s, t['ink1'], 500))
        sh.add(text(P(x + 70), P(yy), b, 11 * s, t['ink3']))
