"""des-arrange — the headers sheet: every state, the folder, widths, heights, resizing, the
context menu and the add-track button. 1280 logical px wide."""
import copy
import aglyphs as AG
import arr_canvas as C
import arr_head as H
import sheet_kit as K
import song_arr as SA
from base import rect, text, mix, tw, HUES, well

W, HH = 1280, 1500
LX, LANE = 24, 330          # left column: header 200 + a lane of 330
CX = LX + 200 + LANE + 18   # captions


def tracks():
    base = {tr['name']: tr for tr in SA.song('docked')}
    vox = SA.T('Vox', 'audio', 'In 1 · Mic (mono)', 44, [(45, 4, 'Take 3')], arm=True, input='In 1 · Mic', hue='Vox')
    return base, vox


def one(name, **kw):
    base, vox = tracks()
    tr = copy.deepcopy(vox if name == 'Vox' else base[name])
    tr.update(depth=0, sel=False, lanes=[] if not kw.get('lanes') else kw.pop('lanes'))
    tr.pop('parent', None)
    tr.update(kw)
    return tr


STATES = [
    ('At rest', 'the name, the instrument under it; keys are letters, no wells', 'Bass', {}, {}),
    ('Under the pointer', 'the keys gather in one well; "more" (the menu) appears', 'Bass', {}, dict(hover='Bass')),
    ('Selected', 'the header takes the track\'s colour; its clips light', 'Chords', dict(sel=True), {}),
    ('Muted', 'M lit neutral; name, stripe and clips go grey', 'Lead', dict(mute=True), {}),
    ('Soloed', 'S lit amber', 'Pluck', dict(solo=True), {}),
    ('Silenced by a solo', 'an outlined M, the clips at half strength', 'Bass', dict(implicit=True), {}),
    ('Armed', 'the input and its meter; a red edge; the lane takes a trace of red', 'Vox', {}, {}),
    ('Frozen', 'the tag in the accent; clips show the render', 'Choir', dict(frozen=True, inst='Frozen · Spatium, Aequalis'), {}),
    ('Automation shown', 'A lit violet; the lane hangs from its track', 'Chords', dict(lanes_open=True, auto=True), dict(lane=True)),
    ('Playing', 'the header\'s meter moves; the clip under the playhead lights a step', 'Kit', {}, dict(playing=True)),
]


def states(t):
    out = [K.section(t, LX, 84, 'States', 'normal width 200, height 44 · at 1:1')]
    y = 98
    for i, (a, b, name, kw, opt) in enumerate(STATES):
        lanes = [SA.A('Cutoff', 'Calor', '688 Hz', [(25, 8, 'Cutoff rise'), (41, 16, 'Cutoff')])] if opt.get('lane') else []
        tr = one(name, lanes=lanes, **kw)
        tr['num'] = {'Bass': 3, 'Chords': 4, 'Lead': 6, 'Pluck': 5, 'Vox': 8, 'Choir': 7, 'Kit': 1}[name]
        svg, end, v = K.stack(t, LX, y, 200, LANE, [tr], hover=opt.get('hover'), playing=opt.get('playing', False),
                              head=43.4 if opt.get('playing') else None, levels={'Kit': 0.64} if opt.get('playing') else None,
                              uid=f'st{i}')
        out.append(svg)
        out.append(K.cap(t, CX, y + 18, a, b))
        y = end + 12
    return ''.join(out), y


def widths(t, x, y):
    out = [K.section(t, x, y, 'Widths', 'drag the column\'s edge; 120 to 360')]
    y += 16
    for mode, w in (('narrow', 136), ('normal', 200), ('wide', 280)):
        for h in (44, 64):
            tr = one('Chords', sel=(mode == 'normal' and h == 64))
            tr.update(num=4, vol=0.72, db='−3.5 dB')
            out.append(H.header(t, x, y, w, h, tr, 4))
            out.append(text(x + w + 10, y + h / 2 + 4, f'{mode} {w} × {h}', 10, t['ink3']))
            y += h + 8
    return ''.join(out), y


