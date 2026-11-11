"""Slate (dir-a), phase B — the first-run audio setup and the export dialog."""
import glyphs as G
from base import rect, text, well, mix, tw
from dlg_kit import (scrim, frame, section, label, dropdown, field, button, lamp, meter_pair, check,
                     segmented, toggle, ROW, LABEL)


def step(t, x, y, n, title, done=False, last=False, h=0):
    """A numbered step: a small disc with its number, a thin line down to the next."""
    out = ''
    if not last:
        out += rect(x + 10, y + 24, 1, h - 26, mix(t['surface'], t['ink4'], 0.6))
    if done:
        out += f'<circle cx="{x + 10.5}" cy="{y + 10.5}" r="10" fill="{mix(t["surface"], t["play"], 0.22)}"/>'
        out += (f'<path d="M{x + 6} {y + 10.5}L{x + 9} {y + 13.5}L{x + 15} {y + 7.5}" fill="none" stroke="{t["play"]}" '
                f'stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/>')
    else:
        out += f'<circle cx="{x + 10.5}" cy="{y + 10.5}" r="10" fill="{t["well"]}" stroke="{t["edge"]}"/>'
        out += text(x + 10.5, y + 15, str(n), 11, t['ink1'], 600, 'middle')
    out += text(x + 32, y + 15, title, 13, t['ink1'], 500)
    return out


def device_row(t, x, y, w, glyph, name, kind, chosen=False, tag=None):
    out = ''
    if chosen:
        out += rect(x, y, w, 36, t['raised'] if t['name'] == 'Night' else '#ffffff', 6)
        out += rect(x, y + 9, 2, 18, t['accent'], 1)
    else:
        out += rect(x, y, w, 36, mix(t['surface'], t['base'], 0.5), 6)
    out += G.nat(glyph, x + 22, y + 18, 20, t['ink1'] if chosen else t['ink2'], t['ink3'])
    out += text(x + 42, y + 22.5, name, 12, t['ink1'], 500 if chosen else 400)
    out += text(x + 42 + tw(name, 12, 500 if chosen else 400) + 8, y + 22.5, kind, 11, t['ink3'])
    if tag:
        tw_ = tw(tag, 10, 500) + 14
        out += (f'<rect x="{x + w - 44 - tw_:.1f}" y="{y + 10.5}" width="{tw_:.1f}" height="15" rx="7.5" fill="none" '
                f'stroke="{mix(t["surface"], t["accent"], 0.7)}"/>')
        out += text(x + w - 44 - tw_ / 2, y + 21.5, tag, 10, t['accent'], 500, 'middle')
    if chosen:
        out += (f'<path d="M{x + w - 26} {y + 18}L{x + w - 22} {y + 22}L{x + w - 15} {y + 14}" fill="none" stroke="{t["accent"]}" '
                f'stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>')
    return out


