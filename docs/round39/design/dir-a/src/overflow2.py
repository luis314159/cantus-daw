"""Slate (dir-a), phase B — the overflow panel built from what the bar folded.

Each folded group becomes a section; small folds (pattern/song, the metre, the grid, undo,
the master meter) become one-line rows under "Also here". One column holds up to three
sections; from four on the panel opens a second column (balanced by height), so it never
grows taller than the window. Column 284 wide, padding 16, gutter 24, rows 26.
"""
import glyphs as G
from base import rect, text, well, mix, tw
from topbar import segmented, meter_h
from overflow import keycap, toggle, slider, graph, series
import topbar2 as TB

CW, P, GUT = 284, 16, 24


def sec_title(t, x, y, s, right=None, rk=None):
    out = text(x, y + 10, s, 11, t['ink3'], 600, track=0.2)
    if rk:
        out += keycap(t, x + CW - tw(rk, 9, 500) / 2 - 4, y, rk)
    elif right:
        out += text(x + CW, y + 10, right, 10, t['ink3'], 400, 'end')
    return out


def s_metronome(t, x, y, st):
    on = st.get('metronome', True)
    out = well(t, x, y, 28, 24, 4, fill=mix(t['well'], t['accent'], 0.20) if on else None)
    out += G.nat(G.metronome, x + 14, y + 12, 24, t['accent'] if on else t['ink2'], mix(t['accent'], t['ink1'], 0.35) if on else t['ink3'])
    out += text(x + 38, y + 11, 'Metronome', 12, t['ink1'], 500)
    out += text(x + 38, y + 23, 'clicks while playing and recording', 10, t['ink3'])
    out += keycap(t, x + CW - 26 - 30, y + 5, 'Ctrl M')
    out += toggle(t, x + CW - 26, y + 5, on)
    yy = y + 36
    out += '<g opacity="0.45">' if not on else '<g>'
    rows = [('Count-in', 'seg', (['Off', '1 bar', '2 bars'], [44, 48, 52], 1)),
            ('Level', 'slider', 0.62), ('Sound', 'seg', (['Classic', 'Wood', 'Beep'], [52, 46, 46], 1)),
            ('Accent', 'toggle', True)]
    for lab, kind, v in rows:
        dim = not on
        out += text(x, yy + 15, lab, 11, t['ink2'] if not dim else t['ink3'])
        if kind == 'seg':
            out += segmented(t, x + 68, yy + 2, v[0], v[1], 20, v[2], 11)
            if lab == 'Count-in':
                out += keycap(t, x + CW - 14, yy + 5, 'Alt C')
        elif kind == 'slider':
            out += slider(t, x + 70, yy + 11, 150, v)
            out += text(x + CW, yy + 15, '−6 dB', 11, t['ink1'], 400, 'end')
        else:
            out += toggle(t, x + 68, yy + 5, v)
            out += text(x + 102, yy + 15, 'the first beat of each bar', 11, t['ink3'])
        yy += 26
    out += '</g>'
    return out, yy - y


def s_windows(t, x, y, st):
    out = sec_title(t, x, y, 'Windows')
    yy = y + 18
    keys = [(G.w_browser, 'Alt F8', 'browser'), (G.w_arrange, 'F5', 'arrange'), (G.w_rack, 'F6', 'rack'),
            (G.w_piano, 'F7', 'piano'), (G.w_plugin, 'F8', 'plugin'), (G.w_mixer, 'F9', 'mixer'),
            (G.w_inspector, 'I', 'inspector')]
    opened = st.get('open', {'arrange', 'mixer', 'inspector'})
    pitch = (CW + 6) / 7
    kw = round(pitch - 6)
    hover = st.get('hover_key', 4)
    focus = st.get('focus_key')
    for k, (g, sc, n) in enumerate(keys):
        kx = round(x + k * pitch)
        on = n in opened
        if focus == k:
            out += rect(kx - 2, yy - 2, kw + 4, 34, mix(t['raised'], t['accent'], 0.55), 6)
        out += well(t, kx, yy, kw, 30, 4, fill=mix(t['well'], t['accent'], 0.16) if on else None, hi=(k == hover))
        ink = t['accent'] if on else (t['ink1'] if k == hover else t['ink2'])
        out += G.nat(g, kx + kw / 2, yy + 15, 28, ink, mix(t['accent'], t['ink1'], 0.4) if on else t['ink3'])
        out += keycap(t, kx + kw / 2, yy + 35, sc)
    yy += 54
    hint = st.get('hint', ('Plug-in window', '— the selected track’s instrument'))
    out += text(x, yy + 10, hint[0], 11, t['ink1'], 500)
    out += text(x + tw(hint[0], 11, 500) + 6, yy + 10, hint[1], 11, t['ink3'])
    return out, yy + 16 - y