def heights(t, x, y):
    out = [K.section(t, x, y, 'Heights', 'the steps of the height menu and Alt-wheel; drag for any 24–240')]
    y += 16
    for name, h in (('small', 24), ('compact', 32), ('normal', 44), ('large', 64), ('tall', 96)):
        tr = one('Lead')
        tr.update(h=h, num=6, vol=0.64, db='−6.0 dB', fx=['Aequalis', 'Chorus'])
        tr['clips'] = [(41, 4, 'Hook')]
        svg, end, v = K.stack(t, x, y, 200, 150, [tr], uid=f'h{h}')
        out.append(svg)
        out.append(K.dim_v(t, x + 200 + 150 + 8, y, y + h, str(h)))
        out.append(text(x + 200 + 150 + 30, y + h / 2 + 4, name, 10, t['ink3']))
        y = end + 8
    return ''.join(out), y


def folder(t, x, y):
    out = [K.section(t, x, y, 'A folder and its children', 'open, and closed')]
    y += 16
    trs = [copy.deepcopy(tr) for tr in SA.song('docked')[:3]]
    svg, end, v = K.stack(t, x, y, 200, 300, trs, uid='fo')
    out.append(svg)
    out.append(K.cap(t, x + 200 + 300 + 12, y + 18, 'Open', 'the folder\'s row sums its'))
    out.append(text(x + 512, y + 46, 'children; they sit a step back,', 10, t['ink3']))
    out.append(text(x + 512, y + 60, 'indented, on its colour', 10, t['ink3']))
    y = end + 10
    f = copy.deepcopy(trs[0])
    f.update(open=False, h=44, inst='Folder · 2 tracks → Drum bus')
    kids = [dict(copy.deepcopy(k), hidden=True) for k in trs[1:]]
    svg, end, v = K.stack(t, x, y, 200, 300, [f] + kids, uid='fc')
    out.append(svg)
    out.append(K.cap(t, x + 512, y + 18, 'Closed', 'one row: the children\'s clips'))
    out.append(text(x + 512, y + 46, 'as bars, the group\'s span tinted', 10, t['ink3']))
    return ''.join(out), end


def resizing(t, x, y):
    out = [K.section(t, x, y, 'Resizing', 'the pointer finds the edges; nothing is drawn until it does')]
    y += 16
    a, b = one('Bass'), one('Chords', sel=True)
    a['num'], b['num'] = 3, 4
    svg, end, v = K.stack(t, x, y, 200, 300, [a, b], uid='rz')
    out.append(svg)
    fy = y + 44
    out.append(rect(x, fy - 1, 500, 2, t['accent']))
    out.append(AG.grip_h(x + 100, fy, t['ink1']))
    out.append(AG.cursor_ud(x + 150, fy, t['ink1'] if t['name'] == 'Night' else '#ffffff', t['gutter'] if t['name'] == 'Night' else t['ink1']))
    out.append(C.tip(t, x + 236, fy - 30, 'Height 44 → 56', 'Alt: every track'))
    y = end + 14
    c = one('Pluck')
    c['num'] = 5
    svg, end, v = K.stack(t, x, y, 200, 300, [c], uid='rz2')
    out.append(svg)
    out.append(rect(x + 199, y - 4, 2, 52, t['accent']))
    out.append(AG.grip_v(x + 200, y + 22, t['ink1']))
    out.append(AG.cursor_lr(x + 200, y + 34, t['ink1'] if t['name'] == 'Night' else '#ffffff', t['gutter'] if t['name'] == 'Night' else t['ink1']))
    out.append(C.tip(t, x + 214, y + 8, 'Headers 200', 'double-click: fit the names'))
    return ''.join(out), end


