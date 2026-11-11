"""des-mixer — the whole window with the mixer maximised: the top bar, the desk, the
inspector of the selected strip, the status line. 1280 x 720 logical."""
import copy
import data as D
import desk as DK
import insp as IN
import topbar
from insp_text import T
from base import defs, rect, text, tw, lit_edge, INSP_X

R = INSP_X - 2
Y0 = 46            # under the top bar and its 2-px seam
STATUS_Y = 700

FROZEN_STATE = dict(playing=False, loop=True, pos=('41', '1', '000'), time='1:17.42')
PLAY_STATE = dict(playing=True, loop=True, pos=('44', '3', '112'), time='1:24.68', master=(0.83, 0.80))


def desk_strips(playing=True, sel='Chords', solo=None, tweak=None):
    """The song's strips as the desk shows them (playing or idle), the cables of `sel` lit."""
    strips = [copy.deepcopy(s) for s in D.STRIPS]
    rets = [copy.deepcopy(s) for s in D.RETURNS]
    master = copy.deepcopy(D.MASTER)
    for s in strips + rets + [master]:
        if not playing:
            s['lv'] = None
        if s.get('name') == 'Chords':
            s['sidechain'] = 'Kick'
    by = {s['name']: s for s in strips + rets if s.get('name')}
    by['Master'] = master
    cmap = []
    if sel and sel in by:
        ss = by[sel]
        dest = ss['out']
        cmap.append((dest, 'out'))
        for nm, _, on in ss.get('sends', []):
            if on:
                cmap.append((nm, 'send'))
        if ss.get('sidechain'):
            cmap.append((ss['sidechain'], 'side'))
        for n, k in cmap:
            if n in by:
                by[n]['sock_lit'] = 'side' if k == 'side' else 'acc'
        ss['sock_lit'] = 'acc'
    if solo:
        for s in strips + rets:
            if s.get('name') and s['kind'] != 'empty':
                if s['name'] in solo:
                    s['solo'] = True
                elif s['kind'] in ('track',) and s['name'] not in ('Drum bus',):
                    s['implicit'] = True
    if tweak:
        tweak(by)
    return strips, rets, master, cmap


def link(all_strips, master, sel):
    """The cables of the selected strip in a custom desk; lights the sockets they reach."""
    by = {s['name']: s for s in all_strips if s.get('name')}
    by['Master'] = master
    by[master['name']] = master
    cmap = []
    if sel in by:
        ss = by[sel]
        cmap.append((ss['out'], 'out'))
        cmap += [(nm, 'send') for nm, _, on in ss.get('sends', []) if on]
        if ss.get('sidechain'):
            cmap.append((ss['sidechain'], 'side'))
        for n, k in cmap:
            if n in by:
                by[n]['sock_lit'] = 'side' if k == 'side' else 'acc'
        ss['sock_lit'] = 'acc'
    return cmap


def resolve_sockets(t, strips):
    for s in strips:
        v = s.get('sock_lit')
        if v == 'acc':
            s['sock_lit'] = t['accent']
        elif v == 'side':
            s['sock_lit'] = t['ink2']


def inspector(t, s, lang='en', open_=('inserts', 'eq', 'sends'), y0=Y0, h=STATUS_Y - Y0 - 2):
    L = T[lang]
    x, w = INSP_X, 1280 - INSP_X
    out, y = IN.head(t, x, w, y0, L, s)
    fns = {'inserts': IN.inserts, 'eq': IN.eq, 'sends': IN.sends, 'routing': IN.routing,
           'sidechain': IN.sidechain, 'recording': IN.recording, 'automation': IN.automation,
           'level': IN.level, 'freeze': IN.freeze}
    summary = {'inserts': (L['inserts'], L['of'].format(n=len(s['fx']), m=10)), 'eq': (L['eq'], '3 bands' if lang == 'en' else '3 bandas'),
               'sends': (L['sends'], str(len(s.get('sends', [])))),
               'routing': (L['routing'], L['master']), 'sidechain': (L['sidechain'], s.get('sidechain') or L['none']),
               'recording': (L['recording'], L['no_input']), 'automation': (L['automation'], L['lanes'].format(n=2)),
               'level': (L['level'], (f'{s["db"]:.1f} dB' if lang == 'en' else f'{s["db"]:.1f} dB'.replace('.', ',')).replace('-', '−')), 'freeze': (L['freeze'], '')}
    for k in ('inserts', 'eq', 'sends', 'routing', 'sidechain', 'recording', 'automation', 'level', 'freeze'):
        if k in open_:
            kw = {'mini': 0, 'hover': 1} if k == 'inserts' else {}
            svg, y = fns[k](t, x, w, y, L, s, **kw)
        else:
            svg, y = IN.folded(t, x, w, y, [summary[k]])
        out += svg
    clip = f'<clipPath id="insp-{t["name"]}"><rect x="{x}" y="{y0}" width="{w}" height="{h}"/></clipPath>'
    bar = ''
    total = y - y0
    if total > h:     # the inspector scrolls: a thin thumb at its right edge, under the head
        th = (h - 30) * h / total
        bar = rect(x + w - 5, y0 + 30, 3, th, t['ink4'], 1.5)
    return clip + f'<g clip-path="url(#insp-{t["name"]})">{out}</g>' + bar


def status(t, hint, lang='en'):
    y = STATUS_Y
    out = rect(0, y, 1280, 20, t['surface']) + lit_edge(t, 0, y, 1280)
    out += text(12, y + 14, hint[0], 11, t['ink1'])
    out += text(12 + tw(hint[0], 11) + 10, y + 14, hint[1], 11, t['ink3'])
    right = '48 kHz · 256 samples · 5.3 ms'
    out += text(1280 - 12, y + 14, 'Saved' if lang == 'en' else 'Guardado', 11, t['ink3'], 400, 'end')
    out += text(1280 - 12 - tw('Saved', 11) - 20, y + 14, right, 11, t['ink3'], 400, 'end')
    return out


def window(t, density='wide', playing=True, sel='Chords', solo=None, lang='en', tweak=None,
           strips=None, scroll=None, open_=('inserts', 'eq', 'sends'), hint=None, right=None):
    import parts as P_
    P_.set_lang(lang)
    st = PLAY_STATE if playing else FROZEN_STATE
    parts = [defs(t), rect(0, 0, 1280, 720, t['gutter']), topbar.topbar(t, st)]
    ss, rets, master, cmap = desk_strips(playing, sel, solo, tweak)
    if lang == 'es':
        master['name'] = 'Máster'
    if strips is not None:
        ss = strips
        if right is not None:
            rets = [s for s in right if s['kind'] == 'return']
        cmap = link(ss + list(right or rets), master, sel)
    resolve_sockets(t, ss + rets + [master] + list(right or []))
    rdock = right if right is not None else rets
    svg, pos = DK.desk(t, 0, Y0, R, STATUS_Y - 2 - Y0, density, ss, master, rdock, sel=sel, scroll=scroll,
                       lang=lang, soloed=len(solo) if solo else 0, tabs=True, maximised=True, cable_map=cmap)
    parts.append(svg)
    by = {s['name']: s for s in ss + rets if s.get('name')}
    if sel in by:
        parts.append(inspector(t, by[sel], lang, open_))
    P_.set_lang('en')
    parts.append(status(t, hint or ('Chords · volume −3.5 dB', 'drag to change · double-click to type · Alt-click for 0 dB'), lang))
    return ''.join(parts)


def svg(body, w=1280, h=720):
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">'
            f'{body}</svg>')
