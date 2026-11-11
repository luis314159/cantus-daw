"""des-mixer — the inspector of a mixer strip: every section, open or folded.

Each section function takes (t, x, w, y, L, s) and returns (svg, y after it). A section
starts with a hairline and its title 20 px below its top; its content starts 10 px under
the title; sections are 8 px apart. Labels sit in a 64-px column (Regular 11, ink 3);
values to their right (Regular 12, ink 1) or right-aligned numbers (Regular 11, ink 1)."""
import math
import glyphs as G
import mglyphs as M
import parts as SP
import insp_parts as I
from insp_parts import P, LAB_W
from insp_text import T
from base import rect, text, well, mix, tw, HUES


def head(t, x, w, y, L, s):
    hue = HUES[s['name']]
    out = rect(x, y, w, 28, t['surface']) + rect(x, y, 3, 28, hue)
    out += rect(x, y, w, 1, mix(t['surface'], t['ink1'], 0.07) if t['name'] == 'Night' else '#f7f9fb')
    out += text(x + P + 2, y + 18, str(s['num']), 10, t['ink3'])
    room = w - P - 16 - tw(L['strip'], 11) - 36
    out += text(x + P + 16, y + 18, SP.fit(s['name'], 13, room, 500), 13, t['ink1'], 500)
    out += text(x + w - P - 20, y + 18, L['strip'], 11, t['ink3'], 400, 'end')
    out += G.more(x + w - P - 6, y + 14, 16, t['ink3'], t['ink3'])
    y += 28
    out += rect(x, y, w, 2000, t['base'])
    # where its sound comes from: the track feeding it, one click from its editor
    y += 10
    out += well(t, x + P, y, w - 2 * P, 28, 4)
    out += G.w_plugin(x + P + 14, y + 14, 20, t['ink1'], t['ink3'])
    out += text(x + P + 28, y + 18, s.get('inst', 'Calor'), 12, t['ink1'], 500)
    out += text(x + P + 28 + tw(s.get('inst', 'Calor'), 12, 500) + 6, y + 18, s.get('preset', 'Glass pad'), 10, t['ink3'])
    out += G.chevron_right(x + w - P - 12, y + 14, 3, t['ink3'], 1.2)
    return out, y + 28 + 8


