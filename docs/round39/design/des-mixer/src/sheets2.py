"""des-mixer — the states sheet, the inspector sheet, and the density/overflow windows."""
import copy
import data as D
import strip as ST
import desk as DK
import insp as IN
import window as W
import topbar
from insp_text import T
from sheets import heading, panel, song, sheet_bg
from base import defs, rect, text, tw


def _wrap(s, size, w):
    words, lines, cur = s.split(' '), [], ''
    for wd in words:
        if tw((cur + ' ' + wd).strip(), size) > w and cur:
            lines.append(cur)
            cur = wd
        else:
            cur = (cur + ' ' + wd).strip()
    return lines + [cur]


def state_list():
    lead = song('Lead')
    st = []

    def add(label, s, note):
        st.append((label, s, note))
    a = copy.deepcopy(lead)
    a['lv'] = None
    add('At rest', a, 'stopped: grooves, no −∞')
    add('Playing', copy.deepcopy(lead), 'bars on the fader\'s law, peak holds')
    c = song('Drum bus')
    c.update(lv=(0.6, -0.4), pk=(1.4, 0.8), clip=True)
    add('Clipping', c, 'clip cell lit; level shows the peak in red')
    v = D.S(17, 'Vox lead', -6.0, 0.0, [('Aequalis', 'EQ', True)], 'Master', (-18, -18), arm=True)
    add('Armed', v, 'arm key red; the meter is the input')
    m = song('Pluck')
    m['mute'] = True
    add('Muted', m, 'M lit ink 2; stripe and name quiet; meter dimmed')
    s = song('Chords')
    s['solo'] = True
    add('Soloed', s, 'S lit amber')
    i = copy.deepcopy(lead)
    i['implicit'] = True
    add('Silenced by a solo', i, 'M ringed in amber, not lit')
    sel = song('Chords')
    sel['sel'] = True
    add('Selected', sel, 'tint 13 %, stripe lit, name Medium')
    b = song('Chords')
    b['fx'] = [('Comprimo', 'Comp', True), ('Aequalis', 'EQ', False), ('Saturo', 'Drive', False)]
    add('Inserts bypassed', b, 'hollow dot, name ink 4')
    se = song('Pluck')
    add('Sends', se, 'Reverb on, Delay off')
    sc = song('Bass')
    sc['sock_lit'] = 'side'
    add('Sidechain key', sc, 'its Comp keyed from Kick')
    vc = copy.deepcopy(D.VCA)
    add('VCA', vc, 'fader only; follows 4 strips')
    fo = song('Clap')
    fo.update(vca='VCA 1', vca_ghost=-8.0)
    add('VCA follower', fo, 'tag VCA 1; a tick where the VCA puts it')
    fz = song('Choir')
    fz['frozen'] = True
    add('Frozen', fz, 'sound block hatched; mix live')
    e = song('Lead')
    e = D.S(11, None, 0.0, kind='empty')
    add('Empty', e, 'number and groove only')
    return st