def s_perf(t, x, y, st):
    out = sec_title(t, x, y, 'Performance', 'last 30 s')
    yy = y + 18
    cw = (CW - 12) / 2
    cpu = st.get('cpu', 7)
    drops = st.get('drops', 0)
    busy = cpu >= 70
    cells = [('CPU', str(cpu), '%', series(1.3, 40, cpu / 100, 0.03 if not busy else 0.08), 'line', t['solo'] if busy else None, 0.25 if not busy else 1.0),
             ('Memory', '412', 'MB', series(0.2, 40, 0.80, 0.015), 'line', None, 1.6),
             ('Voices', '12', 'of 256', series(2.1, 24, 10, 5), 'bars', None, 24),
             ('Dropouts', str(drops), 'since start', [0] * 24, 'bars', t['play'], 1)]
    for k, (lab, v, unit, s, kind, col, top) in enumerate(cells):
        cx = x + (k % 2) * (cw + 12)
        cy = yy + (k // 2) * 52
        out += text(cx, cy + 12, lab, 11, t['ink2'])
        vcol = t['ink1']
        if lab == 'CPU' and busy:
            vcol = t['solo']
        if lab == 'Dropouts' and drops:
            vcol = t['rec']
        out += text(cx + cw, cy + 13, unit, 10, t['ink3'], 400, 'end')
        out += text(cx + cw - tw(unit, 10) - 4, cy + 13, v, 15, vcol, 400, 'end')
        if lab == 'Dropouts':
            out += well(t, cx, cy + 19, cw, 24, 3)
            if drops:
                for j, bx in enumerate((0.08, 0.22, 0.27)):
                    out += rect(round(cx + 4 + (cw - 8) * bx), cy + 23, 2, 16, t['rec'], 1)
                out += text(cx + cw - 6, cy + 35, 'last 0:42 ago', 10, t['ink2'], 400, 'end')
            else:
                out += f'<circle cx="{cx + 11}" cy="{cy + 31}" r="3" fill="{t["play"]}"/>'
                out += text(cx + 19, cy + 35, 'None — keeping time', 10, t['ink2'])
        else:
            out += graph(t, cx, cy + 19, cw, 24, s, kind, col, top)
    return out, 18 + 104


def s_inputs(t, x, y, st):
    out = sec_title(t, x, y, 'Inputs')
    yy = y + 18
    act = st.get('midi_activity', True)
    out += f'<circle cx="{x + 5}" cy="{yy + 11}" r="2.5" fill="{t["play"] if act else t["ink3"]}"/>'
    out += text(x + 14, yy + 15, 'MIDI', 11, t['ink2'])
    out += text(x + 68, yy + 15, 'Arturia KeyStep 37', 12, t['ink1'])
    out += text(x + CW - 8, yy + 15, 'Settings', 11, t['ink2'], 400, 'end') + G.chevron_right(x + CW - 2, yy + 11, 2.5, t['ink3'], 1.2)
    yy += 26
    armed = st.get('armed', 0)
    out += f'<circle cx="{x + 5}" cy="{yy + 11}" r="3.5" fill="none" stroke="{t["rec"] if armed else t["ink3"]}" stroke-width="1.2"/>'
    out += text(x + 14, yy + 15, 'Armed', 11, t['ink2'])
    out += text(x + 68, yy + 15, 'Lead, Choir' if armed else 'No track — click a track’s ring to arm it', 12 if armed else 11, t['ink1'] if armed else t['ink3'])
    return out, yy + 22 - y


def s_tools(t, x, y, st):
    out = sec_title(t, x, y, 'Tools and zoom')
    yy = y + 18
    out += well(t, x, yy, 28, 22, 4)
    out += G.nat(G.picker, x + 14, yy + 11, 20, t['ink2'], t['ink3'])
    out += text(x + 38, yy + 15, 'Pick an element to recolour', 11, t['ink1'])
    out += keycap(t, x + CW - 30, yy + 4, 'Ctrl Shift I')
    yy += 30
    out += text(x, yy + 15, 'Interface zoom', 11, t['ink2'])
    out += segmented(t, x + 100, yy + 2, ['−', '100 %', '+'], [26, 54, 26], 20, 1, 11)
    out += keycap(t, x + CW - 14, yy + 5, 'Ctrl 0')
    return out, yy + 24 - y


def s_also(t, x, y, extras, st):
    """The small folds: one row each, label column and control column."""
    out = sec_title(t, x, y, 'Also here')
    yy = y + 18
    for e in extras:
        lab = {'mode': 'Pattern / song', 'metre': 'Time signature', 'grid': 'Grid', 'undo': 'Undo',
               'meter': 'Master meter'}[e]
        out += text(x, yy + 15, lab, 11, t['ink2'])
        cx = x + 100
        if e == 'mode':
            out += segmented(t, cx, yy + 2, ['Pat', 'Song'], [34, 40], 20, 1, 11)
            out += keycap(t, x + CW - 10, yy + 5, 'L')
        elif e == 'metre':
            out += well(t, cx, yy + 2, 56, 20, 4) + text(cx + 8, yy + 16, '4/4', 12, t['ink1'])
            out += G.chevron_down(cx + 46, yy + 12, 2.5, t['ink3'], 1.2)
        elif e == 'grid':
            out += well(t, cx, yy + 2, 56, 20, 4) + text(cx + 8, yy + 16, '1/16', 12, t['ink1'])
            out += G.chevron_down(cx + 46, yy + 12, 2.5, t['ink3'], 1.2)
        elif e == 'undo':
            out += well(t, cx, yy + 2, 26, 20, 4) + G.nat(G.undo, cx + 13, yy + 12, 20, t['ink2'], t['ink2'])
            out += well(t, cx + 30, yy + 2, 26, 20, 4) + G.nat(G.redo, cx + 43, yy + 12, 20, t['ink4'], t['ink4'])
            out += text(cx + 64, yy + 15, 'Move clip', 11, t['ink3'])
        elif e == 'meter':
            out += well(t, cx, yy + 2, 120, 20, 4) + meter_h(t, cx + 8, yy + 5, 104, st.get('master'))
            out += text(x + CW, yy + 15, 'Peak', 10, t['ink3'], 500, 'end')
        yy += 26
    return out, yy - y


SECTIONS = {'metronome': s_metronome, 'windows': s_windows, 'perf': s_perf, 'inputs': s_inputs,
            'tools': s_tools}


def panel(t, forms, right_x, st, top=40):
    """The panel for the bar's current forms, its right edge at right_x."""
    folded = [g for g in TB.LEFT + TB.RIGHT if forms.get(g, 'full') is None]
    big = [g for g in folded if g in SECTIONS]
    extras = [g for g in ('mode', 'undo', 'meter') if g in folded]
    if forms.get('tempo') == 'compact':
        extras.insert(1, 'metre')
    if forms.get('snap') == 'compact':
        extras.insert(2 if 'metre' in extras else 1, 'grid')
    # measure each section, then fill the columns, shorter first
    secs = []
    for g in big:
        _, h = SECTIONS[g](t, 0, 0, st)
        secs.append((g, h))
    if extras:
        secs.append(('also', 18 + 26 * len(extras)))
    ncol = 1 if len(secs) <= 3 else 2
    cols = [[] for _ in range(ncol)]
    hs = [0] * ncol
    for g, h in secs:
        c = hs.index(min(hs)) if ncol == 2 else 0
        cols[c].append((g, h))
        hs[c] += h + 29
    W = 2 * P + ncol * CW + (ncol - 1) * GUT
    X = max(8, right_x - W)
    body = []
    body.append(text(X + P, top + 26, 'Folded here', 13, t['ink1'], 500))
    n = len(folded)
    body.append(text(X + W - P, top + 26, f'the window is narrow for {n} group{"s" if n != 1 else ""}', 11, t['ink3'], 400, 'end'))
    y0 = top + 46
    for c, items in enumerate(cols):
        x = X + P + c * (CW + GUT)
        y = y0
        for i, (g, h) in enumerate(items):
            if i:
                body.append(rect(x, y - 15, CW, 1, mix(t['raised'], t['ink4'], 0.45)))
            svg, _ = (s_also(t, x, y, extras, st) if g == 'also' else SECTIONS[g](t, x, y, st))
            body.append(svg)
            y += h + 29
    if ncol == 2:
        body.append(rect(X + P + CW + GUT // 2, y0, 1, max(hs) - 29, mix(t['raised'], t['ink4'], 0.45)))
    fy = y0 + max(hs) - 14
    body.append(rect(X, fy, W, 1, mix(t['raised'], t['ink4'], 0.45)))
    body.append(G.nat(G.sliders, X + P + 7, fy + 20, 20, t['ink2'], t['ink3']))
    body.append(text(X + P + 22, fy + 24, 'Customise top bar…', 12, t['ink1']))
    body.append(text(X + W - P, fy + 24, 'choose, order, fold', 11, t['ink3'], 400, 'end'))
    H = fy + 40 - top
    uid = f'{t["name"]}{right_x}{top}'
    edge = mix(t['raised'], t['ink1'], 0.08) if t['name'] == 'Night' else t['line']
    bg = [f'<filter id="of{uid}" x="-20%" y="-20%" width="140%" height="140%"><feDropShadow dx="0" dy="8" stdDeviation="12" flood-color="#000" flood-opacity="{0.5 if t["name"] == "Night" else 0.18}"/></filter>',
          f'<rect x="{X}" y="{top}" width="{W}" height="{H}" rx="8" fill="{t["raised"]}" filter="url(#of{uid})"/>',
          f'<rect x="{X + 0.5}" y="{top + 0.5}" width="{W - 1}" height="{H - 1}" rx="7.5" fill="none" stroke="{edge}"/>']
    kx = right_x - 15
    bg.append(f'<path d="M{kx - 7} {top + 1}L{kx} {top - 6}L{kx + 7} {top + 1}Z" fill="{t["raised"]}"/>')
    return ''.join(bg + body), W, H