def inserts(t, x, w, y, L, s, hover=1, mini=0):
    fx = s['fx']
    out = I.title(t, x, w, y + 20, L['inserts'], L['of'].format(n=len(fx), m=10))
    # the power key: every insert in or out at once
    y += 30
    for k, (nm, kind, on) in enumerate(fx):
        h = 26
        hov = k == hover
        bg = mix(t['surface'], t['ink1'], 0.04) if hov and t['name'] == 'Night' else (t['raised'] if hov else t['surface'])
        out += rect(x + P, y, w - 2 * P, h, bg, 4)
        if hov:
            out += M.grip(x + P + 9, y + 13, t['ink3'])
        else:
            out += text(x + P + 8, y + 17, str(k + 1), 10, t['ink3'])
        col = t['ink1'] if on else t['ink4']
        out += text(x + P + 22, y + 17, nm, 12, col)
        sub = kind if on else f'{kind} · {L["bypassed"]}'
        out += text(x + P + 22 + tw(nm, 12) + 6, y + 17, sub, 10, t['ink3'] if on else t['ink4'])
        if s.get('sidechain') and kind == 'Comp':
            kx = x + w - P - 24 - tw(s['sidechain'], 10) - (52 if hov else 0)
            out += M.key(kx, y + 13, 12, t['ink2'])
            out += text(kx + 7, y + 16.5, s['sidechain'], 10, t['ink2'])
        if hov:          # the mini-view key and the menu, only under the pointer
            out += rect(x + w - P - 46, y + 5, 16, 16, mix(t['surface'], t['edge'], 0.25), 3)
            out += (f'<rect x="{x + w - P - 42.5}" y="{y + 9.5}" width="9" height="7" rx="1" fill="none" stroke="{t["ink2"]}"/>'
                    f'<path d="M{x + w - P - 42.5} {y + 12.5}H{x + w - P - 33.5}" stroke="{t["ink3"]}"/>')
        out += I.led(t, x + w - P - 12, y + 13, on)
        y += h + 2
        if k == mini and kind == 'Comp':     # the mini view of a compressor, open under its row
            mh = 46
            out += rect(x + P, y - 2, w - 2 * P, mh, mix(t['base'], t['surface'], 0.55), 4)
            cw = (w - 2 * P - 16) / 3
            live = bool(s.get('lv'))      # stopped: no reduction, said quietly
            vals = ((L['thr'], '−18.0', 'dB'), (L['ratio'], '4.0', ': 1'), (L['gr'], '−3.2' if live else '0.0', 'dB'))
            for j, (a, v, u) in enumerate(vals):
                cx = x + P + 8 + j * cw
                out += text(cx, y + 12, a, 10, t['ink3'])
                out += text(cx, y + 28, v, 13, t['ink1'] if j < 2 else (t['solo'] if live else t['ink3']))
                out += text(cx + tw(v, 13) + 3, y + 28, u, 10, t['ink3'])
            # the reduction as a bar from the right
            gx, gw = x + P + 8 + 2 * cw, cw - 6
            out += rect(gx, y + 35, gw, 3, mix(t['well'], t['ink4'], 0.3), 1.5)
            if live:
                out += rect(gx + gw - gw * 0.32, y + 35, gw * 0.32, 3, t['solo'], 1.5)
            out += rect(x + P + 8, y + 35, 2 * cw - 14, 3, mix(t['well'], t['ink4'], 0.3), 1.5)
            out += rect(x + P + 8, y + 35, (2 * cw - 14) * 0.55, 3, t['ink2'], 1.5)
            y += mh + 2
    out += I.add_line(t, x, y + 16, L['add_fx'])
    return out, y + 24 + 4


def eq(t, x, w, y, L, s):
    out = I.title(t, x, w, y + 20, L['eq'], L['flat'], right_col=t['ink2'])
    y += 30
    gw = w - 2 * P
    out += SP.eq_thumb(t, x + P, y, gw, 72, s.get('eq', (2, -2.5, 1.6)), dots=True)
    for f, lab in ((100, '100'), (1000, '1k'), (10000, '10k')):
        fx = x + P + 2 + (gw - 4) * (math.log10(f) - math.log10(20)) / 3
        out += text(fx + 3, y + 68, lab, 9, t['ink4'] if t['name'] == 'Night' else t['ink3'])
    y += 72 + 8
    lo, mi, hi = s.get('eq', (2, -2.5, 1.6))
    cols = [(L['low'], '100 Hz', lo, t['solo']), (L['bell'], '1.0 kHz', mi, t['play']), (L['high'], '8.0 kHz', hi, t['accent'])]
    cw = gw / 3
    for k, (a, f, g, col) in enumerate(cols):
        cx = x + P + k * cw
        out += rect(cx, y + 4, 6, 2, col, 1)
        out += text(cx + 9, y + 9, SP.fit(a, 10, cw - 12), 10, t['ink3'])
        out += text(cx, y + 24, f, 11, t['ink2'])
        out += text(cx + cw - 8, y + 24, SP.dbs(g), 11, t['ink1'], 400, 'end')
    return out, y + 32


def sends(t, x, w, y, L, s):
    out = I.title(t, x, w, y + 20, L['sends'], L['after'])
    y += 30
    for nm, db, on in s.get('sends', []):
        # the send's switch: a small well; on, an accent dot inside it (no filled block)
        out += well(t, x + P, y + 2, 14, 14, 3)
        if on:
            out += f'<circle cx="{x + P + 7}" cy="{y + 8.5}" r="3" fill="{t["accent"]}"/>'
        else:
            out += f'<circle cx="{x + P + 7}" cy="{y + 8.5}" r="2.5" fill="none" stroke="{t["ink4"]}"/>'
        out += text(x + P + 22, y + 13, SP.fit(nm, 12, 74), 12, t['ink1'] if on else t['ink4'])
        out += I.hbar(t, x + P + 100, y + 7.5, 64, 1 - SP.frac(db), t['ink2'] if on else t['ink4'])
        out += text(x + w - P, y + 13, SP.dbs(db), 11, t['ink1'] if on else t['ink4'], 400, 'end')
        y += 26
    out += I.add_line(t, x, y + 12, L['add_send'])
    return out, y + 20