def firstrun(t):
    X, Y, W, H = 300, 58, 680, 604
    out = [scrim(t), frame(t, X, Y, W, H, 'Set up your audio', uid='fr' + t['name'])]
    x = X + 28
    cw = W - 56
    y = Y + 62
    out.append(text(x, y, 'Choose what you play through. Cantus picks the best driver and buffer for it —', 12, t['ink2']))
    out.append(text(x, y + 17, 'you can change them any time from the audio key in the top bar.', 12, t['ink2']))
    y += 40
    out.append(step(t, x, y, 1, 'Choose your interface', done=True, h=196))
    rows = [(G.interface, 'Focusrite Scarlett 2i2', 'Audio interface · 2 in, 2 out', True, 'Recommended'),
            (G.speaker, 'Speakers', 'This computer · Realtek', False, None),
            (G.headphones, 'Headphones', 'This computer · Realtek', False, None),
            (G.speaker, 'LG UltraFine', 'Screen speakers', False, None)]
    yy = y + 28
    for g, nm, kind, ch, tag in rows:
        out.append(device_row(t, x + 32, yy, cw - 32, g, nm, kind, ch, tag))
        yy += 40
    y += 196
    out.append(step(t, x, y, 2, 'Listen', done=True, h=70))
    b, bw = button(t, x + 32, y + 26, 'Stop tone', 'secondary', 104, 26,
                   lead=lambda a, c: G.nat(G.stop, a, c, 16, t['accent'], t['ink3']))
    out.append(b)
    out.append(meter_pair(t, x + 32 + 116, y + 34, 120, (0.52, 0.5)))
    out.append(text(x + 32 + 248, y + 43, 'Playing a tone on outputs 1–2. Hear it? Then the outputs are right.', 11, t['ink3']))
    y += 70
    out.append(step(t, x, y, 3, 'Play', h=86))
    for k, (nm, lv) in enumerate((('Input 1', 0.66), ('Input 2', 0.04))):
        iy = y + 28 + k * 18
        out.append(text(x + 32, iy + 9, nm, 11, t['ink2'] if k == 0 else t['ink3']))
        g = mix(t['well'], t['ink4'], 0.28)
        out.append(rect(x + 84, iy + 3, 200, 4, g, 2))
        out.append(rect(x + 84, iy + 3, round(200 * lv), 4, t['meter_lo'], 2))
    out.append(lamp(t, x + 302, y + 36, t['play']))
    out.append(text(x + 312, y + 40, 'Input 1 is moving: that is the one to record from.', 11, t['ink1']))
    y += 86
    out.append(step(t, x, y, 4, 'Latency', last=True))
    out.append(text(x + 32, y + 40, 'ASIO at 48 kHz with a buffer of 128 samples: about 5.3 ms from string to speaker —', 11, t['ink2']))
    out.append(text(x + 32, y + 55, 'tight enough to play through, like an amp.', 11, t['ink2']))
    out.append(button(t, x + cw - 136, y + 30, 'Measure latency', 'secondary', 136, 26)[0])
    out.append(text(x + 32, y + 74, 'Optional: connect output 1 to input 1 with a cable, then measure, so takes land exactly in time.', 10, t['ink3']))
    fy = Y + H - 52
    out.append(rect(X + 1, fy, W - 2, 1, mix(t['surface'], t['ink4'], 0.45)))
    out.append(button(t, X + 20, fy + 12, 'Skip', 'quiet', 60)[0])
    out.append(button(t, X + W - 28 - 80, fy + 12, 'Done', 'primary', 80)[0])
    return ''.join(out)


PRESETS = [('Streaming', 'MP3 320 · −14 LUFS · −1 dBTP'), ('CD', 'WAV 16-bit · 44.1 kHz · dither'),
           ('Stems for mastering', 'Every strip · WAV 24-bit'), ('Podcast', 'Mono MP3 · −16 LUFS'),
           ('Custom', 'Your own choices')]


