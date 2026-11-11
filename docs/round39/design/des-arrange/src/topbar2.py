"""Slate (dir-a), phase B — the top bar as groups that fold.

Each group has forms (full, compact, …) and a measured width; `layout()` packs the left
cluster from the left edge and the right cluster from the right edge, and while they do
not fit it takes the next step of FOLD — a group goes compact or folds into the overflow
panel. The bar is 44 high; every key 24 high at y 10; gaps 10; edges 12.
"""
import glyphs as G
from base import rect, text, well, mix, tw, lit_edge
from topbar import key, joined, segmented, meter_h, logo_at

BAR_H, KY, KH, GAP, EDGE = 44, 10, 24, 10, 12
MENUS = ('File', 'Edit', 'Clip', 'View', 'Options', 'Help')
LEFT = ['brand', 'mode', 'transport', 'metronome', 'tempo', 'clock', 'snap', 'undo', 'windows']
RIGHT = ['perf', 'inputs', 'meter', 'device', 'tools', 'gear', 'more']
FOLD = [('tools', None), ('inputs', None), ('perf', None), ('windows', None), ('device', 'compact'),
        ('metronome', None), ('meter', 'compact'), ('tempo', 'compact'), ('clock', 'compact'),
        ('brand', 'compact'), ('mode', None), ('snap', 'compact'), ('brand', 'minimal'),
        ('undo', None), ('meter', None)]
NAMES = {'brand': 'Menus and project', 'mode': 'Pattern / song', 'transport': 'Transport',
         'metronome': 'Metronome', 'tempo': 'Tempo and metre', 'clock': 'Position',
         'snap': 'Grid and snap', 'undo': 'Undo and redo', 'windows': 'Windows',
         'perf': 'CPU, RAM, voices, dropouts', 'inputs': 'Inputs', 'meter': 'Master meter',
         'device': 'Audio device', 'tools': 'Tools and zoom', 'gear': 'Settings', 'more': 'More'}


def menus_w():
    return sum(tw(m, 12) + 13 for m in MENUS) - 13


def width(g, form, st):
    if g == 'brand':
        if form == 'minimal':
            return 24 + 8 + 28 + 8 + tw('Neon Tide', 13, 500)
        name = tw('Neon Tide', 13, 500) + (8 + tw('9 tracks · 64 bars · 2:03', 11) if form == 'full' else 0)
        return 34 + max(menus_w(), name)
    if g == 'clock':
        big = tw(st.get('pos', ('41', '1'))[0] + '.' + st.get('pos', ('41', '1'))[1], 30, 200) + 1 + tw('.000', 15, 300)
        return big + (12 + tw('bar · beat', 9) + 6 if form == 'full' else 0)
    return {('mode', 'full'): 74, ('transport', 'full'): 160, ('metronome', 'full'): 46,
            ('tempo', 'full'): 116, ('tempo', 'compact'): 78, ('snap', 'full'): 80,
            ('snap', 'compact'): 40, ('undo', 'full'): 56, ('windows', 'full'): 196,
            ('perf', 'full'): 142, ('inputs', 'full'): 112, ('meter', 'full'): 132,
            ('meter', 'compact'): 92, ('device', 'full'): 104, ('device', 'compact'): 30,
            ('tools', 'full'): 84, ('gear', 'full'): 28, ('more', 'full'): 30}[(g, form)]


def layout(W, st):
    forms = {g: 'full' for g in LEFT + RIGHT}
    step = 0

    def total():
        gs = [g for g in LEFT + RIGHT if forms[g]]
        return 2 * EDGE + sum(width(g, forms[g], st) for g in gs) + GAP * (len(gs) - 2) + 24
    while total() > W and step < len(FOLD):
        g, f = FOLD[step]
        forms[g] = f
        step += 1
    return forms


