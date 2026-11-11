"""Slate (dir-a), phase B — Settings (Audio and Appearance pages), the first-run audio
setup and the export dialog, each whole, over the window on a scrim."""
import glyphs as G
import logo as L
from base import rect, text, well, mix, tw, HUES
from dlg_kit import (scrim, frame, section, label, dropdown, field, button, lamp, meter_pair, check,
                     sidebar, segmented, toggle, keycap, ROW, LABEL)

PAGES = [(G.sliders, 'General'), (G.speaker, 'Audio'), (G.g_midi, 'MIDI'), (G.g_look, 'Appearance'),
         (G.g_keys, 'Shortcuts'), (G.w_plugin, 'Plug-ins'), (G.w_browser, 'Library and files'),
         (G.g_about, 'About')]


def settings(t, page='Audio'):
    X, Y, W, H = 196, 64, 888, 592
    out = [scrim(t), frame(t, X, Y, W, H, 'Settings', uid='set' + t['name'])]
    SB = 200
    out.append(sidebar(t, X + 1, Y + 44, SB, H - 45, PAGES, [p for _, p in PAGES].index(page)))
    out.append(rect(X + 1, Y + H - 10, SB, 9, t['base']))
    cx = X + SB + 28
    cw = W - SB - 56
    if page == 'Audio':
        out.append(audio_page(t, cx, Y + 44, cw))
    else:
        out.append(look_page(t, cx, Y + 44, cw))
    # the footer: what is running, and Done
    fy = Y + H - 52
    out.append(rect(X + SB + 1, fy, W - SB - 2, 1, mix(t['surface'], t['ink4'], 0.45)))
    out.append(lamp(t, cx + 4, fy + 26, t['play']))
    out.append(text(cx + 14, fy + 30, 'Now: Focusrite Scarlett 2i2 · ASIO · 48 kHz · 128 samples', 11, t['ink2']))
    b, w = button(t, X + W - 28 - 80, fy + 12, 'Done', 'primary', 80)
    out.append(b)
    out.append(button(t, X + W - 28 - 80 - 12 - 132, fy + 12, 'Restore defaults', 'quiet', 132)[0])
    return ''.join(out)


def audio_page(t, x, y, w):
    out = []
    y += 26
    out.append(section(t, x, y, 'Device', lamp(t, x + w - 52, y - 4, t['play']) + text(x + w, y, 'Running', 11, t['ink2'], 400, 'end')))
    y += 12
    out.append(label(t, x, y, 'Interface'))
    out.append(dropdown(t, x + LABEL, y + 2, 300, 'Focusrite Scarlett 2i2', 'Audio interface',
                        lead=G.nat(G.interface, x + LABEL + 14, y + 14, 20, t['ink1'], t['ink3'])))
    b, bw = button(t, x + LABEL + 312, y + 1, 'Test tone', 'secondary', 96, 26,
                   lead=lambda a, c: G.nat(G.speaker, a, c, 16, t['ink2'], t['ink3']))
    out.append(b)
    y += ROW
    out.append(label(t, x, y, 'Driver'))
    out.append(segmented(t, x + LABEL, y + 3, ['ASIO', 'WASAPI'], [56, 70], 22, 0, 11))
    out.append(text(x + LABEL + 140, y + 17, 'the interface’s own driver: the shortest path', 11, t['ink3']))
    y += ROW
    out.append(label(t, x, y, 'Sample rate'))
    out.append(segmented(t, x + LABEL, y + 3, ['44.1', '48', '88.2', '96'], [44, 38, 44, 38], 22, 1, 11))
    out.append(text(x + LABEL + 172, y + 17, 'kHz', 11, t['ink3']))
    y += ROW
    out.append(label(t, x, y, 'Buffer'))
    out.append(segmented(t, x + LABEL, y + 3, ['64', '128', '256', '512', '1024'], [36, 40, 40, 40, 46], 22, 1, 11))
    out.append(text(x + LABEL + 210, y + 17, 'samples', 11, t['ink3']))
    y += ROW - 4
    # the latency as a drawing: a scale from tight to loose, where this buffer lands
    sx, sw = x + LABEL, 300
    for k, (a, b2, col) in enumerate(((0, 0.33, t['play']), (0.33, 0.66, t['solo']), (0.66, 1.0, t['ink3']))):
        out.append(rect(round(sx + sw * a), y + 8, round(sw * (b2 - a)) - 2, 3, mix(t['surface'], col, 0.55), 1.5))
    px = sx + sw * 0.17
    out.append(f'<circle cx="{px:.1f}" cy="{y + 9.5}" r="4.5" fill="{t["surface"]}" stroke="{t["play"]}" stroke-width="2"/>')
    out.append(text(sx + sw + 12, y + 13, '5.3 ms round trip', 12, t['ink1'], 500))
    out.append(text(sx, y + 30, 'Tight enough to play through, like an amp. Measured: 5.6 ms; takes move by 12 samples.', 11, t['ink3']))
    y += 42
    out.append(label(t, x, y, 'Inputs and outputs'))
    out.append(text(x + LABEL, y + 17, '2 in · 2 out', 12, t['ink1']))
    out.append(text(x + LABEL + 84, y + 12, 'In 1', 9, t['ink3'], 600))
    out.append(meter_pair(t, x + LABEL + 108, y + 7, 90, (0.55, 0.0)))
    b, bw = button(t, x + w - 168, y + 1, 'Channels and routing…', 'secondary', 168, 26)
    out.append(b)
    y += ROW
    out.append(label(t, x, y, 'Latency'))
    out.append(text(x + LABEL, y + 17, 'Connect output 1 to input 1 with a cable, then measure.', 11, t['ink3']))
    out.append(button(t, x + w - 128, y + 1, 'Measure again', 'secondary', 128, 26)[0])
    y += ROW + 22
    out.append(section(t, x, y, 'Processing'))
    y += 12
    out.append(label(t, x, y, 'Audio cores'))
    out.append(dropdown(t, x + LABEL, y + 2, 180, 'Automatic (6)'))
    out.append(text(x + LABEL + 192, y + 17, '6 cores at work: the audio thread and 5 more', 11, t['ink3']))
    y += ROW + 22
    out.append(section(t, x, y, 'When the device goes away'))
    y += 12
    out.append(toggle(t, x, y + 9, True))
    out.append(text(x + 36, y + 20, 'Open it again by itself when it comes back', 12, t['ink1']))
    out.append(text(x + 36, y + 35, 'your song stays open and editable meanwhile', 10, t['ink3']))
    return ''.join(out)