def export(t, exporting=False):
    X, Y, W, H = 190, 40, 900, 640
    out = [scrim(t), frame(t, X, Y, W, H, 'Export', 'Neon Tide · 2:03', uid='ex' + t['name'])]
    SB = 220
    out.append(rect(X + 1, Y + 44, SB, H - 45 - 9, t['base']))
    out.append(rect(X + 1, Y + H - 10, SB, 9, t['base']))
    out.append(text(X + 20, Y + 70, 'Presets', 11, t['ink3'], 600, track=0.2))
    yy = Y + 82
    for k, (nm, hint) in enumerate(PRESETS):
        if k == 0:
            out.append(rect(X + 9, yy, SB - 16, 44, t['raised'] if t['name'] == 'Night' else '#ffffff', 6))
            out.append(rect(X + 9, yy + 13, 2, 18, t['accent'], 1))
        out.append(text(X + 22, yy + 19, nm, 12, t['ink1'], 500 if k == 0 else 400))
        out.append(text(X + 22, yy + 34, hint, 10, t['ink3']))
        yy += 48
    x = X + SB + 28
    cw = W - SB - 56
    y = Y + 70
    out.append(section(t, x, y, 'What'))
    y += 8
    out.append(label(t, x, y, 'Export'))
    out.append(segmented(t, x + LABEL, y + 3, ['Song', 'Selection', 'Loop', 'Stems'], [52, 70, 48, 56], 22, 0, 11))
    out.append(text(x + LABEL + 238, y + 17, 'bars 1–64', 11, t['ink3']))
    y += ROW
    out.append(label(t, x, y, 'Include'))
    out.append(check(t, x + LABEL, y + 7, True, 'The tail'))
    out.append(field(t, x + LABEL + 90, y + 3, 64, '2.0 s', 22))
    out.append(text(x + LABEL + 166, y + 17, 'the reverb’s decay after the last note', 11, t['ink3']))
    y += ROW + 18
    out.append(section(t, x, y, 'Format'))
    y += 8
    out.append(label(t, x, y, 'Format'))
    out.append(segmented(t, x + LABEL, y + 3, ['WAV', 'FLAC', 'MP3', 'OGG'], [48, 50, 48, 48], 22, 2, 11))
    y += ROW
    out.append(label(t, x, y, 'Bitrate'))
    out.append(segmented(t, x + LABEL, y + 3, ['Constant', 'Variable'], [70, 66], 22, 0, 11))
    out.append(dropdown(t, x + LABEL + 148, y + 3, 96, '320 kbps', h=22))
    y += ROW
    out.append(label(t, x, y, 'Sample rate'))
    out.append(dropdown(t, x + LABEL, y + 3, 170, 'Session (48 kHz)', h=22))
    y += ROW
    out.append(label(t, x, y, 'Channels'))
    out.append(segmented(t, x + LABEL, y + 3, ['Stereo', 'Mono'], [60, 52], 22, 0, 11))
    y += ROW
    out.append(text(x, y + 17, 'Dither', 11, t['ink3']))
    out.append(text(x + LABEL, y + 17, 'Not used: MP3 is encoded from 32-bit float.', 11, t['ink3']))
    y += ROW + 18
    out.append(section(t, x, y, 'Loudness'))
    y += 8
    out.append(label(t, x, y, 'Target'))
    out.append(dropdown(t, x + LABEL, y + 3, 200, '−14 LUFS', 'streaming', h=22))
    y += ROW
    out.append(label(t, x, y, 'Ceiling'))
    out.append(field(t, x + LABEL, y + 3, 80, '−1.0 dBTP', 22))
    out.append(check(t, x + LABEL + 96, y + 7, True, 'Hold the peaks with a limiter'))
    y += ROW + 18
    out.append(section(t, x, y, 'Tags'))
    y += 8
    half = (cw - LABEL - 12) // 2
    out.append(label(t, x, y, 'Title and artist'))
    out.append(field(t, x + LABEL, y + 3, half, 'Neon Tide', 22))
    out.append(field(t, x + LABEL + half + 12, y + 3, half, 'Artist', 22, ph=True, focus=False))
    y += ROW
    out.append(label(t, x, y, 'Album and year'))
    out.append(field(t, x + LABEL, y + 3, half, 'Album', 22, ph=True))
    out.append(field(t, x + LABEL + half + 12, y + 3, 64, '2026', 22))
    fy = Y + H - 56
    out.append(rect(X + SB + 1, fy, W - SB - 2, 1, mix(t['surface'], t['ink4'], 0.45)))
    if exporting:
        out.append(text(x, fy + 24, 'Measuring loudness', 12, t['ink1'], 500))
        out.append(text(x + cw - 120, fy + 24, '62 %', 12, t['ink1'], 400, 'end'))
        bw = cw - 120 - 12 - 0
        out.append(rect(x, fy + 33, cw - 120, 4, mix(t['well'], t['ink4'], 0.3), 2))
        out.append(rect(x, fy + 33, round((cw - 120) * 0.62), 4, t['accent'], 2))
        out.append(text(x, fy + 50, 'rendered 2:03 of 2:03 · measured 1:16 · encoding next', 10, t['ink3']))
        out.append(button(t, X + W - 28 - 96, fy + 14, 'Stop', 'secondary', 96)[0])
    else:
        out.append(text(x, fy + 26, 'MP3 320 kbps · 48 kHz stereo · −14 LUFS', 12, t['ink1']))
        out.append(text(x, fy + 42, '2:03 · about 4.9 MB · Documents/Cantus/Neon Tide.mp3', 10, t['ink3']))
        out.append(button(t, X + W - 28 - 96, fy + 14, 'Export…', 'primary', 96)[0])
        out.append(button(t, X + W - 28 - 96 - 10 - 84, fy + 14, 'Cancel', 'secondary', 84)[0])
    return ''.join(out)
