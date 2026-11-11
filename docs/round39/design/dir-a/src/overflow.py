"""Slate (dir-a) — the top bar's overflow panel, designed again: the three groups the
narrow window folds (Metronome, Windows, Performance) and "Customise top bar…".

Hierarchy: one quiet title per group (Title case, ink-3 — no capital bars); a label column
and a control column on one grid; no box inside a box; each window key with its shortcut
under it; live meters as small graphs with their numbers; the customise line as a line.
"""
import math
import glyphs as G
from base import rect, text, well, mix, tw
from topbar import segmented

W = 316
X = 1280 - 10 - W
Y = 40
P = 16
LC = 84      # the control column


def keycap(t, cx, y, s):
    """A shortcut as a small engraved cap: outline only, ink-3 — readable, never loud."""
    w = tw(s, 9, 500) + 8
    return (f'<rect x="{cx - w / 2:.1f}" y="{y + 0.5}" width="{w:.1f}" height="13" rx="2.5" fill="none" '
            f'stroke="{mix(t["raised"], t["ink3"], 0.45)}" stroke-width="1"/>'
            + text(cx, y + 10, s, 9, t['ink3'], 500, 'middle'))


def toggle(t, x, y, on):
    w, h = 26, 14
    if on:
        out = rect(x, y, w, h, t['accent'], 7)
        out += f'<circle cx="{x + w - 7}" cy="{y + 7}" r="5" fill="{t["accent_ink"] if t["name"] == "Paper" else "#eaf4fc"}"/>'
    else:
        out = well(t, x, y, w, h, 7)
        out += f'<circle cx="{x + 7}" cy="{y + 7}" r="4.5" fill="{t["ink3"]}"/>'
    return out


def slider(t, x, y, w, frac):
    out = rect(x, y - 1.5, w, 3, mix(t['well'], t['ink4'], 0.25), 1.5)
    out += rect(x, y - 1.5, w * frac, 3, t['ink2'], 1.5)
    kx = x + w * frac
    out += f'<circle cx="{kx:.1f}" cy="{y}" r="5.5" fill="{t["edge"]}"/>'
    out += f'<circle cx="{kx:.1f}" cy="{y}" r="4.5" fill="{"#3b444e" if t["name"] == "Night" else "#ffffff"}"/>'
    return out


def graph(t, x, y, w, h, series, kind='line', col=None, top=1.0):
    col = col or t['ink2']
    out = well(t, x, y, w, h, 3)
    n = len(series)
    ix0, iy0, iw, ih = x + 2, y + 2, w - 4, h - 5
    if kind == 'bars':
        bw = iw / n
        for k, v in enumerate(series):
            bh = max(1, ih * v / top)
            out += rect(round(ix0 + k * bw, 1), round(iy0 + ih - bh, 1), max(1, bw - 1), round(bh, 1), mix(t['well'], col, 0.75))
        return out
    pts = [(ix0 + iw * k / (n - 1), iy0 + ih - ih * v / top) for k, v in enumerate(series)]
    d = 'M' + 'L'.join(f'{px:.1f} {py:.1f}' for px, py in pts)
    out += f'<path d="{d}L{ix0 + iw} {iy0 + ih}L{ix0} {iy0 + ih}Z" fill="{col}" opacity="0.16"/>'
    out += f'<path d="{d}" fill="none" stroke="{col}" stroke-width="1.2" stroke-linejoin="round"/>'
    return out


def series(seed, n, base, var):
    return [max(0.01, base + var * (math.sin(k * 0.55 + seed) * 0.6 + math.sin(k * 1.7 + seed * 2) * 0.4))
            for k in range(n)]


