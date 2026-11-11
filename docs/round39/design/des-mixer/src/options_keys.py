"""des-mixer — options for the strip's keys (M, S, arm and the polarity row), in three
densities. Each option draws the top of a strip (name, polarity row, pan, keys) in three
states: at rest, muted with polarity on, soloed and armed. Dashed boxes on the first strip
of each group are the hit targets.

A — as built (keys a share of the strip; Wide about 27 x 18).
B — true 24 x 24 targets everywhere (stacked in Compact).
C — one segmented pill per strip, 24 tall, letters 11; arm moves to the head; the width
    knob joins the pan row so the polarity cells can be 26 wide.
"""
import data  # noqa: F401  (the user colours of the sheets)
import mglyphs as M
import parts as P
from base import rect, text, well, mix, tw, HUES, hue_for

WIDTH = {'compact': 30, 'wide': 60, 'large': 94}
FRAG_H = 196


def target(t, x, y, w, h):
    return (f'<rect x="{x + 0.5}" y="{y + 0.5}" width="{w - 1}" height="{h - 1}" fill="none" '
            f'stroke="{t["accent"]}" stroke-width="1" stroke-dasharray="2 2" opacity="0.9"/>')


def joined(t, x, y, ws, h, labels, ons, size=11):
    """A segmented pill: one well, segments split by the well's dark edge; an 'on'
    segment fills with its state colour inside the well."""
    W = sum(ws)
    out = well(t, x, y, W, h, 4)
    xx = x
    for k, (w, lab, on) in enumerate(zip(ws, labels, ons)):
        if on:
            out += rect(xx + 1.5, y + 1.5, w - 3, h - 4, on, 3)
        ink = ('#10151a' if t['name'] == 'Night' else '#ffffff') if on else t['ink3']
        if callable(lab):
            out += lab(xx + w / 2, y + h / 2 - 0.5, ink)
        else:
            out += text(xx + w / 2, y + h / 2 + size * 0.36 - 0.5, lab, size, ink, 600, 'middle')
        if k < len(ws) - 1:
            out += rect(xx + w - 0.5, y + 4, 1, h - 9, t['edge'])
        xx += w
    return out


def head(t, x, y, w, name, num, dens, arm_dot=None):
    out = rect(x, y, w, FRAG_H, f'url(#strip-{t["name"]})', 3)
    out += rect(x, y, w, 3, hue_for(t, name, t['stripe_rest']))
    cx = x + w / 2
    if dens == 'compact':
        out += text(cx, y + 14, num, 10, t['ink3'], 400, 'middle')
        out += f'<g transform="translate({cx + 4},{y + 70}) rotate(-90)">' + text(0, 0, name, 12, t['ink1']) + '</g>'
    else:
        out += text(cx, y + 15, num, 10, t['ink3'], 400, 'middle')
        out += text(cx, y + 30, P.fit(name, 12, w - 6), 12, t['ink1'], 400, 'middle')
    if arm_dot is not None:      # option C: arm in the head, left of the number (a 24 x 24 target)
        ax, ay = (x + 7, y + 10) if dens != 'compact' else (cx, y + 24)
        on = arm_dot
        if on:
            out += f'<circle cx="{ax}" cy="{ay}" r="5" fill="{mix(t["surface"], t["rec"], 0.25)}"/>'
        out += M.arm(ax, ay, 16, t['rec'] if on else t['ink4'])
    return out


def fade(t, x, y, w):
    """The top of the fader under the keys, fading out: the fragment's end."""
    return (rect(x + w / 2 - 10, y, 3, 30, mix(t['well'], t['edge'], 0.55), 1.5) +
            f'<rect x="{x}" y="{y + 8}" width="{w}" height="24" fill="url(#fragfade-{t["name"]})"/>' +
            rect(x, y + 32, w, 260, t['base']))


