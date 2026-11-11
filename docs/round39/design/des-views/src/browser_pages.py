"""des-views — the browser's other pages: a search, the plug-ins and presets, the Packs page."""
import math
import glyphs as G
import covers as C
import browser as B
from base import rect, text, well, mix, tw, lit_edge
from topbar import segmented

W, H = 1280, 720
PH = 652


def frame(t, x, y, w, title, cap):
    out = [text(x, y - 10, cap, 11, t['ink3']), rect(x, y, w, PH, t['base']),
           rect(x, y, w, 28, t['surface']), lit_edge(t, x, y, w),
           text(x + 12, y + 18, title, 13, t['ink1'], 500),
           G.more(x + w - 18, y + 14, 20, t['ink2'], t['ink3'])]
    return ''.join(out)


def hl_text(t, x, y, s, q, size=12, sel=False):
    """A name with the matched part in the accent, SemiBold — one text run, so nothing shifts."""
    from base import esc
    i = s.lower().find(q.lower())
    col = t['ink1'] if sel else t['ink2']
    if i < 0:
        return text(x, y, s, size, col, 500 if sel else 400)
    pre, m, post = esc(s[:i]), esc(s[i:i + len(q)]), esc(s[i + len(q):])
    acc = t['accent']
    return (f'<text x="{x}" y="{y}" font-size="{size}" font-weight="{500 if sel else 400}" fill="{col}">{pre}'
            f'<tspan font-weight="600" fill="{acc}">{m}</tspan>{post}</text>')


def chip(t, x, y, label, close=True):
    w = tw(label, 11) + (26 if close else 16)
    out = rect(x, y, w, 20, mix(t['base'], t['accent'], 0.10), 10)
    out += rect(x + 0.5, y + 0.5, w - 1, 19, 'none', 10, f' stroke="{mix(t["base"], t["accent"], 0.35)}" stroke-width="1"')
    out += text(x + 9, y + 14, label, 11, t['ink1'])
    if close:
        cx, cy = x + w - 10, y + 10
        out += (f'<path d="M{cx - 2.5} {cy - 2.5}L{cx + 2.5} {cy + 2.5}M{cx + 2.5} {cy - 2.5}L{cx - 2.5} {cy + 2.5}" '
                f'stroke="{t["ink2"]}" stroke-width="1.1" stroke-linecap="round"/>')
    return out, w


def search_page(t, x, y, w):
    out = [frame(t, x, y, w, 'Browser', 'Search — the match marked in each name, grouped by kind')]
    out.append(rect(x, y + 28, w, 94, t['surface']))
    out.append(B.search_field(t, x + 12, y + 32, w - 24, query='snare', focus=True))
    out.append(segmented(t, x + 12, y + 62, ['All 30', 'Sounds 24', 'Presets 6', 'Plug-ins'], [56, 72, 70, 66], 24, 0))
    cx = x + 12
    s, cw = chip(t, cx, y + 94, 'Cantus Starter Drums')
    out.append(s)
    s, cw2 = chip(t, cx + cw + 6, y + 94, 'under 1 s')
    out.append(s)
    out.append(text(cx + cw + cw2 + 18, y + 108, '+ Filter', 11, t['accent']))
    yy = y + 130
    rows = [('Sounds', '24', [('Snare hard 1', 'Starter Drums › Snares', '0.36 s', 'sel'), ('Snare hard 2', 'Starter Drums › Snares', '0.34 s', ''),
                              ('Snare buzz', 'Starter Drums › Snares', '0.52 s', ''), ('Snare rimshot medium', 'Starter Drums › Snares', '0.31 s', 'hover'),
                              ('Clap and snare', 'Starter Drums › Claps and Snaps', '0.44 s', '')]),
            ('Presets', '6', [('Snare tight', 'Pulsus › Kits', '', 'preset'), ('Snare room', 'Pulsus › Kits', '', 'preset')])]
    for title, n, items in rows:
        out.append(B.section(t, x + 12, yy, w - 24, title, f'{n}  ›'))
        yy += 24
        for name, place, dur, fl in items:
            rh = 36
            if fl == 'sel':
                out.append(rect(x, yy, w, rh, mix(t['raised'], t['accent'], 0.10 if t['name'] == 'Night' else 0.08)))
                out.append(rect(x, yy, 2, rh, t['accent']))
            elif fl == 'hover':
                out.append(rect(x, yy, w, rh, t['well_hi'] if t['name'] == 'Night' else t['raised']))
            if fl == 'preset':
                # a preset: a small dial in a rounded square — a setting, not a sound
                out.append(rect(x + 12, yy + 11, 14, 14, 'none', 3, f' stroke="{t["ink3"]}" stroke-width="1"'))
                out.append(f'<circle cx="{x + 19}" cy="{yy + 18}" r="3.2" fill="none" stroke="{t["ink2"]}" stroke-width="1.1"/>')
                out.append(f'<path d="M{x + 19} {yy + 18}L{x + 21} {yy + 16}" stroke="{t["ink2"]}" stroke-width="1.1"/>')
            else:
                out.append(B.wave_thumb(t, x + 12, yy + 13, 15, 10, len(name), t['accent'] if fl == 'sel' else t['ink3'], 'snare'))
            out.append(hl_text(t, x + 36, yy + 16, name, 'snare', 12, fl == 'sel'))
            out.append(text(x + 36, yy + 30, place, 10, t['ink3']))
            if fl == 'sel':
                out.append(G.speaker(x + w - 16, yy + 18, 20, t['accent'], mix(t['accent'], t['base'], 0.4)))
            elif dur:
                out.append(text(x + w - 10, yy + 22, dur, 10, t['ink3'], 400, 'end'))
            yy += rh
        yy += 6
    out.append(text(x + 12, yy + 14, 'Show the other 19 sounds', 11, t['accent']))
    ph = 118
    out.append(rect(x, y + PH - ph - 2, w, 2, t['gutter']))
    out.append(B.preview(t, x, y + PH - ph, w, ph, 'Snare hard 1', 'Cantus Starter Drums › Snares', '0.36 s · 48 kHz · 24-bit · mono', 0.52, 3.2))
    return ''.join(out)


