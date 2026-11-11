"""des-arrange — what every build sheet shares: the page, titles, captions, measure marks,
a stack of rows (header + a short lane) without the ruler, menus and tooltips."""
import arr_canvas as C
import arr_head as H
from arr_geo import View
from base import defs, rect, text, mix, tw


def frame(t, W, Hh, title, sub):
    bg = t['gutter'] if t['name'] == 'Night' else '#dfe4e9'
    out = [defs(t), rect(0, 0, W, Hh, bg)]
    out.append(rect(0, 0, W, 56, t['surface']))
    out.append(text(24, 30, title, 20, t['ink1'], 500))
    out.append(text(24, 47, sub, 11, t['ink3']))
    return ''.join(out)


def section(t, x, y, title, sub=''):
    out = text(x, y, title, 13, t['ink1'], 500)
    if sub:
        out += text(x + tw(title, 13, 500) + 10, y, sub, 11, t['ink3'])
    return out


def wrap(s, size, w):
    lines, cur = [], ''
    for word in s.split(' '):
        if cur and tw(cur + ' ' + word, size) > w:
            lines.append(cur)
            cur = word
        else:
            cur = (cur + ' ' + word).strip()
    return lines + ([cur] if cur else [])


def cap(t, x, y, a, b='', anchor='start', w=176):
    """A caption: the state in ink 1 Medium 11, what it means in ink 3 (wrapped to w)."""
    out = text(x, y, a, 11, t['ink1'], 500, anchor)
    for k, ln in enumerate(wrap(b, 10, w) if b else []):
        out += text(x, y + 14 + 12 * k, ln, 10, t['ink3'], 400, anchor)
    return out


def dim_v(t, x, y0, y1, lab):
    """A vertical measure: a hairline with end ticks and its number."""
    c = t['accent']
    return (rect(x, y0, 1, y1 - y0, c) + rect(x - 2, y0, 5, 1, c) + rect(x - 2, y1 - 1, 5, 1, c)
            + text(x + 5, (y0 + y1) / 2 + 3.5, lab, 9, c, 600))


def dim_h(t, x0, x1, y, lab):
    c = t['accent']
    return (rect(x0, y, x1 - x0, 1, c) + rect(x0, y - 2, 1, 5, c) + rect(x1 - 1, y - 2, 1, 5, c)
            + text((x0 + x1) / 2, y - 4, lab, 9, c, 600, 'middle'))


def stack(t, x, y, head_w, lane_w, tracks, bar0=41.0, ppbar=44.0, head=None, playing=False, hover=None,
          sel_clips=(), edited=None, levels=None, uid='s', loop=None, hover_lane=None):
    """Rows of header + lane, no ruler: for the state sheets. Returns (svg, bottom y)."""
    v = View(x, y, head_w + lane_w, 2000, tracks, head_w=head_w, bar0=bar0, ppbar=ppbar, head=head,
             playing=playing, sel_clips=sel_clips, edited=edited, uid=uid, overview=False, sections=False)
    v.y = y - v.top_h      # rows start at y
    rows, end_y = v.rows()
    total = end_y - y
    cid = f'st-{uid}-{t["name"]}'
    out = [rect(v.cx, y, lane_w, total, t['base']),
           f'<clipPath id="{cid}"><rect x="{v.cx}" y="{y}" width="{lane_w}" height="{total}"/></clipPath><g clip-path="url(#{cid})">',
           C.grid(t, v, y, total)]
    for kind, tr, ln, yy, h in rows:
        if kind == 'track':
            out.append(C.track_lane(t, v, tr, yy, h))
            out.append(rect(v.cx, yy + h - 1, lane_w, 1, mix(t['base'], t['gutter'], 0.55)))
        else:
            out.append(C.auto_lane(t, v, tr, ln, yy, h))
    if head is not None:
        out.append(rect(round(v.bx(head)), y, 1, total, t['accent']))
    out.append('</g>')
    num = 0
    for kind, tr, ln, yy, h in rows:
        if kind == 'track':
            num = tr.get('num', num + (0 if tr['kind'] == 'folder' else 1))
            out.append(H.header(t, x, yy, head_w, h, tr, num, hover=(hover == tr['name']),
                                level=(levels or {}).get(tr['name'])))
        else:
            out.append(H.lane_header(t, x, yy, head_w, h, tr, ln, hover=hover_lane == ln['param']))
    out.append(rect(v.cx - 1, y, 1, total, mix(t['surface'], t['gutter'], 0.8)))
    return ''.join(out), end_y, v


def shadow_box(t, x, y, w, h, r=6, uid='m'):
    edge = mix(t['raised'], t['ink1'], 0.08) if t['name'] == 'Night' else t['line']
    fid = f'sh{uid}{t["name"]}'
    return (f'<filter id="{fid}" x="-20%" y="-20%" width="140%" height="140%"><feDropShadow dx="0" dy="6" stdDeviation="9" '
            f'flood-color="#000" flood-opacity="{0.45 if t["name"] == "Night" else 0.16}"/></filter>'
            f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{r}" fill="{t["raised"]}" filter="url(#{fid})"/>'
            f'<rect x="{x + 0.5}" y="{y + 0.5}" width="{w - 1}" height="{h - 1}" rx="{r - 0.5}" fill="none" stroke="{edge}"/>')


def menu(t, x, y, items, w=220, uid='m', hot=None):
    """A menu: rows 24, separators 9, a check, a shortcut, a submenu chevron, a swatch row.
    items: ('-') | (label, shortcut, flags) with flags ⊂ {'check', 'sub', 'danger', 'dim'} |
    ('swatches', colours, chosen)."""
    import glyphs as G
    rh = 24
    hgt = 12 + sum(9 if it == '-' else (30 if it[0] == 'swatches' else rh) for it in items)
    out = [shadow_box(t, x, y, w, hgt, 6, uid)]
    yy = y + 6
    for it in items:
        if it == '-':
            out.append(rect(x + 10, yy + 4, w - 20, 1, mix(t['raised'], t['ink4'], 0.45)))
            yy += 9
            continue
        if it[0] == 'swatches':
            cols, chosen = it[1], it[2]
            for k, c in enumerate(cols):
                cx = x + 28 + k * 19
                if k == chosen:
                    out.append(f'<circle cx="{cx}" cy="{yy + 15}" r="7.5" fill="none" stroke="{t["ink1"]}" stroke-width="1.2"/>')
                out.append(f'<circle cx="{cx}" cy="{yy + 15}" r="5.5" fill="{c}"/>')
            yy += 30
            continue
        lab, sc, fl = it
        if lab == hot:
            out.append(rect(x + 4, yy, w - 8, rh, mix(t['raised'], t['accent'], 0.22), 4))
        if 'check' in fl:
            out.append(f'<path d="M{x + 12} {yy + 12}L{x + 15.5} {yy + 15.5}L{x + 21} {yy + 8.5}" fill="none" '
                       f'stroke="{t["accent"]}" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>')
        ink = t['rec'] if 'danger' in fl else (t['ink3'] if 'dim' in fl else t['ink1'])
        out.append(text(x + 28, yy + 16, lab, 12, ink))
        if sc:
            out.append(text(x + w - (24 if 'sub' in fl else 12), yy + 16, sc, 10, t['ink3'], 400, 'end'))
        if 'sub' in fl:
            out.append(G.chevron_right(x + w - 12, yy + 12, 3, t['ink3'], 1.2))
        yy += rh
    return ''.join(out), hgt
