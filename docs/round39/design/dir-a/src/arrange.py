"""Slate (dir-a) — the arrangement: its tool row, the ruler with the sections, the track
headers and the clips with their content drawn large enough to read from afar."""
import glyphs as G
import song
from base import (lit_edge, rect, text, well, mix, tw, hline, vline, hue_for, note_ink, HUES, INSP_X,
                  ARR_Y, TOOL_H, RULER_Y, RULER_H, TRK_Y, ROW_H, HEAD_W, BAR_W, FIRST_BAR)
from topbar import segmented, joined

R = INSP_X - 2          # the arrangement's right edge
SEL = 'Chords'


def bx(bar):
    return HEAD_W + (bar - FIRST_BAR) * BAR_W


def toolrow(t):
    y, h = ARR_Y, TOOL_H
    out = [rect(0, y, R, h, t['surface']), lit_edge(t, 0, y, R)]
    out.append(text(12, y + 18, 'Arrangement', 13, t['ink1'], 500))
    # the tools: a radio set — the chosen one is a lifted pill, never coloured
    tools = [G.select, G.draw, G.slice_, G.erase, G.mute_tool, G.zoom]
    kx, ky, kw, kh = 104, y + 4, 26, 20
    out.append(joined(t, kx, ky, [kw] * len(tools), kh))
    out.append(rect(kx + 2, ky + 2, kw - 4, kh - 4, t['raised'], 3))
    for i, g in enumerate(tools):
        ink = t['ink1'] if i == 0 else t['ink2']
        out.append(G.nat(g, kx + i * kw + kw / 2, ky + kh / 2, 20, ink, t['ink3']))
    gx = kx + kw * len(tools) + 16
    out.append(text(gx, y + 18, 'Grid', 11, t['ink3']))
    out.append(well(t, gx + 26, ky, 58, kh))
    out.append(text(gx + 34, ky + 14, '1 bar', 11, t['ink1']))
    out.append(G.chevron_down(gx + 26 + 58 - 10, ky + 10, 2.5, t['ink3'], 1.2))
    # right: follow the playhead, and the selection's length — quiet words
    out.append(text(R - 12, y + 18, 'Follow', 11, t['ink3'], 400, 'end'))
    out.append(rect(R - 12 - tw('Follow', 11) - 14, y + 11, 7, 7, mix(t['surface'], t['ink3'], 0.6), 3.5))
    return ''.join(out)


def ruler(t, loop=(41, 49), head=41.0):
    y, h = RULER_Y, RULER_H
    out = [rect(0, y, R, h, t['surface'])]
    out.append(well(t, 10, y + 2, 20, 20))
    out.append(G.plus(20, y + 12, 4, t['ink2'], 1.2))
    out.append(text(38, y + 16, '9 tracks', 11, t['ink3']))
    out.append(f'<clipPath id="rclip"><rect x="{HEAD_W}" y="{y}" width="{R - HEAD_W}" height="{h}"/></clipPath><g clip-path="url(#rclip)">')
    # the loop: a quiet band on the numbers' half, the accent at its ends
    if loop:
        a, b = bx(loop[0]), bx(loop[1])
        out.append(rect(a, y + 13, b - a, 11, mix(t['surface'], t['accent'], 0.16)))
        out.append(rect(a, y + 22, b - a, 2, t['accent']))
    for bar in range(FIRST_BAR, 58):
        x = bx(bar)
        sec = next((nm for s, nm in song.SECTIONS if s == bar), None)
        out.append(rect(x, y + (12 if not sec else 3), 1, 12 if not sec else 21, t['ink4'] if not sec else t['ink3']))
        out.append(text(x + 6, y + 21, str(bar), 10, t['ink1'] if sec else t['ink3'], 500 if sec else 400))
        if sec:
            out.append(text(x + 5, y + 11, sec, 11, t['ink2'], 500))
    # the section that began off-screen keeps its name pinned at the left edge
    out.append(text(HEAD_W + 5, y + 11, 'Break', 11, t['ink2'], 500))
    out.append(rect(HEAD_W, y + 3, 1, 9, t['ink3']))
    if head is not None:
        px = bx(head)
        out.append(f'<path d="M{px - 4} {y + 14}H{px + 5}V{y + 18}L{px + 0.5} {y + 22.5}L{px - 4} {y + 18}Z" fill="{t["accent"]}"/>')
    out.append('</g>')
    return ''.join(out)


def header(t, i, name, kind, y, sel=False, state=None):
    h = ROW_H
    bg = mix(t['surface'], HUES[name], t['head_sel']) if sel else t['surface']
    if t['name'] == 'Paper' and sel:
        bg = mix(t['surface'], HUES[name], 0.16)
    out = [rect(0, y, HEAD_W, h - 1, bg)]
    out.append(rect(0, y, 3, h - 1, HUES[name] if sel else hue_for(t, name, t['stripe_rest'])))
    out.append(text(20, y + 19, str(i), 10, t['ink3'], 400, 'end'))
    gk = {'drum': G.kind_drum, 'synth': G.kind_synth, 'audio': G.kind_audio}[kind]
    out.append(gk(32, y + 14.5, t['ink2'] if not sel else t['ink1'], t['ink3']))
    out.append(text(44, y + 19, name, 12, t['ink1'], 500 if sel else 400))
    st = (state or {}).get(name, '')
    # mute, solo, arm: quiet marks at rest, lit only when engaged
    for k, (lab, on, col) in enumerate((('M', 'm' in st, t['ink2']), ('S', 's' in st, t['solo']))):
        cx = 128 + k * 16
        if on:
            out.append(rect(cx - 6, y + 8, 13, 13, col, 2))
            out.append(text(cx + 0.5, y + 18.5, lab, 10, t['accent_ink'] if t['name'] == 'Night' else '#fff', 600, 'middle'))
        else:
            out.append(text(cx + 0.5, y + 18.5, lab, 10, t['ink3'], 500, 'middle'))
    out.append(f'<circle cx="164.5" cy="{y + 14.5}" r="3.5" fill="none" stroke="{t["ink3"]}" stroke-width="1.1"/>')
    return ''.join(out)


