"""des-views (round 39, Slate) — the browser: head, search, kinds, the tree (packs with their
covers, folders, sounds with a thumbnail of their own waveform), the preview at its foot.
Also its other pages: Packs (covers, installed / to download / downloading) and a search."""
import math
import glyphs as G
import covers as C
from base import rect, text, well, mix, tw, lit_edge
from topbar import segmented

ROW = 22
IND = 14


# ------------------------------------------------------------------ small marks, drawn here
def folder(t, x, y, open_=False, ink=None):
    """A folder at 12 x 10: a tab and a body; open, the body's front leans back."""
    i = ink or t['ink2']
    if not open_:
        return (f'<path d="M{x} {y + 1.5}Q{x} {y} {x + 1.5} {y}H{x + 4.2}L{x + 5.6} {y + 1.6}H{x + 10.5}Q{x + 12} {y + 1.6} {x + 12} {y + 3.1}'
                f'V{y + 8.5}Q{x + 12} {y + 10} {x + 10.5} {y + 10}H{x + 1.5}Q{x} {y + 10} {x} {y + 8.5}Z" fill="{mix(i, t["base"], 0.55)}"/>'
                f'<path d="M{x} {y + 3.6}H{x + 12}" stroke="{i}" stroke-opacity="0.6" stroke-width="1"/>')
    return (f'<path d="M{x} {y + 1.5}Q{x} {y} {x + 1.5} {y}H{x + 4.2}L{x + 5.6} {y + 1.6}H{x + 10}V{y + 3.4}" fill="none" stroke="{mix(i, t["base"], 0.3)}" stroke-width="1"/>'
            f'<path d="M{x} {y + 1.6}V{y + 10}H{x + 10.4}L{x + 12.6} {y + 4.6}H{x + 2.4}L{x} {y + 10}" fill="{mix(i, t["base"], 0.55)}"/>')


def wave_thumb(t, x, y, w, h, seed, ink, kind='kick'):
    """A sound's own waveform at row size, one column every other pixel: what it is, before
    it is heard (a kick's attack and tail, a snare's body, a pad's breath)."""
    out = []
    mid = y + h / 2
    n = int(w) // 2
    for i in range(n):
        f = i / n
        if kind == 'kick':
            env = math.exp(-f * (2.6 + 0.4 * (seed % 3)))
        elif kind == 'snare':
            env = 0.25 + 0.75 * math.exp(-f * 2.2)
        else:
            env = 0.35 + 0.45 * math.sin(math.pi * min(1, f * 1.2))
        a = env * (0.62 + 0.38 * abs(math.sin(i * 1.7 + seed)))
        hh = max(2, round(a * h))
        hh += hh % 2           # even heights stay centred on the pixel grid
        out.append(rect(x + 2 * i, mid - hh / 2, 1, hh, ink))
    return ''.join(out)


def search_field(t, x, y, w, query=None, h=24, focus=False):
    out = well(t, x, y, w, h, 4)
    if focus:
        out += rect(x + 0.5, y + 0.5, w - 1, h - 1, 'none', 4, f' stroke="{t["accent"]}" stroke-width="1"')
    out += G.zoom(x + 13, y + h / 2, 16, t['ink2'], t['ink3'])
    if query:
        out += text(x + 26, y + h / 2 + 4, query, 12, t['ink1'])
        out += rect(x + 26 + tw(query, 12) + 1, y + 6, 1, h - 12, t['accent'])
        cx, cy = x + w - 12, y + h / 2
        out += (f'<path d="M{cx - 3} {cy - 3}L{cx + 3} {cy + 3}M{cx + 3} {cy - 3}L{cx - 3} {cy + 3}" '
                f'stroke="{t["ink3"]}" stroke-width="1.2" stroke-linecap="round"/>')
    else:
        out += text(x + 26, y + h / 2 + 4, 'Search sounds, presets, plug-ins', 12, t['ink3'])
        out += text(x + w - 8, y + h / 2 + 3.5, 'Ctrl F', 9, t['ink3'], 600, 'end')
    return out


def section(t, x, y, w, title, right=None, right_col=None):
    out = text(x, y + 15, title, 11, t['ink3'], 600)
    if right:
        out += text(x + w, y + 15, right, 11, right_col or t['ink3'], 400, 'end')
    return out


