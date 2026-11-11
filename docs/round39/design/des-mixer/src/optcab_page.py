"""des-mixer — the cables sheet: directions A, B, C × states (selected with Show all off,
Show all on, hover, drag to a valid target, drag refused as a loop, and one extra cell).

python -I optcab_page.py  ->  ../build/optcab-{night,paper}.html (render at 1.25)
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import optcab as OC  # noqa: E402
import strip as ST  # noqa: E402
from base import NIGHT, PAPER, defs, rect, text, mix, tw, page, HUES  # noqa: E402
from sheets import song, heading  # noqa: E402
from insp_parts import toggle  # noqa: E402

W, HT = 1480, 1530
CW, CH = 470, 206
LVL = {('Kick', 'Drum bus'): 0.85, ('Kick', 'Reverb'): 0.6, ('Kick', 'Bass'): 0.85, ('Chords', 'Reverb'): 0.5,
       ('Chords', 'Delay'): 0.35, ('Drum bus', 'Master'): 0.9, ('Reverb', 'Master'): 0.45}


def desk(t, x0, foot, sel):
    """The strips' feet; returns positions {name: (in_x, out_x, cx)} and the jack row y."""
    out = []
    pos = {}
    x = x0
    for nm in ['Master'] + OC.NAMES:
        s = song(nm)
        w = ST.MASTER_W['wide'] if s['kind'] == 'master' else 60
        out.append(ST.strip(t, x, foot - H_, H_, s, 'wide', sel=nm == sel))
        cover = mix(t['surface'], HUES[nm], t['head_sel']) if (nm == sel and nm in HUES) else t['surface']
        out.append(rect(x + 1, foot - 19, w - 2, 18, cover))
        cx = x + w / 2
        # the strip's name over the top of its visible foot (the sheet shows only the foot)
        top = foot - OC.SHOW
        out.append(rect(x + 1, top, w - 2, 17, cover) + rect(x, top, w, 2, HUES.get(nm, t['ink3']) if nm == sel else
                   mix(t['surface'], HUES.get(nm, t['ink3']), 0.7)))
        out.append(text(cx, top + 13, nm, 11, t['ink1'], 500 if nm == sel else 400, 'middle'))
        pos[nm] = (cx - 11, cx + 11, cx)
        x += w + 2
    return ''.join(out), pos, foot - 9, x - 2


H_ = OC.H


def cell(t, px, py, style, state, show_all):
    T = OC.tok(t)
    sel = {'drag_ok': 'Chords', 'drag_bad': 'Drum bus'}.get(state, 'Kick')
    out = [rect(px, py, CW, CH, t['base'], 8)]
    x0 = px + 12
    foot = py + 34 + OC.SHOW
    clip = f'cl-{style}-{state}-{show_all}-{t["name"]}'
    body, pos, ys, xend = desk(t, x0, foot, sel)
    out.append(f'<clipPath id="{clip}"><rect x="{px}" y="{py + 34}" width="{CW}" height="{CH - 34}" rx="8"/></clipPath>')
    g = [body, rect(x0, foot + 2, xend - x0, OC.BAND - 4, T['channel'], 4)]
    ln = OC.lanes(OC.ROUTES, pos)
    playing = style == 'B' and state != 'rest'
    hover_chain = {('Kick', 'Drum bus'), ('Drum bus', 'Master')}
    lit_sockets = {}
    for r in OC.ROUTES:
        a, b, kind, db = r
        mine = a == sel or b == sel
        if not show_all and not mine and state != 'hover':
            continue
        op = 1.0
        if state == 'hover':
            op = 1.0 if (a, b) in hover_chain else 0.22
        elif show_all and not mine:
            op = 0.45
        col = T['out'] if kind == 'out' else T['send'] if kind == 'send' else T['key']
        w = OC.width(kind, db) + (0.5 if (state == 'hover' and op == 1.0) else 0)
        d = OC.path_d(style, pos[a][1], pos[b][0], ys, r, ln)
        pulse = (LVL[(a, b)], (hash((a, b)) % 50)) if playing else None
        g.append(OC.cable(t, d, col, w, dash=kind == 'key', op=op, pulse=pulse))
        if op >= 0.45:      # a jack with several cables takes the strongest kind: out, then send, then key
            rank = {'out': 0, 'send': 1, 'key': 2}[kind]
            for jk in ((a, 'out'), (b, 'in')):
                if jk not in lit_sockets or rank < lit_sockets[jk][0]:
                    lit_sockets[jk] = (rank, col)
    # the drag
    extra = []
    if state in ('drag_ok', 'drag_bad'):
        src, dst = ('Chords', 'Drum bus') if state == 'drag_ok' else ('Drum bus', 'Kick')
        col = T['out'] if state == 'drag_ok' else T['refused']
        tx, ty = pos[dst][0] + 3, ys - 3
        d = OC.bez(pos[src][1], tx, ys, ty)
        extra.append(OC.cable(t, d, col, 2.0))
        extra.append(OC.plug(t, tx, ty, col))
        lit_sockets[(src, 'out')] = (-1, col)
        extra.append(OC.pointer(t, tx + 2, ty + 2))
        if state == 'drag_ok':
            msg, mc = 'Route Chords to Drum bus', t['ink1']
        else:
            msg, mc = 'Refused: Kick already feeds Drum bus (a loop)', T['refused']
        extra.append(OC.tip(t, min(max(tx, px + 130), px + CW - 130), ys - 44, msg, mc))
    if state == 'hover':
        mx = (pos['Kick'][1] + pos['Drum bus'][0]) / 2
        extra.append(OC.pointer(t, mx + 6, ys + 26))
        extra.append(OC.tip(t, mx + 40, ys - 40, 'Kick → Drum bus → Master'))
    for nm, (ix, ox, cx) in pos.items():
        bad = state == 'drag_bad' and nm == 'Kick'
        halo = T['out'] if (state == 'drag_ok' and nm == 'Drum bus') else None
        li, lo = lit_sockets.get((nm, 'in')), lit_sockets.get((nm, 'out'))
        g.append(OC.socket(t, ix, ys, 'in', li[1] if li else None, halo, bad))
        g.append(OC.socket(t, ox, ys, 'out', lo[1] if lo else None))
    out.append(f'<g clip-path="url(#{clip})">' + ''.join(g) + '</g>')
    out.extend(extra)
    # the caption: the state, and the Show all switch as the desk header has it
    names = {'sel': 'Kick selected · Show all off', 'all': 'Show all on (Kick selected)', 'hover': 'Hover a cable: its whole route',
             'drag_ok': 'Drag to connect: a valid target', 'drag_bad': 'Drag to connect: refused, it would loop',
             'rest': 'Stopped: nothing moves'}
    if playing and state in ('sel', 'all'):
        names[state] += ' · playing'
    out.append(text(px + 12, py + 21, names[state], 12, t['ink1'], 500))
    out.append(text(px + CW - 44, py + 21, 'Show all', 10, t['ink3'], 400, 'end'))
    out.append(toggle(t, px + CW - 38, py + 10, show_all))
    return ''.join(out)