def draw_group(t, g, form, x, st):
    y, h = KY, KH
    cy = y + h / 2 - 0.5
    i1, i2, i3 = t['ink1'], t['ink2'], t['ink3']
    out = []
    if g == 'brand':
        out.append(logo_at(x, y, 24))
        if form == 'minimal':
            out.append(key(t, x + 32, y, 28, h))
            out.append(G.nat(G.menu_lines, x + 46, cy + 0.5, 20, i2, i3))
            out.append(text(x + 68, 27, 'Neon Tide', 13, i1, 500))
            return ''.join(out)
        mx = x + 34
        for m in MENUS:
            out.append(text(mx, 19, m, 12, i2))
            mx += tw(m, 12) + 13
        out.append(text(x + 34, 36, 'Neon Tide', 13, i1, 500))
        if form == 'full':
            out.append(text(x + 34 + tw('Neon Tide', 13, 500) + 8, 36, '9 tracks · 64 bars · 2:03', 11, i3))
    elif g == 'mode':
        out.append(segmented(t, x, y, ['Pat', 'Song'], [34, 40], h, 0 if st.get('pattern') else 1))
    elif g == 'transport':
        tr = st.get('transport', 'stopped')
        ws = [30, 32, 30, 30]
        out.append(joined(t, x, y, ws, h))
        out.append(G.nat(G.to_start, x + 15, cy, 24, i2, i3))
        if tr == 'playing' or tr == 'recording':
            out.append(rect(x + 31, y + 2, 30, h - 4, mix(t['well'], t['play'], 0.18), 3))
            out.append(G.nat(G.play_hinted, x + 46, cy, 24, t['play'], mix(t['play'], t['well'], 0.35)))
        else:
            out.append(G.nat(G.play_hinted, x + 46, cy, 24, i1, i2))
        out.append(G.nat(G.stop, x + 77, cy, 24, i2, i3))
        if tr in ('recording', 'countin'):
            out.append(rect(x + 93, y + 2, 28, h - 4, mix(t['well'], t['rec'], 0.22), 3))
            out.append(G.rec(x + 107, cy + 0.5, 24, t['rec'], True, t['rec']))
            if tr == 'countin':
                out.append(rect(x + 114, y - 4, 13, 12, t['rec'], 6))
                out.append(text(x + 120.5, y + 5, '2', 9, '#ffffff', 600, 'middle'))
        else:
            out.append(G.rec(x + 107, cy + 0.5, 24, mix(t['well'], t['rec'], 0.82)))
        lx = x + 130
        lon = st.get('loop', True)
        out.append(key(t, lx, y, 30, h, on=lon))
        out.append(G.nat(G.loop_hinted, lx + 15, cy + 0.5, 24, t['accent'] if lon else i2, i2))
    elif g == 'metronome':
        on = st.get('metronome', True)
        out.append(joined(t, x, y, [30, 16], h))
        if on:
            out.append(rect(x + 2, y + 2, 27, h - 4, mix(t['well'], t['accent'], 0.20), 3))
        out.append(G.nat(G.metronome, x + 15, cy + 0.5, 24, t['accent'] if on else i2, mix(t['accent'], i1, 0.35) if on else i3))
        out.append(G.chevron_down(x + 38, cy + 0.5, 2.5, i3, 1.2))
    elif g == 'tempo':
        ed = st.get('tempo_edit')
        if ed:
            out.append(rect(x - 2, y - 2, 82, h + 4, mix(t['surface'], t['accent'], 0.45), 6))
        out.append(well(t, x, y, 78, h))
        out.append(text(x + 9, y + 17, '124', 15, i1))
        out.append(text(x + 9 + tw('124', 15), y + 17, '.000', 11, i3 if not ed else i1))
        if ed:
            out.append(rect(round(x + 10 + tw('124.000', 15) - 4), y + 5, 1, 14, t['accent']))
        out.append(text(x + 71, y + 16, 'TAP' if st.get('tapping') else 'BPM', 9, t['accent'] if st.get('tapping') else i3, 600, 'end', track=0.6))
        if form == 'full':
            out.append(well(t, x + 82, y, 34, h))
            out.append(text(x + 99, y + 16.5, '4/4', 12, i2, 400, 'middle'))
    elif g == 'clock':
        pos = st.get('pos', ('41', '1', '000'))
        big = f'{pos[0]}.{pos[1]}'
        col = i1 if st.get('transport') != 'recording' else t['rec']
        out.append(text(x, 34, big, 30, col, 200))
        bw = tw(big, 30, 200) + 1
        out.append(text(x + bw, 34, f'.{pos[2]}', 15, i3, 300))
        if form == 'full':
            rx = x + bw + tw('.000', 15, 300) + 12
            out.append(text(rx, 20, st.get('time', '1:17.42'), 12, i2))
            out.append(text(rx, 33, 'bar · beat', 9, i3, 500, track=0.3))
    elif g == 'snap':
        on = st.get('snap', True)
        out.append(joined(t, x, y, [28, 52] if form == 'full' else [28, 12], h))
        if on:
            out.append(rect(x + 2, y + 2, 25, h - 4, mix(t['well'], t['accent'], 0.20), 3))
        out.append(G.nat(G.magnet, x + 14, cy + 0.5, 24, t['accent'] if on else i2, mix(t['accent'], i1, 0.55) if on else i3))
        if form == 'full':
            out.append(text(x + 36, y + 16.5, '1/16', 12, i1))
            out.append(G.chevron_down(x + 70, cy + 0.5, 2.5, i3, 1.2))
        else:
            out.append(G.chevron_down(x + 33, cy + 0.5, 2, i3, 1.1))
    elif g == 'undo':
        out.append(joined(t, x, y, [28, 28], h))
        out.append(G.nat(G.undo, x + 14, cy + 0.5, 24, i2, i2))
        out.append(G.nat(G.redo, x + 42, cy + 0.5, 24, t['ink4'], t['ink4']))
    elif g == 'windows':
        opened = st.get('open', {'arrange', 'mixer', 'inspector'})
        names = ['browser', 'arrange', 'rack', 'piano', 'plugin', 'mixer', 'inspector']
        out.append(joined(t, x, y, [28] * 7, h))
        for k, n in enumerate(names):
            on = n in opened
            if on:
                out.append(rect(x + k * 28 + 2, y + 2, 24, h - 4, mix(t['well'], t['accent'], 0.16), 3))
            fn = getattr(G, 'w_' + n)
            out.append(G.nat(fn, x + k * 28 + 14, cy + 0.5, 20, t['accent'] if on else i2, mix(t['accent'], i1, 0.4) if on else i3))
    elif g == 'perf':
        out.append(perf_cell(t, x, y, st))
    elif g == 'inputs':
        out.append(inputs_cell(t, x, y, st))
    elif g == 'meter':
        lv = st.get('master')
        mw = 132 if form == 'full' else 92
        out.append(well(t, x, y, mw, h))
        if form == 'full':
            lab = 'LUFS' if st.get('lufs') else 'Peak'
            out.append(text(x + 8, y + 16, lab, 10, i3, 500))
        mx0 = x + (34 if form == 'full' else 8)
        out.append(meter_h(t, mx0, y + 6, mw - (44 if form == 'full' else 16), lv))
        if st.get('clipping'):
            out.append(rect(x + mw - 7, y + 5, 3, 9, t['clip'], 1))
    elif g == 'device':
        bad = st.get('no_device')
        if form == 'compact':
            out.append(key(t, x, y, 30, h))
            out.append(G.nat(G.speaker, x + 15, cy + 0.5, 20, i2, i3))
            if bad:
                out.append(G.warn(x + 24, y + 6, 7, t['solo'], t['well']))
        else:
            out.append(well(t, x, y, 104, h))
            if bad:
                out.append(G.warn(x + 13, cy + 0.5, 10, t['solo'], t['well']))
                out.append(text(x + 25, y + 16.5, 'No device', 11, t['solo'], 500))
            else:
                out.append(G.nat(G.speaker, x + 13, cy + 0.5, 20, i2, i3))
                out.append(text(x + 25, y + 16.5, 'Realtek Audio', 11, i2))
            out.append(G.chevron_down(x + 94, cy + 0.5, 2.5, i3, 1.2))
    elif g == 'tools':
        out.append(key(t, x, y, 28, h, on=st.get('picking', False)))
        out.append(G.nat(G.picker, x + 14, cy + 0.5, 20, t['accent'] if st.get('picking') else i2, i3))
        out.append(well(t, x + 32, y, 52, h))
        out.append(text(x + 58, y + 16.5, '100 %', 11, i2, 400, 'middle'))
    elif g == 'gear':
        out.append(key(t, x, y, 28, h))
        out.append(G.nat(G.gear, x + 14, cy + 0.5, 24, i2, i3))
    elif g == 'more':
        op = st.get('overflow_open')
        out.append(key(t, x, y, 30, h, pressed=op))
        out.append(G.nat(G.more, x + 15, cy + 0.5, 24, i1 if op else i2, i2))
        n = st.get('folded_n', 0)
        if n:
            out.append(rect(x + 21, y - 3, 13, 11, t['raised'], 5.5))
            out.append(f'<rect x="{x + 21.5}" y="{y - 2.5}" width="12" height="10" rx="5" fill="none" stroke="{t["edge"]}"/>')
            out.append(text(x + 27.5, y + 5.5, str(n), 8.5, i2, 600, 'middle'))
    return ''.join(out)