def states(t):
    out = [defs(t), rect(0, 0, 1280, 1180, sheet_bg(t))]
    out.append(heading(t, 24, 36, 'The states — one strip each, Wide (520) above, Compact (400) below',
                       'Slate · 1:1 · every state the strip has; the colours are the tokens (rec, solo, clip, accent, auto)'))
    sts = state_list()
    for s_ in sts:
        if s_[1].get('sock_lit') == 'side':
            s_[1]['sock_lit'] = t['ink2']
    x0, y0 = 24, 80
    step = 60 + 22
    out.append(panel(t, 12, y0 - 10, 1256, 520 + 96))
    for k, (lab, s, note) in enumerate(sts):
        x = x0 + k * step
        for j, ln in enumerate(_wrap(f'{k + 1}  {lab}', 11, step - 6)[:2]):
            out.append(text(x, y0 + 10 + j * 13, ln, 11, t['ink1'], 500))
        out.append(ST.strip(t, x, y0 + 36, 520, s, 'wide', sel=bool(s.get('sel'))))
        for j, ln in enumerate(_wrap(note, 10, step - 4)[:3]):
            out.append(text(x, y0 + 36 + 520 + 16 + j * 12, ln, 10, t['ink3']))
    # Compact row
    y1 = y0 + 520 + 112
    out.append(panel(t, 12, y1 - 10, 700, 400 + 56))
    out.append(text(24, y1 + 10, 'Compact — the same states, names up the strip', 12, t['ink1'], 500))
    for k, (lab, s, note) in enumerate(sts):
        x = 24 + k * 44
        out.append(ST.strip(t, x, y1 + 28, 400, s, 'compact', sel=bool(s.get('sel'))))
        out.append(text(x + 15, y1 + 28 + 400 + 14, str(k + 1), 10, t['ink3'], 400, 'middle'))
    # routing shown: the selected strip's cables
    rx = 728
    out.append(panel(t, rx - 12, y1 - 10, 1268 - rx + 12, 400 + 56))
    out.append(text(rx, y1 + 10, 'Routing shown — Chords selected', 12, t['ink1'], 500))
    names = ['Kick', 'Bass', 'Chords', 'Reverb', 'Delay']
    mas = song('Master')
    row = [mas] + [song(n) for n in names]
    row[3]['sidechain'] = 'Kick'
    pos = {}
    x = rx
    for s in row:
        if s['name'] in ('Master', 'Reverb', 'Delay'):
            s['sock_lit'] = t['accent']
        if s['name'] == 'Kick':
            s['sock_lit'] = t['ink2']
        if s['name'] == 'Chords':
            s['sock_lit'] = t['accent']
        out.append(ST.strip(t, x, y1 + 28, 400, s, 'wide', sel=s['name'] == 'Chords'))
        w = ST.MASTER_W['wide'] if s['kind'] == 'master' else 60
        pos[s['name']] = x + w / 2
        x += w + 2
    pl = ST.plan('wide', 400)
    by = y1 + 28 + pl['sock'][0] + 7
    out.append(DK.cables(t, pos['Chords'], [(pos['Master'], 'out'), (pos['Reverb'], 'send'), (pos['Delay'], 'send'),
                                            (pos['Kick'], 'side')], by))
    lx = x + 8
    for k, (a, kind) in enumerate((('output', 'out'), ('send', 'send'), ('sidechain key', 'side'))):
        yy = y1 + 60 + k * 22
        col = t['accent'] if kind != 'side' else t['ink2']
        dash = ' stroke-dasharray="3 2"' if kind == 'side' else ''
        out.append(f'<path d="M{lx} {yy}H{lx + 22}" stroke="{col}" stroke-width="{1.5 if kind == "out" else 1}"{dash}/>')
        out.append(text(lx, yy + 14, a, 10, t['ink3']))
    return ''.join(out), 1280, 1180


def inspector_sheet(t, lang='en'):
    L = T[lang]
    w = 246
    out = [defs(t), rect(0, 0, 1280, 770, sheet_bg(t))]
    out.append(heading(t, 24, 36, 'The inspector — a mixer strip, every section open' if lang == 'en' else 'El inspector — un canal, cada sección abierta',
                       'Slate · 1:1 · 246 px wide (the window\'s right column) · read top to bottom, column by column; the last column: folded and frozen'))
    s = song('Chords')
    s['sidechain'] = 'Kick'
    s['arm'] = False
    cols = [[('head', {}), ('inserts', {'mini': 0, 'hover': 1}), ('eq', {}), ('sends', {})],
            [('routing', {}), ('sidechain', {}), ('recording', {}), ('automation', {})],
            [('level', {}), ('freeze', {}), ('freeze_on', {}), ('folded', {})],
            [('states', {})]]
    fns = {'inserts': IN.inserts, 'eq': IN.eq, 'sends': IN.sends, 'routing': IN.routing, 'sidechain': IN.sidechain,
           'recording': IN.recording, 'automation': IN.automation, 'level': IN.level, 'freeze': IN.freeze}
    rows = [(L['inserts'], L['of'].format(n=3, m=10)), (L['eq'], '3 bands'), (L['sends'], '2'),
            (L['routing'], L['master']), (L['sidechain'], 'Kick'), (L['recording'], L['no_input']),
            (L['automation'], L['lanes'].format(n=2)), (L['level'], '−3.5 dB')]
    notes = ['head · inserts (a mini view open, the pointer on row 2, row 3 bypassed) · EQ · sends',
             'routing · sidechain · recording · automation', 'level · freeze (live, then frozen) · every section folded',
             'an insert row in its states · empty sections']
    for c, secs in enumerate(cols):
        x = 24 + c * (w + 24)
        y = 72
        body = rect(x, y, w, 640, t['base'])
        for name, kw in secs:
            if name == 'head':
                svg, y = IN.head(t, x, w, y, L, s)
            elif name == 'freeze_on':
                svg, y = IN.freeze(t, x, w, y + 4, L, dict(s, frozen=True))
            elif name == 'folded':
                svg, y = IN.folded(t, x, w, y + 8, rows)
            elif name == 'states':
                svg, y = IN.row_states(t, x, w, y, L)
            else:
                svg, y = fns[name](t, x, w, y, L, s, **kw)
            body += svg
        out.append(f'<clipPath id="ic{c}-{t["name"]}"><rect x="{x}" y="72" width="{w}" height="640" rx="6"/></clipPath>'
                   f'<g clip-path="url(#ic{c}-{t["name"]})">{body}</g>')
        for j, ln in enumerate(_wrap(notes[c], 10, w)):
            out.append(text(x, 730 + j * 13, ln, 10, t['ink3']))
    return ''.join(out), 1280, 770


