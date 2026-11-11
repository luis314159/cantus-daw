"""des-arrange — the canvas: the grid at every zoom, selection, the clip open in the editor,
moving and resizing, automation lanes, and the empty song (what a new user sees)."""
import copy
import aglyphs as AG
import arr as AR
import arr_canvas as C
import clipdraw as CD
import glyphs as G
import sheet_kit as K
import song_arr as SA
from arr_geo import View
from base import rect, text, mix, tw, well, hue_for, HUES

W = 1280
X0, CW = 24, 1232


def pick(*names, **upd):
    by = {tr['name']: tr for tr in SA.song('docked')}
    out = []
    for n in names:
        tr = copy.deepcopy(by[n])
        tr.update(lanes=[], depth=0, sel=False)
        tr.pop('parent', None)
        tr.update(upd.get(n, {}))
        out.append(tr)
    return out


def panel_h(trs, sections=True, tempo=False):
    return 24 + (20 if sections else 0) + sum(tr['h'] + sum(l['h'] for l in tr['lanes']) for tr in trs)


ZOOMS = [('Far', 7.0, 1.0, 'MIDI as its contour, audio as loudness'), ('Default', 44.0, 37.0, 'notes in rows, audio as its envelope'),
         ('Near', 120.0, 41.0, 'notes at their length, sixteenths in the grid'), ('Close', 360.0, 41.0, 'every attack; the 16th lines')]


def zooms(t, y):
    out = [K.section(t, X0, y, 'The grid at every zoom', '4-bar groups by value · bars · beats from 6 px · sixteenths from 40 px a beat · '
                     'a section\'s start a step stronger · clip content is des-views\' rule, unchanged')]
    y += 12
    for k, (nm, pp, b0, what) in enumerate(ZOOMS):
        trs = pick('Kit', 'Chords', 'Choir')
        v = View(X0, y, CW, panel_h(trs), trs, bar0=b0, ppbar=pp, head=None, loop=(41, 49), uid=f'gz{k}', overview=False,
                 add_row=False)
        out.append(AR.panel(t, v))
        out.append(rect(X0, y, 200, 24, t['surface']))
        out.append(text(X0 + 10, y + 16, f'{nm} · {pp:g} px a bar', 11, t['ink1'], 500))
        out.append(rect(X0, y + 24, 200, 20, mix(t['surface'], t['base'], 0.35)))
        out.append(text(X0 + 10, y + 38, what, 10, t['ink3']))
        y += v.h + 10
    return ''.join(out), y


def selection(t, y):
    out = [K.section(t, X0, y, 'Selection', 'clips are lit and edged; a time range is a neutral veil with edges; the clip open in the '
                     'piano roll carries the accent at its foot and the editor\'s mark')]
    y += 12
    w = 604
    trs = pick('Bass', 'Chords', 'Pluck')
    v = View(X0, y, w, panel_h(trs), trs, bar0=40.0, ppbar=50.0, head=None, loop=None, uid='sl1', overview=False, add_row=False,
             sel_clips=[('Bass', 1), ('Pluck', 3)], edited=('Chords', 4))
    out.append(AR.panel(t, v))
    out.append(K.cap(t, X0, y + v.h + 16, 'Two clips selected, one open in the piano roll',
                     'Bass and Arp selected (edge, name); Stabs is in the piano roll (accent foot, mark)', w=560))
    x2 = X0 + w + 24
    trs = pick('Bass', 'Chords', 'Pluck')
    v2 = View(x2, y, w, panel_h(trs), trs, bar0=40.0, ppbar=50.0, head=None, loop=None, uid='sl2', overview=False,
              add_row=False, sel_range=(43, 46, 0, 2))
    out.append(AR.panel(t, v2))
    out.append(C.tip(t, v2.bx(43) + 6, y + 24 + 20 + 132 - 24, '43.1 – 46.1', '3 bars · 3 tracks'))
    out.append(K.cap(t, x2, y + v.h + 16, 'A time range across three tracks',
                     'drag on empty canvas, or Shift-drag in the ruler; Ctrl D repeats it, Del clears it', w=560))
    return ''.join(out), y + v.h + 48


