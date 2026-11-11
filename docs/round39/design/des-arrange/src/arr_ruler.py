"""des-arrange — the top lanes: the ruler (bars and beats, the loop band, the playhead's cap,
time-signature changes), sections and markers, the tempo lane; and the corner over the headers.
"""
import aglyphs as AG
import glyphs as G
import song_arr as SA
from arr_geo import RULER_H, SECT_H, TEMPO_H, NAME_X
from base import rect, text, mix, tw, well


def label_every(ppbar):
    for n in (1, 2, 4, 8, 16):
        if ppbar * n >= 26:
            return n
    return 32


def corner(t, v):
    """The corner over the headers: add a track, the count, the top lanes' menu; then each top
    lane's own header."""
    x, y, w = v.x, v.y, v.head_w
    out = [rect(x, y, w, v.top_h, t['surface'])]
    out.append(well(t, x + 8, y + 3, 20, 18, 4))
    out.append(G.plus(x + 18, y + 12, 4, t['ink2'], 1.2))
    n = sum(1 for tr in v.tracks if tr['kind'] != 'folder')
    out.append(text(x + 36, y + 16, f'{n} tracks', 11, t['ink3']))
    out.append(AG.lanes(x + w - 20, y + 12, t['ink2'], t['ink3']))
    yy = y + RULER_H
    if v.sections:
        out.append(rect(x, yy, w, SECT_H, mix(t['surface'], t['base'], 0.35)))
        out.append(text(x + NAME_X - 20, yy + 14, 'Sections', 11, t['ink3']))
        yy += SECT_H
    if v.tempo:
        out.append(rect(x, yy, w, TEMPO_H, mix(t['surface'], t['base'], 0.55)))
        out.append(text(x + NAME_X - 20, yy + 16, 'Tempo', 11, t['ink2'], 500))
        out.append(text(x + w - 40, yy + 17, '124.00', 13, t['ink1'], 400, 'end'))
        out.append(text(x + w - 37, yy + 17, 'BPM', 9, t['ink3'], 600))
        out.append(text(x + NAME_X - 20, yy + 32, 'Metre 4/4', 10, t['ink3']))
        out.append(text(x + w - 14, yy + 32, '60–200', 10, t['ink3'], 400, 'end'))
    out.append(rect(x + w - 1, y, 1, v.top_h, mix(t['surface'], t['gutter'], 0.75)))
    return ''.join(out)


