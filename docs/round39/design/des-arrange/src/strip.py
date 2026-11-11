"""des-mixer — one strip, any density, any height, any state.

A strip reads top to bottom in three blocks, the same in every density:
  the sound  — number, name, inserts, EQ, polarity/swap/width;
  the mix    — pan, mute/solo/arm, the fader with its meter, the level;
  the route  — sends, the output, the socket of the cable band.
The desk fits first, then scrolls: as the height shrinks, rows leave in a fixed order
(EQ, the polarity row, the insert list folds to one summary line, the level, the sends,
the socket, the output) and the fader takes what is left.
"""
import mglyphs as M
import parts as P
from base import rect, text, mix, tw, HUES, hue_for

# density -> strip body width; the pitch is the width + 2 (2 px of the desk between strips)
WIDTH = {'compact': 30, 'wide': 60, 'large': 94}
MASTER_W = {'compact': 40, 'wide': 68, 'large': 110}
GAP = 2

# rows per density at full height: name -> height (fader is flexible)
ROWS = {
    'compact': [('head', 112), ('ins', 10), ('pan', 22), ('msr', 30), ('fader', 0), ('out', 12), ('sock', 18)],
    'wide': [('head', 36), ('ins', 68), ('eq', 22), ('phase', 16), ('pan', 28), ('msr', 16), ('fader', 0),
             ('readout', 16), ('sends', 28), ('out', 14), ('sock', 18)],
    'large': [('head', 44), ('ins', 94), ('eq', 36), ('phase', 18), ('pan', 40), ('msr', 18), ('fader', 0),
              ('readout', 18), ('sends', 60), ('out', 18), ('sock', 18)],
}
# the order rows leave when the desk is short (ADR 0041's, kept)
DROP = ['eq', 'phase', 'ins_fold', 'sends', 'sock', 'readout', 'out']
GAPS = {'compact': 4, 'wide': 4, 'large': 6}
MIN_FADER = {'compact': 120, 'wide': 112, 'large': 150}


def plan(density, H):
    """y and height of each row for a strip of height H (the fader takes the rest)."""
    rows = list(ROWS[density])
    g = GAPS[density]
    dropped = []

    def need(rs):
        return sum(h for _, h in rs) + g * (len(rs) - 1) + 20     # 20: the cap's overhang

    for d in DROP:
        if need(rows) + MIN_FADER[density] <= H:
            break
        if d == 'ins_fold':
            rows = [(n, (14 if density != 'compact' else h) if n == 'ins' else h) for n, h in rows]
        else:
            rows = [(n, h) for n, h in rows if n != d]
        dropped.append(d)
    flex = H - need(rows)
    y = 0
    out = {}
    for n, h in rows:
        if n == 'fader':
            out[n] = (y + 10, flex)
            y += flex + 20 + g
        else:
            out[n] = (y, h)
            y += h + g
    out['_dropped'] = dropped
    return out


def body(t, x, y, w, h, s, sel):
    kind = s['kind']
    if kind == 'empty':
        return rect(x, y, w, h, mix(t['surface'], t['base'], 0.55), 3)
    if sel:
        return rect(x, y, w, h, mix(t['surface'], HUES[s['name']], t['head_sel']), 3)
    return rect(x, y, w, h, f'url(#strip-{t["name"]})', 3)


def stripe(t, x, y, w, s, sel):
    kind = s['kind']
    if kind == 'master':
        return rect(x, y, w, 3, t['ink3'])
    if kind == 'vca':      # a VCA has no colour of its own: two hairlines, the colour of its followers
        return rect(x, y, w, 1, t['ink3']) + rect(x, y + 2, w, 1, t['ink3'])
    col = HUES[s['name']] if sel else hue_for(t, s['name'], t['stripe_rest'])
    if s.get('mute') or s.get('implicit'):
        col = hue_for(t, s['name'], 0.32)
    return rect(x, y, w, 3, col)


