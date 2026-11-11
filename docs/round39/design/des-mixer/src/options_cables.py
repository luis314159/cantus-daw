"""des-mixer — options for the routing cables, on a busy desk: Kick is selected; it goes to
the Drum bus (a bus), sends to Reverb (a return) and keys Bass's compressor (a sidechain).
Chords sends to Reverb and Delay. The master is implied (never drawn as a cable).

A — as built: the selected strip's routes on lanes of one band at the desk's foot.
B — curved cables from the source's output to the destination's input, under the strips;
    the selected strip's lit, the rest (with 'Show all' on) faint.
C — no cables by default: each strip names where it goes; selecting (or hovering) a strip
    lights its destinations with a small arrow and the kind; cables only on demand.
"""
import copy
import data as D
import strip as ST
import parts as P
import mglyphs as M
import desk as DK
from base import rect, text, mix, tw
from sheets import song

H = 360
BAND = 46


def desk_row(t, x, y, opt):
    names = ['Kick', 'Bass', 'Chords', 'Drum bus', 'Reverb', 'Delay']
    row = [song('Master')] + [song(n) for n in names]
    by = {s['name']: s for s in row}
    by['Bass']['sidechain'] = 'Kick'
    by['Kick']['sends'] = [('Reverb', -12.0, True)]
    by['Kick']['out'] = 'Drum bus'
    by['Chords']['sends'] = [('Reverb', -16.0, True), ('Delay', -22.0, True)]
    sel = 'Kick'
    pos = {}
    out = []
    xx = x
    lit = {'Drum bus': 'out', 'Reverb': 'send', 'Bass': 'side'}
    for s in row:
        if opt in ('A', 'B'):
            if s['name'] in lit:
                s['sock_lit'] = t['ink2'] if lit[s['name']] == 'side' else t['accent']
            if s['name'] == sel:
                s['sock_lit'] = t['accent']
        w = ST.MASTER_W['wide'] if s['kind'] == 'master' else 60
        out.append(ST.strip(t, xx, y, H, s, 'wide', sel=s['name'] == sel))
        pos[s['name']] = (xx, w)
        xx += w + 2
    pl = ST.plan('wide', H)
    sock_y = y + pl['sock'][0] + 7
    return ''.join(out), pos, sock_y, xx


def curve(t, sx, tx, by, kind, lit, label=None):
    dist = abs(tx - sx)
    d = min(BAND - 8, 10 + dist * 0.12)
    col = (t['accent'] if kind != 'side' else t['ink2']) if lit else mix(t['base'], t['ink3'], 0.55)
    w = 1.5 if (kind == 'out' and lit) else 1
    dash = ' stroke-dasharray="3 2"' if kind == 'side' else ''
    p = f'M{sx} {by + 3}C{sx} {by + d * 1.3} {tx} {by + d * 1.3} {tx} {by + 4}'
    out = f'<path d="{p}" fill="none" stroke="{col}" stroke-width="{w}"{dash}/>'
    # the input: a small arrow into the destination's socket
    out += f'<path d="M{tx - 2.5} {by + 8}L{tx} {by + 4.5}L{tx + 2.5} {by + 8}" fill="none" stroke="{col}" stroke-width="1" stroke-linejoin="round"/>'
    if label and lit:
        mx, my = (sx + tx) / 2, by + d * 0.98
        lw = tw(label, 9, 600) + 8
        out += rect(mx - lw / 2, my - 6, lw, 12, t['base'], 2)
        out += text(mx, my + 3, label, 9, col, 600, 'middle')
    return out


def option(t, x, y, opt, show_all=False):
    out = []
    body, pos, sock_y, x_end = desk_row(t, x, y, opt)
    out.append(body)
    c = lambda n: pos[n][0] + pos[n][1] / 2  # noqa: E731
    if opt == 'A':
        out.append(DK.cables(t, c('Kick'), [(c('Drum bus'), 'out'), (c('Reverb'), 'send'), (c('Bass'), 'side')], sock_y))
    elif opt == 'B':
        by = y + H
        # the band under the strips, where cables hang
        out.append(rect(x, by + 2, x_end - x - 2, BAND, mix(t['base'], t['gutter'], 0.35), 3))
        routes = [('Kick', 'Drum bus', 'out', True, None), ('Kick', 'Reverb', 'send', True, '−12'),
                  ('Kick', 'Bass', 'side', True, 'key')]
        if show_all:
            routes = [('Chords', 'Reverb', 'send', False, None), ('Chords', 'Delay', 'send', False, None)] + routes
        for a, b, k, lit, lab in routes:
            out.append(curve(t, c(a), c(b), by - 6, k, lit, lab))
        # the toggle, at the band's right
        lab = 'Show all'
        tx = x_end - 10
        from insp_parts import toggle
        out.append(text(tx - 32, by + BAND - 6, lab, 10, t['ink3'], 400, 'end'))
        out.append(toggle(t, tx - 28, by + BAND - 16, show_all))
    else:
        # C: the destinations light, each with an arrow and its kind, in its socket row
        for n, kind, lab in (('Drum bus', 'out', 'out'), ('Reverb', 'send', 'send −12'), ('Bass', 'side', 'key')):
            sx, sw = pos[n]
            col = t['ink2'] if kind == 'side' else t['accent']
            out.append(f'<rect x="{sx + 0.5}" y="{y + 0.5}" width="{sw - 1}" height="{H - 1}" rx="3" fill="none" '
                       f'stroke="{col}" stroke-width="1" opacity="0.8"/>')
            out.append(rect(sx + 1, y + H - 20, sw - 2, 19, t['surface']))
            lw = tw(lab, 9, 600) + 16
            lx = sx + sw / 2 - lw / 2
            out.append(rect(lx, sock_y - 6, lw, 13, mix(t['surface'], col, 0.20), 3))
            out.append(f'<path d="M{lx + 4} {sock_y - 2.5}L{lx + 7.5} {sock_y + 0.5}L{lx + 4} {sock_y + 3.5}" fill="none" stroke="{col}" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/>')
            out.append(text(lx + 10, sock_y + 3.5, lab, 9, col, 600))
        # the source says where it goes, all of it
        kx, kw = pos['Kick']
        from base import HUES
        out.append(rect(kx + 1, y + H - 20, kw - 2, 19, mix(t['surface'], HUES['Kick'], t['head_sel'])))
        out.append(text(kx + kw / 2, sock_y + 3.5, '3 routes', 9, t['ink2'], 600, 'middle'))
        # 'Cables' key in the desk header area above (drawn in the panel's caption row)
    return ''.join(out), x_end