def row(t, x, y, w, level, label, kind, right='', open_=False, sel=False, hover=False, playing=False,
        pack=None, seed=0, wkind='kick', guides=()):
    out = []
    if sel:
        out.append(rect(x, y, w, ROW, mix(t['raised'], t['accent'], 0.10 if t['name'] == 'Night' else 0.08)))
        out.append(rect(x, y, 2, ROW, t['accent']))
    elif hover:
        out.append(rect(x, y, w, ROW, t['well_hi'] if t['name'] == 'Night' else t['raised']))
    for gl in guides:
        out.append(rect(x + 12 + gl * IND + 5, y, 1, ROW, mix(t['base'], t['ink4'], 0.55)))
    ix = x + 12 + level * IND
    cy = y + ROW / 2
    if kind in ('folder', 'pack'):
        out.append((G.chevron_down if open_ else G.chevron_right)(ix + 5, cy, 2.5, t['ink3'], 1.2))
    ix += 14
    if kind == 'folder':
        out.append(folder(t, ix, cy - 5, open_))
    elif kind == 'pack':
        out.append(C.cover(ix - 1, cy - 7, 14, pack))
    elif kind == 'sound':
        out.append(wave_thumb(t, ix - 1, round(cy) - 5, 15, 10, seed, t['accent'] if sel else t['ink3'], wkind))
    nx = ix + 20
    out.append(text(nx, y + 15.5, label, 12, t['ink1'] if (sel or kind != 'sound') else t['ink2'] if not hover else t['ink1'],
                    500 if (sel or kind == 'pack') else 400))
    if playing:
        out.append(G.speaker(x + w - 16, cy, 20, t['accent'], mix(t['accent'], t['base'], 0.4)))
    elif right:
        out.append(text(x + w - 10, y + 15, right, 10, t['ink3'], 400, 'end'))
    return ''.join(out)


TREE = [
    # (level, label, kind, right, open, flags, extra)
    ('title', 'Packs', 'Get more'),
    (0, 'AFR Kicks 2024', 'pack', '64', False, '', dict(pack='AFR Kicks 2024')),
    (0, 'Cantus Starter Drums', 'pack', '142', True, '', dict(pack='Cantus Starter Drums')),
    (1, 'Claps and Snaps', 'folder', '18', False, '', {}),
    (1, 'Cymbals', 'folder', '12', False, '', {}),
    (1, 'Hats', 'folder', '22', False, '', {}),
    (1, 'Kicks', 'folder', '5', True, '', {}),
    (2, 'Acoustic Kick hard 1', 'sound', '0.42 s', False, 'sel play', dict(seed=0)),
    (2, 'Acoustic Kick hard 2', 'sound', '0.40 s', False, '', dict(seed=1)),
    (2, 'Acoustic Kick medium', 'sound', '0.38 s', False, 'hover', dict(seed=2)),
    (2, 'Acoustic Kick soft', 'sound', '0.35 s', False, '', dict(seed=3)),
    (2, 'Acoustic Kick with snares', 'sound', '0.61 s', False, '', dict(seed=4, wkind='snare')),
    (1, 'Percussion', 'folder', '31', False, '', {}),
    (1, 'Snares', 'folder', '10', False, '', {}),
    (1, 'Toms', 'folder', '8', False, '', {}),
    (0, 'Choir Voices', 'pack', '40', False, '', dict(pack='Choir Voices')),
    ('title', 'My folders', None),
    (0, 'My Sounds', 'folder', '', True, '', {}),
    (1, 'Chops', 'folder', '14', False, '', {}),
    (1, 'Guitar stab', 'sound', '1.20 s', False, '', dict(seed=5, wkind='pad')),
]


def tree(t, x, y, w, y_max):
    out = [f'<clipPath id="trc"><rect x="{x}" y="{y}" width="{w}" height="{y_max - y}"/></clipPath><g clip-path="url(#trc)">']
    yy = y
    open_levels = []
    for it in TREE:
        if it[0] == 'title':
            yy += 4 if yy > y else 0
            out.append(section(t, x + 12, yy, w - 24, it[1], it[2], t['accent'] if it[2] else None))
            yy += ROW
            continue
        lv, lab, kind, right, op, fl, ex = it
        guides = tuple(range(lv))
        out.append(row(t, x, yy, w, lv, lab, kind, right, op, 'sel' in fl, 'hover' in fl, 'play' in fl, guides=guides, **ex))
        yy += ROW
    out.append('</g>')
    # more below: the list fades into the preview's edge, and scrolls
    for k in range(8):
        out.append(rect(x, y_max - 8 + k, w, 1, t['base'], 0, f' fill-opacity="{(k + 1) / 9:.2f}"'))
    # the list scrolls: a thin thumb at the right
    out.append(rect(x + w - 5, y + 30, 3, 150, mix(t['base'], t['ink3'], 0.5), 1.5))
    return ''.join(out)