def strip(t, x, y, H, s, density='wide', sel=False, sel_name=None):
    """Draw strip `s` with its top-left at (x, y), height H."""
    kind = s['kind']
    w = MASTER_W[density] if kind == 'master' else WIDTH[density]
    cx = x + w / 2
    pl = plan(density, H)
    out = [body(t, x, y, w, H, s, sel)]
    if kind == 'empty':
        return ''.join(out) + _empty(t, x, y, w, H, s, density, pl)
    out.append(stripe(t, x, y, w, s, sel))
    frozen = s.get('frozen')
    muted = s.get('mute') or s.get('implicit')
    name_col = t['ink3'] if muted else t['ink1']
    # ---------------------------------------------------------------- head
    hy, hh = pl['head']
    num = 'M' if kind == 'master' else str(s['num'])
    if density == 'compact':
        out.append(text(cx, y + 14, num, 10 if len(num) < 3 else 9, t['ink3'], 600 if kind == 'master' else 400, 'middle'))
        nm = P.fit(s['name'], 12, hh - 26)
        ny = y + hy + hh - 4
        out.append(f'<g transform="translate({cx + 4},{ny}) rotate(-90)">'
                   + text(0, 0, nm, 12, name_col, 500 if sel else 400) + '</g>')
        if kind == 'vca':
            out.append(text(cx, y + 26, 'VCA', 8, t['ink2'], 600, 'middle'))
        if frozen:
            out.append(M.snow(cx, y + 28, 12, t['accent']))
    else:
        ny = y + 15
        if s.get('vca'):
            gw = tw(num, 10) + 4 + tw(s['vca'], 9, 600)
            gx = cx - gw / 2
            out.append(text(gx, ny, num, 10, t['ink3']))
            out.append(text(gx + tw(num, 10) + 4, ny, s['vca'], 9, t['ink2'], 600))
        else:
            out.append(_num_or_wrapped(t, cx, ny, num, s, w, density, name_col, sel) if not s.get('vca') else '')
        if kind == 'return':
            out.append(M.ret(x + 9, ny - 3.5, 12, t['ink3']))
        if frozen:
            out.append(M.snow(x + w - 9, ny - 3.5, 12, t['accent']))
        if kind == 'vca':
            out.append(text(x + 5, ny, 'VCA', 9, t['ink2'], 600))
        size = 12 if density == 'wide' else 13
        room = w - 6
        fs = size if tw(s['name'], size) <= room else size - 1
        if not _wraps(s, w, density):
            out.append(text(cx, y + 30 if density == 'wide' else y + 31, P.fit(s['name'], fs, room), fs, name_col,
                            500 if (sel or kind == 'master') else 400, 'middle'))
        if density == 'large':
            W_ = P.WORDS
            sub = {'master': 'Out 1–2', 'bus': W_['bus'].format(n=len(s.get('receives', []))),
                   'return': W_['ret'].format(n=len(s.get('receives', []))),
                   'vca': W_['strips'].format(n=len(s.get('follows', []))), 'track': s.get('src_label', W_['track'])}[kind]
            out.append(text(cx, y + 42, sub, 10, t['ink3'], 400, 'middle'))
    # ---------------------------------------------------------------- the sound
    if kind != 'vca':
        out.append(_sound(t, x, y, w, s, density, pl, frozen))
    # ---------------------------------------------------------------- the mix
    out.append(_mix(t, x, y, w, s, density, pl, sel))
    # ---------------------------------------------------------------- the route
    out.append(_route(t, x, y, w, s, density, pl, sel_name))
    if frozen and 'ins' in pl:      # the sound block is a render: hatched, cold, still readable
        iy, ih = pl['ins']
        end = pl['phase'][0] + pl['phase'][1] if 'phase' in pl else (pl['eq'][0] + pl['eq'][1] if 'eq' in pl else iy + ih)
        out.append(P.hatch(t, x + 1, y + iy - 2, w - 2, end - iy + 4, t['accent'], 0.22, 4, f'{s["num"]}{density}'))
    return ''.join(out)


def _wraps(s, w, density):
    """A name wider than the strip at 11 px takes both head lines (Wide and Large)."""
    return density != 'compact' and s['kind'] != 'vca' and tw(s['name'], 11) > w - 6 and ' ' in s['name']


def _num_or_wrapped(t, cx, ny, num, s, w, density, col, sel):
    if not _wraps(s, w, density):
        return text(cx, ny, num, 10, t['ink3'], 600 if s['kind'] == 'master' else 400, 'middle')
    room = w - 6
    words = s['name'].split(' ')
    line1 = ''
    while words and tw(num + ' ' + (line1 + ' ' + words[0]).strip(), 11) <= room:
        line1 = (line1 + ' ' + words.pop(0)).strip()
    if not line1:
        line1 = words.pop(0)
    line2 = P.fit(' '.join(words), 11, room)
    wt = 500 if sel else 400
    g1 = tw(num, 10) + 3 + tw(line1, 11, wt)
    x1 = cx - g1 / 2
    out = text(x1, ny, num, 10, t['ink3'])
    out += text(x1 + tw(num, 10) + 3, ny, P.fit(line1, 11, room - tw(num, 10) - 3, wt), 11, col, wt)
    out += text(cx, ny + 14, line2, 11, col, wt, 'middle')
    return out


