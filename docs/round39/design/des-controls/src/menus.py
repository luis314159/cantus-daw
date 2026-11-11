"""des-controls — menus: the menu bar's menus, submenus, context menus, a drop-down's list.

A menu is a raised sheet (6-px corners, one soft shadow, a 1-px edge). Rows are 24 px
(36 at 150 %); a 26-px column at the left holds the check or the radio dot, so labels
align whether or not a row is checked. Shortcuts sit right in ink 3; a submenu's chevron
takes their place. A section is a quiet heading in ink 3 — never a capital bar.
"""
from slate import f, rect, text, mix, tw, popover, path, circle, frame
from fields import chevron

ROW = {1: 24, 1.5: 36}
SEP = {1: 9, 1.5: 13}
HEAD = {1: 24, 1.5: 35}


def height(s, items):
    h = 0
    for it in items:
        k = it.get('k', 'item')
        h += SEP[s] if k == 'sep' else (HEAD[s] if k == 'head' else ROW[s])
    return h + 10 * s


def check(t, s, cx, cy, col):
    w = 4.5 * s
    return path(f'M{f(cx - w)} {f(cy)}L{f(cx - w * 0.3)} {f(cy + w * 0.7)}L{f(cx + w)} {f(cy - w * 0.75)}',
                col, 1.6 if s == 1 else 2.2)


def menu(t, s, x, y, w, items, hot=None, scroll=False, max_h=None):
    """Draw a menu; return (svg, {label: row_y}) so a submenu can align to its row."""
    rh = ROW[s]
    H = height(s, items)
    clip_h = min(H, max_h) if max_h else H
    out = popover(t, x, y, w, clip_h, 6 if s == 1 else 8)
    rows = {}
    yy = y + 5 * s
    gut = 26 * s
    hov_fill = mix(t['raised'], t['accent'], 0.24 if t['name'] == 'Night' else 0.13)
    body = ''
    for it in items:
        k = it.get('k', 'item')
        if k == 'sep':
            body += rect(x + 10 * s, round(yy + SEP[s] / 2), w - 20 * s, 1, mix(t['raised'], t['ink4'], 0.5 if t['name'] == 'Night' else 0.45))
            yy += SEP[s]
            continue
        if k == 'head':
            body += text(x + 12 * s, yy + HEAD[s] - 8 * s, it['label'], 10.5 * s, t['ink3'], 600)
            if it.get('swatch'):
                sx = x + 12 * s + tw(it['label'], 10.5 * s, 600) + 8 * s
                body += rect(sx, yy + HEAD[s] - 15 * s, 8 * s, 8 * s, it['swatch'], 2 * s)
            yy += HEAD[s]
            continue
        lab = it['label']
        rows[lab] = yy
        dis = it.get('dis')
        is_hot = hot == lab
        if is_hot:
            fill = mix(t['raised'], t['rec'], 0.20 if t['name'] == 'Night' else 0.12) if it.get('danger') else hov_fill
            body += rect(x + 4 * s, yy, w - 8 * s, rh, fill, 4 * s if s == 1 else 5)
        ty = yy + rh / 2 + 12 * s * 0.36
        col = t['ink4'] if dis else (t['rec'] if it.get('danger') and is_hot else t['ink1'])
        if it.get('check') == 'check':
            body += check(t, s, x + 14 * s, yy + rh / 2, t['ink4'] if dis else t['accent'])
        elif it.get('check') == 'radio':
            body += circle(x + 14 * s, yy + rh / 2, 3 * s, t['accent'])
        elif it.get('check') == 'radio-off':
            body += (f'<circle cx="{f(x + 14 * s)}" cy="{f(yy + rh / 2)}" r="{f(3.5 * s)}" fill="none" '
                     f'stroke="{mix(t["raised"], t["ink3"], 0.6)}" stroke-width="1"/>')
        if it.get('swatch'):
            body += rect(x + 9 * s, yy + rh / 2 - 5 * s, 10 * s, 10 * s, it['swatch'], 2 * s)
        body += text(x + gut, ty, lab, 12 * s, col, 500 if it.get('bold') else 400)
        if it.get('note'):
            body += text(x + gut + tw(lab, 12 * s) + 6 * s, ty, it['note'], 11 * s, t['ink3'])
        if it.get('sub'):
            body += chevron(t, s, x + w - 14 * s, yy + rh / 2, t['ink1'] if is_hot else t['ink3'], 'right')
        elif it.get('short'):
            body += text(x + w - 12 * s, ty, it['short'], 11 * s, t['ink4'] if dis else t['ink3'], 400, 'end')
        yy += rh
    if max_h and H > max_h:
        cid = f'mclip-{int(x)}-{int(y)}'
        out += (f'<clipPath id="{cid}"><rect x="{f(x)}" y="{f(y + 1)}" width="{f(w)}" height="{f(clip_h - 2)}" rx="6"/></clipPath>'
                f'<g clip-path="url(#{cid})">{body}</g>')
        # the overflow: a fade and a chevron say there is more below
        fy = y + clip_h - 34 * s
        out += (f'<linearGradient id="{cid}-g" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{t["raised"]}" stop-opacity="0"/>'
                f'<stop offset="0.55" stop-color="{t["raised"]}"/></linearGradient>'
                f'<rect x="{f(x + 1)}" y="{f(fy)}" width="{f(w - 8 * s)}" height="{f(33 * s)}" rx="5" fill="url(#{cid}-g)"/>')
        out += chevron(t, s, x + w / 2, y + clip_h - 9 * s, t['ink2'], 'down')
        if scroll:
            # the thin scroll thumb along the right edge
            th = clip_h * clip_h / H
            out += rect(x + w - 5 * s, y + 6 * s, 2 * s, th - 12 * s, mix(t['raised'], t['ink3'], 0.6), 1 * s)
    else:
        out += body
    return out, rows


def menubar(t, s, x, y, names, open_=None):
    """The menu bar: words at 12 px, the open one a lifted pill."""
    out = ''
    xx = x
    pos = {}
    for nm in names:
        w = tw(nm, 12 * s) + 16 * s
        if nm == open_:
            out += rect(xx, y, w, 22 * s, t['raised'], 4 * s if s == 1 else 5)
        out += text(xx + 8 * s, y + 15 * s, nm, 12 * s, t['ink1'] if nm == open_ else t['ink2'])
        pos[nm] = xx
        xx += w
    return out, pos
