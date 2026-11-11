"""des-arrange — the canvas: the grid at every zoom, the lanes, the clips (des-views' content,
unchanged), a folder's summary, automation lanes, the time selection, the playhead, and the
song map that is also the horizontal scrollbar."""
import aglyphs as AG
import clipdraw as CD
import song_full as S
import song_arr as SA  # noqa: F401  (patches song_full's notes and automation)
from arr_geo import SCROLL_H
from base import rect, text, mix, tw, hue_for, HUES


def grid(t, v, y, h, lane_bg=None):
    """The grid under one band of rows: 4-bar groups by value, bars, beats, sixteenths."""
    out = []
    p = v.ppb
    b0, b1 = int(v.bar0) - 1, int(v.last_bar()) + 2
    group = 4 if v.ppbar >= 8 else 16
    for bar in range(b0, b1):
        x = v.bx(bar)
        if ((bar - 1) // group) % 2 == 1:
            out.append(rect(x, y, v.ppbar, h, lane_bg or t['lane2']))
    for bar in range(b0, b1):
        x = v.bx(bar)
        if p >= 6:
            for q in range(1, 4):
                out.append(rect(x + q * p, y, 1, h, t['grid_beat']))
        if p >= 40:
            for s in range(1, 16):
                if s % 4:
                    out.append(rect(x + s * p / 4, y, 1, h, mix(t['base'], t['grid_beat'], 0.55)))
        strong = (bar - 1) % group == 0
        if v.ppbar >= 8 or strong:
            out.append(rect(x, y, 1, h, mix(t['grid_bar'], t['ink4'], 0.35) if strong and v.ppbar < 24 else t['grid_bar']))
    for bar, name in SA.SECTIONS:      # a section starts: a line a step stronger than a bar's
        x = v.bx(bar)
        out.append(rect(x, y, 1, h, mix(t['grid_bar'], t['ink4'], 0.6)))
    return ''.join(out)


def clip_state(v, tr, c, k):
    key = (tr['name'], k)
    if tr.get('mute'):
        return 'mute'
    if tr.get('implicit'):
        return 'dim'
    if key in v.sel_clips:
        return 'sel'
    if v.playing and v.head is not None and c[0] <= v.head < c[0] + c[1]:
        return 'play'
    if tr.get('sel'):
        return 'lit'
    return ''


def kind_of(tr):
    if tr.get('frozen'):
        return 'audio2'
    return tr['kind']


def track_lane(t, v, tr, y, h):
    out = []
    if tr.get('sel'):
        out.append(rect(v.cx, y, v.cw, h, mix(t['base'], HUES[tr['hue']], 0.05), extra=' fill-opacity="1"'))
    if tr.get('arm'):
        out.append(rect(v.cx, y, v.cw, h, t['rec'], extra=' fill-opacity="0.04"'))
    if tr['kind'] == 'folder':
        return ''.join(out) + folder_summary(t, v, tr, y, h)
    for k, c in enumerate(tr['clips']):
        x0 = v.bx(c[0])
        w = c[1] * v.ppbar - 1
        if x0 + w < v.cx or x0 > v.cx + v.cw:
            continue
        st = clip_state(v, tr, c, k)
        uid = f'c{v.uid}{t["name"][0]}{tr["name"][:3]}{k}{int(y)}'
        out.append(CD.clip(t, (tr['name'], tr['hue'], kind_of(tr)), c, x0, y + 1, w, h - 3, v.ppb, v.band(), st, uid))
        if tr.get('frozen'):
            out.append(rect(x0 + 1, y + 2, w - 2, 12, 'none'))
            out.append(AG.snow(x0 + w - 10, y + 8, mix(t['base'], t['accent'], 0.85), 3.5))
        if x0 < v.cx and x0 + w > v.cx + 40 and h - 3 >= 40:
            out.append(pinned_name(t, tr, c, v.cx, y + 1, st))
        if v.edited == (tr['name'], k):
            out.append(edited_mark(t, tr, x0, y + 1, w, h - 3, c[2], max(x0, v.cx)))
    return ''.join(out)


def pinned_name(t, tr, c, x, y, st):
    """A clip that began before the view keeps its name at the canvas's left edge."""
    body = hue_for(t, tr['hue'], t['tint_sel'] if st in ('sel', 'lit') else t['tint_rest'])
    strip = mix(body, t['base'], 0.35 if t['name'] == 'Night' else 0.2)
    ink = mix(HUES[tr['hue']], t['ink1'], 0.55) if t['name'] == 'Night' else t['ink1']
    return (rect(x, y + 1, tw(c[2], 10, 500) + 16, 12, strip) + text(x + 5, y + 10.5, '‹ ' + c[2] if False else c[2], 10, ink, 500))


def edited_mark(t, tr, x, y, w, h, name, lx):
    """The clip open in the piano roll: a 2-px accent foot and the editor's mark after its name."""
    out = [rect(x + 1, y + h - 2, w - 2, 2, t['accent'], 1)]
    mx = lx + 5 + tw(name, 10, 500) + 6
    out.append(rect(mx, y + 1, 16, 11, mix(t['base'], t['accent'], 0.30), 2))
    out.append(AG.pencil_roll(mx + 8, y + 6.5, t['accent']))
    return ''.join(out)


def folder_summary(t, v, tr, y, h):
    """A folder's lane: each child's clips as one thin bar, in the child's colour — the group's
    shape at a glance, open or closed."""
    kids = [k for k in v.tracks if k.get('parent') == tr['name']]
    out = []
    n = len(kids)
    if not n:
        return ''
    pad = 7
    step = (h - 2 * pad) / n
    bh = max(2, min(5 if tr.get('open', True) else 8, step - (2 if tr.get('open', True) else 5)))
    spans = sorted((c[0], c[0] + c[1]) for kd in kids for c in kd['clips'])
    merged = []
    for a, b in spans:
        if merged and a <= merged[-1][1]:
            merged[-1][1] = max(merged[-1][1], b)
        else:
            merged.append([a, b])
    for a, b in merged:     # the group's body: where any child sounds, a quiet tint of the folder
        x0, x1 = v.bx(a), v.bx(b) - 1
        out.append(rect(x0, y + 3, x1 - x0, h - 7, hue_for(t, tr['hue'], 0.10), 3))
        out.append(rect(x0 + 1, y + 3, x1 - x0 - 2, 1, hue_for(t, tr['hue'], 0.30)))
    for i, kd in enumerate(kids):
        yy = y + pad + i * step + (step - bh) / 2
        for c in kd['clips']:
            x0 = max(v.cx, v.bx(c[0]))
            x1 = min(v.cx + v.cw, v.bx(c[0] + c[1]) - 1)
            if x1 > x0:
                col = hue_for(t, kd['hue'], 0.55 if not tr.get('mute') else 0.2)
                out.append(rect(x0, yy, x1 - x0, bh, col, 1.5))
    return ''.join(out)


def auto_lane(t, v, tr, ln, y, h):
    """An automation lane: a line over a faint fill, its points; one value between the clips'
    spans (held), drawn dashed where nothing is written."""
    bg = mix(t['base'], t['gutter'], 0.28)
    out = [rect(v.cx, y, v.cw, h - 1, bg)]
    out.append(grid(t, v, y, h - 1, mix(bg, t['lane2'], 0.5)))
    pad = 5
    ink = t['auto']

    def Y(val):
        return y + pad + (1 - val) * (h - 1 - 2 * pad)
    pts = []
    for c in ln['clips']:
        for b, val, k in S.auto_points(c[2], c[1]):
            pts.append(((c[0] - 1) * 4 + b, val, k))
    pts.sort()
    if not pts:      # nothing written yet: the value holds, a dashed line at it
        yv = Y(ln.get('hold', 0.5))
        col = t['rec'] if ln.get('mode') == 'Write' else t['ink3']
        out.append(rect(v.cx, yv, v.cw, 1, col, extra=' fill-opacity="0.8"'))
        if ln.get('mode') == 'Write' and v.head is not None:   # writing: the line follows the playhead in red
            hx = v.bx(v.head)
            out.append(rect(v.cx, y, hx - v.cx, h - 1, t['rec'], extra=' fill-opacity="0.06"'))
            pts_w = ' '.join(f'{x},{Y(0.5 + 0.18 * __import__("math").sin((x - v.cx) / 37)):.1f}' for x in range(int(v.bx(42)), int(hx)))
            out.append(f'<polyline points="{pts_w}" fill="none" stroke="{t["rec"]}" stroke-width="1.5"/>')
        out.append(rect(v.cx, y + h - 1, v.cw, 1, mix(t['base'], t['gutter'], 0.7)))
        return ''.join(out)

    def val_at(beat):
        if beat <= pts[0][0]:
            return pts[0][1]
        for (b0, v0, k), (b1, v1, _) in zip(pts, pts[1:]):
            if b0 <= beat <= b1:
                return S.auto_value([(0, v0, k), (b1 - b0, v1, 0)], beat - b0)
        return pts[-1][1]
    xs = range(int(v.cx), int(v.cx + v.cw) + 1)
    line = ' '.join(f'{x},{Y(val_at((v.bar0 - 1) * 4 + (x - v.cx) / v.ppb)):.1f}' for x in xs)
    out.append(f'<polygon points="{v.cx},{y + h - 1} {line} {v.cx + v.cw},{y + h - 1}" fill="{ink}" '
               f'fill-opacity="{0.10 if t["name"] == "Night" else 0.12}"/>')
    out.append(f'<polyline points="{line}" fill="none" stroke="{ink}" stroke-width="1.5" stroke-linejoin="round"/>')
    r = 2.5 if v.band() == 'default' else 3
    sel_pts = ln.get('sel_pts', ())
    for i, (b, val, k) in enumerate(pts):
        px = v.cx + (b - (v.bar0 - 1) * 4) * v.ppb
        if px < v.cx - 4 or px > v.cx + v.cw + 4:
            continue
        py = Y(val)
        out.append(f'<circle cx="{px:.1f}" cy="{py:.1f}" r="{r + 1.2}" fill="{bg}"/>')
        if i in sel_pts:
            out.append(f'<circle cx="{px:.1f}" cy="{py:.1f}" r="{r + 0.5}" fill="{t["ink1"]}" stroke="{ink}" stroke-width="1.2"/>')
        else:
            out.append(f'<circle cx="{px:.1f}" cy="{py:.1f}" r="{r}" fill="{ink}"/>')
    hv = ln.get('hover_seg')
    if hv is not None and hv + 1 < len(pts):
        b0, v0, k = pts[hv]
        b1 = pts[hv + 1][0]
        mb = (b0 + b1) / 2
        hx = v.cx + (mb - (v.bar0 - 1) * 4) * v.ppb
        hy = Y(val_at(mb))
        out.append(f'<rect x="{hx - 3:.1f}" y="{hy - 3:.1f}" width="6" height="6" transform="rotate(45 {hx:.1f} {hy:.1f})" '
                   f'fill="{bg}" stroke="{ink}" stroke-width="1.2"/>')
        out.append(tip(t, hx + 10, hy - 22, 'Curve  +0.40', 'drag up or down'))
    out.append(rect(v.cx, y + h - 1, v.cw, 1, mix(t['base'], t['gutter'], 0.7)))
    return ''.join(out)


def tip(t, x, y, a, b=None):
    """A tooltip: raised, r 4, one shadow; the value in ink 1, how in ink 3."""
    w = tw(a, 11, 500) + (tw(b, 10) + 10 if b else 0) + 14
    out = [f'<rect x="{x}" y="{y + 1.5}" width="{w}" height="19" rx="4" fill="#000" fill-opacity="{0.35 if t["name"] == "Night" else 0.10}"/>',
           rect(x, y, w, 19, t['raised'], 4)]
    if t['name'] == 'Paper':
        out.append(rect(x + 0.5, y + 0.5, w - 1, 18, 'none', 3.5, f' stroke="{t["line"]}"'))
    out.append(text(x + 7, y + 13.5, a, 11, t['ink1'], 500))
    if b:
        out.append(text(x + 14 + tw(a, 11, 500), y + 13.5, b, 10, t['ink3']))
    return ''.join(out)


def overview(t, v, y):
    """The song map under the canvas, which is also its horizontal scrollbar: each track a
    1-px line where it has clips (its colour at half strength), the part in view a lifted frame."""
    x0, w = v.cx, v.cw
    out = [rect(v.x, y, v.head_w, SCROLL_H, t['surface']), rect(x0, y, w, SCROLL_H, t['surface'])]
    bars = v.song_bars
    sx = w / bars
    tracks = [tr for tr in v.tracks if tr['kind'] != 'folder']
    n = len(tracks)
    top = y + 3
    lh = (SCROLL_H - 6) / max(1, n)
    for i, tr in enumerate(tracks):
        for c in tr['clips']:
            out.append(rect(x0 + (c[0] - 1) * sx, top + i * lh, c[1] * sx - 0.5, max(1, lh - 0.4),
                            hue_for(t, tr['hue'], 0.55 if not tr.get('sel') else 0.9)))
    a = x0 + (v.bar0 - 1) * sx
    b = x0 + min(bars, v.last_bar() - 1) * sx
    out.append(rect(a, y + 1, b - a, SCROLL_H - 2, 'none', 3, f' stroke="{t["ink3"]}" stroke-width="1"'))
    out.append(rect(a, y + 1, b - a, SCROLL_H - 2, t['ink1'], 3, ' fill-opacity="0.06"'))
    if v.head is not None:
        out.append(rect(x0 + (v.head - 1) * sx, y + 1, 1, SCROLL_H - 2, t['accent']))
    if v.loop and v.loop_on:
        out.append(rect(x0 + (v.loop[0] - 1) * sx, y, (v.loop[1] - v.loop[0]) * sx, 1, t['accent']))
    out.append(text(v.x + 10, y + 10.5, f'{bars} bars · 2:03', 9, t['ink3'], 500))
    return ''.join(out)