def moving(t, y):
    out = [K.section(t, X0, y, 'Moving and resizing', 'nothing is drawn on a clip until the pointer finds it; then the part it would move')]
    y += 12
    w = 604
    trs = pick('Bass', 'Chords', 'Percussion')
    # the Chords clip at 41 is being moved to 45: drawn by hand (ghost + lifted copy)
    moved = trs[1]['clips'].pop(4)
    trs[1]['clips'] = [c for c in trs[1]['clips'] if c[0] != 49]
    v = View(X0, y, w, panel_h(trs), trs, bar0=40.0, ppbar=50.0, head=None, loop=None, uid='mv1', overview=False, add_row=False)
    row_y = y + 44 + 44
    ex = []
    gx, gw = v.bx(41), 8 * 50 - 1
    ex.append(rect(gx + 0.5, row_y + 1.5, gw - 1, 40, 'none', 3, f' stroke="{t["ink3"]}" stroke-width="1" stroke-dasharray="4 3"'))
    nx = v.bx(44)
    ex.append(f'<rect x="{nx + 1}" y="{row_y + 4}" width="{gw}" height="41" rx="3" fill="#000" fill-opacity="{0.35 if t["name"] == "Night" else 0.12}"/>')
    ex.append(CD.clip(t, ('Chords', 'Chords', 'chords'), (44, 8, 'Stabs'), nx, row_y - 1, gw, 41, v.ppb, v.band(), 'sel', 'mvc'))
    ex.append(rect(nx, y + 24 + 20, 1, v.h - 44, t['accent']))
    ex.append(AG.cursor_hand(nx + 70, row_y + 22, t['ink1'] if t['name'] == 'Night' else '#ffffff',
                             t['gutter'] if t['name'] == 'Night' else t['ink1']))
    v.hover['canvas_extra'] = ''.join(ex)
    out.append(AR.panel(t, v))
    out.append(C.tip(t, nx - 150, row_y - 34, 'Stabs → 44.1', '+3 bars · snapped · Alt: free'))
    out.append(K.cap(t, X0, y + v.h + 16, 'Moving', 'the slot it left is dashed; the clip rides lifted with a shadow; the snap line '
                     'is the accent through every row', w=560))
    # resizing and audio fades
    x2 = X0 + w + 24
    trs = pick('Bass', 'Chords', 'Percussion')
    v2 = View(x2, y, w, panel_h(trs), trs, bar0=40.0, ppbar=50.0, head=None, loop=None, uid='mv2', overview=False, add_row=False)
    ex = []
    cy = y + 44 + 44
    rx = v2.bx(49) - 1
    ex.append(rect(rx - 5, cy + 2, 4, 39, mix(HUES['Chords'], '#ffffff', 0.2), 1.5))
    ex.append(AG.cursor_lr(rx + 4, cy + 26, t['ink1'] if t['name'] == 'Night' else '#ffffff',
                           t['gutter'] if t['name'] == 'Night' else t['ink1']))
    tips = [C.tip(t, rx - 236, cy + 14, 'Length 8 bars', 'drag the edge · Alt: stretch')]
    # the loop handle at the clip's top right
    ex.append(f'<path d="M{rx - 10} {cy + 2}H{rx - 1}V{cy + 11}Z" fill="{mix(HUES["Chords"], "#ffffff", 0.2)}"/>')
    ay = y + 44 + 88
    fx = v2.bx(41)
    ex.append(f'<path d="M{fx + 1} {ay + 15}H{fx + 44}Q{fx + 14} {ay + 18} {fx + 1} {ay + 42}Z" fill="{t["base"]}" fill-opacity="0.6"/>')
    ex.append(f'<path d="M{fx + 1} {ay + 42}Q{fx + 14} {ay + 18} {fx + 44} {ay + 15}" fill="none" stroke="{t["ink1"]}" stroke-width="1.2"/>')
    ex.append(rect(fx + 41, ay + 12, 7, 7, t['ink1'], 1.5))
    tips.append(C.tip(t, fx + 50, ay + 14, 'Fade in 0:00.12', 'drag the corner'))
    ex.append(rect(fx + 1, ay + 26, v2.bx(49) - fx - 2, 1, t['ink2'], extra=' fill-opacity="0.6"'))
    ex.append(text(v2.bx(46) + 4, ay + 24, '−2.0 dB', 9, t['ink2'], 600))
    v2.hover['canvas_extra'] = ''.join(ex)
    out.append(AR.panel(t, v2))
    out += tips
    out.append(K.cap(t, x2, y + v2.h + 16, 'Resizing, looping, fades and gain',
                     'an edge under the pointer lights 4 px; the corner notch loops the clip; an audio clip shows its fade '
                     'corner and its gain line', w=560))
    return ''.join(out), y + v.h + 60


def automation(t, y):
    out = [K.section(t, X0, y, 'Automation lanes', 'under their track · the lane\'s header names the parameter, its owner, the value '
                     'at the playhead, the mode · points r 2.5, r 3 nearer')]
    y += 12
    trs = pick('Chords', Chords=dict(sel=True, auto=True, lanes_open=True))
    cut = SA.A('Cutoff', 'Calor', '688 Hz', [(25, 8, 'Cutoff rise'), (41, 16, 'Cutoff')], h=56, sel_pts=(3, 4), hover_seg=5)
    snd = SA.A('Reverb send', 'Mixer', '−16.0 dB', [(41, 16, 'Send')], h=36, mode='Touch')
    wr = SA.A('Pan', 'Mixer', 'L 12', [], h=36, mode='Write')
    trs[0]['lanes'] = [cut, snd, wr]
    v = View(X0, y, CW, panel_h(trs), trs, bar0=37.0, ppbar=60.0, head=44.3, loop=None, uid='au', overview=False,
             add_row=False, playing=False, hover=dict(lane='Cutoff'))
    out.append(AR.panel(t, v))
    out.append(K.cap(t, X0, y + v.h + 16, 'Read · Touch · Write', 'Read in ink 2; Touch and Latch in violet; Write in red. '
                     'Selected points are filled ink 1 in a violet ring; a segment under the pointer shows its curve handle.', w=900))
    return ''.join(out), y + v.h + 48


