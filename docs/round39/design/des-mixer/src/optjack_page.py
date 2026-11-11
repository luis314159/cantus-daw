"""des-mixer — style D "Jack" in Slate: every state on the busy desk, the cloth-cord vintage
look in one panel, the plug's anatomy at x3.

python -I optjack_page.py  ->  ../build/jack-{night,paper}.html (render at 1.25)
"""
import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import optjack as J  # noqa: E402
import optcab as OC  # noqa: E402
import optcab_page as CP  # noqa: E402
import optjack_vintage as V  # noqa: E402
from base import NIGHT, PAPER, defs, rect, text, mix, tw, page  # noqa: E402
from sheets import heading  # noqa: E402
from insp_parts import toggle  # noqa: E402

W, HT = 1480, 960
CW, CH = 470, 240
LVL = CP.LVL


def settle(s0, t_ms):
    """The drop's settle: the sag overshoots and rings down; exact and still after 300 ms."""
    if t_ms >= 300:
        return s0
    return s0 * (1 + 0.12 * math.exp(-t_ms / 90) * math.cos(2 * math.pi * t_ms / 160))


def cell(t, px, py, state, show_all):
    T = J.tok(t)
    gid = f'jm-{t["name"]}'
    sel = {'drag_ok': 'Chords', 'drag_bad': 'Drum bus', 'settle': 'Chords'}.get(state, 'Kick')
    out = [rect(px, py, CW, CH, t['base'], 8)]
    x0 = px + 12
    foot = py + 34 + OC.SHOW
    body, pos, ys, xend = CP.desk(t, x0, foot, sel)
    clip = f'jc-{state}-{t["name"]}'
    out.append(f'<clipPath id="{clip}"><rect x="{px}" y="{py + 34}" width="{CW}" height="{CH - 34}" rx="8"/></clipPath>')
    g = [body, rect(x0, foot + 1, xend - x0, CH - 34 - OC.SHOW - 8, T['channel'], 4)]
    yb = foot + J.PLUG_LEN
    col = {'out': T['out'], 'send': T['send'], 'key': T['key']}
    chain = {('Kick', 'Drum bus'), ('Drum bus', 'Master')}
    bands = {}
    cables = []
    for a, b, kind, db in OC.ROUTES:
        mine = a == sel or b == sel
        if not show_all and not mine and state != 'hover':
            continue
        if state == 'settle' and (a, b) == ('Chords', 'Drum bus'):
            continue
        op = 1.0
        if state == 'hover':
            op = 1.0 if (a, b) in chain else 0.25
        elif show_all and not mine:
            op = 0.5
        sx, tx = pos[a][1], pos[b][0]
        d = J.cable_d(sx, tx, yb, J.sag_for(abs(tx - sx)))
        gl = (LVL[(a, b)], hash((a, b)) % 50) if state == 'play' else None
        cables.append(J.cable(t, d, col[kind], J.width(kind, db), key=kind == 'key', op=op,
                              lift=state == 'hover' and (a, b) in chain, gleam=gl))
        rank = ['out', 'send', 'key'].index(kind)
        for k in ((a, 'out'), (b, 'in')):
            if k not in bands or rank < bands[k][0]:
                bands[k] = (rank, col[kind], op)
    extra = []
    if state == 'settle':      # Chords → Drum bus, just dropped: the sag at 0, 80, 160 ms, then still at 300
        sx, tx = pos['Chords'][1], pos['Drum bus'][0]
        s0 = J.sag_for(abs(tx - sx))
        for t_ms, op in ((0, 0.22), (80, 0.32), (160, 0.45)):
            cables.append(J.cable(t, J.cable_d(sx, tx, yb, settle(s0, t_ms)), T['out'], 3.2, op=op))
        cables.append(J.cable(t, J.cable_d(sx, tx, yb, s0), T['out'], 3.2))
        bands[('Chords', 'out')] = (0, T['out'], 1)
        bands[('Drum bus', 'in')] = (0, T['out'], 1)
        extra.append(text(px + 14, py + CH - 12, 'faint: the sag at t = 0, 80, 160 ms · solid: still from 300 ms', 10, t['ink3']))
    for nm, (ix, ox, cx) in pos.items():
        for side, jx in (('in', ix), ('out', ox)):
            b = bands.get((nm, side))
            halo = T['out'] if (state == 'drag_ok' and (nm, side) == ('Drum bus', 'in')) else None
            bad = state == 'drag_bad' and (nm, side) == ('Kick', 'in')
            g.append(J.jack(t, jx, foot, empty=b is None, halo=halo, bad=bad))
    g.extend(cables)
    for (nm, side), (rank, c, op) in bands.items():
        jx = pos[nm][0] if side == 'in' else pos[nm][1]
        p = J.plug_seated(t, jx, foot, c, gid)
        g.append(p if op >= 1 else f'<g opacity="{max(op, 0.6)}">{p}</g>')
    if state in ('drag_ok', 'drag_bad'):
        src, dst = ('Chords', 'Drum bus') if state == 'drag_ok' else ('Drum bus', 'Kick')
        c = T['out']
        fx, fy = pos[dst][0] + (4 if state == 'drag_ok' else 6), foot + 5
        g.append(J.cable(t, J.free_d(pos[src][1], yb, fx, fy + 36), c, 3.2, lift=True))
        g.append(J.plug_free(t, fx, fy, c, gid, T['refused'] if state == 'drag_bad' else None))
        g.append(OC.pointer(t, fx + 5, fy + 24))
        if state == 'drag_ok':
            extra.append(OC.tip(t, fx + 60, foot - 42, 'Route Chords to Drum bus'))
        else:
            extra.append(OC.tip(t, fx + 110, foot - 42, 'Refused: Kick already feeds Drum bus (a loop)', T['refused']))
    if state == 'hover':
        mx = (pos['Kick'][1] + pos['Drum bus'][0]) / 2
        g.append(OC.pointer(t, mx + 4, yb + 34))
        extra.append(OC.tip(t, mx + 60, foot - 42, 'Kick → Drum bus → Master'))
    out.append(f'<g clip-path="url(#{clip})">' + ''.join(g) + '</g>')
    out.extend(extra)
    names = {'sel': 'Kick selected · Show all off', 'all': 'Show all on (Kick selected)',
             'hover': 'Hover: the route lifts and lights', 'drag_ok': 'Drag: the plug follows, a valid jack',
             'drag_bad': 'Drag refused: a loop, the boot turns red', 'settle': 'Dropped: the cable settles (300 ms)',
             'play': 'Playing: the gleam runs along the cords'}
    out.append(text(px + 12, py + 21, names[state], 12, t['ink1'], 500))
    out.append(text(px + CW - 44, py + 21, 'Show all', 10, t['ink3'], 400, 'end'))
    out.append(toggle(t, px + CW - 38, py + 10, show_all))
    return ''.join(out)


