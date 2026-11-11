"""des-arrange — the ruler and the top lanes: the whole stack, the ruler at every zoom, the loop,
the time selection, the playhead, sections and markers, the tempo lane, the lanes menu."""
import copy
import aglyphs as AG
import arr as AR
import arr_canvas as C
import arr_ruler as R
import sheet_kit as K
import song_arr as SA
from arr_geo import View, RULER_H, SECT_H
from base import rect, text, mix

W = 1280
X0, CW = 24, 1232


def two(names=('Chords', 'Lead')):
    by = {tr['name']: tr for tr in SA.song('docked')}
    out = []
    for n in names:
        tr = copy.deepcopy(by[n])
        tr.update(lanes=[], depth=0)
        tr.pop('parent', None)
        out.append(tr)
    return out


def whole(t, y):
    out = [K.section(t, X0, y, 'The top lanes, whole', 'ruler 24 · sections and markers 20 · tempo 40 (shown from the lanes menu) · '
                     'playing at 44.3, started at 41, looping 41–49')]
    y += 12
    trs = two()
    v = View(X0, y, CW, 24 + 20 + 40 + 2 * 44 + 14 + 28, trs, bar0=21.0, ppbar=28.0, head=44.55, loop=(41, 49),
             tempo=True, playing=True, uid='rw', hover=dict(start=41))
    out.append(AR.panel(t, v))
    y += v.h
    # call-outs under it
    cy = y + 16
    out.append(K.cap(t, X0, cy, 'Ruler', 'bar numbers Medium 11 in ink 2 (the playhead\'s bar in ink 1); beats 4-px ticks; '
                     'the loop band the top 5 px', w=230))
    out.append(K.cap(t, X0 + 260, cy, 'Sections', 'a name, its length when there is room; every other section a step '
                     'lighter; the playing one in ink 1', w=230))
    out.append(K.cap(t, X0 + 520, cy, 'Markers', 'a pennant and a name, ink 2, on the sections\' lane', w=200))
    out.append(K.cap(t, X0 + 760, cy, 'Tempo', 'a line in ink 2 with its points; the lane zooms to the range the song uses', w=220))
    out.append(K.cap(t, X0 + 1000, cy, 'Metre', 'a change is a chip on the tempo lane\'s foot (or after the bar number when the lane is hidden): 2/4 at 56', w=220))
    return ''.join(out), cy + 44


ZOOMS = [('Far', 7.0, 1.0, 'labels every 4 bars; sections only'), ('Default', 44.0, 37.0, 'every bar, beats ticked'),
         ('Near', 120.0, 41.0, 'beats ticked, sixteenths in the grid'), ('Close', 360.0, 41.0, 'beats named: 41.2, 41.3 …')]


def zooms(t, y):
    out = [K.section(t, X0, y, 'The ruler at every zoom', 'the same rule as the clips: far < 6 px a beat, default 6–20, near 20–60, close ≥ 60')]
    y += 12
    for k, (nm, pp, b0, what) in enumerate(ZOOMS):
        v = View(X0, y, CW, RULER_H + SECT_H, [], bar0=b0, ppbar=pp, head=41.0, loop=(41, 49), uid=f'z{k}',
                 overview=False)
        out.append(R.corner(t, v))
        out.append(R.ruler(t, v))
        out.append(rect(X0, y, 200, RULER_H + SECT_H, t['surface']))
        out.append(K.cap(t, X0 + 12, y + 16, f'{nm} · {pp:g} px a bar', what, w=180))
        y += RULER_H + SECT_H + 10
    return ''.join(out), y + 6


def ruler_box(t, x, y, w, b0, pp, uid, **kw):
    hv = kw.pop('hover', {})
    v = View(x - 200, y, w + 200, RULER_H + SECT_H, kw.pop('tracks', []), bar0=b0, ppbar=pp, uid=uid, overview=False,
             hover=hv, **kw)
    return R.ruler(t, v), v


def loops(t, y):
    out = [K.section(t, X0, y, 'The loop', 'drag in the ruler\'s top band to set it; its ends move it; click the band: on or off')]
    y += 14
    w = 380
    cases = [('On', dict(loop=(41, 45), loop_on=True), {}), ('Off', dict(loop=(41, 45), loop_on=False), {}),
             ('An end under the pointer', dict(loop=(41, 45), loop_on=True), dict(loop_end=True))]
    for k, (lab, kw, hv) in enumerate(cases):
        x = X0 + k * (w + 26)
        svg, v = ruler_box(t, x, y + 6, w, 40.0, 60.0, f'lp{k}', head=None, hover=hv, **kw)
        out.append(svg)
        out.append(text(x, y + 6 + RULER_H + SECT_H + 14, lab, 11, t['ink1'], 500))
        if hv.get('loop_end'):
            ex = v.bx(45)
            out.append(AG.cursor_lr(ex, y + 6 + 16, t['ink1'] if t['name'] == 'Night' else '#ffffff',
                                    t['gutter'] if t['name'] == 'Night' else t['ink1']))
            out.append(C.tip(t, ex - 150, y + 6 + 26, 'Loop 41 – 45', '4 bars · 0:07.74'))
    return ''.join(out), y + 6 + RULER_H + SECT_H + 30


