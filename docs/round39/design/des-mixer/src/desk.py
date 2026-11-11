"""des-mixer — the desk: its header, the master and the left dock, the strips that scroll,
the right dock (VCAs, returns), the cable band and the scrollbar."""
import mglyphs as M
import glyphs as G
import strip as ST
import parts as P
from base import rect, text, well, mix, tw, lit_edge
from topbar import segmented

HEAD_H = 28

L10N = {
    'en': dict(mixer='Mixer', piano='Piano roll', rack='Channel rack', dens=['Compact', 'Wide', 'Large'],
               rows='Rows', snaps='Snapshots', soloed='{n} soloed', clear='Clear', more='{n} more',
               returns='Returns', vca='VCA', add='Add a strip'),
    'es': dict(mixer='Mezclador', piano='Piano roll', rack='Rack de canales', dens=['Compacto', 'Ancho', 'Grande'],
               rows='Filas', snaps='Instantáneas', soloed='{n} en solo', clear='Quitar', more='{n} más',
               returns='Retornos', vca='VCA', add='Añadir un canal'),
}


def header(t, x0, y, w, density, lang='en', soloed=0, tabs=True, maximised=True):
    L = L10N[lang]
    out = rect(x0, y, w, HEAD_H, t['surface']) + lit_edge(t, x0, y, w)
    x = x0 + 12
    labs = (L['mixer'], L['piano'], L['rack']) if tabs else (L['mixer'],)
    for k, lab in enumerate(labs):
        on = k == 0
        out += text(x, y + 18, lab, 13 if on else 12, t['ink1'] if on else t['ink3'], 500 if on else 400)
        if on:
            out += rect(x, y + HEAD_H - 2, tw(lab, 13, 500), 2, t['ink2'])
        x += tw(lab, 13 if on else 12, 500 if on else 400) + 20
    sel = ['compact', 'wide', 'large'].index(density)
    ws = [max(44, round(tw(d, 11) + 18)) for d in L['dens']]
    out += segmented(t, x + 8, y + 4, L['dens'], ws, 20, sel, 11)
    x += 8 + sum(ws) + 12
    # rows: which rows the strips show (a menu)
    rw = round(tw(L['rows'], 11) + 22)
    out += well(t, x, y + 4, rw, 20, 4)
    out += text(x + 8, y + 18, L['rows'], 11, t['ink2'])
    out += G.chevron_down(x + rw - 9, y + 14, 2.5, t['ink3'], 1.2)
    x += rw + 8
    if soloed:
        s = L['soloed'].format(n=soloed)
        sw = round(tw(s, 11, 500) + tw(L['clear'], 11) + 26)
        out += rect(x, y + 4, sw, 20, mix(t['surface'], t['solo'], 0.16), 4)
        out += text(x + 8, y + 18, s, 11, t['solo'], 500)
        out += text(x + sw - 8, y + 18, L['clear'], 11, t['ink2'], 400, 'end')
    # right side: snapshots, add, restore
    rx = x0 + w - 10
    if maximised:
        rx -= 24
        out += well(t, rx, y + 4, 24, 20, 4)
        # restore: two overlapping frames, drawn small
        out += (f'<rect x="{rx + 8.5}" y="{y + 9.5}" width="7" height="6" fill="none" stroke="{t["ink2"]}"/>'
                f'<path d="M{rx + 10.5} {y + 9.5}V{y + 7.5}H{rx + 17.5}V{y + 13.5}H{rx + 15.5}" fill="none" stroke="{t["ink3"]}"/>')
        rx -= 6
    rx -= 24
    out += well(t, rx, y + 4, 24, 20, 4) + G.plus(rx + 12, y + 14, 3.5, t['ink2'], 1.2)
    rx -= 8
    sn = L['snaps']
    out += text(rx, y + 18, sn, 11, t['ink3'], 400, 'end')
    out += G.chevron_down(rx - tw(sn, 11) - 9, y + 14, 2.5, t['ink3'], 1.2)
    return out


def cables(t, src_x, targets, base_y):
    """The selected strip's cables in the band under the sockets: the output solid, sends
    thin, a sidechain dashed — each on its own lane so none crosses another."""
    out = ''
    lane = {'out': 6, 'send': 9, 'side': 12}
    for tx, kind in targets:
        ly = base_y + lane[kind]
        col = t['accent'] if kind != 'side' else t['ink2']
        w = 1.5 if kind == 'out' else 1
        dash = ' stroke-dasharray="3 2"' if kind == 'side' else ''
        op = '' if kind == 'out' else ' opacity="0.85"'
        r = 3
        sx = src_x
        dirn = 1 if tx > sx else -1
        d = (f'M{sx} {base_y + 2.5}V{ly - r}Q{sx} {ly} {sx + dirn * r} {ly}'
             f'H{tx - dirn * r}Q{tx} {ly} {tx} {ly - r}V{base_y + 2.5}')
        out += f'<path d="{d}" fill="none" stroke="{col}" stroke-width="{w}"{dash}{op}/>'
    return out