def starter(t, x, y, w, h, glyph, title, sub, key, hot=False):
    out = [well(t, x, y, w, h, 6, hi=hot)]
    if hot:
        out.append(rect(x + 0.5, y + 0.5, w - 1, h - 1, 'none', 6, f' stroke="{t["accent"]}" stroke-width="1"'))
    out.append(glyph(x + 22, y + 24, t['ink1'] if hot else t['ink2'], t['ink3']))
    out.append(text(x + 38, y + 28, title, 13, t['ink1'], 500))
    out.append(text(x + 38, y + 46, sub, 11, t['ink3']))
    kw = tw(key, 9, 500) + 10
    out.append(rect(x + w - 12 - kw + 0.5, y + 16.5, kw - 1, 14, 'none', 3, f' stroke="{t["ink4"]}" stroke-width="1"'))
    out.append(text(x + w - 12 - kw / 2, y + 27, key, 9, t['ink3'], 500, 'middle'))
    return ''.join(out)


def empty(t, y, w=CW, h=430, x=X0, uid='em'):
    """What a new user sees: no tracks; three ways in, each one key; the canvas is a drop zone."""
    out = [K.section(t, x, y, 'The empty song', 'a new project: the canvas says how to begin, in three choices and one drop zone')]
    y += 12
    v = View(x, y, w, h, [], bar0=1.0, ppbar=44.0, head=1.0, loop=None, uid=uid, overview=False, add_row=False,
             song_bars=0, sections=False)
    out.append(AR.panel(t, v))
    hy = y + 24
    out.append(rect(x, hy, 200, h - 24, mix(t['surface'], t['base'], 0.55)))
    from arr_head import add_row
    out.append(add_row(t, x, hy, 200, 28, hover=True))
    out.append(text(x + 16, hy + 50, 'No tracks yet', 11, t['ink3']))
    cx = v.cx + v.cw / 2
    out.append(text(cx, hy + 92, 'Start your song', 17, t['ink1'], 500, 'middle'))
    out.append(text(cx, hy + 114, 'Pick a first track, or drag a sound, a loop or a MIDI file from the browser onto the canvas.',
                    12, t['ink3'], 400, 'middle'))
    sw, sh, gap = 236, 64, 16
    sx = cx - (3 * sw + 2 * gap) / 2
    sy = hy + 138
    out.append(starter(t, sx, sy, sw, sh, lambda a, b, i, j: AG.keys_kind(a, b, i, j), 'Instrument track',
                       'Calor, Chorda, Unda and five more', 'Ctrl T', hot=True))
    out.append(starter(t, sx + sw + gap, sy, sw, sh, lambda a, b, i, j: AG.pads_kind(a, b, i, j), 'Drum kit',
                       'Pulsus, sixteen pads', 'Ctrl K'))
    out.append(starter(t, sx + 2 * (sw + gap), sy, sw, sh, lambda a, b, i, j: AG.wave_kind(a, b, i, j), 'Audio track',
                       'record, or drop a file', 'Ctrl Shift T'))
    dz_y = sy + sh + 26
    dz_h = h - 24 - (dz_y - hy) - 26
    out.append(rect(sx + 0.5, dz_y + 0.5, 3 * sw + 2 * gap - 1, dz_h - 1, 'none', 8,
                    f' stroke="{t["ink4"]}" stroke-width="1" stroke-dasharray="5 4"'))
    out.append(text(cx, dz_y + dz_h / 2 + 4, 'Drop sounds here — each becomes a track at bar 1', 12, t['ink3'], 400, 'middle'))
    out.append(text(cx, y + h - 14, 'Or open a demo song: Neon Tide', 11, t['accent'], 500, 'middle'))
    return ''.join(out), y + h


def sheet(t):
    out = [K.frame(t, W, 3000, 'The arrangement — the canvas',
                   'Slate ' + t['name'] + ' · des-arrange · round 39 · logical px at 1:1')]
    s, y = zooms(t, 84)
    out.append(s)
    s, y = selection(t, y + 18)
    out.append(s)
    s, y = moving(t, y + 14)
    out.append(s)
    s, y = automation(t, y + 14)
    out.append(s)
    s, y = empty(t, y + 14)
    out.append(s)
    return ''.join(out), y + 24