def routing(t, x, w, y, L, s):
    out = I.title(t, x, w, y + 20, L['routing'])
    y += 30
    fx = x + P + LAB_W
    fw = w - P - LAB_W - P
    out += I.label(t, x, y + 16, L['output'])
    out += I.field(t, fx, y, fw, L['master'] if s['out'] == 'Master' else s['out'],
                   icon=lambda a, b: M.out_arrow(a, b, 12, t['ink3']))
    y += 30
    out += I.label(t, x, y + 16, L['input'])
    out += I.field(t, fx, y, fw, s['name'], L['track'], icon=lambda a, b: M.into(a, b, 12, t['ink3']))
    y += 30
    out += I.label(t, x, y + 16, L['channel'])
    # three switches drawn as glyph + word (no well at rest, an accent pill when on);
    # they start in the value column and wrap under the label when the words are long
    total = sum(6 + 12 + 3 + tw(lb, 10) + 6 + 2 for lb in (L['polarity'], L['swap'], L['mono']))
    kx = fx if total <= fw else x + P
    if total > fw:
        y += 20
    for k, (g, lab, on) in enumerate(((M.phase, L['polarity'], s.get('phase_on')), (M.swap, L['swap'], s.get('swap_on')),
                                       (M.mono, L['mono'], s.get('mono_on')))):
        kw = 6 + 12 + 3 + tw(lab, 10) + 6
        if on:
            out += rect(kx, y + 3, kw, 18, mix(t['base'], t['accent'], 0.20), 4)
        ink = t['accent'] if on else t['ink2']
        out += g(kx + 12, y + 11.5, 16, ink, t['accent'] if on else t['ink3'])
        out += text(kx + 21, y + 16, lab, 10, ink if on else t['ink3'])
        kx += kw + 2
    return out, y + 30


def sidechain(t, x, w, y, L, s):
    src = s.get('sidechain')
    out = I.title(t, x, w, y + 20, L['sidechain'], src or L['none'])
    y += 30
    fx = x + P + LAB_W
    fw = w - P - LAB_W - P
    out += I.label(t, x, y + 16, L['key_in'])
    out += I.field(t, fx, y, fw, 'Comprimo', '1', icon=lambda a, b: M.key(a, b, 14, t['ink2']))
    y += 30
    out += I.label(t, x, y + 16, L['key_src'])
    hue = HUES.get(src or 'Kick')
    out += I.field(t, fx, y, fw, src or L['none'], L['pre'],
                   icon=lambda a, b: f'<rect x="{a - 3}" y="{b - 3}" width="6" height="6" rx="1.5" fill="{hue}"/>')
    y += 30
    out += I.label(t, x, y + 12, L['listen'])
    out += I.toggle(t, x + w - P - 26, y + 1, False)
    return out, y + 22


def recording(t, x, w, y, L, s):
    armed = s.get('arm')
    out = I.title(t, x, w, y + 20, L['recording'], L['arm'] if armed else L['no_input'],
                  right_col=t['rec'] if armed else None)
    y += 30
    fx = x + P + LAB_W
    fw = w - P - LAB_W - P
    out += I.label(t, x, y + 16, L['input'])
    out += I.field(t, fx, y, fw - 28, 'In 1–2', 'Realtek', icon=lambda a, b: M.into(a, b, 12, t['ink3']))
    out += SP.key(t, x + w - P - 24, y, 24, 24, None, t['rec'] if armed else None,
                  glyph=lambda a, b, ink: M.arm(a, b + 0.5, 18, ink if armed else t['ink3']))
    y += 30
    out += I.label(t, x, y + 14, L['monitor'])
    out += I.seg(t, fx, y, L['mon'], 1)[0]
    y += 26
    out += I.label(t, x, y + 14, L['record'])
    out += I.seg(t, fx, y, L['rec'], 1, pad=14)[0]
    y += 26
    out += I.label(t, x, y + 12, L['low_lat'])
    out += I.toggle(t, x + w - P - 26, y + 1, False)
    return out, y + 22