def desk(t, x0, y0, w, h, density, strips, master, right=(), sel=None, scroll=None, lang='en',
         soloed=0, tabs=True, maximised=True, cable_map=None):
    """The whole desk in the box (x0, y0, w, h): header, docks, strips."""
    out = [header(t, x0, y0, w, density, lang, soloed, tabs, maximised)]
    sy = y0 + HEAD_H
    out.append(rect(x0, sy, w, h - HEAD_H, t['base']))
    top = sy + 4
    H = h - HEAD_H - 8
    pitch = ST.WIDTH[density] + ST.GAP
    # the left dock: the master
    mx = x0 + 8
    pos = {}
    out.append(ST.strip(t, mx, top, H, master, density))
    pos['Master'] = mx + ST.MASTER_W[density] / 2
    left = mx + ST.MASTER_W[density] + 8
    # the right dock, pinned
    rw = len(right) * pitch + (8 if right else 0)
    rx = x0 + w - 8 - len(right) * pitch + ST.GAP
    # the middle scrolls
    mid_w = rx - 8 - left if right else x0 + w - 8 - left
    vis = int((mid_w + ST.GAP) // pitch)
    first = scroll or 0
    shown = strips[first:first + vis]
    out.append(f'<clipPath id="mid-{t["name"]}-{density}"><rect x="{left}" y="{sy}" width="{mid_w}" height="{h}"/></clipPath>')
    g = []
    x = left
    for s in shown:
        g.append(ST.strip(t, x, top, H, s, density, sel=s.get('name') == sel))
        if s.get('name'):
            pos[s['name']] = x + ST.WIDTH[density] / 2
        x += pitch
    out.append(f'<g clip-path="url(#mid-{t["name"]}-{density})">' + ''.join(g) + '</g>')
    hidden = len(strips) - len(shown) - first
    for k, s in enumerate(right):
        xx = rx + k * pitch
        out.append(ST.strip(t, xx, top, H, s, density, sel=s.get('name') == sel))
        pos[s['name']] = xx + ST.WIDTH[density] / 2
    if right:          # the dock's edge: a 1-px darker seam and the dock's own label
        out.append(rect(rx - 6, top, 1, H, mix(t['base'], t['edge'], 0.6)))
    # the scrollbar, only when the middle overflows
    if hidden > 0 or first > 0:
        n = len(strips)
        bar_y = y0 + h - 4
        tw_ = mid_w * len(shown) / n
        tx = left + mid_w * first / n
        out.append(rect(left, bar_y, mid_w, 3, mix(t['base'], t['edge'], 0.3), 1.5))
        out.append(rect(tx, bar_y, tw_, 3, t['ink4'], 1.5))
        for side, n in (('r', hidden), ('l', first)):
            if n <= 0:
                continue
            fid = f'fade{side}-{t["name"]}-{density}'
            x1, x2 = ('0', '1') if side == 'r' else ('1', '0')
            out.append(f'<defs><linearGradient id="{fid}" x1="{x1}" x2="{x2}"><stop offset="0" stop-color="{t["base"]}" stop-opacity="0"/>'
                       f'<stop offset="1" stop-color="{t["base"]}" stop-opacity="0.85"/></linearGradient></defs>')
            fx = left + mid_w - 20 if side == 'r' else left
            out.append(rect(fx, top, 20, H, f'url(#{fid})'))
            lab = str(n)
            bw = tw(lab, 10, 600) + 20
            bx = left + mid_w - bw - 2 if side == 'r' else left + 2
            by = top + H / 2 - 9
            out.append(rect(bx, by, bw, 18, t['raised'], 9) + rect(bx, by, bw, 1, mix(t['raised'], t['ink1'], 0.08)))
            if side == 'r':
                out.append(text(bx + 7, by + 12.5, lab, 10, t['ink2'], 600))
                out.append(G.chevron_right(bx + bw - 7, by + 9, 2.5, t['ink2'], 1.2))
            else:
                out.append(text(bx + bw - 7, by + 12.5, lab, 10, t['ink2'], 600, 'end'))
                out.append(f'<path d="M{bx + 8.25} {by + 6.5}L{bx + 6.75} {by + 9}L{bx + 8.25} {by + 11.5}" fill="none" '
                           f'stroke="{t["ink2"]}" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/>')
    # the cable band for the selected strip
    if sel and sel in pos and cable_map:
        pl = ST.plan(density, H)
        if 'sock' in pl:
            by = top + pl['sock'][0] + 7
            tg = [(pos[n], k) for n, k in cable_map if n in pos]
            out.append(cables(t, pos[sel], tg, by))
    return ''.join(out), pos