def _empty(t, x, y, w, H, s, density, pl):
    cx = x + w / 2
    out = text(cx, y + 14 if density == 'compact' else y + 15, str(s['num']), 10, t['ink4'], 400, 'middle')
    fy, fh = pl['fader']
    gx = x + (10 if density == 'compact' else 20 if density == 'wide' else 28)
    out += P.fader(t, gx, y + fy, y + fy + fh, 0.0, quiet=True, ticks=False,
                   cap_w=14 if density == 'compact' else 20)
    if 'sock' in pl:
        sy, sh = pl['sock']
        out += P.socket(t, cx, y + sy + 7, s.get('sock_lit'))
    return out


def _sound(t, x, y, w, s, density, pl, frozen):
    out = []
    cx = x + w / 2
    fx = s['fx']
    if 'ins' in pl:
        iy, ih = pl['ins']
        if density == 'compact':
            n = len(fx)
            px0 = cx - (min(n, 5) * 5 - 2) / 2 + 1.5
            for k, (_, _, on) in enumerate(fx[:5]):
                if on:
                    out.append(f'<circle cx="{px0 + k * 5}" cy="{y + iy + 5}" r="1.5" fill="{t["ink4"] if frozen else t["ink3"]}"/>')
                else:
                    out.append(f'<circle cx="{px0 + k * 5}" cy="{y + iy + 5}" r="1.25" fill="none" stroke="{t["ink4"]}" stroke-width="0.75"/>')
        elif ih <= 16:          # folded: one summary line, the kinds joined
            line = ' · '.join(k for _, k, on in fx if on) or ''
            off = sum(1 for _, _, on in fx if not on)
            out.append(text(cx, y + iy + 11, P.fit(line, 10, w - 8 - (12 if off else 0)), 10,
                            t['ink4'] if frozen else t['ink3'], 400, 'middle'))
        else:
            rows = 4 if density == 'wide' else 5
            rh = 16 if density == 'wide' else 18
            out.append(P.insert_rows(t, x + 3, y + iy, w - 6, rows, fx, rh, 10 if density == 'wide' else 11,
                                     show_unit=density == 'large', frozen=frozen,
                                     key_from=s.get('sidechain')))
    if 'eq' in pl:
        ey, eh = pl['eq']
        out.append(P.eq_thumb(t, x + 3, y + ey, w - 6, eh, s.get('eq', (0, 0, 0)), dots=density == 'large', dim=frozen))
    if 'phase' in pl:
        py, ph = pl['phase']
        kh = 16 if density == 'wide' else 18
        kw = 16 if density == 'wide' else 22
        gh = 16 if density == 'wide' else 18
        ph_on, sw_on, mo_on = s.get('phase_on'), s.get('swap_on'), s.get('mono_on')
        x0 = x + 3
        for k, (g, on) in enumerate(((M.phase, ph_on), (M.swap, sw_on))):
            kx = x0 + k * (kw + 2)
            # at rest a glyph alone (no box); on, an accent-tinted well
            if on:
                out.append(P.key(t, kx, y + py, kw, kh, on=mix(t['surface'], t['accent'], 0.35)))
                out.append(g(kx + kw / 2, y + py + kh / 2 - 0.5, gh, t['accent'], t['accent']))
            else:
                out.append(g(kx + kw / 2, y + py + kh / 2 - 0.5, gh, t['ink3'], t['ink3']))
        if density == 'large':
            kx = x0 + 2 * (kw + 2)
            out.append(M.mono(kx + kw / 2, y + py + kh / 2 - 0.5, gh, t['accent'] if mo_on else t['ink3'], t['ink4']))
        # width: a small knob from the minimum (0 % mono … 100 % … 200 %)
        wx = x + w - 3 - 9 if density == 'wide' else x + w - 3 - 11
        out.append(P.knob(t, wx, y + py + ph / 2, 6 if density == 'wide' else 7, s.get('width', 1.0) / 2, bipolar=False))
    return ''.join(out)