def long_desk():
    out = []
    import random
    rnd = random.Random(39)
    for k, (nm, dest) in enumerate(D.LONG):
        fx = [('Aequalis', 'EQ', True)] + ([('Comprimo', 'Comp', True)] if k % 3 == 0 else [])
        lv = -8 - rnd.random() * 14
        out.append(D.S(k + 1, nm, round(-2 - rnd.random() * 9, 1), round(rnd.uniform(-0.3, 0.3), 2), fx, dest,
                       (lv, lv - rnd.random()), eq=(rnd.choice((0, 1.5, -2)), 0, rnd.choice((0, 1))),
                       sends=[('Reverb', -14.0, True)] if k % 2 else []))
    return out


def window_page(t, **kw):
    return W.window(t, **kw), 1280, 720


def overflow_wide(t):
    ss = long_desk()
    sel = 'Vox double (left)'
    for s in ss:
        if s['name'] == sel:
            s['sends'] = [('Reverb', -12.0, True), ('Delay', -20.0, True)]
    right = [copy.deepcopy(D.VCA)] + [copy.deepcopy(s) for s in D.RETURNS]
    return W.window(t, 'wide', strips=ss, sel=sel, right=right, scroll=6,
                    hint=('Vox double (left) · strip 15', '6 strips to the left, 6 to the right · Shift+wheel scrolls · Ctrl+wheel changes the width')), 1280, 720


def overflow_es(t):
    ss = long_desk()
    tr = {'Vox lead': 'Voz principal', 'Vox double (left)': 'Coros dobles (izquierda)', 'Bass': 'Bajo', 'Chords': 'Acordes',
          'Drums': 'Batería', 'Pads': 'Pad'}
    for s in ss:
        s['name'] = tr.get(s['name'], s['name'])
        if s['out'] == 'Drum bus':
            s['out'] = 'Drum bus'
    sel = 'Acordes'
    for s in ss:
        if s['name'] == sel:
            s.update(fx=[('Comprimo', 'Comp', True), ('Aequalis', 'EQ', True), ('Saturo', 'Drive', False)],
                     sends=[('Reverberación', -16.0, True), ('Retardo', -22.0, True)], eq=(2, -2.5, 1.6))
        if s['name'] == 'Voz principal':
            s['solo'] = True
        elif s['kind'] == 'track':
            s['implicit'] = True
    rets = [copy.deepcopy(s) for s in D.RETURNS]
    rets[0]['name'], rets[1]['name'] = 'Reverberación', 'Retardo'
    right = [copy.deepcopy(D.VCA)] + rets
    right[0]['name'] = 'Batería'
    old = dict(topbar.TXT)
    topbar.TXT.update(menus=('Archivo', 'Editar', 'Clip', 'Ver', 'Opciones', 'Ayuda'), sub='24 pistas · 64 compases · 2:03',
                      pat=['Pat', 'Canción'], bb='compás · tiempo', peak='Pico')
    body = W.window(t, 'wide', strips=ss, sel=sel, right=right, lang='es', solo=['Voz principal'], scroll=7,
                    open_=('inserts', 'sends', 'routing'),
                    hint=('Acordes · volumen −3,5 dB', 'arrastra para cambiar · doble clic para escribir · Alt+clic para 0 dB'))
    topbar.TXT.clear()
    topbar.TXT.update(old)
    return body, 1280, 720


def compact_long(t):
    ss = long_desk()
    sel = 'Chords'
    for s in ss:
        if s['name'] == sel:
            s.update(fx=[('Comprimo', 'Comp', True), ('Aequalis', 'EQ', True), ('Saturo', 'Drive', False)],
                     sends=[('Reverb', -16.0, True), ('Delay', -22.0, True)], eq=(2, -2.5, 1.6), sidechain='Kick')
        if s['name'] == 'Vox lead':
            s['arm'] = True
        if s['name'] == 'Riser':
            s['mute'] = True
        if s['name'] == 'Strings':
            s['frozen'] = True
        if s['name'] in ('Kick', 'Snare top', 'Tom fills', 'Room mics', 'Shaker'):
            s['vca'] = 'V1'
    right = [copy.deepcopy(D.VCA)] + [copy.deepcopy(s) for s in D.RETURNS]
    return W.window(t, 'compact', strips=ss, sel=sel, right=right,
                    hint=('Chords · strip 12', 'Ctrl+wheel over the desk: Compact, Wide, Large')), 1280, 720


PAGES = {
    'states': states,
    'inspector': inspector_sheet,
    'mixer-compact': lambda t: compact_long(t),
    'mixer-large': lambda t: window_page(t, density='large'),
    'overflow-wide': overflow_wide,
    'overflow-es': overflow_es,
}