def frag(t, x, y, dens, opt, state, show_targets):
    w = WIDTH[dens]
    cx = x + w / 2
    name = {'compact': 'Kick', 'wide': 'Chords', 'large': 'Lead'}[dens]
    name = [name, 'Pluck', 'Vox lead'][state] if dens != 'compact' else ['Kick', 'Clap', 'Vox'][state]
    num = str([1, 7, 17][state])
    muted, solo, arm, ph = state == 1, state == 2, state == 2, state == 1
    mute_c = t['ink2'] if t['name'] == 'Night' else t['ink3']
    out = head(t, x, y, w, name, num, dens, arm if opt == 'C' else None)
    tg = ''
    g16 = lambda g, on: (lambda a, b, ink: g(a, b, 16, t['accent'] if on else t['ink3'], t['accent'] if on else t['ink3']))  # noqa: E731
    if dens == 'compact':
        if opt == 'A':
            ky = y + 104
            out += P.knob(t, cx, ky - 14, 7, 0.0)
            out += P.key(t, x + 1, ky, 13, 14, 'M', mute_c if muted else None, 9)
            out += P.key(t, x + 16, ky, 13, 14, 'S', t['solo'] if solo else None, 9)
            if arm:
                out += P.key(t, x + 1, ky + 16, 28, 14, None, t['rec'], glyph=lambda a, b, ink: M.arm(a, b, 14, ink))
            else:
                out += M.arm(cx, ky + 22.5, 14, t['ink4'])
            if show_targets:
                tg += target(t, x + 1, ky, 13, 14) + target(t, x + 16, ky, 13, 14) + target(t, x + 1, ky + 16, 28, 14)
            out += fade(t, x, ky + 34, w)
        elif opt == 'B':
            ky = y + 92
            out += P.knob(t, cx, ky - 12, 7, 0.0)
            for k, (lab, on) in enumerate((('M', mute_c if muted else None), ('S', t['solo'] if solo else None))):
                out += P.key(t, x + 3, ky + k * 26, 24, 24, lab, on, 11)
                if show_targets:
                    tg += target(t, x + 3, ky + k * 26, 24, 24)
            out += P.key(t, x + 3, ky + 52, 24, 24, None, t['rec'] if arm else None,
                         glyph=lambda a, b, ink: M.arm(a, b, 16, ink if arm else t['ink3']))
            if show_targets:
                tg += target(t, x + 3, ky + 52, 24, 24)
            out += fade(t, x, ky + 78, w)
        else:
            ky = y + 98
            out += P.knob(t, cx, ky - 14, 7, 0.0)
            # a vertical pill: M over S, each 26 x 22 (24 with the gap shared)
            out += well(t, x + 2, ky, 26, 46, 4)
            for k, (lab, on) in enumerate((('M', mute_c if muted else None), ('S', t['solo'] if solo else None))):
                yy = ky + k * 23
                if on:
                    out += rect(x + 3.5, yy + 1.5, 23, 20, on, 3)
                ink = ('#10151a' if t['name'] == 'Night' else '#ffffff') if on else t['ink3']
                out += text(cx, yy + 16, lab, 11, ink, 600, 'middle')
            out += rect(x + 6, ky + 22.5, 18, 1, t['edge'])
            if show_targets:
                tg += target(t, x + 2, ky - 1, 26, 24) + target(t, x + 2, ky + 23, 26, 24)
                tg += target(t, cx - 12, y + 12, 24, 24)
            out += fade(t, x, ky + 50, w)
    else:
        L = dens == 'large'
        py = y + 42 if not L else y + 44
        if opt == 'A':
            # the polarity row, glyphs without wells; width knob at the right; then pan; keys a share
            out += g16(M.phase, ph)(x + 11, py + 8, None) + g16(M.swap, False)(x + 29, py + 8, None)
            if L:
                out += g16(M.mono, False)(x + 47, py + 8, None)
            out += P.knob(t, x + w - 12, py + 8, 6, 0.5, bipolar=False)
            out += P.knob(t, cx, py + 34, 9 if not L else 11, 0.0)
            ky = py + 52 if not L else py + 56
            keys = [('M', mute_c if muted else None), ('S', t['solo'] if solo else None)]
            if L:
                keys.append(('arm', t['rec'] if arm else None))
            n = len(keys)
            kw = (w - 6 - 2 * (n - 1)) / n
            for k, (lab, on) in enumerate(keys):
                kx = x + 3 + k * (kw + 2)
                if lab == 'arm':
                    out += P.key(t, kx, ky, kw, 18, None, on, glyph=lambda a, b, ink: M.arm(a, b, 16, ink if arm else t['ink3']))
                else:
                    out += P.key(t, kx, ky, kw, 18, lab, on, 10)
                if show_targets:
                    tg += target(t, kx, ky, kw, 18)
            if show_targets:
                tg += target(t, x + 3, py, 16, 16) + target(t, x + 21, py, 16, 16)
            out += fade(t, x, ky + 22, w)
        elif opt == 'B':
            cells = [M.phase, M.swap] + ([M.mono] if L else [])
            for k, g in enumerate(cells):
                kx = x + 4 + k * 26 if not L else x + 4 + k * 26
                out += g16(g, ph and k == 0)(kx + 12, py + 12, None)
                if show_targets:
                    tg += target(t, kx, py, 24, 24)
            wx = x + w - 15
            if not L:          # Wide: the width knob joins the pan row (no room for a third 24)
                out += P.knob(t, cx - 10, py + 44, 9, 0.0) + P.knob(t, x + w - 13, py + 44, 7, 0.5, bipolar=False)
                if show_targets:
                    tg += target(t, cx - 22, py + 32, 24, 24) + target(t, x + w - 25, py + 32, 24, 24)
            else:
                out += P.knob(t, wx, py + 12, 7, 0.5, bipolar=False)
                out += P.knob(t, cx, py + 44, 11, 0.0)
                if show_targets:
                    tg += target(t, wx - 12, py, 24, 24)
            ky = py + 64
            keys = [('M', mute_c if muted else None), ('S', t['solo'] if solo else None), ('arm', t['rec'] if arm else None)]
            for k, (lab, on) in enumerate(keys):
                if not L:      # Wide: M S on one row, arm under M
                    kx, kyy = (x + 4 + (k % 2) * 28, ky + (k // 2) * 26)
                else:
                    kx, kyy = x + 4 + k * 30 + 1, ky
                kw = 24 if not L else 26
                if lab == 'arm':
                    out += P.key(t, kx, kyy, kw, 24, None, on, glyph=lambda a, b, ink: M.arm(a, b, 16, ink if arm else t['ink3']))
                else:
                    out += P.key(t, kx, kyy, kw, 24, lab, on, 11)
                if show_targets:
                    tg += target(t, kx, kyy, kw, 24)
            out += fade(t, x, ky + (54 if not L else 28), w)
        else:
            # C: polarity cells in one quiet pill (26 or 28 wide, 22 tall); width beside pan
            cells = [M.phase, M.swap] + ([M.mono] if L else [])
            cw = 26 if not L else 28
            out += joined(t, x + 4, py, [cw] * len(cells), 22, [g16(g, ph and k == 0) for k, g in enumerate(cells)],
                          [mix(t['surface'], t['accent'], 0.30) if (ph and k == 0) else None for k in range(len(cells))])
            if ph:     # the lit cell's glyph again in accent over its tint
                out += M.phase(x + 4 + cw / 2, py + 10.5, 16, t['accent'], t['accent'])
            if show_targets:
                for k in range(len(cells)):
                    tg += target(t, x + 4 + k * cw, py - 1, cw, 24)
            pr = 9 if not L else 11
            out += P.knob(t, cx - (10 if not L else 14), py + 46, pr, 0.0)
            out += P.knob(t, x + w - (13 if not L else 18), py + 46, 7, 0.5, bipolar=False)
            if show_targets:
                tg += target(t, cx - (10 if not L else 14) - 12, py + 34, 24, 24) + target(t, x + w - (13 if not L else 18) - 12, py + 34, 24, 24)
                tg += target(t, x + 7 - 12, y + 10 - 12, 24, 24)
            ky = py + 68
            n = 2
            ws = [(w - 8) / n] * n
            out += joined(t, x + 4, ky, ws, 24, ['M', 'S'], [mute_c if muted else None, t['solo'] if solo else None], 12)
            if show_targets:
                tg += target(t, x + 4, ky, ws[0], 24) + target(t, x + 4 + ws[0], ky, ws[1], 24)
            out += fade(t, x, ky + 28, w)
    return out, tg


SPEC = {
    'A': ('A · as built', 'keys a share of the strip; arm a dot at rest',
          {'compact': (13, 14), 'wide': (27, 18), 'large': (28, 18)}),
    'B': ('B · true 24 × 24 targets', 'every key 24 × 24; stacked where the strip is narrow',
          {'compact': (24, 24), 'wide': (24, 24), 'large': (26, 24)}),
    'C': ('C · one pill, taller (recommended)', 'M | S one segmented pill 24 tall, letters 12; arm in the head; polarity cells 26 × 22',
          {'compact': (26, 23), 'wide': (26, 24), 'large': (43, 24)}),
}


def size_line(wh):
    w, h = wh
    return f'{w} × {h} logical · {w * 1.25:g} × {h * 1.25:g} px at 125 %'