def _mix(t, x, y, w, s, density, pl, sel):
    out = []
    kind = s['kind']
    cx = x + w / 2
    if 'pan' in pl and kind != 'vca':
        py, ph = pl['pan']
        r = {'compact': 7, 'wide': 9, 'large': 11}[density]
        kcy = y + py + (ph / 2 if density != 'large' else 15)
        out.append(P.knob(t, cx, kcy, r, s.get('pan', 0.0), auto=s.get('pan_auto', False)))
        if density == 'large':
            p = s.get('pan', 0.0)
            pv = 'C' if abs(p) < 0.01 else (f'{round(abs(p) * 100)} ' + ('L' if p < 0 else 'R'))
            out.append(text(cx, y + py + ph - 1, pv, 10, t['ink3'], 400, 'middle'))
    my, mh = pl['msr']
    out.append(_msr(t, x, y + my, w, s, density))
    fy, fh = pl['fader']
    top, bot = y + fy, y + fy + fh
    lv = s.get('lv')
    dim = s.get('mute') or s.get('implicit')
    hue = HUES.get(s['name']) if sel else None
    geo = {'compact': (10, 16, 10, 19, 2), 'wide': (21, 22, 13, 37, 4), 'large': (28, 26, 14, 64, 5)}[density]
    if kind == 'master':
        geo = {'compact': (14, 18, 10, 26, 3), 'wide': (31, 22, 13, 47, 4), 'large': (42, 26, 14, 70, 6)}[density]
    gx, cw, ch, mx, mw = geo
    if kind == 'master' or density == 'large':
        nx = x + (gx - cw / 2 - 3 if kind == 'master' else gx + cw / 2 + 12)
        anchor = 'end' if kind == 'master' else 'middle'
        if density == 'large' and kind != 'master':
            nx = x + (gx + cw / 2 + mx) / 2 + 1
        if density != 'compact':
            for d in P.TICKS:
                yy = round(top + fh * P.frac(d)) + 3
                lab = ('+' + str(d)) if d > 0 else str(d).replace('-', '−')
                out.append(text(nx, yy, lab, 9, t['ink2'] if d == 0 else t['ink3'], 400, anchor))
    ticks = density == 'wide' and kind != 'master'
    out.append(P.fader(t, x + gx, top, bot, s['db'], cw, ch, hue=hue, ticks=ticks, tick_x=x + gx + cw / 2 + 3,
                       ghost=s.get('vca_ghost')))
    if kind != 'vca':
        pk = s.get('pk') or ((lv[0] + 1.5, lv[1] + 2) if lv else None)
        out.append(P.meter(t, x + mx, top, bot, lv, pk, mw, clip=s.get('clip', False), dim=dim))
    # ---------------------------------------------------------------- the level, in numbers
    if 'readout' in pl:
        ry, rh = pl['readout']
        val = P.dbs(s['db'])
        col = t['ink1'] if sel else t['ink2']
        if density == 'large' and kind != 'vca':
            out.append(text(x + 6, y + ry + 13, val, 11, col, 400))
            if lv or s.get('clip'):
                pkv = max((s.get('pk') or (lv[0] + 1.5, lv[1] + 2)))
                out.append(text(x + w - 6, y + ry + 13, P.dbs(pkv), 11, t['clip'] if pkv > 0 else t['ink3'], 400, 'end'))
        else:
            if s.get('clip'):
                pkv = max(s.get('pk'))
                out.append(rect(x + 4, y + ry, w - 8, rh, mix(t['surface'], t['clip'], 0.22), 3))
                out.append(text(cx, y + ry + 12, P.dbs(pkv), 11, t['clip'], 600, 'middle'))
            else:
                out.append(text(cx, y + ry + 12, val, 11, col, 400, 'middle'))
    return ''.join(out)


