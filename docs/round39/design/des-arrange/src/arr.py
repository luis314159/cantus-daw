"""des-arrange — the arrangement panel, whole: corner and top lanes, headers, lanes, the time
selection, the playhead, the scrollbars. Everything below the panel's tool row."""
import arr_canvas as C
import arr_head as H
import arr_ruler as R
from arr_geo import ADD_H, SCROLL_H
from base import rect, text, mix, tw

LEVELS = {'Kit': 0.62, 'Percussion': 0.48, 'Bass': 0.66, 'Chords': 0.58, 'Pluck': 0.44, 'Lead': 0.55,
          'Choir': 0.40, 'Drums': 0.70, 'Vox': 0.0}


def panel(t, v, levels=None):
    out = []
    rows, end_y = v.rows()
    ly, lh = v.lanes_y, v.lanes_h
    # the canvas: base and grid over the whole height, so the space under the tracks still reads
    out.append(rect(v.cx, ly, v.cw, lh, t['base']))
    cid = f'cv-{v.uid}-{t["name"]}'
    out.append(f'<clipPath id="{cid}"><rect x="{v.cx}" y="{ly}" width="{v.cw}" height="{lh}"/></clipPath>')
    out.append(f'<g clip-path="url(#{cid})">')
    out.append(C.grid(t, v, ly, lh))
    if end_y < ly + lh:    # below the last track the grid steps back
        out.append(rect(v.cx, end_y, v.cw, ly + lh - end_y, t['base'], extra=' fill-opacity="0.55"'))
        if ly + lh - end_y >= 90 and v.tracks:
            mx = v.cx + v.cw / 2
            my = end_y + (ly + lh - end_y) / 2
            out.append(text(mx, my, 'Drop a sound, a loop or a MIDI file here for a new track', 12, t['ink3'], 400, 'middle',
                            extra=f' stroke="{t["base"]}" stroke-width="6" paint-order="stroke" stroke-linejoin="round"'))
    end_x = v.bx(v.song_bars + 1)
    if end_x < v.cx + v.cw:    # after the song's end the canvas steps back too
        out.append(rect(end_x, ly, v.cx + v.cw - end_x, lh, t['gutter'], extra=' fill-opacity="0.35"'))
        out.append(rect(end_x, ly, 1, lh, t['ink4']))
    for kind, tr, ln, y, h in rows:
        if y > ly + lh or y + h < ly:
            continue
        if kind == 'track':
            out.append(C.track_lane(t, v, tr, y, h))
            out.append(rect(v.cx, y + h - 1, v.cw, 1, mix(t['base'], t['gutter'], 0.55)))
        else:
            out.append(C.auto_lane(t, v, tr, ln, y, h))
    # the time selection: a veil over the chosen rows, with edges
    if v.sel_range:
        a, b, i0, i1 = v.sel_range
        ys = [r for r in rows if r[0] == 'track']
        y0, y1 = ys[i0][3], ys[i1][3] + ys[i1][4]
        xa, xb = v.bx(a), v.bx(b)
        out.append(rect(xa, y0, xb - xa, y1 - y0, t['ink1'], extra=f' fill-opacity="{0.07 if t["name"] == "Night" else 0.06}"'))
        out.append(rect(xa, y0, 1, y1 - y0, t['ink3']))
        out.append(rect(xb - 1, y0, 1, y1 - y0, t['ink3']))
    if v.head is not None:
        px = round(v.bx(v.head))
        out.append(rect(px, ly, 1, lh, t['accent']))
    out.append(v.hover.get('canvas_extra', ''))
    out.append('</g>')
    # the headers
    hid = f'hd-{v.uid}-{t["name"]}'
    out.append(rect(v.x, ly, v.head_w, lh, mix(t['surface'], t['base'], 0.55)))
    out.append(f'<clipPath id="{hid}"><rect x="{v.x}" y="{ly}" width="{v.head_w}" height="{lh}"/></clipPath><g clip-path="url(#{hid})">')
    num = 0
    lv = levels if levels is not None else (LEVELS if v.playing else {})
    for kind, tr, ln, y, h in rows:
        if kind == 'track':
            if tr['kind'] != 'folder':
                num += 1
            out.append(H.header(t, v.x, y, v.head_w, h, tr, num, hover=(v.hover_track == tr['name']),
                                level=lv.get(tr['name'])))
        else:
            out.append(H.lane_header(t, v.x, y, v.head_w, h, tr, ln, hover=v.hover.get('lane') == ln['param']))
    if v.add_row:
        out.append(H.add_row(t, v.x, end_y, v.head_w, ADD_H, hover=v.hover.get('add', False)))
    out.append('</g>')
    # the splitter between headers and canvas: value only (the canvas is darker)
    out.append(rect(v.cx - 1, ly, 1, lh, mix(t['surface'], t['gutter'], 0.8)))
    # the top lanes and their corner
    out.append(R.corner(t, v))
    out.append(R.ruler(t, v))
    # vertical scroll thumb when the rows overflow
    total = end_y + (ADD_H if v.add_row else 0) - (ly - v.scroll_y)
    if total > lh:
        th = lh * lh / total
        ty = ly + (lh - th) * (v.scroll_y / max(1, total - lh))
        out.append(rect(v.x + v.w - 5, ty + 2, 3, th - 4, t['ink4'], 1.5))
    if v.overview:
        out.append(C.overview(t, v, ly + lh))
    return ''.join(out)