def anatomy(t, px, py, w, h):
    T = J.tok(t)
    gid = f'jm-{t["name"]}'
    out = [rect(px, py, w, h, t['base'], 8), text(px + 12, py + 21, 'The plug, ×3 (drawn by the kit)', 12, t['ink1'], 500)]
    ax = px + 30
    out.append(f'<g transform="translate({ax},{py + 40}) scale(3)">' + J.jack(t, 10, 0, empty=False) +
               J.plug_seated(t, 10, 0, T['out'], gid) + J.cable(t, 'M10 22Q10 32 20 40', T['out'], 3.2) + '</g>')
    out.append(f'<g transform="translate({ax + 70},{py + 40}) scale(3)">' + J.plug_free(t, 10, 0, T['out'], gid) + '</g>')
    lx = ax + 140
    rows = [(1, 'tip'), (5, 'ring'), (11, 'sleeve'), (19, 'barrel'), (26.7, 'kind band'), (31, 'boot')]
    last = -99
    for yy, lab in rows:
        y = py + 40 + yy * 3
        ty = max(y, last + 13)      # labels never collide; the leader bends to its place
        last = ty
        out.append(f'<path d="M{ax + 70 + 36} {y}H{lx - 6}L{lx} {ty}" fill="none" stroke="{t["ink3"]}" stroke-width="0.6"/>')
        out.append(text(lx + 4, ty + 3.5, lab, 10, t['ink2']))
    out.append(text(px + 12, py + h - 66, 'Seated (left): nut, collar, barrel, band, boot hang below', 10, t['ink3']))
    out.append(text(px + 12, py + h - 52, 'the strip; free (right): tip, ring, sleeve, as dragged.', 10, t['ink3']))
    for k, (lab, c, key) in enumerate((('out', T['out'], False), ('send', T['send'], False), ('key', T['key'], True))):
        x = px + 12 + k * 108
        out.append(J.cable(t, f'M{x} {py + h - 30}Q{x + 30} {py + h - 18} {x + 60} {py + h - 30}', c, J.width(lab, -12), key=key))
        out.append(text(x + 66, py + h - 24, lab, 10, t['ink2']))
    return ''.join(out)


def sheet(t):
    gid = f'jm-{t["name"]}'
    out = [defs(t), '<defs>' + J.metal_grad(t, gid) + '</defs>', rect(0, 0, W, HT, t['gutter']),
           heading(t, 24, 36, 'Cable style D — "Jack": real 1/4" patch cables, selectable in every theme',
                   'Slate · drawn at 125 % · the same busy desk as options-cables · the vintage theme will make it its default · 4× crops under the sheet')]
    y = 70
    states = [('sel', False), ('all', True), ('hover', True), ('drag_ok', False), ('drag_bad', False), ('settle', False)]
    for k, (st, sa) in enumerate(states):
        out.append(cell(t, 16 + (k % 3) * (CW + 16), y + (k // 3) * (CH + 12), st, sa))
    y3 = y + 2 * (CH + 12)
    out.append(cell(t, 16, y3, 'play', True))
    # the cloth-cord vintage look, one panel (the vintage theme's materials)
    vx = 16 + CW + 16
    out.append(rect(vx, y3, 590, 330, t['base'], 8))
    out.append(text(vx + 12, y3 + 21, 'The vintage look: cloth cords, nickel plugs, bakelite boots', 12, t['ink1'], 500))
    out.append(f'<g transform="translate({vx - 10},{y3 - 44})">' + V.panel(t) + '</g>')
    out.append(anatomy(t, vx + 606, y3, W - (vx + 606) - 16, 330))
    return f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{HT}" viewBox="0 0 {W} {HT}">{"".join(out)}</svg>'


if __name__ == '__main__':
    d = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'build')
    for t in (NIGHT, PAPER):
        with open(os.path.join(d, f'jack-{t["name"].lower()}.html'), 'w', encoding='utf-8') as f:
            f.write(page(sheet(t), W, HT, t['gutter']))
    print('ok')