def _msr(t, x, y, w, s, density):
    """Mute, solo and arm. Off: recessed wells with a letter in ink 3. On: the state's colour.
    A strip silenced by another's solo keeps its key off, ringed faintly in the solo colour."""
    kind = s['kind']
    out = []
    mute_on = (t['ink2'] if t['name'] == 'Night' else t['ink3']) if s.get('mute') else None
    solo_on = t['solo'] if s.get('solo') else None
    if density == 'compact':
        kw, kh = 13, 14
        x0 = x + (w - (2 * kw + 2)) / 2
        out.append(P.key(t, x0, y, kw, kh, 'M', mute_on, 9) if not s.get('implicit')
                   else P.ghost_ring(t, x0, y, kw, kh, t['solo']) + text(x0 + kw / 2, y + 10, 'M', 9, t['ink3'], 600, 'middle'))
        out.append(P.key(t, x0 + kw + 2, y, kw, kh, 'S', solo_on, 9))
        if kind in ('track',):
            if s.get('arm'):
                out.append(P.key(t, x0, y + kh + 2, 2 * kw + 2, kh, None, t['rec'],
                                 glyph=lambda a, b, ink: M.arm(a, b, 14, ink)))
            else:      # at rest the dot alone, no well: one row less of boxes across the desk
                out.append(M.arm(x + w / 2, y + kh + 2 + kh / 2 - 0.5, 14, t['ink4']))
        return ''.join(out)
    kh = 16 if density == 'wide' else 18
    has_arm = kind == 'track'
    if kind == 'master':
        keys = [('M', mute_on), ('mono', t['accent'] if s.get('mono_on') else None)]
    else:
        keys = [('M', mute_on), ('S', solo_on)] + ([('arm', t['rec'] if s.get('arm') else None)] if has_arm else [])
    n = len(keys)
    kw = (w - 6 - 2 * (n - 1)) / n
    for k, (lab, on) in enumerate(keys):
        kx = x + 3 + k * (kw + 2)
        if lab == 'arm':
            out.append(P.key(t, kx, y, kw, kh, None, on, glyph=lambda a, b, ink: M.arm(a, b, kh if kh in (16, 18) else 16,
                                                                                      ink if on else t['ink3'])))
        elif lab == 'mono':
            out.append(P.key(t, kx, y, kw, kh, None, on, glyph=lambda a, b, ink: M.mono(a, b, kh, ink, t['ink4'])))
        elif lab == 'M' and s.get('implicit'):
            out.append(P.ghost_ring(t, kx, y, kw, kh, t['solo']))
            out.append(text(kx + kw / 2, y + kh / 2 + 3.6, 'M', 10, t['ink3'], 600, 'middle'))
        else:
            out.append(P.key(t, kx, y, kw, kh, lab, on, 10 if density == 'wide' else 11))
    return ''.join(out)


def _route(t, x, y, w, s, density, pl, sel_name):
    out = []
    kind = s['kind']
    cx = x + w / 2
    if 'sends' in pl and density != 'compact':
        sy, sh = pl['sends']
        if kind in ('track', 'bus') and density == 'large':
            out.append(P.send_rows_large(t, x + 6, y + sy, w - 12, s.get('sends', []), 3))
        elif kind in ('track', 'bus'):
            rows = 2 if density == 'wide' else 3
            rh = 14 if density == 'wide' else 16
            out.append(P.send_rows(t, x + 5, y + sy, w - 10, s.get('sends', []), rows, rh,
                                   10 if density == 'wide' else 11, show_value=density == 'large',
                                   short=density == 'wide'))
        elif kind == 'return':
            n = len(s.get('receives', []))
            if density == 'wide':
                out.append(text(cx, y + sy + 11, P.WORDS['sends_in'].format(n=n), 10, t['ink3'], 400, 'middle'))
            else:
                for k, nm in enumerate(s['receives'][:3]):
                    out.append(text(x + 6, y + sy + 12 + k * 20, '← ' + nm, 11, t['ink3']))
                if n > 3:
                    out.append(text(x + w - 6, y + sy + 12 + 2 * 20, f'+{n - 3}', 9, t['ink3'], 600, 'end'))
        elif kind == 'vca':
            out.append(text(cx, y + sy + 11, P.WORDS['strips'].format(n=len(s['follows'])), 10, t['ink3'], 400, 'middle'))
    if 'out' in pl:
        oy, oh = pl['out']
        if density == 'compact':
            if s['out'] not in ('Master', 'Out 1–2') and kind != 'vca':
                out.append(f'<circle cx="{cx}" cy="{y + oy + 6}" r="2.5" fill="{hue_for(t, s["out"], 0.9)}"/>')
        elif kind == 'master' and density == 'large':
            v = f'{s["lufs"]:.1f}'.replace('-', '−') if s.get('lv') else '—'
            lw = tw(v, 11) + 4 + tw(P.WORDS['lufs'], 9)
            out.append(text(cx - lw / 2, y + oy + 12, v, 11, t['ink1'] if s.get('lv') else t['ink4']))
            out.append(text(cx - lw / 2 + tw(v, 11) + 4, y + oy + 12, P.WORDS['lufs'], 9, t['ink3']))
        elif kind != 'vca':
            out.append(P.out_chip(t, cx, y + oy, s['out'], w, 10, full=kind == 'master'))
    if 'sock' in pl:
        sy, sh = pl['sock']
        out.append(P.socket(t, cx, y + sy + 7, s.get('sock_lit')))
    return ''.join(out)