def perf_cell(t, x, y, st):
    import math
    cpu = st.get('cpu', 7)
    busy = cpu >= 70
    drops = st.get('drops', 0)
    out = well(t, x, y, 142, KH)
    gx, gw = x + 6, 34
    col = t['solo'] if busy else t['ink3']
    pts = []
    for k in range(18):
        v = (cpu / 100) * (0.75 + 0.25 * math.sin(k * 0.9 + 1.3))
        pts.append((gx + gw * k / 17, y + 18 - 12 * min(1, v * 1.4)))
    d = 'M' + 'L'.join(f'{a:.1f} {b:.1f}' for a, b in pts)
    out += f'<path d="{d}L{gx + gw} {y + 18}L{gx} {y + 18}Z" fill="{col}" opacity="0.22"/>'
    out += f'<path d="{d}" fill="none" stroke="{col}" stroke-width="1.1"/>'
    out += text(x + 46, y + 16.5, f'{cpu} %', 12, t['solo'] if busy else t['ink1'], 400)
    out += rect(x + 76, y + 6, 1, 12, t['edge'])
    out += text(x + 84, y + 16.5, '412 MB', 12, t['ink1'])
    if drops:
        out += rect(x + 124, y + 6, 14, 12, t['rec'], 6)
        out += text(x + 131, y + 15.5, str(drops), 9, '#ffffff', 600, 'middle')
    return out


