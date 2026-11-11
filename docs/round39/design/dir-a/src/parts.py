"""Slate (dir-a) — the parts sheet: every key drawn for each size it is shown at (16, 20,
24 px and their 150 %: 24, 30, 36 physical px), the states, knobs, fader and meter, a
strip, tabs, a field, a menu with its submenu, a segmented control, a toggle, scrollbars.

The sheet is drawn in physical pixels and rendered at scale 1, so the 1:1 PNG is the truth;
`zoom.py` cuts it and enlarges by 4 with nearest-neighbour — no resampling softens it.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import glyphs as G  # noqa: E402
from base import NIGHT, PAPER, defs, rect, text, well, mix, page  # noqa: E402

SHEET_W, SHEET_H = 1260, 970

TRANSPORT = [('Start', G.to_start), ('Play', G.play_hinted), ('Pause', G.pause), ('Stop', G.stop),
             ('Record', None), ('Loop', G.loop_hinted), ('Metronome', G.metronome)]
TOOLS = [('Select', G.select), ('Draw', G.draw), ('Slice', G.slice_), ('Erase', G.erase),
         ('Mute', G.mute_tool), ('Zoom', G.zoom), ('Snap', G.magnet), ('Undo', G.undo),
         ('Redo', G.redo), ('Settings', G.gear), ('More', G.more)]
WINDOWS = [('Browser', G.w_browser), ('Arrange', G.w_arrange), ('Rack', G.w_rack),
           ('Piano', G.w_piano), ('Plug-in', G.w_plugin), ('Mixer', G.w_mixer),
           ('Inspector', G.w_inspector)]
ALL = TRANSPORT + TOOLS + WINDOWS
SIZES = [(16, '16 px', 30), (20, '20 px', 34), (24, '24 px', 38),
         (24, '150 % of 16', 38), (30, '150 % of 20', 46), (36, '150 % of 24', 54)]
KW = {16: 20, 20: 26, 24: 30, 30: 38, 36: 46}


def glyph_in_key(t, name, g, cx, cy, H, state='rest'):
    i1 = t['ink1'] if state in ('hover', 'pressed') or name in ('Play',) else t['ink2']
    i2 = t['ink3']
    if state == 'on':
        col = t['play'] if name == 'Play' else t['accent']
        i1, i2 = col, mix(col, t['ink1'], 0.4)
    if state == 'disabled':
        i1, i2 = t['ink4'], t['ink4']
    if name == 'Record':
        on = state == 'on'
        col = t['rec'] if on else mix(t['well'], t['rec'], 0.82)
        if state == 'disabled':
            col = t['ink4']
        return G.rec(cx, cy + 0.5, H, col, on, t['rec'])
    return g(cx, cy, H, i1, i2)


def key_state(t, x, y, w, h, state, name):
    if state == 'on':
        col = t['play'] if name == 'Play' else (t['rec'] if name == 'Record' else t['accent'])
        return well(t, x, y, w, h, 4 if h >= 20 else 3, fill=mix(t['well'], col, 0.20))
    if state == 'pressed':
        return well(t, x, y, w, h, 4 if h >= 20 else 3, fill=t['edge'])
    if state == 'disabled':
        return f'<g opacity="0.55">{well(t, x, y, w, h, 4 if h >= 20 else 3)}</g>'
    return well(t, x, y, w, h, 4 if h >= 20 else 3, hi=(state == 'hover'))


def keys_grid(t, x0, y0):
    out = [text(x0, y0, 'Keys, each drawn for its size', 14, t['ink1'], 500)]
    out.append(text(x0 + 220, y0, 'transport · tools · windows — glyph about a third of the key (transport), a little over half (line icons)', 11, t['ink3']))
    col0 = x0 + 112
    pitch = 45
    y = y0 + 22
    # column names, and the three families' spans
    for k, (nm, _) in enumerate(ALL):
        cx = col0 + k * pitch + pitch / 2
        out.append(f'<text x="{cx}" y="{y + 8}" font-size="9" fill="{t["ink3"]}" text-anchor="middle">{nm}</text>')
    y += 16
    for H, lab, rh in SIZES:
        out.append(text(x0, y + rh / 2 + 4, lab, 11, t['ink2'] if '150' not in lab else t['ink3']))
        w = KW[H]
        for k, (nm, g) in enumerate(ALL):
            cx = col0 + k * pitch + pitch / 2
            kx = round(cx - w / 2)
            ky = round(y + (rh - H) / 2)
            out.append(key_state(t, kx, ky, w, H, 'rest', nm))
            out.append(glyph_in_key(t, nm, g, kx + w / 2, ky + H / 2 - 0.5, H))
        y += rh
    # family dividers
    for k in (len(TRANSPORT), len(TRANSPORT) + len(TOOLS)):
        xx = col0 + k * pitch
        out.append(rect(xx, y0 + 22, 1, y - y0 - 22, mix(t['base'], t['line'], 0.8)))
    return ''.join(out), y


def states(t, x0, y0):
    out = [text(x0, y0, 'States, at 24 px', 14, t['ink1'], 500)]
    names = ['rest', 'hover', 'pressed', 'on', 'disabled']
    col0 = x0 + 92
    pitch = 70
    y = y0 + 22
    for k, s in enumerate(names):
        out.append(text(col0 + k * pitch + pitch / 2, y + 8, s.capitalize(), 10, t['ink3'], 400, 'middle'))
    y += 16
    rows = [('Play', G.play_hinted), ('Record', None), ('Loop', G.loop_hinted), ('Snap', G.magnet),
            ('Mixer', G.w_mixer)]
    for nm, g in rows:
        out.append(text(x0, y + 21, nm, 11, t['ink2']))
        for k, s in enumerate(names):
            cx = col0 + k * pitch + pitch / 2
            w = 30
            kx, ky = round(cx - w / 2), y + 6
            out.append(key_state(t, kx, ky, w, 24, s, nm))
            out.append(glyph_in_key(t, nm, g, kx + w / 2, ky + 11.5, 24, s))
        y += 34
    out.append(text(x0, y + 12, 'Colour is spent only on state: on = the accent (play green, record red); a radio choice is a lifted pill instead.', 10, t['ink3']))
    return ''.join(out), y + 16


def sheet(t):
    import parts2
    out = [defs(t), rect(0, 0, SHEET_W, SHEET_H, t['base'])]
    out.append(text(24, 34, f'Slate — parts · {t["name"]}', 20, t['ink1'], 500))
    out.append(text(24, 52, 'dir-a, round 39 · drawn at 1:1 in physical pixels · Barlow Semi Condensed', 11, t['ink3']))
    g, y = keys_grid(t, 24, 88)
    out.append(g)
    s, y2 = states(t, 24, y + 34)
    out.append(s)
    out.append(parts2.knobs(t, 520, y + 34))
    out.append(parts2.lower(t, 24, y2 + 24))
    body = ''.join(out)
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="{SHEET_W}" height="{SHEET_H}" '
            f'viewBox="0 0 {SHEET_W} {SHEET_H}">{body}</svg>')


if __name__ == '__main__':
    outdir = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'build')
    os.makedirs(outdir, exist_ok=True)
    for t in (NIGHT, PAPER):
        with open(os.path.join(outdir, f'parts-{t["name"].lower()}.html'), 'w', encoding='utf-8') as f:
            f.write(page(sheet(t), SHEET_W, SHEET_H, t['base']))
    print('ok')
