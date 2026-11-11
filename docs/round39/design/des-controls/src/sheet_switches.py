"""des-controls — the switches sheet: tabs, segmented, switches, check boxes, radio buttons,
the strip's keys and lamp, scrollbars."""
from slate import Sheet, text, rect, mix, tw, well, frame
from switches import panel_tabs, section_tabs, segmented, switch, checkbox, radio, textkey, lamp

W, H = 1280, 830


def vscroll(t, s, x, y, h, a, b, state='rest'):
    """A vertical scrollbar along a list's right edge (x = the list's right edge)."""
    out = ''
    if state == 'rest':
        w = 4 * s
        out += rect(x - w - 3 * s, y + h * a, w, h * (b - a), mix(t['base'], t['ink3'], 0.55), w / 2)
    else:
        tw_ = 12 * s
        out += rect(x - tw_, y, tw_, h, mix(t['base'], t['well'], 0.8))
        out += rect(x - tw_, y, 1, h, mix(t['base'], t['edge'], 0.5))
        w = 8 * s
        col = mix(t['well'], t['ink2'], 0.55) if state == 'hover' else (t['ink2'] if t['name'] == 'Night' else t['ink3'])
        out += rect(x - tw_ + 2 * s, y + h * a + 2 * s, w, h * (b - a) - 4 * s, col, w / 2)
    return out


def hzoom(t, s, x, y, w, a, b, state='rest', hot=None):
    """The arrangement's scroll-and-zoom bar: drag the body to scroll, an end to zoom."""
    h = 12 * s
    out = well(t, x, y, w, h, h / 2)
    x0, x1 = x + w * a, x + w * b
    body = mix(t['well'], t['ink3'], 0.45) if state == 'rest' else mix(t['well'], t['ink2'], 0.55)
    out += rect(x0, y + 2 * s, x1 - x0, h - 4 * s - 1, body, (h - 4 * s) / 2)
    for k, ex in enumerate((x0, x1)):
        if state == 'rest':
            continue
        lit = hot == k
        gx = ex + (5 * s if k == 0 else -5 * s)
        col = t['accent'] if lit else (t['ink1'] if t['name'] == 'Night' else '#ffffff')
        if lit:
            # the end under the pointer lights as a cap of its own
            out += rect(ex - (0 if k == 0 else 10 * s), y + 2 * s, 10 * s, h - 4 * s - 1, mix(body, t['accent'], 0.45), (h - 4 * s) / 2)
            col = '#ffffff'
        for d in (-1.5 * s, 1.5 * s):
            out += rect(round(gx + d), y + 3.5 * s, 1 if s == 1 else 1.5, h - 7 * s - 1, col)
    return out