def panel(t, state):
    out = []
    x0, y = X + P, Y + 26
    out.append(text(x0, y, 'Folded here', 13, t['ink1'], 500))
    out.append(text(X + W - P, y, 'the window is narrow for 3 groups', 11, t['ink3'], 400, 'end'))
    y += 20

    # ---- Metronome: the group's key, its name, its shortcut, its switch — one line
    out.append(well(t, x0, y, 28, 24, 4, fill=mix(t['well'], t['accent'], 0.20)))
    out.append(G.nat(G.metronome, x0 + 14, y + 12, 24, t['accent'], mix(t['accent'], t['ink1'], 0.35)))
    out.append(text(x0 + 38, y + 11, 'Metronome', 12, t['ink1'], 500))
    out.append(text(x0 + 38, y + 23, 'clicks while playing and recording', 10, t['ink3']))
    out.append(keycap(t, X + W - P - 26 - 30, y + 5, 'Ctrl M'))
    out.append(toggle(t, X + W - P - 26, y + 5, True))
    y += 36
    rows = [('Count-in', 'seg', (['Off', '1 bar', '2 bars'], [44, 48, 52], 1)),
            ('Level', 'slider', 0.62),
            ('Sound', 'seg', (['Classic', 'Wood', 'Beep'], [52, 46, 46], 1)),
            ('Accent', 'toggle', True)]
    for lab, kind, v in rows:
        out.append(text(x0, y + 15, lab, 11, t['ink2']))
        if kind == 'seg':
            out.append(segmented(t, X + LC, y + 2, v[0], v[1], 20, v[2], 11))
            if lab == 'Count-in':
                out.append(keycap(t, X + W - P - 18, y + 5, 'Alt C'))
        elif kind == 'slider':
            out.append(slider(t, X + LC + 2, y + 11, 150, v))
            out.append(text(X + W - P, y + 15, '−6 dB', 11, t['ink1'], 400, 'end'))
        else:
            out.append(toggle(t, X + LC, y + 5, v))
            out.append(text(X + LC + 34, y + 15, 'The first beat of each bar', 11, t['ink3']))
        y += 26
    y += 8
    out.append(rect(X + P, y, W - 2 * P, 1, mix(t['raised'], t['ink4'], 0.45)))
    y += 14

    # ---- Windows: seven keys, each shortcut under its own key
    out.append(text(x0, y + 10, 'Windows', 11, t['ink3'], 600, track=0.2))
    y += 18
    keys = [(G.w_browser, 'Alt F8', False), (G.w_arrange, 'F5', True), (G.w_rack, 'F6', False),
            (G.w_piano, 'F7', False), (G.w_plugin, 'F8', False), (G.w_mixer, 'F9', True),
            (G.w_inspector, 'I', True)]
    pitch = (W - 2 * P + 6) / 7
    kw = pitch - 6
    hover = 4
    for k, (g, sc, on) in enumerate(keys):
        kx0 = round(x0 + k * pitch)
        if on:
            out.append(well(t, kx0, y, round(kw), 30, 4, fill=mix(t['well'], t['accent'], 0.16)))
        else:
            out.append(well(t, kx0, y, round(kw), 30, 4, hi=(k == hover)))
        ink = t['accent'] if on else (t['ink1'] if k == hover else t['ink2'])
        ink2 = mix(t['accent'], t['ink1'], 0.4) if on else t['ink3']
        out.append(G.nat(g, kx0 + kw / 2, y + 15, 28, ink, ink2))
        out.append(keycap(t, kx0 + kw / 2, y + 35, sc))
    y += 54
    # the panel's own help line: what the pointer is over
    out.append(text(x0, y + 10, 'Plug-in window', 11, t['ink1'], 500))
    out.append(text(x0 + tw('Plug-in window', 11, 500) + 6, y + 10, '— the selected track’s instrument', 11, t['ink3']))
    y += 22
    out.append(rect(X + P, y, W - 2 * P, 1, mix(t['raised'], t['ink4'], 0.45)))
    y += 14

    # ---- Performance: small graphs with their numbers
    out.append(text(x0, y + 10, 'Performance', 11, t['ink3'], 600, track=0.2))
    out.append(text(X + W - P, y + 10, 'last 30 s', 10, t['ink3'], 400, 'end'))
    y += 18
    cw = (W - 2 * P - 12) / 2
    cells = [('CPU', '7', '%', series(1.3, 40, 0.07, 0.03), 'line', None, 0.25),
             ('Memory', '412', 'MB', series(0.2, 40, 0.80, 0.015), 'line', None, 1.6),
             ('Voices', '12', 'of 256', series(2.1, 24, 10, 5), 'bars', None, 24),
             ('Dropouts', '0', 'since start', [0] * 24, 'bars', t['play'], 1)]
    for k, (lab, v, unit, s, kind, col, top) in enumerate(cells):
        cx = x0 + (k % 2) * (cw + 12)
        cy = y + (k // 2) * 52
        out.append(text(cx, cy + 12, lab, 11, t['ink2']))
        vx = cx + cw
        out.append(text(vx, cy + 13, unit, 10, t['ink3'], 400, 'end'))
        out.append(text(vx - tw(unit, 10) - 4, cy + 13, v, 15, t['ink1'], 400, 'end'))
        if lab == 'Dropouts':
            out.append(well(t, cx, cy + 19, cw, 24, 3))
            out.append(f'<circle cx="{cx + 11}" cy="{cy + 31}" r="3" fill="{t["play"]}"/>')
            out.append(text(cx + 19, cy + 35, 'None — keeping time', 10, t['ink2']))
        else:
            out.append(graph(t, cx, cy + 19, cw, 24, s, kind, col, top))
    y += 2 * 52 + 6
    out.append(rect(X, y, W, 1, mix(t['raised'], t['ink4'], 0.45)))
    # ---- Customise: a line, not a big grey button
    out.append(G.nat(G.sliders, x0 + 7, y + 20, 20, t['ink2'], t['ink3']))
    out.append(text(x0 + 22, y + 24, 'Customise top bar…', 12, t['ink1']))
    out.append(text(X + W - P, y + 24, 'choose, order, fold', 11, t['ink3'], 400, 'end'))
    H = y + 40 - Y
    bg = [f'<filter id="ofs" x="-20%" y="-20%" width="140%" height="140%"><feDropShadow dx="0" dy="8" stdDeviation="12" flood-color="#000" flood-opacity="{0.5 if t["name"] == "Night" else 0.18}"/></filter>',
          f'<rect x="{X}" y="{Y}" width="{W}" height="{H}" rx="8" fill="{t["raised"]}" filter="url(#ofs)"/>',
          f'<rect x="{X + 0.5}" y="{Y + 0.5}" width="{W - 1}" height="{H - 1}" rx="7.5" fill="none" stroke="{mix(t["raised"], t["ink1"], 0.08) if t["name"] == "Night" else t["line"]}"/>']
    kx = 1280 - 10 - 15
    bg.append(f'<path d="M{kx - 7} {Y + 1}L{kx} {Y - 6}L{kx + 7} {Y + 1}Z" fill="{t["raised"]}"/>')
    return ''.join(bg + out)
