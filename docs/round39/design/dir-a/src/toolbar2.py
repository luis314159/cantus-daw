"""Slate (dir-a), phase B — a panel's toolbar (the arrangement's), as groups that fold
with the panel's width. Row 28 high, keys 20 high at y 4, gaps 12, edges 12.

Left: the panel's name, the tools (a radio set), the grid. Right: follow, the panel menu.
Fold order: the grid's word, follow's word, the name becomes its glyph, the tools become
the chosen tool and a chevron, the grid becomes its value only.
"""
import glyphs as G
from base import rect, text, well, mix, tw, lit_edge
from topbar import joined, key

H, KY, KH, GAP, EDGE = 28, 4, 20, 12, 12
TOOLS = [('select', G.select, 'Select', 'V'), ('draw', G.draw, 'Draw', 'P'), ('slice', G.slice_, 'Slice', 'C'),
         ('erase', G.erase, 'Erase', 'D'), ('mute', G.mute_tool, 'Mute', 'T'), ('zoom', G.zoom, 'Zoom', 'Z')]
FOLD = [('grid', 'value'), ('follow', 'key'), ('title', 'glyph'), ('tools', 'chosen'), ('menu', None)]


def width(g, form):
    return {('title', 'full'): tw('Arrangement', 13, 500), ('title', 'glyph'): 20,
            ('tools', 'full'): 26 * 6, ('tools', 'chosen'): 40,
            ('grid', 'full'): 30 + 58, ('grid', 'value'): 58,
            ('follow', 'full'): 24 + 6 + tw('Follow', 11), ('follow', 'key'): 24,
            ('menu', 'full'): 24}[(g, form)]


def layout(W):
    forms = {'title': 'full', 'tools': 'full', 'grid': 'full', 'follow': 'full', 'menu': 'full'}
    k = 0

    def total():
        gs = [g for g in forms if forms[g]]
        return 2 * EDGE + sum(width(g, forms[g]) for g in gs) + GAP * (len(gs) - 1) + 16
    while total() > W and k < len(FOLD):
        g, f = FOLD[k]
        forms[g] = f
        k += 1
    return forms


def draw_group(t, g, form, x, st, y0=0):
    y = y0 + KY
    out = []
    i1, i2, i3 = t['ink1'], t['ink2'], t['ink3']
    if g == 'title':
        if form == 'full':
            out.append(text(x, y0 + 18, 'Arrangement', 13, i1, 500))
        else:
            out.append(G.nat(G.w_arrange, x + 10, y0 + 14, 20, i1, i3))
    elif g == 'tools':
        chosen = st.get('tool', 'select')
        hover = st.get('tool_hover')
        if form == 'full':
            out.append(joined(t, x, y, [26] * 6, KH))
            for k, (n, fn, _, _) in enumerate(TOOLS):
                if n == chosen:
                    out.append(rect(x + k * 26 + 2, y + 2, 22, KH - 4, t['raised'], 3))
                    out.append(rect(x + k * 26 + 2, y + 2, 22, 1, mix(t['raised'], i1, 0.10)))
                elif n == hover:
                    out.append(rect(x + k * 26 + 2, y + 2, 22, KH - 4, t['well_hi'], 3))
                ink = i1 if n in (chosen, hover) else i2
                out.append(G.nat(fn, x + k * 26 + 13, y + KH / 2, 20, ink, i3))
        else:
            fn = next(f for n, f, _, _ in TOOLS if n == chosen)
            out.append(joined(t, x, y, [26, 14], KH))
            out.append(rect(x + 2, y + 2, 22, KH - 4, t['raised'], 3))
            out.append(G.nat(fn, x + 13, y + KH / 2, 20, i1, i3))
            out.append(G.chevron_down(x + 33, y + KH / 2, 2, i3, 1.1))
    elif g == 'grid':
        fx = x
        if form == 'full':
            out.append(text(x, y0 + 18, 'Grid', 11, i3))
            fx = x + 30
        op = st.get('grid_open')
        if op:
            out.append(rect(fx - 2, y - 2, 62, KH + 4, mix(t['surface'], t['accent'], 0.45), 5))
        out.append(well(t, fx, y, 58, KH))
        out.append(text(fx + 8, y + 14, st.get('grid', '1 bar'), 11, i1))
        out.append(G.chevron_down(fx + 48, y + 10, 2.5, i3, 1.2))
    elif g == 'follow':
        on = st.get('follow', True)
        out.append(key(t, x, y, 24, KH, on=on))
        out.append(G.nat(G.follow, x + 12, y + KH / 2, 20, t['accent'] if on else i2, mix(t['accent'], i1, 0.4) if on else i3))
        if form == 'full':
            out.append(text(x + 30, y0 + 18, 'Follow', 11, i2 if on else i3))
    elif g == 'menu':
        out.append(key(t, x, y, 24, KH))
        out.append(G.nat(G.more, x + 12, y + KH / 2, 16, i2, i3))
    return ''.join(out)