def inputs_cell(t, x, y, st):
    act = st.get('midi_activity', True)
    armed = st.get('armed', 0)
    out = well(t, x, y, 112, KH)
    lamp = t['play'] if act else mix(t['well'], t['ink3'], 0.5)
    if act:
        out += f'<circle cx="{x + 10}" cy="{y + 12}" r="4.5" fill="{t["play"]}" opacity="0.2"/>'
    out += f'<circle cx="{x + 10}" cy="{y + 12}" r="2.5" fill="{lamp}"/>'
    out += text(x + 18, y + 16.5, 'MIDI', 11, t['ink2'])
    out += rect(x + 50, y + 6, 1, 12, t['edge'])
    ac = t['rec'] if armed else mix(t['well'], t['ink3'], 0.5)
    out += f'<circle cx="{x + 60}" cy="{y + 12}" r="3.5" fill="none" stroke="{ac}" stroke-width="1.2"/>'
    if armed:
        out += f'<circle cx="{x + 60}" cy="{y + 12}" r="1.6" fill="{ac}"/>'
    out += text(x + 68, y + 16.5, 'Armed', 11, t['ink2'])
    out += text(x + 104, y + 16.5, str(armed), 11, t['rec'] if armed else t['ink3'], 500 if armed else 400, 'end')
    return out


def bar(t, W, st, y0=0, forms=None):
    """The bar at width W (logical px). Returns (svg, forms, x of the 'more' key)."""
    forms = forms or layout(W, st)
    st = dict(st, folded_n=sum(1 for g in LEFT + RIGHT if forms[g] is None))
    out = [f'<g transform="translate(0 {y0})">', rect(0, 0, W, BAR_H, t['surface']), lit_edge(t, 0, 0, W)]
    x = EDGE
    for g in LEFT:
        if forms[g]:
            out.append(draw_group(t, g, forms[g], x, st))
            x += width(g, forms[g], st) + GAP
    x = W - EDGE
    more_x = None
    for g in reversed(RIGHT):
        if forms[g]:
            w = width(g, forms[g], st)
            x -= w
            if g == 'more':
                more_x = x
            out.append(draw_group(t, g, forms[g], x, st))
            x -= GAP
    out.append('</g>')
    return ''.join(out), forms, more_x
