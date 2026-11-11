"""des-views — the browser's Packs page (the browser widened, or its own window): covers at
120 px, what each holds, its licence, and its state — installed, to get, getting, updated."""
import glyphs as G
import covers as C
import browser as B
from base import rect, text, well, mix, tw, lit_edge
from topbar import segmented

PH = 652


def pack_card(t, x, y, name, state, prog=None):
    hue, motif, what, n, size, lic = C.PACKS[name]
    s = 128
    out = [C.cover(x, y, s, name)]
    if state == 'installed':
        # a quiet check in the cover's corner: it is here, ready
        cx, cy = x + s - 13, y + 13
        out.append(f'<circle cx="{cx}" cy="{cy}" r="8" fill="{t["base"]}" fill-opacity="0.85"/>')
        out.append(f'<path d="M{cx - 3.5} {cy}L{cx - 1} {cy + 2.6}L{cx + 3.8} {cy - 2.6}" fill="none" stroke="{t["play"]}" '
                   f'stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>')
    out.append(text(x, y + s + 18, name, 12, t['ink1'], 500))
    out.append(text(x, y + s + 33, f'{what} · {n} · {lic}', 10, t['ink3']))
    by = y + s + 42
    if state == 'installed':
        out.append(text(x, by + 14, 'Installed', 11, t['ink2']))
        out.append(text(x + s, by + 14, 'Open', 11, t['accent'], 500, 'end'))
    elif state == 'get':
        out.append(well(t, x, by, s, 22, 4, fill=mix(t['well'], t['accent'], 0.18)))
        out.append(text(x + s / 2, by + 15, f'Get · {size}', 11, t['accent'] if t['name'] == 'Night' else t['ink1'], 500, 'middle'))
    elif state == 'getting':
        out.append(rect(x, by + 8, s, 4, mix(t['base'], t['ink4'], 0.6), 2))
        out.append(rect(x, by + 8, s * prog, 4, t['accent'], 2))
        out.append(text(x, by + 26, f'{round(prog * 100)} % of {size}', 10, t['ink2']))
        out.append(text(x + s, by + 26, 'Pause', 10, t['ink3'], 400, 'end'))
    elif state == 'update':
        out.append(text(x, by + 14, 'Installed', 11, t['ink2']))
        out.append(text(x + s, by + 14, 'Update · 4 MB', 11, t['accent'], 500, 'end'))
    return ''.join(out)


def packs_page(t, x, y, w):
    out = [text(x, y - 10, 'Packs — the browser widened: covers made from what each pack holds; '
                'free packs come on demand, never in the installer', 11, t['ink3']),
           rect(x, y, w, PH, t['base']), rect(x, y, w, 28, t['surface']), lit_edge(t, x, y, w),
           text(x + 12, y + 18, 'Packs', 13, t['ink1'], 500), text(x + 58, y + 18, '8 free packs · 3 installed · 471 MB on disk', 11, t['ink3']),
           G.more(x + w - 18, y + 14, 20, t['ink2'], t['ink3'])]
    out.append(rect(x, y + 28, w, 36, t['surface']))
    out.append(B.search_field(t, x + 12, y + 34, 220))
    out.append(segmented(t, x + 244, y + 34, ['All', 'Installed', 'Drums', 'Instruments', 'Textures'], [42, 64, 54, 80, 66], 24, 0))
    out.append(text(x + w - 12, y + 50, 'Sort  Newest', 11, t['ink3'], 400, 'end'))
    cards = [('Cantus Starter Drums', 'installed'), ('AFR Kicks 2024', 'installed'), ('Choir Voices', 'update'),
             ('Aula Spaces', 'getting', 0.42), ('Lo-fi Keys', 'get'), ('Analog Bass', 'get'),
             ('Field Recordings', 'get'), ('Street Percussion', 'get')]
    gx, gy = x + 20, y + 80
    pitch = (w - 40 - 128) / 3
    for k, c in enumerate(cards):
        cx = gx + (k % 4) * pitch
        cy = gy + (k // 4) * 232
        out.append(pack_card(t, round(cx), cy, *c))
    # the credit line: CC BY packs name their authors where the pack is shown (ADR 0042)
    out.append(rect(x, y + PH - 36, w, 36, t['surface']))
    out.append(text(x + 12, y + PH - 14, 'CC BY packs show their authors on the pack’s page and in Help ▸ Credits.', 11, t['ink3']))
    out.append(text(x + w - 12, y + PH - 14, 'Where packs are kept  ›', 11, t['accent'], 400, 'end'))
    return ''.join(out)
