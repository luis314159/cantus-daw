"""des-mixer — the jack cables as a reference for the future vintage theme (not Slate):
cloth-covered cords and 1/4" plugs on vintage materials — a black panel (Night) or cream
enamel (Paper), a brushed-metal jack rail, nickel plugs, bakelite boots.

One panel per theme: the busy desk patched (Kick → Drum bus, sends to Reverb and Delay, the
key Kick → Bass, Drum bus → Master, Reverb → Master) and a loop refused while dragging (red
boot), with the plug's anatomy at x3.

python -I optjack_vintage.py  ->  ../build/vjack-{night,paper}.html (render at 1.25)
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import optjack as J  # noqa: E402
from base import NIGHT, PAPER, rect, text, mix, page  # noqa: E402

W, HT = 1000, 400
NAMES = ['Master', 'Kick', 'Bass', 'Chords', 'Drum bus', 'Reverb', 'Delay']
ROUTES = [('Kick', 'Drum bus', 'out', 0), ('Kick', 'Reverb', 'send', -12), ('Kick', 'Bass', 'key', 0),
          ('Chords', 'Reverb', 'send', -16), ('Chords', 'Delay', 'send', -22),
          ('Drum bus', 'Master', 'out', 0), ('Reverb', 'Master', 'out', 0)]


def vtok(t):
    n = t['name'] == 'Night'
    return dict(panel='#17140f' if n else '#ebe2ca', engr='#e6dbc0' if n else '#26221b',
                engr2='#9c927b' if n else '#6f6656', bg='#0f0d0a' if n else '#d9cfb4',
                rail_a='#8d949a', rail_b='#c9ced2', window='#e9dfc3', window_ink='#2a241b',
                out='#38608f', send='#cfc4a6', key='#c8902c', bad='#c0392b',
                boot='#1c1915', boot_hi='#3d352d')


def rail(t, x, y, w):
    """Brushed metal: fine horizontal streaks over a vertical sheen."""
    V = vtok(t)
    out = (f'<defs><linearGradient id="rail-{t["name"]}" x1="0" x2="0" y1="0" y2="1"><stop offset="0" stop-color="{V["rail_b"]}"/>'
           f'<stop offset="0.5" stop-color="{V["rail_a"]}"/><stop offset="1" stop-color="{mix(V["rail_a"], "#000000", 0.25)}"/></linearGradient></defs>')
    out += rect(x, y, w, 14, f'url(#rail-{t["name"]})')
    for k in range(12):
        out += rect(x, y + 1 + k, w, 0.5, '#ffffff' if k % 3 else '#000000', extra=f' opacity="{0.10 if k % 3 else 0.06}"')
    return out + rect(x, y + 13.5, w, 0.5, '#000000', extra=' opacity="0.5"')


def panel(t):
    V = vtok(t)
    gid = f'vmetal-{t["name"]}'
    out = ['<defs>' + J.metal_grad(t, gid) + '</defs>', rect(20, 70, 560, 296, V['bg'], 6)]
    pos = {}
    x = 34
    for nm in NAMES:
        w = 72 if nm == 'Master' else 64
        out.append(rect(x, 80, w, 92, V['panel'], 2))
        out.append(rect(x, 80, w, 1, mix(V['panel'], '#ffffff', 0.12)))
        out.append(text(x + w / 2, 98, nm.upper(), 9, V['engr'], 600, 'middle', track=1.2))
        # the level in a small cream window, like an engraved scale's readout
        out.append(rect(x + w / 2 - 18, 108, 36, 16, V['window'], 2))
        out.append(text(x + w / 2, 120, {'Master': '0.0', 'Kick': '−4.0', 'Bass': '−5.0', 'Chords': '−3.5', 'Drum bus': '−2.0',
                                         'Reverb': '−8.0', 'Delay': '−12.0'}[nm], 10, V['window_ink'], 500, 'middle'))
        out.append(text(x + 14, 158, 'IN', 7, V['engr2'], 600, 'middle', track=1))
        out.append(text(x + w - 14, 158, 'OUT', 7, V['engr2'], 600, 'middle', track=1))
        cx = x + w / 2
        pos[nm] = (cx - 13, cx + 13)
        x += w + 6
    out.append(rail(t, 30, 168, x - 30))
    ys = 175
    col = {'out': V['out'], 'send': V['send'], 'key': V['key']}
    plugged = set()
    cables = []
    for a, b, kind, db in ROUTES:
        sx, tx = pos[a][1], pos[b][0]
        d = J.cable_d(sx, tx, ys + J.PLUG_LEN, J.sag_for(abs(tx - sx)))
        cables.append(J.cable(t, d, col[kind], J.width(kind, db) + 0.6, key=kind == 'key', cloth=True))
        plugged.add((a, 'out', col[kind]))
        plugged.add((b, 'in', col[kind]))
    # the refused drag: Drum bus → Kick would loop; the boot turns red, Kick's jack ringed
    sx = pos['Drum bus'][1]
    fx, fy = pos['Kick'][0] + 8, ys + 8
    plugged.add(('Drum bus', 'out', V['bad']))
    bands = {}
    for nm, side, c in plugged:      # the strongest kind wins the band colour (out, send, key)
        key = (nm, side)
        rank = [V['bad'], V['out'], V['send'], V['key']].index(c)
        if key not in bands or rank < bands[key][0]:
            bands[key] = (rank, c)
    for nm, (ix, ox) in pos.items():
        for side, jx in (('in', ix), ('out', ox)):
            out.append(J.jack(t, jx, ys, empty=(nm, side) not in bands, bad=(nm == 'Kick' and side == 'in')))
    out.extend(cables)
    out.append(J.cable(t, J.free_d(sx, ys + J.PLUG_LEN, fx, fy + 36), V['bad'], 3.6, cloth=True, lift=True))
    boot = {}
    for (nm, side), (rank, c) in bands.items():
        jx = pos[nm][0] if side == 'in' else pos[nm][1]
        red = (nm, side) == ('Drum bus', 'out')
        out.append(J.plug_seated(t, jx, ys, V['out'] if red else c, gid, None))
        boot[(nm, side)] = c
    out.append(J.plug_free(t, fx, fy, V['bad'], gid, V['bad']))
    out.append(f'<path d="M{fx + 6} {fy + 24}V{fy + 37}L{fx + 9.5} {fy + 33.5}L{fx + 12} {fy + 39}L{fx + 14} {fy + 38}L{fx + 11.5} {fy + 32.5}H{fx + 16}Z" '
               f'fill="#ffffff" stroke="#10151a" stroke-width="1"/>')
    out.append(rect(fx - 70, fy - 30, 230, 18, '#2a2520', 3))
    out.append(text(fx + 45, fy - 17.5, 'Refused: Kick already feeds Drum bus (a loop)', 10, '#f0b2a8', 500, 'middle'))
    return ''.join(out)


def anatomy(t):
    V = vtok(t)
    gid = f'vmetal-{t["name"]}'
    ink, ink2 = (('#e3e8ed', '#a6b0ba') if t['name'] == 'Night' else ('#17202a', '#4a5867'))
    out = [text(612, 92, 'The plug, ×3', 13, ink, 500)]
    out.append(f'<g transform="translate(640,104) scale(3)">' + J.plug_free(t, 10, 0, V['out'], gid) +
               J.cable(t, f'M10 36Q10 46 18 54', V['out'], 3.8, cloth=True) + '</g>')
    labels = [(1, 'tip'), (5, 'ring'), (11, 'sleeve'), (25, 'nickel barrel, knurled'), (26.6, 'kind band'),
              (31, 'bakelite boot, three ribs'), (40, 'cloth cord: a braid of two dash layers')]
    for yy, lab in labels:
        y = 104 + yy * 3
        out.append(f'<path d="M{640 + 15 * 3} {y}H{760}" stroke="{ink2}" stroke-width="0.6"/>')
        out.append(text(766, y + 4, lab, 10, ink2))
    notes = ['Seated: collar, barrel, band, boot hang below the rail;', 'the cord leaves the boot vertically and',
             'hangs on a catenary to the other plug.', 'Colour keeps A/B\'s meaning: cobalt out, ivory',
             'send (thicker when louder), mustard key with', 'its spiral; a refused drop turns the boot red.']
    for k, n in enumerate(notes):
        out.append(text(612, 260 + k * 15, n, 10, ink2))
    return ''.join(out)


def sheet(t):
    ink, ink3 = (('#e3e8ed', '#7a8692') if t['name'] == 'Night' else ('#17202a', '#6a7684'))
    body = (rect(0, 0, W, HT, t['gutter']) +
            text(24, 36, 'Jack cables — a reference for the vintage round', 20, ink, 500) +
            text(24, 56, f'not Slate · {"black panel" if t["name"] == "Night" else "cream enamel"}, brushed-metal jack rail, '
                 'nickel plugs, bakelite boots, cloth cords · drawn at 125 % · 4× crop under it', 11, ink3) +
            panel(t) + anatomy(t))
    return f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{HT}" viewBox="0 0 {W} {HT}">{body}</svg>'


if __name__ == '__main__':
    d = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'build')
    for t in (NIGHT, PAPER):
        with open(os.path.join(d, f'vjack-{t["name"].lower()}.html'), 'w', encoding='utf-8') as f:
            f.write(page(sheet(t), W, HT, t['gutter']))
    print('ok')
