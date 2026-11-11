"""Slate (dir-a) — the inspector at the right (the selected strip: Chords) and the status
line with the hint that names what is under the pointer."""
import math
import glyphs as G
from base import lit_edge, rect, text, well, mix, tw, HUES, INSP_X, ARR_Y, STATUS_Y

X = INSP_X
W = 1280 - INSP_X
P = 12           # side padding


def title(t, y, s, right=None):
    out = text(X + P, y, s, 11, t['ink3'], 600, track=0.2)
    if right:
        out += text(X + W - P, y, right, 10, t['ink3'], 400, 'end')
    return out


def led(t, cx, cy, on=True):
    if on:
        return (f'<circle cx="{cx}" cy="{cy}" r="4.5" fill="{t["play"]}" opacity="0.18"/>'
                f'<circle cx="{cx}" cy="{cy}" r="2.5" fill="{t["play"]}"/>')
    return f'<circle cx="{cx}" cy="{cy}" r="2.5" fill="none" stroke="{t["ink4"]}" stroke-width="1"/>'


def eq_graph(t, x, y, w, h):
    out = well(t, x, y, w, h, 4)
    gx0, gx1 = x + 1, x + w - 1
    for f in (100, 1000, 10000):
        fx = gx0 + (gx1 - gx0) * (math.log10(f) - math.log10(20)) / 3
        out += rect(round(fx), y + 2, 1, h - 4, mix(t['well'], t['ink4'], 0.35))
    mid = y + h / 2
    out += rect(gx0, round(mid), gx1 - gx0, 1, mix(t['well'], t['ink4'], 0.5))

    def resp(f):
        lf = math.log10(f)
        g = 2.0 / (1 + math.exp((lf - 2.0) * 6))            # low shelf +2 dB at 100 Hz
        g += -2.5 * math.exp(-((lf - 3.0) ** 2) / 0.05)     # a bell −2.5 dB at 1 kHz
        g += 1.6 / (1 + math.exp(-(lf - 3.9) * 6))          # high shelf +1.6 dB at 8 kHz
        return g
    pts = []
    for k in range(0, gx1 - gx0 + 1):
        lf = math.log10(20) + 3 * k / (gx1 - gx0)
        pts.append((gx0 + k, mid - resp(10 ** lf) * (h / 2 - 6) / 6))
    d = 'M' + 'L'.join(f'{px:.1f} {py:.1f}' for px, py in pts)
    fill = d + f'L{gx1} {mid}L{gx0} {mid}Z'
    out += f'<path d="{fill}" fill="{t["accent"]}" opacity="{0.10 if t["name"] == "Night" else 0.08}"/>'
    out += f'<path d="{d}" fill="none" stroke="{t["ink1"]}" stroke-width="1.4"/>'
    bands = [(100, t['solo']), (1000, t['play']), (8000, t['accent'])]
    for f, col in bands:
        fx = gx0 + (gx1 - gx0) * (math.log10(f) - math.log10(20)) / 3
        fyy = mid - resp(f) * (h / 2 - 6) / 6
        out += f'<circle cx="{fx:.1f}" cy="{fyy:.1f}" r="3.5" fill="{t["well"]}" stroke="{col}" stroke-width="1.5"/>'
    for f, lab in ((100, '100'), (1000, '1k'), (10000, '10k')):
        fx = gx0 + (gx1 - gx0) * (math.log10(f) - math.log10(20)) / 3
        out += text(fx + 3, y + h - 4, lab, 9, t['ink3'])
    return out


def hbar(t, x, y, w, frac, col=None):
    out = rect(x, y, w, 3, mix(t['well'], t['ink4'], 0.3), 1.5)
    out += rect(x, y, round(w * frac), 3, col or t['ink2'], 1.5)
    out += f'<circle cx="{x + w * frac:.1f}" cy="{y + 1.5}" r="3.5" fill="{t["raised"] if t["name"] == "Night" else "#ffffff"}" stroke="{t["edge"]}" stroke-width="1"/>'
    return out


