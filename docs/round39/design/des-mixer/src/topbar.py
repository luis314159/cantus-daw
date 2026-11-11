"""Slate (dir-a) — the top bar: menus and the project, the transport, tempo, the clock,
snap, undo, the master meter, the device, settings and the overflow key."""
import glyphs as G
from base import rect, text, well, mix, tw, vline, lit_edge, TOP_H

TXT = dict(menus=('File', 'Edit', 'Clip', 'View', 'Options', 'Help'), sub='9 tracks · 64 bars · 2:03',
           pat=['Pat', 'Song'], bb='bar · beat', peak='Peak')

LOGO = 'file:///F:/projects/daw-r39/dir-a/crates/cantus-ui/assets/logos/02.svg'


def key(t, x, y, w, h, r=4, on=False, hi=False, pressed=False):
    """A single key: a recessed well; on = an accent tint inside the well."""
    if on:
        return well(t, x, y, w, h, r, fill=mix(t['well'], t['accent'], 0.20))
    if pressed:
        return well(t, x, y, w, h, r, fill=t['edge'])
    return well(t, x, y, w, h, r, hi=hi)


def joined(t, x, y, widths, h, r=4):
    """Several keys in one well, divided by the well's own dark edge."""
    W = sum(widths)
    out = well(t, x, y, W, h, r)
    xx = x
    for w in widths[:-1]:
        xx += w
        out += vline(xx - 1, y + 4, y + h - 4, t['edge'])
    return out


def segmented(t, x, y, labels, widths, h, sel, size=11):
    out = well(t, x, y, sum(widths), h, 4)
    xx = x
    for i, (lab, w) in enumerate(zip(labels, widths)):
        if i == sel:
            out += rect(xx + 2, y + 2, w - 4, h - 4, t['raised'], 3)
            out += rect(xx + 2, y + 2, w - 4, 1, mix(t['raised'], t['ink1'], 0.10), 0)
        col = t['ink1'] if i == sel else t['ink3']
        out += text(xx + w / 2, y + h / 2 + size * 0.36, lab, size, col, 500 if i == sel else 400, 'middle')
        xx += w
    return out


def meter_h(t, x, y, w, levels=None, peak=None):
    """The master meter, horizontal: two slim bars, the scale under them. Idle: the
    grooves and the scale only — no −∞, no black slab."""
    out = ''
    groove = mix(t['well'], t['ink4'], 0.28)
    for k in range(2):
        out += rect(x, y + k * 5, w, 3, groove, 1)
    if levels:
        for k, lv in enumerate(levels):
            lw = round(w * lv)
            g1, g2 = round(w * 0.72), round(w * 0.88)
            out += rect(x, y + k * 5, min(lw, g1), 3, t['meter_lo'], 1)
            if lw > g1:
                out += rect(x + g1, y + k * 5, min(lw, g2) - g1, 3, t['meter_mid'], 0)
            if lw > g2:
                out += rect(x + g2, y + k * 5, lw - g2, 3, t['meter_hi'], 0)
    for db, fr in ((-48, 0.12), (-24, 0.42), (-12, 0.62), (-6, 0.76), (0, 0.9)):
        xx = round(x + w * fr)
        out += rect(xx, y + 11, 1, 2 if db else 3, t['ink3'] if db == 0 else t['ink4'])
    return out