def legend(t, px, py, style):
    T = OC.tok(t)
    out = [rect(px, py, CW, CH, t['base'], 8), text(px + 12, py + 21, 'The cables, by kind', 12, t['ink1'], 500)]
    rows = [('Output', T['out'], 2.0, False, 'accent · 2 px'), ('Send −6 dB', T['send'], OC.width('send', -6), False, 'send · 1 + 1.6 × level'),
            ('Send −22 dB', T['send'], OC.width('send', -22), False, 'thinner when quieter'),
            ('Sidechain key', T['key'], 1.5, True, 'key · 1.5 px, dash 4/3'), ('Refused', T['refused'], 2.0, False, 'rec · while dragging only')]
    for k, (a, col, w, dash, note) in enumerate(rows):
        y = py + 46 + k * 30
        x = px + 16
        d = (f'M{x} {y}L{x + 60} {y}' if style == 'C' else f'M{x} {y - 6}C{x} {y + 6} {x + 60} {y + 6} {x + 60} {y - 6}')
        out.append(OC.cable(t, d, col, w, dash))
        out.append(text(x + 76, y + 2, a, 11, t['ink2']))
        out.append(text(x + 176, y + 2, note, 10, t['ink3']))
    y = py + 46 + 5 * 30
    out.append(OC.socket(t, px + 22, y, 'in') + text(px + 32, y + 4, 'input jack', 10, t['ink3']))
    out.append(OC.socket(t, px + 112, y, 'out') + text(px + 122, y + 4, 'output jack', 10, t['ink3']))
    out.append(OC.socket(t, px + 212, y, 'in', T['out']) + text(px + 222, y + 4, 'connected', 10, t['ink3']))
    out.append(OC.socket(t, px + 302, y, 'in', None, T['out']) + text(px + 312, y + 4, 'drop here', 10, t['ink3']))
    out.append(OC.socket(t, px + 392, y, 'in', None, None, True) + text(px + 402, y + 4, 'refused', 10, t['ink3']))
    return ''.join(out)


ROWS = [('A', 'A · patch cables', 'one cubic per cable, hanging from its jacks; a darker under-stroke of its own colour; width by kind and level'),
        ('B', 'B · signal flow (recommended)', 'A, plus a gleam travelling along each cable while audio flows — speed and brightness by level; stopped, it is A'),
        ('C', 'C · subway', 'orthogonal routes, corners r 4, a lane per kind in the channel (out, sends, keys), sub-lanes where they overlap')]


def sheet(t):
    out = [defs(t), rect(0, 0, W, HT, t['gutter']),
           heading(t, 24, 36, 'The routing cables — three directions',
                   'Slate · drawn at 125 % · Kick → Drum bus, sends to Reverb and Delay, a sidechain key Kick → Bass, '
                   'Drum bus → Master, a return to the master, a loop refused · 4× crops under the sheet')]
    y = 74
    for style, title, sub in ROWS:
        out.append(text(24, y + 16, title, 15, t['ink1'], 500))
        out.append(text(24 + tw(title, 15, 500) + 14, y + 16, sub, 11, t['ink3']))
        y += 28
        states = [('sel', False), ('all', True), ('hover', True), ('drag_ok', False), ('drag_bad', False)]
        for k, (st, sa) in enumerate(states):
            px = 16 + (k % 3) * (CW + 16)
            py = y + (k // 3) * (CH + 12)
            out.append(cell(t, px, py, style, st, sa))
        px, py = 16 + 2 * (CW + 16), y + CH + 12
        out.append(cell(t, px, py, style, 'rest', True) if style == 'B' else legend(t, px, py, style))
        y += 2 * (CH + 12) + 14
    return f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{HT}" viewBox="0 0 {W} {HT}">{"".join(out)}</svg>'


if __name__ == '__main__':
    d = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'build')
    os.makedirs(d, exist_ok=True)
    for t in (NIGHT, PAPER):
        with open(os.path.join(d, f'optcab-{t["name"].lower()}.html'), 'w', encoding='utf-8') as f:
            f.write(page(sheet(t), W, HT, t['gutter']))
    print('ok')