def automation(t, x, w, y, L, s):
    out = I.title(t, x, w, y + 20, L['automation'], L['lanes'].format(n=2))
    y += 30
    fx = x + P + LAB_W
    out += I.label(t, x, y + 14, L['mode'])
    out += I.seg(t, fx, y, L['modes'], 0, pad=12, minw=30)[0]
    y += 28
    lanes = [(L['volume'], [(0, .55), (.3, .55), (.42, .8), (.7, .8), (1, .62)], 5),
             ('Comprimo · ' + L['thr'], [(0, .3), (.5, .3), (.62, .7), (1, .7)], 4)]
    for nm, pts, n in lanes:
        out += text(x + P, y + 12, SP.fit(nm, 11, 110), 11, t['ink2'])
        out += text(x + w - P, y + 12, L['points'].format(n=n), 10, t['ink3'], 400, 'end')
        out += I.sparkline(t, x + P + 118, y + 2, 56, 12, pts, t['auto'])
        y += 22
    out += text(x + P, y + 12, L['show_lanes'], 11, t['accent'])
    return out, y + 20


def level(t, x, w, y, L, s):
    out = I.title(t, x, w, y + 20, L['level'])
    y += 26
    rows = [(L['volume'], SP.dbs(s['db']) + ' dB', 1 - SP.frac(s['db']), False),
            (L['pan'], L['centre'], 0.5 + s.get('pan', 0) / 2, True),
            (L['width'], f'{round(s.get("width", 1) * 100)} %', s.get('width', 1) / 2, False)]
    for a, v, fr, bip in rows:
        out += I.label(t, x, y + 16, a)
        out += I.hbar(t, x + P + LAB_W, y + 11, 96, fr, bipolar=bip)
        out += text(x + w - P, y + 16, v, 11, t['ink1'], 400, 'end')
        y += 24
    y += 4
    fx = x + P + LAB_W
    out += I.label(t, x, y + 16, L['vca'])
    out += I.field(t, fx, y, w - P - LAB_W - P, s.get('vca') and 'Drums' or L['none'], 'VCA 1' if s.get('vca') else None)
    y += 30
    for k, (a, on) in enumerate(((L['safe_snap'], False), (L['safe_solo'], False))):
        out += text(x + P, y + 12, a, 11, t['ink3'])
        out += I.toggle(t, x + w - P - 26, y + 1, on)
        y += 22
    return out, y


def freeze(t, x, w, y, L, s):
    fr = s.get('frozen')
    out = I.title(t, x, w, y + 20, L['freeze'], L['frozen'] if fr else None, right_col=t['accent'] if fr else None)
    y += 30
    kw = round(tw(L['unfreeze'] if fr else L['freeze_k'], 11, 500) + 34)
    out += well(t, x + P, y, kw, 24, 4)
    out += M.snow(x + P + 12, y + 12, 14, t['accent'] if fr else t['ink2'])
    out += text(x + P + 22, y + 16, L['unfreeze'] if fr else L['freeze_k'], 11, t['ink1'], 500)
    d = L['frozen_d'] if fr else L['freeze_d']
    # the description wraps in two lines under the key
    words, lines, cur = d.split(' '), [], ''
    for wd in words:
        if tw(cur + ' ' + wd, 10) > w - 2 * P and cur:
            lines.append(cur)
            cur = wd
        else:
            cur = (cur + ' ' + wd).strip()
    lines.append(cur)
    for k, ln in enumerate(lines):
        out += text(x + P, y + 40 + k * 13, ln, 10, t['ink3'])
    return out, y + 40 + len(lines) * 13


