"""des-views — the channel rack's head, ruler, rows and foot (parts in rack.py)."""
import glyphs as G
import song_rack as S
from base import rect, text, well, mix, lit_edge, HUES
from rack import (ROW, KID, STEP_W, step_x, page_w, mini_knob, lamp, hue_col, steps, roll, wave)


def head(t, x, y, w):
    out = [rect(x, y, w, 28, t['surface']), lit_edge(t, x, y, w)]
    out.append(text(x + 12, y + 18, 'Channel rack', 13, t['ink1'], 500))
    px = x + 104
    out.append(well(t, px, y + 4, 96, 20, 4))
    out.append(text(px + 8, y + 18, 'Pattern', 10, t['ink3']))
    out.append(text(px + 44, y + 18, 'Drop', 12, t['ink1'], 500))
    out.append(G.chevron_down(px + 96 - 11, y + 14, 2.5, t['ink3'], 1.2))
    fx = px + 104
    out.append(well(t, fx, y + 4, 92, 20, 4))
    out.append(text(fx + 8, y + 18, 'Show', 10, t['ink3']))
    out.append(text(fx + 36, y + 18, 'All', 12, t['ink1']))
    out.append(G.chevron_down(fx + 92 - 11, y + 14, 2.5, t['ink3'], 1.2))
    sx = fx + 108
    out.append(text(sx, y + 18, 'Swing', 11, t['ink3']))
    out.append(mini_knob(t, sx + 44, y + 14, 0.12, r=6))
    out.append(text(sx + 58, y + 18, '12 %', 11, t['ink1']))
    # the pattern's map: eight bars, the page in view lifted; a cell's marks are its density
    mx = x + w - 12 - 8 * 22
    out.append(text(mx - 10, y + 18, 'Bars 1 – 2 of 8', 11, t['ink3'], 400, 'end'))
    out.append(well(t, mx, y + 5, 8 * 22, 18, 4))
    out.append(rect(mx + 2, y + 7, 44 - 2, 14, t['raised'], 3))
    for b in range(8):
        cx = mx + b * 22
        for k in range(4):
            hgt = 3 + ((b * 7 + k * 3) % 5)
            col = t['ink2'] if b < 2 else mix(t['well'], t['ink3'], 0.7)
            out.append(rect(cx + 5 + k * 4, y + 18 - hgt, 2, hgt, col, 0.5))
    return ''.join(out)