def row(t, W, st, y0=0, forms=None):
    forms = forms or layout(W)
    out = [rect(0, y0, W, H, t['surface']), lit_edge(t, 0, y0, W)]
    x = EDGE
    for g in ('title', 'tools', 'grid'):
        if forms[g]:
            out.append(draw_group(t, g, forms[g], x, st, y0))
            x += width(g, forms[g]) + GAP
    x = W - EDGE
    for g in ('menu', 'follow'):
        if forms[g]:
            x -= width(g, forms[g])
            out.append(draw_group(t, g, forms[g], x, st, y0))
            x -= GAP
    return ''.join(out), forms


def grid_menu(t, x, y, chosen='1 bar'):
    """The grid's menu: values in musical order, the chosen one checked, two options."""
    items = ['Bar', '½ bar', 'Beat', '1/8', '1/16', '1/32', '-', 'Triplets', 'Follow the zoom']
    W, rh = 168, 24
    hgt = sum(9 if i == '-' else rh for i in items) + 12
    uid = f'gm{t["name"]}{x}{y}'
    edge = mix(t['raised'], t['ink1'], 0.08) if t['name'] == 'Night' else t['line']
    out = (f'<filter id="{uid}" x="-20%" y="-20%" width="140%" height="140%"><feDropShadow dx="0" dy="6" stdDeviation="9" flood-color="#000" flood-opacity="{0.45 if t["name"] == "Night" else 0.16}"/></filter>'
           f'<rect x="{x}" y="{y}" width="{W}" height="{hgt}" rx="6" fill="{t["raised"]}" filter="url(#{uid})"/>'
           f'<rect x="{x + 0.5}" y="{y + 0.5}" width="{W - 1}" height="{hgt - 1}" rx="5.5" fill="none" stroke="{edge}"/>')
    yy = y + 6
    for it in items:
        if it == '-':
            out += rect(x + 10, yy + 4, W - 20, 1, mix(t['raised'], t['ink4'], 0.45))
            yy += 9
            continue
        if it == '1/16':
            out += rect(x + 4, yy, W - 8, rh, mix(t['raised'], t['accent'], 0.22), 4)
        if it == 'Bar':
            out += (f'<path d="M{x + 12} {yy + 12}L{x + 15.5} {yy + 15.5}L{x + 21} {yy + 8.5}" fill="none" '
                    f'stroke="{t["accent"]}" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>')
        if it == 'Follow the zoom':
            out += (f'<rect x="{x + 11.5}" y="{yy + 6.5}" width="11" height="11" rx="2.5" fill="none" stroke="{t["ink3"]}"/>')
        out += text(x + 28, yy + 16, it, 12, t['ink1'], 500 if it == 'Bar' else 400)
        if it == 'Triplets':
            out += text(x + W - 12, yy + 16, 'Shift 3', 10, t['ink3'], 400, 'end')
        yy += rh
    return out