def unit_mark(t, x, y, kind, own=True):
    """A unit's mark at 16 px: an instrument shows two keys, an effect a curve. Cantus's own
    units sit in a filled square; hosted plug-ins in an outline. The format is said in words."""
    i1 = t['ink1'] if own else t['ink3']
    i2 = t['ink2'] if own else t['ink3']
    if own:
        out = rect(x, y, 16, 16, mix(t['base'], t['ink2'], 0.28), 4)
    else:
        out = rect(x + 0.5, y + 0.5, 15, 15, 'none', 4, f' stroke="{t["ink3"]}" stroke-width="1"')
    if kind == 'inst':      # a keyboard's front: three white keys, two black between
        out += rect(x + 3.5, y + 4.5, 9, 7, 'none', 1, f' stroke="{i2}" stroke-width="1"')
        out += rect(x + 6, y + 4, 1.5, 4.5, i1) + rect(x + 9, y + 4, 1.5, 4.5, i1)
    else:                   # a filter's response: flat, then rolling off
        out += (f'<path d="M{x + 3} {y + 7}H{x + 8}C{x + 10.5} {y + 7} {x + 11} {y + 12} {x + 13} {y + 12}" '
                f'fill="none" stroke="{i1}" stroke-width="1.2" stroke-linecap="round"/>')
    return out


def star(t, cx, cy, on, r=5):
    pts = ' '.join(f'{cx + (r if k % 2 == 0 else r * 0.45) * math.sin(math.pi * k / 5):.2f},'
                   f'{cy - (r if k % 2 == 0 else r * 0.45) * math.cos(math.pi * k / 5):.2f}' for k in range(10))
    if on:
        return f'<polygon points="{pts}" fill="{t["solo"]}"/>'
    return f'<polygon points="{pts}" fill="none" stroke="{t["ink4"]}" stroke-width="1" stroke-linejoin="round"/>'


def plugins_page(t, x, y, w):
    out = [frame(t, x, y, w, 'Browser', 'Plug-ins and presets — favourites first; the format in words')]
    out.append(rect(x, y + 28, w, 66, t['surface']))
    out.append(B.search_field(t, x + 12, y + 32, w - 24))
    out.append(segmented(t, x + 12, y + 62, ['All', 'Sounds', 'Plug-ins', 'Presets'], [50, 66, 66, 82], 24, 2))
    yy = y + 100
    groups = [('Favourites', None, [('Chorda', 'inst', True, 'Cantus', True, ''), ('Comprimo', 'fx', True, 'Cantus', True, ''),
                                    ('Spatium', 'fx', True, 'Cantus', True, '')]),
              ('Instruments', '12', [('Chorda', 'inst', True, 'Cantus', True, 'open'), ('Exemplar', 'inst', True, 'Cantus', False, ''),
                                     ('Pulsus', 'inst', True, 'Cantus', False, 'sel'), ('Organum', 'inst', True, 'Cantus', False, ''),
                                     ('Surge XT', 'inst', False, 'CLAP', False, ''), ('Dexed', 'inst', False, 'VST3', False, '')]),
              ('Effects', '31', [('Aequalis', 'fx', True, 'Cantus', False, ''), ('Aula', 'fx', True, 'Cantus · new', False, ''),
                                 ('Comprimo', 'fx', True, 'Cantus', True, ''), ('Spatium', 'fx', True, 'Cantus', True, ''),
                                 ('Dragonfly Hall Reverb', 'fx', False, 'CLAP', False, '')])]
    for title, n, items in groups:
        out.append(B.section(t, x + 12, yy, w - 24, title, n))
        yy += 24
        for name, kind, own, fmt, fav, fl in items:
            rh = 26
            if fl == 'sel':
                out.append(rect(x, yy, w, rh, mix(t['raised'], t['accent'], 0.10 if t['name'] == 'Night' else 0.08)))
                out.append(rect(x, yy, 2, rh, t['accent']))
            out.append(unit_mark(t, x + 14, yy + 5, kind, own))
            out.append(text(x + 40, yy + 17, name, 12, t['ink1'], 500 if fl == 'sel' else 400))
            out.append(text(x + w - 30, yy + 17, fmt, 10, t['accent'] if 'new' in fmt else t['ink3'], 400, 'end'))
            out.append(star(t, x + w - 16, yy + 12.5, fav))
            yy += rh
        yy += 6
    return ''.join(out)


def sheet(t):
    import browser_packs as BP
    out = [rect(0, 0, W, H, t['gutter'])]
    y = 46
    out.append(search_page(t, 12, y, 288))
    out.append(plugins_page(t, 312, y, 288))
    out.append(BP.packs_page(t, 612, y, 656))
    out.append(text(12, 20, f'The browser’s other pages — Slate {t["name"]}', 15, t['ink1'], 500))
    return ''.join(out)