THEMES = [('Slate', 'the default', ('#14191e', '#1d2329', '#262d34', '#5cafe9'), ('#e4e9ee', '#f4f6f8', '#d6dde3', '#1b64b2')),
          ('Graphite and gold', '', ('#111111', '#181818', '#1e1e1d', '#e6b450'), ('#ecead6', '#f5f4f1', '#fcfbf9', '#7c5307')),
          ('Studio ink', '', ('#08090b', '#0e1013', '#15181c', '#62b4ff'), ('#eeeeec', '#f6f6f4', '#ffffff', '#1e64d0'))]


def thumb(t, x, y, w, h, night, paper, chosen):
    """A theme in miniature: its Night on the left, its Paper on the right, the user's
    colours as a few clips — enough to recognise it, nothing to read."""
    out = ''
    if chosen:
        out += rect(x - 3, y - 3, w + 6, h + 6, t['accent'], 8)
        out += rect(x - 1, y - 1, w + 2, h + 2, t['surface'], 6)
    hw = w // 2
    for k, pal in enumerate((night, paper)):
        ox = x + k * hw
        out += rect(ox, y, hw, h, pal[0], 0 if k else 0)
        out += rect(ox, y, hw, 12, pal[2])
        out += rect(ox + 4, y + 4, 10, 4, pal[3], 1)
        out += rect(ox, y + 14, hw, h * 0.52, pal[1])
        for j, hue in enumerate(('Kick', 'Chords', 'Lead', 'Choir')):
            cc = mix(pal[1], HUES[hue], 0.35 if k == 0 else 0.3)
            out += rect(ox + 6 + (j % 2) * 18, y + 18 + j * 7, 30 + (j % 3) * 8, 5, cc, 1)
        for j in range(5):
            out += rect(ox + 6 + j * 16, y + h * 0.52 + 20, 12, h - h * 0.52 - 26, pal[2], 1)
    out += f'<rect x="{x + 0.5}" y="{y + 0.5}" width="{w - 1}" height="{h - 1}" rx="0" fill="none" stroke="{mix(t["surface"], t["ink1"], 0.12)}"/>'
    return out


def look_page(t, x, y, w):
    out = []
    y += 26
    out.append(section(t, x, y, 'Theme'))
    y += 14
    cw = (w - 32) // 3
    for k, (nm, tag, n, p) in enumerate(THEMES):
        tx = x + k * (cw + 16)
        out.append(thumb(t, tx, y, cw, 96, n, p, k == 0))
        out.append(text(tx, y + 118, nm, 12, t['ink1'], 500 if k == 0 else 400))
        if tag:
            out.append(text(tx + tw(nm, 12, 500) + 8, y + 118, tag, 11, t['ink3']))
    y += 138
    out.append(label(t, x, y, 'Mode'))
    out.append(segmented(t, x + LABEL, y + 3, ['Night', 'Paper', 'Follow the system'], [56, 56, 120], 22, 0 if t['name'] == 'Night' else 1, 11))
    y += ROW
    out.append(label(t, x, y, 'Interface size'))
    out.append(segmented(t, x + LABEL, y + 3, ['90 %', '100 %', '125 %', '150 %', '200 %'], [44, 48, 48, 48, 48], 22, 1, 11))
    out.append(keycap(t, x + LABEL + 268, y + 7, 'Ctrl −'))
    out.append(keycap(t, x + LABEL + 312, y + 7, 'Ctrl +'))
    y += ROW
    out.append(label(t, x, y, 'Colours'))
    b, bw = button(t, x + LABEL, y + 1, 'Edit colours…', 'secondary', 120, 26)
    out.append(b)
    out.append(button(t, x + LABEL + 132, y + 1, 'Back to the default theme', 'quiet', 180, 26)[0])
    y += ROW + 22
    out.append(section(t, x, y, 'Taskbar icon'))
    y += 12
    tints = [('As designed', None), ('Amber', ('#ffe7a8', '#e8a42e', '#9a5a0c')),
             ('Verdigris', ('#bff0e0', '#3e9e86', '#1d5a4c')), ('Blue', ('#cfe6ff', '#4b8fd6', '#1d4f8f')),
             ('Silver', ('#ffffff', '#b9c2cc', '#6c7884'))]
    for k, (nm, wave) in enumerate(tints):
        ix = x + k * 104
        if k == 0:
            out.append(rect(ix - 3, y + 1, 38, 38, t['accent'], 8) + rect(ix - 1, y + 3, 34, 34, t['surface'], 7))
        out.append(L.mark(32, ix, y + 4, uid=f'tk{k}{t["name"]}', wave=wave))
        out.append(text(ix + 40, y + 24, nm, 11, t['ink1'] if k == 0 else t['ink2']))
    y += 46
    out.append(text(x, y + 10, 'The window’s and the taskbar’s icon change at once; pinned shortcuts keep the designed colours.', 10.5, t['ink3']))
    return ''.join(out)