def ruler(t, x0, y):
    out = []
    for i in range(32):
        xx = step_x(x0, i)
        if i % 16 == 0:
            out.append(text(xx + 1, y + 13, str(i // 16 + 1), 11, t['ink1'], 600))
        elif i % 4 == 0:
            out.append(text(xx + 1, y + 13, f'{i // 16 + 1}.{(i % 16) // 4 + 1}', 9, t['ink3']))
        out.append(rect(xx + STEP_W / 2 - 0.5, y + 16, 1, 2, t['ink4']))
    return ''.join(out)


def channel(t, x, y, w, c, x_steps):
    name, hue, kind, lv, mix_n, pan, vol, data, fl = c
    h = KID if lv else ROW
    sel, muted = 'sel' in fl, 'muted' in fl
    night = t['name'] == 'Night'
    bg = mix(t['surface'], HUES[hue], 0.10 if night else 0.12) if sel else t['surface']
    out = [rect(x, y, w, h - 1, bg)]
    out.append(rect(x, y, 3, h - 1, HUES[hue] if sel else hue_col(t, hue, 0.72)))
    cy = y + (h - 1) / 2
    out.append(lamp(t, x + 16, cy, not muted))
    out.append(mini_knob(t, x + 36, cy, pan, bip=True))
    out.append(mini_knob(t, x + 58, cy, vol))
    # the mixer track it feeds: a number in a small well
    out.append(well(t, x + 72, cy - 9, 24, 18, 3))
    out.append(text(x + 84, cy + 4, str(mix_n), 11, t['ink2'], 500, 'middle'))
    # the name: a plate, lit in the channel's colour when selected; a kit's pads indented
    nx = x + 104 + (14 if lv else 0)
    nw = 150 - (14 if lv else 0)
    if lv:
        g = mix(t['surface'], t['ink4'], 0.7)
        out.append(rect(x + 110, y - 1, 1, h if c[0] != 'Ride' else h / 2 + 1, g))
        out.append(rect(x + 110, cy, 6, 1, g))
    plate = (mix(t['raised'], HUES[hue], 0.22 if night else 0.20) if sel
             else t['raised'] if night else mix(t['raised'], t['surface'], 0.2))
    out.append(rect(nx, cy - 10, nw, 20, mix(plate, t['edge'], 0.5), 3))
    out.append(rect(nx, cy - 10, nw, 19, plate, 3))
    if kind == 'kit':
        out.append(G.chevron_down(nx + 10, cy, 2.5, t['ink2'], 1.2))
        out.append(text(nx + 20, cy + 4, name, 12, t['ink1'], 500))
        out.append(text(nx + nw - 8, cy + 4, '3 pads', 10, t['ink3'], 400, 'end'))
    else:
        out.append(text(nx + 9, cy + 4, name, 12 if not lv else 11, t['ink1'] if not muted else t['ink3'], 500 if sel else 400))
    # the target mark: which channel the piano roll and the keys play
    out.append(rect(x + 262, cy - 7, 3, 14, t['accent'] if sel else mix(t['surface'], t['ink4'], 0.35), 1.5))
    sh = h - 8
    sy = y + 4 - 0.5
    if kind == 'steps':
        out.append(steps(t, x_steps, sy, sh, hue, data, sel, muted))
    elif kind == 'roll':
        out.append(roll(t, x_steps, sy, sh, hue, data, sel))
    elif kind == 'perc':
        out.append(wave(t, x_steps, sy, sh, hue, 'perc'))
    elif kind == 'pad':
        out.append(wave(t, x_steps, sy, sh, hue, 'pad'))
    elif kind == 'kit':     # the kit's own row: its pads' hits merged, one quiet mark each
        W = page_w()
        out.append(rect(x_steps, sy + 6, W, sh - 12, mix(t['surface'], t['well'], 0.5), 3))
        for dd, k in ((S.CLOSED, 0.45), (S.OPEN, 0.7)):
            for i in dd:
                xx = step_x(x_steps, i)
                out.append(rect(xx + STEP_W / 2 - 1, sy + 9, 2, sh - 18, hue_col(t, hue, k), 1))
    return ''.join(out)


def panel(t, x, y, w, h):
    night = t['name'] == 'Night'
    out = [rect(x, y, w, h, t['base']), head(t, x, y, w)]
    xs = x + w - 12 - page_w()
    out.append(rect(x, y + 28, w, 22, t['surface']))
    out.append(ruler(t, xs, y + 30))
    yy = y + 52
    for c in S.CHANNELS:
        out.append(channel(t, x, yy, w, c, xs))
        yy += KID if c[3] else ROW
    # the foot: add a channel, or drop a sound from the browser
    yy += 8
    out.append(well(t, x + 12, yy, 150, 24, 4))
    out.append(G.plus(x + 28, yy + 12, 4, t['ink2'], 1.2))
    out.append(text(x + 40, yy + 16, 'Add a channel', 12, t['ink2']))
    dz = f' stroke="{mix(t["base"], t["ink4"], 0.9)}" stroke-width="1" stroke-dasharray="4 3"'
    out.append(rect(xs + 0.5, yy + 0.5, page_w() - 1, 56, 'none', 6, dz))
    out.append(text(xs + page_w() / 2, yy + 26, 'Drop a sound here for a new channel', 12, t['ink3'], 400, 'middle'))
    out.append(text(xs + page_w() / 2, yy + 42, 'or onto a channel to replace its sample', 11,
                    t['ink4'] if not night else mix(t['ink4'], t['ink3'], 0.5), 400, 'middle'))
    return ''.join(out)