def inspector(t, state):
    hue = HUES['Chords']
    out = [rect(X, ARR_Y, W, 28, t['surface']), lit_edge(t, X, ARR_Y, W)]
    out.append(rect(X, ARR_Y, 3, 28, hue))
    out.append(text(X + P + 2, ARR_Y + 18, '6', 10, t['ink3']))
    out.append(text(X + P + 14, ARR_Y + 18, 'Chords', 13, t['ink1'], 500))
    out.append(text(X + W - P, ARR_Y + 18, 'Instrument track', 11, t['ink3'], 400, 'end'))
    body_top = ARR_Y + 28
    out.append(rect(X, body_top, W, 698 - body_top, t['base']))
    y = body_top + 12
    # ---- the instrument
    out.append(well(t, X + P, y, W - 2 * P, 28, 4))
    out.append(G.w_plugin(X + P + 14, y + 14, 20, t['ink1'], t['ink3']))
    out.append(text(X + P + 28, y + 18, 'Chorda', 12, t['ink1'], 500))
    out.append(text(X + P + 28 + tw('Chorda', 12, 500) + 6, y + 18, 'Glass pad', 11, t['ink3']))
    out.append(G.chevron_right(X + W - P - 12, y + 14, 3, t['ink3'], 1.2))
    y += 28 + 22
    # ---- inserts: the used slots, then one quiet line to add — no "Slot 3 … Slot 10"
    out.append(title(t, y, 'Inserts', '2 of 10'))
    y += 8
    for k, (nm, kind) in enumerate((('Comprimo', 'Compressor'), ('Aequalis', 'Equaliser'))):
        ry = y + k * 26
        out.append(rect(X + P, ry, W - 2 * P, 24, t['surface'], 4))
        out.append(text(X + P + 8, ry + 16, str(k + 1), 10, t['ink3']))
        out.append(text(X + P + 22, ry + 16, nm, 12, t['ink1']))
        out.append(text(X + P + 22 + tw(nm, 12) + 6, ry + 16, kind, 10, t['ink3']))
        out.append(led(t, X + W - P - 12, ry + 12))
    ry = y + 2 * 26
    out.append(G.plus(X + P + 10, ry + 12, 3.5, t['ink3'], 1.2))
    out.append(text(X + P + 22, ry + 16, 'Add an effect', 11, t['ink3']))
    y = ry + 24 + 18
    # ---- the EQ, drawn
    out.append(title(t, y, 'Equaliser', 'Aequalis'))
    y += 8
    out.append(eq_graph(t, X + P, y, W - 2 * P, 70))
    y += 70 + 6
    cols = [('Low shelf', '100 Hz', '+2.0'), ('Bell', '1.0 kHz', '−2.5'), ('High shelf', '8.0 kHz', '+1.6')]
    cw = (W - 2 * P) / 3
    bands = [t['solo'], t['play'], t['accent']]
    for k, (a, f, g) in enumerate(cols):
        cx = X + P + k * cw
        out.append(rect(cx, y + 4, 6, 2, bands[k], 1))
        out.append(text(cx + 9, y + 9, a, 10, t['ink3']))
        out.append(text(cx, y + 23, f, 11, t['ink2']))
        out.append(text(cx + cw - 8, y + 23, g, 11, t['ink1'], 400, 'end'))
    y += 28 + 18
    # ---- sends
    out.append(title(t, y, 'Sends'))
    y += 10
    for k, (nm, db, frac, pp) in enumerate((('Reverb', '−16.0', 0.42, 'Post'), ('Delay', '−22.0', 0.3, 'Post'))):
        ry = y + k * 26
        out.append(text(X + P, ry + 13, nm, 12, t['ink1']))
        out.append(hbar(t, X + P + 52, ry + 8, 96, frac))
        out.append(text(X + W - P - 30, ry + 13, db, 11, t['ink2'], 400, 'end'))
        out.append(text(X + W - P, ry + 13, pp, 10, t['ink3'], 400, 'end'))
    y += 2 * 26 + 12
    # ---- output
    out.append(title(t, y, 'Output'))
    y += 8
    out.append(well(t, X + P, y, W - 2 * P, 24, 4))
    out.append(text(X + P + 8, y + 16, 'Master', 12, t['ink1']))
    out.append(G.chevron_down(X + W - P - 10, y + 12, 2.5, t['ink3'], 1.2))
    y += 24 + 18
    # ---- the strip, in numbers: label and value, nothing that collides
    out.append(title(t, y, 'Level'))
    y += 6
    rows = [('Volume', '−3.5 dB', 0.72), ('Pan', 'Centre', 0.5), ('Width', '100 %', 1.0)]
    for k, (a, v, fr) in enumerate(rows):
        ry = y + k * 24
        out.append(text(X + P, ry + 16, a, 11, t['ink3']))
        out.append(hbar(t, X + P + 52, ry + 11, 96, fr))
        out.append(text(X + W - P, ry + 16, v, 11, t['ink1'], 400, 'end'))
    # ---- the folded sections: one quiet line each, their state on the right
    y += 3 * 24 + 14
    for k, (a, v) in enumerate((('Recording', 'No input'), ('Automation', '2 lanes'),
                                ('Sidechain', 'None'))):
        ry = y + k * 26
        out.append(rect(X + P, ry, W - 2 * P, 1, mix(t['base'], t['line'], 0.7)))
        out.append(G.chevron_right(X + P + 4, ry + 13.5, 2.5, t['ink3'], 1.2))
        out.append(text(X + P + 14, ry + 17, a, 11, t['ink2'], 500))
        out.append(text(X + W - P, ry + 17, v, 11, t['ink3'], 400, 'end'))
    return ''.join(out)


def status(t, state):
    y = STATUS_Y
    out = rect(0, y, 1280, 20, t['surface']) + lit_edge(t, 0, y, 1280)
    hint = state.get('hint', ('Chords · volume −3.5 dB', 'drag to change · double-click to type · Alt-click for 0 dB'))
    out += text(12, y + 14, hint[0], 11, t['ink1'])
    out += text(12 + tw(hint[0], 11) + 10, y + 14, hint[1], 11, t['ink3'])
    right = '48 kHz · 256 samples · 5.3 ms'
    out += text(1280 - 12, y + 14, 'Saved', 11, t['ink3'], 400, 'end')
    out += text(1280 - 12 - tw('Saved', 11) - 20, y + 14, right, 11, t['ink3'], 400, 'end')
    return out
