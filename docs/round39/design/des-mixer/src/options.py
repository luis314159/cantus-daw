"""des-mixer — the options sheet: the strip's keys (A, B, C) and the routing cables (A, B, C),
one page per theme, rendered at 125 %; then compose.py adds the 4x crops under it.

python -I options.py  ->  ../build/options-{night,paper}.html
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import options_keys as K  # noqa: E402
import options_cables as C  # noqa: E402
from base import NIGHT, PAPER, defs, rect, text, page  # noqa: E402
from sheets import heading, panel  # noqa: E402

W, HT = 1200, 1770
GX = {'compact': 270, 'wide': 396, 'large': 612}
ROW0, ROW_H = 92, 226
CAB_Y = 800


def para(t, x, y, lines, size=11, col=None, lh=15, first_col=None):
    out = ''
    for k, ln in enumerate(lines):
        out += text(x, y + k * lh, ln, size, (first_col if (k == 0 and first_col) else (col or t['ink2'])),
                    500 if (k == 0 and first_col) else 400)
    return out


def keys_section(t):
    out = [text(24, 76, 'The strip\'s keys — M, S, arm and the polarity row', 15, t['ink1'], 500),
           text(400, 76, 'dashed: the hit target · strips 1–3 of each group: at rest · muted with polarity on · soloed and armed',
                10, t['ink3'])]
    for r, opt in enumerate('ABC'):
        y = ROW0 + r * ROW_H
        title, desc, sizes = K.SPEC[opt]
        out.append(panel(t, 12, y - 8, 1176, ROW_H - 12))
        out.append(text(24, y + 14, title, 13, t['ink1'], 500))
        words, line, lines = desc.split(' '), '', []
        for wd in words:
            if len(line + ' ' + wd) > 40:
                lines.append(line)
                line = wd
            else:
                line = (line + ' ' + wd).strip()
        lines.append(line)
        out.append(para(t, 24, y + 32, lines, 10, t['ink3'], 13))
        yy = y + 40 + len(lines) * 13
        for dens in ('compact', 'wide', 'large'):
            out.append(text(24, yy, dens.capitalize(), 10, t['ink2'], 600))
            out.append(text(84, yy, K.size_line(sizes[dens]), 10, t['ink3']))
            yy += 14
        for dens in ('compact', 'wide', 'large'):
            gx = GX[dens]
            w = K.WIDTH[dens]
            out.append(text(gx, y + 6, f'{dens.capitalize()} · {w}', 10, t['ink3']))
            tgs = ''
            for k in range(3):
                x = gx + k * (w + 4)
                body, tg = K.frag(t, x, y + 14, dens, opt, k, k == 0)
                out.append(f'<clipPath id="fc-{opt}{dens}{k}-{t["name"]}"><rect x="{x}" y="{y + 14}" width="{w}" height="{K.FRAG_H - 10}"/></clipPath>'
                           f'<g clip-path="url(#fc-{opt}{dens}{k}-{t["name"]})">{body}</g>')
                tgs += tg
            out.append(tgs)
    # the recommendation, beside the rows
    rx = 930
    out.append(para(t, rx, ROW0 + 14, [
        'Recommended: C',
        'One shape per strip instead of two or',
        'three boxes: calmer (§6j, boxes inside',
        'boxes), and every target is 24 tall and',
        '26 wide or more: B\'s reach without B\'s',
        'second row of keys. Letters at 12 read',
        'at arm\'s length. Arm moves to the head',
        '(a 24 × 24 dot by the number): it is the',
        'key a mix touches least, and recording',
        'mode already arms from the track.',
        'Polarity and swap sit in one quiet pill;',
        'the width knob joins pan. B is the safe',
        'choice for touch; A misses 24 in height',
        'in every density.'], 11, t['ink2'], 14, t['ink1']))
    return ''.join(out)


def cables_section(t):
    out = [text(24, CAB_Y - 12, 'The routing cables — Kick selected: to the Drum bus (a bus), a send to Reverb (a return), '
                'the key of Bass\'s compressor (a sidechain)', 15, t['ink1'], 500)]
    cells = [('A', False, 'A · as built', 'one band at the desk\'s foot, a lane per kind, a dot per strip'),
             ('B', False, 'B · curved cables, the selected strip', 'from the output (dot) to the input (arrow), under the strips'),
             ('B', True, 'B · with "Show all" on', 'the other strips\' routes faint; the master is implied, never drawn'),
             ('C', False, 'C · no cables by default (recommended)', 'the destinations light with an arrow and the kind; "3 routes" under Kick')]
    for k, (opt, show, title, sub) in enumerate(cells):
        px = 24 + (k % 2) * 496
        py = CAB_Y + 8 + (k // 2) * 480
        out.append(panel(t, px - 12, py - 8, 484, 468))
        out.append(text(px, py + 10, title, 12, t['ink1'], 500))
        out.append(text(px, py + 25, sub, 10, t['ink3']))
        body, _ = C.option(t, px, py + 40, opt, show)
        out.append(body)
    rx = 1010
    out.append(para(t, rx, CAB_Y + 18, [
        'Recommended: C,',
        'with B on demand',
        '',
        'A draws every route on one',
        'line: where a cable starts',
        'and ends is guesswork, and',
        'lanes overlap on a busy desk.',
        'B reads at once but becomes',
        'a web with "Show all".',
        'C costs nothing at rest:',
        'each strip names where it',
        'goes; selecting or hovering',
        'lights its destinations with',
        'the kind (out, send, key).',
        'A "Cables" key in the desk',
        'header shows B\'s curves',
        'when the whole picture is',
        'wanted, "Show all" inside it.'], 11, t['ink2'], 15, t['ink1']))
    return ''.join(out)


def sheet(t):
    fade = (f'<defs><linearGradient id="fragfade-{t["name"]}" x1="0" y1="0" x2="0" y2="1">'
            f'<stop offset="0" stop-color="{t["base"]}" stop-opacity="0"/><stop offset="1" stop-color="{t["base"]}"/>'
            '</linearGradient></defs>')
    body = (defs(t) + fade + rect(0, 0, W, HT, t['gutter']) +
            heading(t, 24, 36, 'Options — the strip\'s keys and the routing cables',
                    'Slate · drawn at 125 % (1 logical px = 1.25 physical) · the 4× crops are under the sheet · the owner chooses') +
            keys_section(t) + cables_section(t))
    return f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{HT}" viewBox="0 0 {W} {HT}">{body}</svg>'


if __name__ == '__main__':
    out = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'build')
    os.makedirs(out, exist_ok=True)
    for t in (NIGHT, PAPER):
        with open(os.path.join(out, f'options-{t["name"].lower()}.html'), 'w', encoding='utf-8') as f:
            f.write(page(sheet(t), W, HT, t['gutter']))
    print('ok')