def row_states(t, x, w, y, L):
    """An insert row in each of its states: rest, under the pointer, bypassed, dragged
    (lifted, with the line where it will land), and the empty states of three sections."""
    es = L['eq'] != 'Equaliser'
    S = (('Una fila de insert, sus estados', 'Reposo', 'Puntero encima', 'Puenteado', 'Arrastrado', 'Secciones vacías')
         if es else ('An insert row, its states', 'Rest', 'Pointer over it', 'Bypassed', 'Dragged', 'Empty sections'))
    out = text(x + P, y + 18, S[0], 12, t['ink1'], 500)
    y += 30
    for lab, st in ((S[1], 'rest'), (S[2], 'hover'), (S[3], 'off'), (S[4], 'drag')):
        out += text(x + P, y + 10, lab, 10, t['ink3'])
        y += 16
        h = 26
        if st == 'drag':      # lifted: the raised surface, a soft shadow, the landing line above
            out += rect(x + P, y - 4, w - 2 * P, 2, t['accent'], 1)
            out += '<defs><filter id="blur3"><feGaussianBlur stdDeviation="3"/></filter></defs>'
            out += (f'<rect x="{x + P + 4}" y="{y + 6}" width="{w - 2 * P - 8}" height="{h}" rx="4" '
                    f'fill="{t["shadow"]}" filter="url(#blur3)"/>')
            out += rect(x + P + 4, y + 2, w - 2 * P - 8, h, t['raised'], 4)
            bx, bw = x + P + 4, w - 2 * P - 8
        else:
            hov = st == 'hover'
            bg = (t['raised'] if t['name'] == 'Paper' else mix(t['surface'], t['ink1'], 0.04)) if hov else t['surface']
            out += rect(x + P, y, w - 2 * P, h, bg, 4)
            bx, bw = x + P, w - 2 * P
        yy = y + (2 if st == 'drag' else 0)
        on = st != 'off'
        if st in ('hover', 'drag'):
            out += M.grip(bx + 9, yy + 13, t['ink3'])
        else:
            out += text(bx + 8, yy + 17, '2', 10, t['ink3'])
        out += text(bx + 22, yy + 17, 'Aequalis', 12, t['ink1'] if on else t['ink4'])
        out += text(bx + 22 + tw('Aequalis', 12) + 6, yy + 17, 'EQ' if on else f'EQ · {L["bypassed"]}', 10,
                    t['ink3'] if on else t['ink4'])
        if st == 'hover':
            out += rect(bx + bw - 34, yy + 5, 16, 16, mix(t['surface'], t['edge'], 0.25), 3)
            out += (f'<rect x="{bx + bw - 30.5}" y="{yy + 9.5}" width="9" height="7" rx="1" fill="none" stroke="{t["ink2"]}"/>'
                    f'<path d="M{bx + bw - 30.5} {yy + 12.5}H{bx + bw - 21.5}" stroke="{t["ink3"]}"/>')
        out += I.led(t, bx + bw - 12, yy + 13, on)
        y += h + 10
    out += text(x + P, y + 14, S[5], 12, t['ink1'], 500)
    y += 16
    for a, b in ((L['inserts'], L['add_fx']), (L['sends'], L['add_send']), (L['sidechain'], None)):
        out += I.title(t, x, w, y + 20, a, '0' if b else L['none'])
        y += 30
        if b:
            out += I.add_line(t, x, y + 6, b)
            y += 16
    return out, y


def folded(t, x, w, y, rows):
    """Folded sections: one quiet line each, their state on the right."""
    out = ''
    for a, v in rows:
        out += I.title(t, x, w, y + 20, a, v, folded=True)
        y += 28
    return out, y


SECTIONS = [inserts, eq, sends, routing, sidechain, recording, automation, level, freeze]