def preview(t, x, y, w, h, name='Acoustic Kick hard 1', place='Cantus Starter Drums › Kicks',
            info='0.42 s · 48 kHz · 24-bit · mono', heard_f=0.34, decay=5.5):
    """The sound under the cursor, heard and seen: name and place, its waveform with the part
    already heard lit, the keys to hear it, and what it is in numbers."""
    out = [rect(x, y, w, h, t['surface']), lit_edge(t, x, y, w)]
    p = 12
    out.append(text(x + p, y + 20, name, 13, t['ink1'], 500))
    # favourite: a star drawn here — an outline at rest
    sx, sy, R = x + w - p - 7, y + 15, 6
    pts = ' '.join(f'{sx + (R if k % 2 == 0 else R * 0.45) * math.sin(math.pi * k / 5):.2f},'
                   f'{sy - (R if k % 2 == 0 else R * 0.45) * math.cos(math.pi * k / 5):.2f}' for k in range(10))
    out.append(f'<polygon points="{pts}" fill="none" stroke="{t["ink3"]}" stroke-width="1.1" stroke-linejoin="round"/>')
    out.append(text(x + p, y + 35, place, 11, t['ink3']))
    # the waveform in a well, heard part lit
    wy, wh = y + 44, 38
    out.append(well(t, x + p, wy, w - 2 * p, wh, 4))
    n = int(w - 2 * p - 8)
    heard = int(n * heard_f)
    mid = wy + wh / 2
    for i in range(n):
        f = i / n
        a = math.exp(-f * decay) * (0.92 if f > 0.012 else 0.4 + f * 40) * (0.82 + 0.18 * math.sin(i * 1.9) * math.sin(i * 0.31))
        a = max(a, 0.02)
        hh = a * (wh - 8)
        col = t['accent'] if i < heard else mix(t['well'], t['ink2'], 0.8)
        out.append(rect(x + p + 4 + i, round(mid - hh / 2, 1), 1, max(1, round(hh, 1)), col))
    out.append(rect(x + p + 4 + heard, wy + 3, 1, wh - 6, t['ink1']))
    # keys: play (sounding), loop, sync; the level; the numbers
    ky = y + h - 30
    out.append(well(t, x + p, ky, 26, 20, 4, fill=mix(t['well'], t['play'], 0.20)))
    out.append(G.play(x + p + 13, ky + 10, 20, t['play'], mix(t['play'], t['well'], 0.35)))
    out.append(well(t, x + p + 30, ky, 26, 20, 4))
    out.append(G.loop(x + p + 30 + 13, ky + 10, 20, t['ink2'], t['ink3']))
    # the level: a slim slider, no box
    lx = x + p + 66
    out.append(rect(lx, ky + 9, 56, 2, mix(t['surface'], t['ink4'], 0.7), 1))
    out.append(rect(lx, ky + 9, 40, 2, t['ink2'], 1))
    out.append(f'<circle cx="{lx + 40}" cy="{ky + 10}" r="4" fill="{t["raised"]}" stroke="{t["edge"]}" stroke-width="1"/>')
    out.append(text(x + w - p, ky + 14, info, 10, t['ink3'], 400, 'end'))
    return ''.join(out)


def panel(t, x, y, w, h):
    out = [rect(x, y, w, h, t['base'])]
    # head
    out.append(rect(x, y, w, 28, t['surface']))
    out.append(lit_edge(t, x, y, w))
    out.append(text(x + 12, y + 18, 'Browser', 13, t['ink1'], 500))
    for k, g in enumerate((G.more, None)):
        pass
    out.append(G.more(x + w - 18, y + 14, 20, t['ink2'], t['ink3']))
    out.append(G.plus(x + w - 42, y + 14, 4, t['ink2'], 1.2))
    out.append(rect(x, y + 28, w, 66, t['surface']))
    out.append(search_field(t, x + 12, y + 32, w - 24))
    out.append(segmented(t, x + 12, y + 62, ['All', 'Sounds', 'Plug-ins', 'Presets'], [50, 66, 66, 82], 24, 1))
    ph = 118
    out.append(tree(t, x, y + 100, w, y + h - ph - 2))
    out.append(rect(x, y + h - ph - 2, w, 2, t['gutter']))
    out.append(preview(t, x, y + h - ph, w, ph))
    return ''.join(out)