def clip_content(t, name, kind, clip, x0, y0, w, h, sel):
    start, bars, label = clip
    ink = note_ink(t, name, sel)
    out = []
    px_beat = BAR_W / 4
    if kind == 'audio':
        env = song.audio_env(name, start, bars, x0, x0 + w, px_beat)
        mid = y0 + h / 2
        amp = h / 2 - 3
        top = ' '.join(f'{x},{mid - a * amp:.1f}' for x, a in env)
        bot = ' '.join(f'{x},{mid + a * amp:.1f}' for x, a in reversed(env))
        out.append(f'<polygon points="{top} {bot}" fill="{ink}"/>')
        out.append(rect(x0, round(mid), w, 1, mix(ink, t['base'], 0.35)))
        return ''.join(out)
    ns = song.notes(name, start, bars, label)
    pitches = sorted({p for _, _, _, p, _ in ns})
    pad = 4
    inner = h - 2 * pad
    if len(pitches) == 1:
        rh, rows = min(10, inner * 0.5), {pitches[0]: y0 + h / 2 - min(10, inner * 0.5) / 2}
    else:
        step = inner / len(pitches)
        rh = max(2, min(3.5, step - 1 if step > 3 else step))
        rows = {p: y0 + pad + (len(pitches) - 1 - k) * step + (step - rh) / 2 for k, p in enumerate(pitches)}
    for b, q, ln, p, v in ns:
        nx = x0 + b * BAR_W + q * px_beat
        nw = max(1.6, ln * px_beat - 0.8)
        if kind == 'drum':
            hh = rh * (0.55 + 0.45 * v) if len(pitches) == 1 else rh
            yy = rows[p] + (rh - hh) / 2
            out.append(f'<rect x="{nx:.2f}" y="{yy:.2f}" width="{max(2, min(nw, 3)):.2f}" height="{hh:.2f}" rx="0.8" fill="{ink}"/>')
        else:
            out.append(f'<rect x="{nx:.2f}" y="{rows[p]:.2f}" width="{nw:.2f}" height="{rh:.2f}" rx="{min(1, rh / 2):.2f}" fill="{ink}"/>')
    return ''.join(out)


def lanes(t, head=41.0, state=None):
    top = TRK_Y
    n = len(song.TRACKS)
    bottom = top + n * ROW_H
    out = [rect(HEAD_W, top, R - HEAD_W, bottom - top, t['base'])]
    out.append(f'<clipPath id="lclip"><rect x="{HEAD_W}" y="{top}" width="{R - HEAD_W}" height="{bottom - top}"/></clipPath><g clip-path="url(#lclip)">')
    for bar in range(FIRST_BAR, 58):
        x = bx(bar)
        if ((bar - 1) // 4) % 2 == 1:
            out.append(rect(x, top, BAR_W * 1, bottom - top, t['lane2']))
        for q in range(1, 4):
            out.append(rect(x + q * BAR_W // 4, top, 1, bottom - top, t['grid_beat']))
        out.append(rect(x, top, 1, bottom - top, t['grid_bar']))
    for i in range(1, n):
        out.append(rect(HEAD_W, top + i * ROW_H - 1, R - HEAD_W, 1, t['grid_bar']))
    for i, (name, kind, clips) in enumerate(song.TRACKS):
        y = top + i * ROW_H
        sel = name == SEL
        for c in clips:
            x0 = bx(c[0])
            w = c[1] * BAR_W - 1
            cy, chh = y + 1, ROW_H - 3
            live = PLAYING[0] and head is not None and c[0] <= head < c[0] + c[1]
            k = t['tint_sel'] if sel else (t['tint_rest'] + 0.06 if live else t['tint_rest'])
            body = hue_for(t, name, k)
            out.append(rect(x0, cy, w, chh, body, 3))
            out.append(rect(x0 + 1, cy, w - 2, 1, hue_for(t, name, 0.75 if sel or live else 0.42)))
            out.append(clip_content(t, name, kind, c, x0 + 1, cy + 1, w - 2, chh - 1, sel))
            if sel:
                lab = c[2]
                lx = max(x0, HEAD_W) + 5
                lw = tw(lab, 10, 500) + 8
                out.append(rect(lx - 3, cy + 2, lw, 12, mix(body, t['base'], 0.35), 2))
                out.append(text(lx + 1, cy + 11, lab, 10, mix(HUES[name], t['ink1'], 0.55) if t['name'] == 'Night' else t['ink1'], 500))
    if head is not None:
        px = bx(head)
        out.append(rect(px, top, 1, bottom - top, t['accent']))
    out.append('</g>')
    for i, (name, kind, clips) in enumerate(song.TRACKS):
        out.append(header(t, i + 1, name, kind, top + i * ROW_H, name == SEL, state))
    # the horizontal scroll: a thin thumb on the panel's own colour
    sy = bottom
    out.append(rect(0, sy, R, 8, t['surface']))
    out.append(rect(HEAD_W + 120, sy + 2, 330, 4, mix(t['surface'], t['ink3'], 0.55), 2))
    return ''.join(out)


PLAYING = [False]


def arrangement(t, state):
    import toolbar2
    PLAYING[0] = bool(state.get('playing'))
    return toolbar2.row(t, R, state, ARR_Y)[0] + ruler(t, head=state.get('head', 41.0)) + lanes(t, state.get('head', 41.0), state.get('tracks'))