def context(t, x, y):
    out = [K.section(t, x, y, 'The track\'s menu', 'right-click the header, or "more" under the pointer')]
    y += 16
    tr = one('Chords', sel=True)
    tr['num'] = 4
    svg, end, v = K.stack(t, x, y, 200, 120, [tr], hover='Chords', uid='cm')
    out.append(svg)
    kx = x + 200 - 8 - 3 - 4 - 4 * 18
    out.append(rect(kx - 21, y + 7, 16, 14, mix(t['surface'], t['ink1'], 0.12), 3))
    out.append(AG.more(kx - 13, y + 14, t['ink1']))
    sw = [HUES[k] for k in ('Kit', 'Clap', 'Hats', 'Percussion', 'Bass', 'Chords', 'Pluck', 'Lead', 'Choir', 'Drum group')]
    items = [('Rename', 'F2', ()), ('swatches', sw, 5), '-',
             ('Instrument: Calor', '', ('sub',)), ('Height', 'Normal', ('sub',)), ('Automation', '1 lane', ('sub',)), '-',
             ('Insert a track above', '', ()), ('Insert a track below', 'Ctrl T', ()), ('Duplicate', 'Ctrl D', ()),
             ('Put in a new folder', 'Ctrl G', ()), '-', ('Freeze', '', ()), ('Show in the mixer', 'F9', ()), '-',
             ('Delete track', 'Del', ('danger',))]
    m, mh = K.menu(t, x + 120, y + 26, items, 236, 'cm', hot='Height')
    out.append(m)
    sub, _ = K.menu(t, x + 120 + 232, y + 26 + 6 + 30 + 9 + 24 + 9 - 6 + 24, [
        ('Small', '24', ()), ('Compact', '32', ()), ('Normal', '44', ('check',)), ('Large', '64', ()), ('Tall', '96', ()),
        '-', ('Fit the window', 'Ctrl Alt F', ())], 168, 'cms')
    out.append(sub)
    return ''.join(out), y + 26 + mh


def add(t, x, y):
    out = [K.section(t, x, y, 'Add a track', 'under the last track, and the + over the headers')]
    y += 16
    out.append(H.add_row(t, x, y, 200, 28))
    out.append(text(x + 210, y + 18, 'at rest', 10, t['ink3']))
    out.append(H.add_row(t, x, y + 36, 200, 28, hover=True))
    out.append(text(x + 210, y + 54, 'under the pointer', 10, t['ink3']))
    items = [('Instrument track', 'Ctrl T', ()), ('Audio track', 'Ctrl Shift T', ()), ('Drum kit (Pulsus)', '', ()),
             ('Folder', 'Ctrl G', ()), '-', ('From a preset…', '', ('sub',)), ('From a file…', '', ()),
             '-', ('Return', '', ()), ('Automation lane', 'A', ('dim',))]
    m, mh = K.menu(t, x + 8, y + 70, items, 208, 'ad', hot='Instrument track')
    out.append(m)
    return ''.join(out), y + 70 + mh


def keys4(t, x, y):
    """The keys alone, every state, for the 4x cut."""
    out = [K.section(t, x, y, 'The keys', 'M S arm A · 18 × 16, joined · rest, pointer, on')]
    y += 14
    base_tr = one('Bass')
    rows = [('rest', {}, False), ('pointer', {}, True), ('mute', dict(mute=True), True), ('solo', dict(solo=True), True),
            ('silenced', dict(implicit=True), False), ('armed', dict(arm=True), True),
            ('lanes', dict(lanes_open=True), True), ('has lanes', dict(auto=True), False)]
    for k, (lab, kw, hv) in enumerate(rows):
        xx = x + (k % 4) * 128
        yy = y + (k // 4) * 30
        tr = dict(base_tr, **kw)
        out.append(rect(xx, yy, 120, 24, t['surface'], 2))
        out.append(H.keys(t, xx + 6, yy + 12, tr, 'MSRA', hv))
        out.append(text(xx + 84, yy + 16, lab, 10, t['ink3']))
    return ''.join(out), y + 60


def sheet(t):
    out = [K.frame(t, W, HH, 'The arrangement — track headers',
                   'Slate ' + t['name'] + ' · des-arrange · round 39 · logical px at 1:1 (the 1.25× render is the same sheet at 125 %)')]
    s, y = states(t)
    out.append(s)
    s, y2 = keys4(t, LX, y + 22)
    out.append(s)
    s, y3 = folder(t, LX, y2 + 28)
    out.append(s)
    s, y4 = resizing(t, LX, y3 + 30)
    out.append(s)
    RX = 760
    s, ry = widths(t, RX, 84)
    out.append(s)
    s, ry = heights(t, RX, ry + 22)
    out.append(s)
    s, ry2 = add(t, RX, ry + 26)
    out.append(s)
    s, ry3 = context(t, RX, ry2 + 30)
    out.append(s)
    bottom = max(y4, ry3, ry2) + 24
    return ''.join(out), bottom