def playheads(t, y):
    out = [K.section(t, X0, y, 'The playhead', 'its cap in the accent at the ruler\'s foot; where play started, a hollow cap')]
    y += 14
    w = 380
    cases = [('Stopped at 43', dict(head=43.0), {}), ('Playing, started at 41', dict(head=43.6, playing=True), dict(start=41.0)),
             ('The pointer over the ruler', dict(head=43.0), dict(scrub=44.5))]
    for k, (lab, kw, hv) in enumerate(cases):
        x = X0 + k * (w + 26)
        svg, v = ruler_box(t, x, y + 6, w, 40.0, 60.0, f'ph{k}', loop=None, hover=hv, **kw)
        out.append(svg)
        out.append(text(x, y + 6 + RULER_H + SECT_H + 14, lab, 11, t['ink1'], 500))
        if hv.get('scrub'):
            gx = v.bx(44.5)
            out.append(C.tip(t, gx + 8, y + 6 + RULER_H + 2, '44.3.1', '1:22.06 · click to go there'))
    return ''.join(out), y + 6 + RULER_H + SECT_H + 30


def sections(t, y):
    out = [K.section(t, X0, y, 'Sections and markers', 'a section is a span: drag it to move its bars with every track; its end to resize it')]
    y += 14
    w = 590
    cases = [('A section under the pointer', dict(sect_hover=41)), ('A section selected, being moved', dict(sect_sel=49))]
    for k, (lab, hv) in enumerate(cases):
        x = X0 + k * (w + 28)
        svg, v = ruler_box(t, x, y + 6, w, 39.0, 36.0, f'sc{k}', head=None, loop=None, hover=hv)
        out.append(svg)
        out.append(text(x, y + 6 + RULER_H + SECT_H + 14, lab, 11, t['ink1'], 500))
        if hv.get('sect_sel'):
            out.append(C.tip(t, v.bx(49) + 40, y + 6 + RULER_H + SECT_H + 2, 'Drop 2 → bar 53', 'Ctrl: a copy · Shift: insert'))
        else:
            out.append(C.tip(t, v.bx(41) + 70, y + 6 + RULER_H + SECT_H + 2, 'Drop · 8 bars', 'double-click to rename'))
    return ''.join(out), y + 6 + RULER_H + SECT_H + 30


def lanes_menu(t, y):
    out = [K.section(t, X0, y, 'The top lanes\' menu', 'the corner\'s key; what shows above the tracks')]
    y += 14
    v = View(X0, y + 4, 200, RULER_H + SECT_H, SA.song('docked'), uid='lm', overview=False)
    out.append(R.corner(t, v))
    out.append(rect(X0 + 200 - 30, y + 4 + 3, 20, 18, mix(t['surface'], t['ink1'], 0.10), 4))
    out.append(AG.lanes(X0 + 180, y + 4 + 12, t['ink1'], t['ink2']))
    m, mh = K.menu(t, X0 + 96, y + 30, [('Sections and markers', 'Alt S', ('check',)), ('Tempo', 'Alt T', ('check',)),
                                        ('Metre changes', '', ('check',)), '-', ('Time in bars and beats', '', ('check',)),
                                        ('Time in minutes and seconds', '', ()), '-', ('Add a marker at the playhead', 'M', ())],
                   250, 'lm', hot='Tempo')
    out.append(m)
    return ''.join(out), y + 30 + mh


def sheet(t):
    out = [K.frame(t, W, 2000, 'The arrangement — the ruler and the top lanes',
                   'Slate ' + t['name'] + ' · des-arrange · round 39 · logical px at 1:1')]
    s, y = whole(t, 84)
    out.append(s)
    s, y = zooms(t, y + 14)
    out.append(s)
    s, y = loops(t, y + 14)
    out.append(s)
    s, y = playheads(t, y + 14)
    out.append(s)
    s, y = sections(t, y + 14)
    out.append(s)
    s, y = lanes_menu(t, y + 14)
    out.append(s)
    return ''.join(out), y + 24