def build(t, s):
    sh = Sheet(t, s, W, H, 'Slate — tabs, segments, switches and scrollbars',
               f'des-controls, round 39 · {t["name"]} · {int(s * 100)} % · drawn for this scale')
    P = sh.P
    # tabs
    sh.head(24, 84, 'Panel tabs', 'cut from the panel\'s body; a 2-px accent on top only while the panel has the focus')
    for k, (foc, lab) in enumerate(((True, 'focused panel, "Piano roll" hovered'), (False, 'unfocused panel, 3 more tabs'))):
        y = 102 + k * 64
        sh.add(rect(P(24), P(y), P(560), P(28), t['surface']))
        sh.add(rect(P(24), P(y + 28), P(560), P(20), t['base']))
        sh.add(panel_tabs(t, s, P(24), P(y), ['Mixer', 'Piano roll', 'Channel rack'], 'Mixer',
                          'Piano roll' if foc else None, foc, extra=0 if foc else 3))
        sh.label(600, y + 18, lab)
    sh.head(800, 84, 'Section tabs', 'inside a panel: words, a rule')
    sh.add(section_tabs(t, s, P(800), P(102), ['Track', 'Clip', 'Notes', 'Automation'], 'Track', 'Notes'))
    sh.label(800, 150, 'Active Medium 12 ink 1 over a 2 (3) px ink-1 rule;', size=11)
    sh.label(800, 165, 'hover ink 2 over 1 px ink 4; rest ink 3. 18 (27) px apart.', size=11)

    # segmented
    y = 250
    sh.head(24, y, 'Segmented controls', 'a choice among equals: the chosen segment lifts, neutral, never in colour')
    for k, (h, lab) in enumerate(((24, '24 px'), (20, '20 px'))):
        yy = y + 22 + k * 40
        sh.label(24, yy + 15, lab, col=t['ink2'])
        sh.add(segmented(t, s, P(80), P(yy), ['Compact', 'Wide', 'Large'], 1, h))
        sh.add(segmented(t, s, P(300), P(yy), ['Compact', 'Wide', 'Large'], 1, h, hover=2))
        sh.add(segmented(t, s, P(520), P(yy), ['Compact', 'Wide', 'Large'], 1, h, disabled=True))
    for i, lab in enumerate(('Rest', 'A segment hovered', 'Disabled')):
        sh.label(80 + i * 220, y + 104, lab, size=10)
    # an icon segmented control: the density of rows, drawn as bars
    def bars(n):
        def draw(cx, cy, col):
            o = ''
            hh = 10 * s
            gap = hh / n
            for j in range(n):
                o += rect(round(cx - 6 * s), round(cy - hh / 2 + j * gap + (gap - (1.5 if s == 1 else 2)) / 2),
                          12 * s, 1.5 if s == 1 else 2, col, 0.5)
            return o
        return draw
    sh.add(segmented(t, s, P(760), P(y + 22), [bars(2), bars(3), bars(4)], 0, 24, widths=[P(30)] * 3))
    sh.label(760, y + 104, 'Glyphs: row density', size=10)

    # toggles
    y = 390
    sh.head(24, y, 'Switches, check boxes, radio buttons', 'a thing switched on takes the accent')
    cols = ['Off', 'Off, hover', 'On', 'On, hover', 'Focused', 'Disabled']
    for i, c in enumerate(cols):
        sh.label(140 + i * 90, y + 26, c, size=10)
    rows = [('Switch', lambda x, yy, i: switch(t, s, x, yy, i in (2, 3, 4), ['rest', 'hover', 'rest', 'hover', 'focus', 'disabled'][i])),
            ('Check box', lambda x, yy, i: checkbox(t, s, x, yy, i in (2, 3, 4), ['rest', 'hover', 'rest', 'hover', 'focus', 'disabled'][i])),
            ('Radio', lambda x, yy, i: radio(t, s, x, yy, i in (2, 3, 4), ['rest', 'hover', 'rest', 'hover', 'rest', 'disabled'][i]))]
    for r_, (nm, fn) in enumerate(rows):
        yy = y + 40 + r_ * 34
        sh.label(24, yy + 12, nm, col=t['ink2'])
        for i in range(6):
            sh.add(fn(P(140 + i * 90), P(yy), i))
    sh.label(24, y + 152, 'Mixed (some of a selection):', size=11)
    sh.add(checkbox(t, s, P(200), P(y + 141), 'mixed'))
    sh.add(text(P(222), P(y + 152), 'Snap to clips', 12 * s, t['ink1']))
    sh.add(switch(t, s, P(330), P(y + 140), True))
    sh.add(text(P(366), P(y + 152), 'Follow the playhead', 12 * s, t['ink1']))

    # keys and lamp
    x = 700
    sh.head(x, y, 'The strip\'s keys and lamp')
    for r_, h in enumerate((16, 20)):
        yy = y + 40 + r_ * 40
        sh.label(x, yy + 12, f'{h} px', col=t['ink2'])
        xx = x + 50
        for letter, kind in (('M', 'mute'), ('S', 'solo'), ('R', 'arm')):
            for on in (False, True):
                svg, w = textkey(t, s, P(xx), P(yy), letter, on, kind, h)
                sh.add(svg)
                xx += w / s + 6
            xx += 12
    sh.label(x + 50, y + 124, 'off · on, for Mute, Solo, Arm; muted greys out (it takes sound away)', size=10)
    sh.label(x, y + 152, 'Enable lamp', col=t['ink2'])
    for i, (on, hv, lab) in enumerate(((True, False, 'on'), (False, False, 'bypassed'), (False, True, 'bypassed, hover'))):
        sh.add(lamp(t, s, P(x + 100 + i * 90), P(y + 148), on, hv))
        sh.label(x + 112 + i * 90, y + 152, lab, size=10)

    # scrollbars
    y = 600
    sh.head(24, y, 'Scrollbars', 'over the content, thin at rest; they grow under the pointer')
    for i, (st, lab) in enumerate((('rest', 'Rest: a 4 (6) px thumb'), ('hover', 'Hover: a 12 (18) px lane'), ('drag', 'Dragged'))):
        x = 24 + i * 170
        sh.add(rect(P(x), P(y + 22), P(150), P(150), t['surface'], 4 * s))
        for j in range(7):
            sh.add(rect(P(x + 10), P(y + 34 + j * 20), P(90 - (j * 13) % 40), P(6), mix(t['surface'], t['ink3'], 0.35), 2 * s))
        # the fade at the bottom says there is more
        sh.add(f'<linearGradient id="fade-{i}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{t["surface"]}" stop-opacity="0"/><stop offset="1" stop-color="{t["surface"]}"/></linearGradient>')
        sh.add(rect(P(x), P(y + 146), P(150), P(26), f'url(#fade-{i})'))
        sh.add(vscroll(t, s, P(x + 150), P(y + 22), P(150), 0.12, 0.52, st))
        sh.label(x, y + 190, lab, size=10)
    x = 560
    sh.label(x, y + 30, 'Scroll and zoom (arrangement): drag the body to scroll, an end to zoom', size=11)
    for i, (st, hot, lab) in enumerate((('rest', None, 'Rest'), ('hover', None, 'Hover: the grips show'),
                                        ('hover', 1, 'An end under the pointer'))):
        yy = y + 50 + i * 40
        sh.add(hzoom(t, s, P(x), P(yy), P(460), 0.22, 0.58, st, hot))
        sh.label(x + 480, yy + 10, lab, size=10)
    return sh