def topbar(t, state):
    W = 1280
    out = [rect(0, 0, W, TOP_H, t['surface']), lit_edge(t, 0, 0, W)]
    # ---- the mark, the menus, the project
    out.append(f'<image href="{LOGO}" x="12" y="10" width="24" height="24"/>')
    x = 46
    for m in TXT['menus']:
        out.append(text(x, 19, m, 12, t['ink2'], 400))
        x += tw(m, 12) + 13
    out.append(text(46, 36, 'Neon Tide', 13, t['ink1'], 500))
    out.append(text(46 + tw('Neon Tide', 13, 500) + 8, 36, TXT['sub'], 11, t['ink3']))

    y, h = 10, 24
    # ---- pattern / song
    out.append(segmented(t, 262, y, TXT['pat'], [34, 40], h, 1))
    # ---- transport: one well, four keys
    tx = 344
    ws = [30, 32, 30, 30]
    out.append(joined(t, tx, y, ws, h))
    cy = y + h / 2 - 0.5
    playing = state.get('playing')
    i1, i2 = t['ink1'], t['ink2']
    out.append(G.to_start(tx + 15, cy, 24, i2, t['ink3']))
    if playing:
        out.append(rect(tx + 31, y + 2, 30, h - 4, mix(t['well'], t['play'], 0.18), 3))
        out.append(G.play(tx + 30 + 16, cy, 24, t['play'], mix(t['play'], t['well'], 0.35)))
    else:
        out.append(G.play(tx + 30 + 16, cy, 24, i1, i2))
    out.append(G.stop(tx + 62 + 15, cy, 24, i2 if playing else i2, t['ink3']))
    rec_col = mix(t['well'], t['rec'], 0.82)
    out.append(G.rec(tx + 92 + 15, cy + 0.5, 24, rec_col))
    # ---- loop (on)
    lx = tx + sum(ws) + 6
    out.append(key(t, lx, y, 30, h, on=state.get('loop', True)))
    out.append(G.loop(lx + 15, cy + 0.5, 24, t['accent'] if state.get('loop', True) else i2, i2))

    # ---- tempo and metre
    bx = lx + 30 + 12
    out.append(well(t, bx, y, 78, h))
    out.append(text(bx + 9, y + 17, '124', 15, t['ink1'], 400))
    out.append(text(bx + 9 + tw('124', 15), y + 17, '.000', 11, t['ink3'], 400))
    out.append(text(bx + 78 - 7, y + 16, 'BPM', 9, t['ink3'], 600, 'end', track=0.6))
    mx = bx + 78 + 4
    out.append(well(t, mx, y, 34, h))
    out.append(text(mx + 17, y + 16.5, '4/4', 12, t['ink2'], 400, 'middle'))

    # ---- the clock: large, thin, no box — the bar's focal point
    cx = mx + 34 + 18
    pos = state.get('pos', ('41', '1', '000'))
    big = f'{pos[0]}.{pos[1]}'
    out.append(text(cx, 34, big, 30, t['ink1'], 200))
    bw = tw(big, 30, 200) + 1
    out.append(text(cx + bw, 34, f'.{pos[2]}', 15, t['ink3'], 300))
    rx = cx + bw + tw('.000', 15, 300) + 12
    out.append(text(rx, 20, state.get('time', '1:17.42'), 12, t['ink2'], 400))
    out.append(text(rx, 33, TXT['bb'], 9, t['ink3'], 500, track=0.3))

    # ---- snap and grid; undo and redo
    sx = 812
    out.append(joined(t, sx, y, [28, 52], h))
    out.append(rect(sx + 2, y + 2, 25, h - 4, mix(t['well'], t['accent'], 0.20), 3))
    out.append(G.magnet(sx + 14, cy + 0.5, 24, t['accent'], mix(t['accent'], t['ink1'], 0.55)))
    out.append(text(sx + 28 + 8, y + 16.5, '1/16', 12, t['ink1'], 400))
    out.append(G.chevron_down(sx + 80 - 10, cy + 0.5, 2.5, t['ink3'], 1.2))
    ux = sx + 80 + 8
    out.append(joined(t, ux, y, [28, 28], h))
    out.append(G.undo(ux + 14, cy + 0.5, 24, i2, i2))
    out.append(G.redo(ux + 28 + 14, cy + 0.5, 24, t['ink3'], t['ink3']))

    # ---- the right side: master meter, device, settings, the overflow
    ox = W - 10 - 30
    out.append(key(t, ox, y, 30, h, pressed=state.get('overflow_open', False)))
    out.append(G.more(ox + 15, cy + 0.5, 24, t['ink1'] if state.get('overflow_open') else i2, i2))
    gx = ox - 4 - 28
    out.append(key(t, gx, y, 28, h))
    out.append(G.gear(gx + 14, cy + 0.5, 24, i2, t['ink3']))
    dx = gx - 6 - 104
    out.append(well(t, dx, y, 104, h))
    out.append(G.speaker(dx + 13, cy + 0.5, 20, i2, t['ink3']))
    out.append(text(dx + 25, y + 16.5, 'Realtek Audio', 11, t['ink2'], 400))
    out.append(G.chevron_down(dx + 104 - 10, cy + 0.5, 2.5, t['ink3'], 1.2))
    mw = 132
    mxx = dx - 8 - mw
    out.append(well(t, mxx, y, mw, h))
    out.append(text(mxx + 8, y + 16, TXT['peak'], 10, t['ink3'], 500))
    out.append(meter_h(t, mxx + 34, y + 6, mw - 44, state.get('master'), None))
    return ''.join(out)