def ruler(t, v):
    x0, y = v.cx, v.y
    cw = v.cw
    cid = f'rul-{v.uid}-{t["name"]}'
    out = [rect(x0, y, cw, RULER_H, t['surface']),
           f'<clipPath id="{cid}"><rect x="{x0}" y="{y}" width="{cw}" height="{v.top_h}"/></clipPath><g clip-path="url(#{cid})">']
    every = label_every(v.ppbar)
    # the time selection, neutral, behind the numbers
    if v.sel_range:
        a, b = v.bx(v.sel_range[0]), v.bx(v.sel_range[1])
        out.append(rect(a, y + 7, b - a, RULER_H - 7, mix(t['surface'], t['ink1'], 0.08)))
        out.append(rect(a, y + 7, 1, RULER_H - 7, t['ink3']))
        out.append(rect(b - 1, y + 7, 1, RULER_H - 7, t['ink3']))
    # the loop band: the top 7 px, the accent when on, a quiet grey when off
    if v.loop:
        a, b = v.bx(v.loop[0]), v.bx(v.loop[1])
        on = v.loop_on
        fill = mix(t['surface'], t['accent'], 0.32 if t['name'] == 'Night' else 0.24) if on else mix(t['surface'], t['ink4'], 0.45)
        endc = t['accent'] if on else t['ink3']
        out.append(rect(a, y + 1, b - a, 5, fill, 1.5))
        out.append(rect(a, y + 1, 2, 7, endc, 1))
        out.append(rect(b - 2, y + 1, 2, 7, endc, 1))
        if v.hover.get('loop_end'):   # the end under the pointer: a wider grip, lit
            out.append(rect(b - 3, y, 4, 10, mix(endc, '#ffffff', 0.25), 1.5))
        if on:   # the looped bars keep a trace under the numbers
            out.append(rect(a, y + 6, b - a, RULER_H - 6, mix(t['surface'], t['accent'], 0.06)))
    b0 = int(v.bar0)
    b1 = int(v.last_bar()) + 2
    for bar in range(b0, b1):
        bx = v.bx(bar)
        lab = (bar - 1) % every == 0
        out.append(rect(bx, y + (8 if lab else 17), 1, RULER_H - (8 if lab else 17), t['ink3'] if lab else t['ink4']))
        if v.ppb >= 6:
            for q in range(1, 4):
                out.append(rect(bx + q * v.ppb, y + RULER_H - 4, 1, 4, t['ink4']))
        if v.ppb >= 40:
            for s in range(1, 16):
                if s % 4:
                    out.append(rect(bx + s * v.ppb / 4, y + RULER_H - 2, 1, 2, mix(t['surface'], t['ink4'], 0.7)))
            for q in range(1, 4):
                out.append(text(bx + q * v.ppb + 3, y + 17, f'{bar}.{q + 1}', 9, t['ink3']))
        if lab:
            cur = v.head is not None and bar <= v.head < bar + 1
            out.append(text(bx + 5, y + 17, str(bar), 11, t['ink1'] if cur else t['ink2'], 500))
        sig = next((s for b_, s in SA.SIGS if b_ == bar and bar > 1), None)
        if sig and v.ppbar >= 36 and not v.tempo:
            sx = bx + 5 + tw(str(bar), 11, 500) + 5
            out.append(rect(sx, y + 7, tw(sig, 9, 600) + 8, 13, t['raised'], 3))
            out.append(text(sx + 4, y + 17, sig, 9, t['ink2'], 600))
    # sections and markers
    yy = y + RULER_H
    if v.sections:
        out.append(sections(t, v, yy))
        yy += SECT_H
    if v.tempo:
        out.append(tempo(t, v, yy))
        yy += TEMPO_H
    # where play started (it returns there on stop): a hollow cap, ink 2
    st = v.hover.get('start')
    if st is not None:
        sx = round(v.bx(st))
        out.append(f'<path d="M{sx - 3.5} {y + 17.5}H{sx + 4.5}V{y + 20}L{sx + 0.5} {y + 23.5}L{sx - 3.5} {y + 20}Z" '
                   f'fill="{t["surface"]}" stroke="{t["ink2"]}" stroke-width="1"/>')
    sc = v.hover.get('scrub')
    if sc is not None:    # the pointer over the ruler: where a click would put the playhead
        gx = round(v.bx(sc))
        out.append(rect(gx, y + 6, 1, v.top_h - 6, t['ink3']))
    # the playhead: its cap at the ruler's foot, its line through the top lanes
    if v.head is not None:
        px = round(v.bx(v.head))
        out.append(rect(px, y + RULER_H, 1, v.top_h - RULER_H, t['accent']))
        out.append(f'<path d="M{px - 4} {y + 17}H{px + 5}V{y + 20}L{px + 0.5} {y + 24.5}L{px - 4} {y + 20}Z" fill="{t["accent"]}"/>')
    out.append('</g>')
    out.append(rect(x0, y + RULER_H - 1, cw, 1, mix(t['surface'], t['gutter'], 0.6)))
    return ''.join(out)


