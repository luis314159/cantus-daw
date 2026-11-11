"""des-arrange — the main window, whole, with the arrangement as its subject: the top bar
(dir-a's, approved), the arrangement's tool row (dir-a's toolbar, approved), the arrangement
(drawn here), the mixer docked below and the inspector (des-mixer's), the status line.

Any logical size: 1920 × 1080 at 100 %, 1536 × 864 at 125 % (both 1920 × 1080 physical)."""
import arr as AR
import desk as DK
import dwin as DW
import insp as IN
import song_arr as SA
import toolbar2
import topbar2
from arr_geo import View
from base import defs, rect, text, tw, lit_edge
from insp_text import T as INSP_T

IDLE = dict(playing=False, transport='stopped', loop=True, pos=('41', '1', '000'), time='1:17.42', head=41.0,
            master=None, cpu=4, tool='select', follow=True, grid='1 bar')
PLAYING = dict(IDLE, playing=True, transport='playing', cpu=11, pos=('44', '3', '112'), time='1:24.68', head=44.55,
               master=(0.83, 0.80), meters=True)
INSP_W = 246
STATUS_H = 20


def inspector(t, s, x, y0, h, open_=('inserts', 'eq', 'sends')):
    L = INSP_T['en']
    w = INSP_W
    out, y = IN.head(t, x, w, y0, L, s)
    fns = {'inserts': IN.inserts, 'eq': IN.eq, 'sends': IN.sends}
    summary = {'routing': (L['routing'], L['master']), 'sidechain': (L['sidechain'], s.get('sidechain') or L['none']),
               'recording': (L['recording'], L['no_input']), 'automation': (L['automation'], L['lanes'].format(n=1)),
               'level': (L['level'], f'{s["db"]:.1f} dB'.replace('-', '−')), 'freeze': (L['freeze'], '')}
    for k in ('inserts', 'eq', 'sends', 'routing', 'sidechain', 'recording', 'automation', 'level', 'freeze'):
        if k in open_:
            kw = {'mini': 0, 'hover': None} if k == 'inserts' else {}
            svg, y = fns[k](t, x, w, y, L, s, **kw)
        else:
            svg, y = IN.folded(t, x, w, y, [summary[k]])
        out += svg
    cid = f'insp-{t["name"]}-{x}'
    return (rect(x, y0, w, h, t['surface']) + f'<clipPath id="{cid}"><rect x="{x}" y="{y0}" width="{w}" height="{h}"/></clipPath>'
            + f'<g clip-path="url(#{cid})">{out}</g>')


def status(t, W, H, hint):
    y = H - STATUS_H
    out = rect(0, y, W, STATUS_H, t['surface']) + lit_edge(t, 0, y, W)
    out += text(12, y + 14, hint[0], 11, t['ink1'])
    out += text(12 + tw(hint[0], 11) + 10, y + 14, hint[1], 11, t['ink3'])
    out += text(W - 12, y + 14, 'Saved', 11, t['ink3'], 400, 'end')
    out += text(W - 12 - tw('Saved', 11) - 20, y + 14, '48 kHz · 256 samples · 5.3 ms', 11, t['ink3'], 400, 'end')
    return out


def window(t, W, H, playing=False, docked=True, mix_h=None, bar0=None, ppbar=44.0, tracks=None, inspector_on=True,
           view_kw=None, hint=None):
    st = PLAYING if playing else IDLE
    parts = [defs(t), rect(0, 0, W, H, t['gutter'])]
    bar_svg, forms, more_x = topbar2.bar(t, W, st)
    parts.append(bar_svg)
    AW = W - (INSP_W + 2 if inspector_on else 0)
    top = 46
    parts.append(toolbar2.row(t, AW, st, top)[0])
    status_y = H - STATUS_H
    if docked:
        mh = mix_h or round((status_y - top) * 0.36)
        mix_y = status_y - 2 - mh
    else:
        mix_y = status_y - 28 - 2       # the docks folded to their tabs
    tracks = tracks if tracks is not None else SA.song('docked' if docked else 'max')
    vk = dict(head=st['head'], playing=playing, uid=f'w{W}{int(docked)}')
    vk.update(view_kw or {})
    v = View(0, top + 28, AW, mix_y - 2 - top - 28, tracks,
             bar0=bar0 if bar0 is not None else 33.0, ppbar=ppbar, **vk)
    parts.append(AR.panel(t, v))
    ss, rets, master, cmap = DW.desk_strips(playing, 'Chords')
    DW.resolve_sockets(t, ss + rets + [master])
    if docked:
        svg, pos = DK.desk(t, 0, mix_y, AW, status_y - 2 - mix_y, 'wide', ss, master, rets, sel='Chords',
                           tabs=True, maximised=False, cable_map=cmap)
        parts.append(svg)
    if not docked:
        parts.append(folded_dock(t, AW, mix_y))
    if inspector_on:
        by = {s['name']: s for s in ss + rets if s.get('name')}
        sel = dict(by['Chords'], inst='Calor', preset='Glass pad')
        parts.append(inspector(t, sel, W - INSP_W, top, status_y - 2 - top))
    parts.append(status(t, W, H, hint or ('Chords · Stabs', 'click to select · double-click to open in the piano roll · drag to move')))
    return ''.join(parts)


def folded_dock(t, w, y):
    """The bottom docks folded: their tabs on one 28-px row; a click or F9 brings them back."""
    import glyphs as G
    out = rect(0, y, w, 28, t['surface']) + lit_edge(t, 0, y, w)
    x = 12
    for lab, key in (('Mixer', 'F9'), ('Piano roll', 'F7'), ('Channel rack', 'F6')):
        out += text(x, y + 18, lab, 12, t['ink2'])
        x += tw(lab, 12) + 6
        out += text(x, y + 18, key, 9, t['ink3'], 500)
        x += tw(key, 9, 500) + 22
    out += (f'<path d="M{w - 23} {y + 15.5}L{w - 20} {y + 12.5}L{w - 17} {y + 15.5}" fill="none" stroke="{t["ink2"]}" '
            f'stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>')
    out += text(w - 32, y + 18, 'Show the docks', 11, t['ink3'], 400, 'end')
    return out


def svg(body, W, H):
    return f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="0 0 {W} {H}">{body}</svg>'