def sections(t, v, y):
    bg = mix(t['surface'], t['base'], 0.35)
    out = [rect(v.cx, y, v.cw, SECT_H, bg)]
    secs = SA.SECTIONS
    for k, (bar, name) in enumerate(secs):
        if name is None:
            continue
        nxt = secs[k + 1][0] if k + 1 < len(secs) else bar + 8
        a, b = v.bx(bar), v.bx(nxt)
        if b <= v.cx + 1 or a > v.cx + v.cw:
            continue
        cur = v.head is not None and bar <= v.head < nxt
        if k % 2:
            out.append(rect(a, y, b - a, SECT_H, mix(bg, t['ink1'], 0.035)))
        if v.hover.get('sect_hover') == bar:
            out.append(rect(a + 1, y + 1, b - a - 2, SECT_H - 2, mix(bg, t['ink1'], 0.08), 3))
            out.append(rect(b - 3, y + 5, 2, SECT_H - 10, t['ink3'], 1))
        if v.hover.get('sect_sel') == bar:
            out.append(rect(a + 1, y + 1, b - a - 2, SECT_H - 2, mix(bg, t['accent'], 0.20), 3))
            out.append(rect(a + 1, y + 1, b - a - 2, SECT_H - 2, 'none', 3, f' stroke="{t["accent"]}" stroke-width="1"'))
            cur = True
        out.append(rect(a, y + 2, 1, SECT_H - 2, t['ink3'] if cur else mix(bg, t['ink3'], 0.7)))
        lx = max(a, v.cx) + 6
        ink = t['ink1'] if cur else t['ink2']
        out.append(text(lx, y + 14, name, 11, ink, 500 if cur else 400))
        span = f'{nxt - bar} bars'
        if b - lx > tw(name, 11) + tw(span, 10) + 24 and v.ppbar >= 20:
            out.append(text(lx + tw(name, 11, 500) + 7, y + 14, span, 10, t['ink3']))
    for bar, name in SA.MARKERS:
        mx = round(v.bx(bar))
        out.append(rect(mx, y + 2, 1, SECT_H - 2, t['ink2']))
        out.append(AG.marker(mx + 1, y + 3, t['ink2']))
        if v.ppbar >= 20:     # far out, a marker is its pennant; the name shows under the pointer
            out.append(text(mx + 10, y + 14, name, 10, t['ink2']))
    out.append(rect(v.cx, y + SECT_H - 1, v.cw, 1, mix(t['surface'], t['gutter'], 0.6)))
    return ''.join(out)


def tempo_at(bar):
    pts = SA.TEMPO
    for (b0, v0), (b1, v1) in zip(pts, pts[1:]):
        if b0 <= bar <= b1:
            return v0 + (v1 - v0) * (bar - b0) / max(1e-6, b1 - b0)
    return pts[-1][1]


def tempo(t, v, y):
    bg = mix(t['surface'], t['base'], 0.55)
    out = [rect(v.cx, y, v.cw, TEMPO_H, bg)]
    lo, hi = 112, 132          # the lane's view of the range (it zooms to what the song uses)
    pad = 7

    def Y(bpm):
        return y + pad + (hi - bpm) / (hi - lo) * (TEMPO_H - 2 * pad)
    ink = t['ink2']
    xs = range(int(v.cx), int(v.cx + v.cw) + 1, 2)
    line = ' '.join(f'{x},{Y(tempo_at(v.bar0 + (x - v.cx) / v.ppbar)):.1f}' for x in xs)
    out.append(f'<polygon points="{v.cx},{y + TEMPO_H} {line} {v.cx + v.cw},{y + TEMPO_H}" fill="{ink}" fill-opacity="0.07"/>')
    out.append(f'<polyline points="{line}" fill="none" stroke="{ink}" stroke-width="1.5" stroke-linejoin="round"/>')
    for bar, bpm in SA.TEMPO:
        px = v.bx(bar)
        if v.cx - 4 <= px <= v.cx + v.cw:
            out.append(f'<circle cx="{px:.1f}" cy="{Y(bpm):.1f}" r="3.5" fill="{bg}"/>')
            out.append(f'<circle cx="{px:.1f}" cy="{Y(bpm):.1f}" r="2.5" fill="{ink}"/>')
            out.append(text(px + 6, Y(bpm) - 5 if bpm > 122 else Y(bpm) + 13, f'{bpm:.0f}', 10, t['ink2'], 500))
    for bar, sig in SA.SIGS[1:]:      # metre changes ride on the tempo lane's foot when it is shown
        sx = v.bx(bar)
        if v.cx <= sx <= v.cx + v.cw - 20:
            out.append(rect(sx, y + TEMPO_H - 15, 1, 14, t['ink3']))
            out.append(rect(sx + 1, y + TEMPO_H - 15, 24, 13, t['raised'], 2))
            out.append(text(sx + 5, y + TEMPO_H - 5, sig, 9, t['ink2'], 600))
    out.append(rect(v.cx, y + TEMPO_H - 1, v.cw, 1, mix(t['surface'], t['gutter'], 0.6)))
    return ''.join(out)
