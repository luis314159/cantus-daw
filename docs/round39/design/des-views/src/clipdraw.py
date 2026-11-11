"""des-views — one clip and its content, by zoom band (far, default, near, close)."""
import song_full as S
from base import rect, text, mix, tw, hue_for, note_ink, HUES

HEAD = 13          # a clip's name strip, in lanes of 44 px and taller


def body(t, hue, x, y, w, h, state):
    sel, live, muted = state == 'sel', state == 'play', state == 'mute'
    if muted:
        k = 0.07
        top = hue_for(t, hue, 0.22)
    else:
        k = t['tint_sel'] if sel else (t['tint_rest'] + 0.06 if live else t['tint_rest'])
        top = hue_for(t, hue, 0.75 if sel or live else 0.42)
    out = [rect(x, y, w, h, hue_for(t, hue, k), 3), rect(x + 1, y, w - 2, 1, top)]
    if sel:
        out.append(rect(x + 0.5, y + 0.5, w - 1, h - 1, 'none', 3, f' stroke="{mix(HUES[hue], t["ink1"], 0.35)}" stroke-width="1"'))
    return ''.join(out), hue_for(t, hue, k)


def ink_of(t, hue, state):
    if state == 'mute':
        return mix(t['base'], t['ink3'], 0.45)
    return note_ink(t, hue, state == 'sel' or state == 'play')


def midi(t, kind, hue, clip, x, y, w, h, ppb, band, state):
    start, bars, label = clip
    ns = S.notes(kind, start, bars, label)
    ink = ink_of(t, hue, state)
    out = []
    pitches = sorted({p for _, _, p, _ in ns})
    lo, hi = pitches[0], pitches[-1]
    pad = 3
    if band == 'far':
        if kind == 'drums':        # the pulse: one tick a kick or clap, its height the velocity
            soft = mix(t['base'], ink, 0.62)
            for T, ln, p, v in ns:
                if p in (36, 39):
                    hh = (h - 2 * pad) * (0.45 + 0.5 * v) * (1 if p == 36 else 0.7)
                    out.append(rect(round(x + T * ppb, 1), y + h - pad - hh, 1, hh, soft))
            return ''.join(out)
        # the contour: per column, from the lowest to the highest note sounding there
        span = max(1, hi - lo)
        for c in range(int(w)):
            b0, b1 = c / ppb, (c + 1) / ppb
            on = [p for T, ln, p, v in ns if T < b1 and T + ln > b0]
            if not on:
                continue
            ya = y + pad + (hi - max(on)) / span * (h - 2 * pad - 2)
            yb = y + pad + (hi - min(on)) / span * (h - 2 * pad - 2) + 3
            out.append(rect(x + c, round(ya, 1), 1, round(yb - ya, 1), ink))
        return ''.join(out)
    # rows: ranked by pitch for chords and drums (and any lane under 40 px), true pitch otherwise
    inner = h - 2 * pad
    linear = kind == 'melody' and h >= 26
    if linear:
        span = hi - lo + 1
        step = inner / span
        rh = max(3, min(6, step * 1.8)) if band == 'default' else max(4 if band == 'near' else 5, min(8, step * (2.2 if band == 'near' else 2.8)))  # one voice: rows may overlap

        def ry(p):
            return y + pad + (hi - p) * step + (step - rh) / 2
    else:
        step = inner / len(pitches)
        rh = max(2, min(5 if band != 'default' else 3.5, step - 1))
        rank = {p: k for k, p in enumerate(pitches)}

        def ry(p):
            return y + pad + (len(pitches) - 1 - rank[p]) * step + (step - rh) / 2
    for T, ln, p, v in ns:
        nx = x + T * ppb
        if kind == 'drums':
            nw = 2 if band == 'default' else max(3, ln * ppb - 1.5)
            col = ink if band == 'default' else mix(t['base'], ink, 0.45 + 0.55 * v) if state != 'mute' else ink
            if p in (42, 46) and state != 'mute':      # the hats sit back; kick and clap carry the beat
                col = mix(t['base'], col, 0.62)
            out.append(rect(round(nx, 1), round(ry(p), 1), nw, round(rh, 1), col, 0.8))
            continue
        nw = max(1.6, ln * ppb - (0.8 if band == 'default' else 1.5))
        col = ink if band == 'default' else (mix(t['base'], ink, 0.62 + 0.38 * v) if state != 'mute' else ink)
        out.append(rect(round(nx, 1), round(ry(p), 1), round(nw, 1), round(rh, 1), col, min(1, rh / 2)))
        if band == 'close' and state != 'mute':      # the attack: a brighter first column
            out.append(rect(round(nx, 1), round(ry(p), 1), 1, round(rh, 1), mix(ink, '#ffffff', 0.4) if t['name'] == 'Night' else mix(ink, '#000000', 0.25)))
    return ''.join(out)


def _noise(i):
    """A deterministic 0..1 hash: the grain of real audio inside an envelope."""
    i = (i * 2654435761) & 0xffffffff
    i ^= i >> 15
    i = (i * 2246822519) & 0xffffffff
    return ((i >> 8) & 0xffff) / 65535


def audio(t, kind, hue, clip, x, y, w, h, ppb, band, state):
    start = clip[0]
    ink = ink_of(t, hue, state)
    rms_ink = mix(ink, '#ffffff', 0.25) if t['name'] == 'Night' else mix(ink, '#000000', 0.25)
    pk_ink = mix(t['base'], ink, 0.55) if band in ('near', 'close') else ink
    chans = 2 if (kind == 'audio2' and h >= 40 and band in ('near', 'close')) else 1
    out = []
    ch_h = h / chans
    for ch in range(chans):
        mid = y + ch_h * ch + ch_h / 2
        amp = (ch_h / 2 - 2) * (1.15 if chans == 2 else 1)
        top, bot, rt, rb = [], [], [], []
        for c in range(int(w) + 1):
            beat = c / ppb
            pk, rms = S.audio(kind, start, beat)
            n = _noise(c * 7 + ch * 131 + int(start) * 17)
            if band == 'far':
                # loudness: the mean of the envelope under the column, no texture
                vals = [S.audio(kind, start, beat + d / (ppb * 6))[0] for d in range(6)]
                pk = min(0.92, sum(vals) / 6 * (1.15 if kind == 'audio2' else 1.5))
            elif band == 'close':
                # the detail: the peak of a few samples under this column, not the envelope's
                s_ = abs(S.sample(kind, start, beat, ch)) / max(0.05, pk)
                pk *= (0.50 + 0.30 * n + 0.20 * min(1, s_)) if kind == 'audio' else (0.80 + 0.20 * n)
                rms *= 0.75 + 0.25 * n
            else:
                pk *= (0.62 + 0.38 * n) if kind == 'audio' else (0.85 + 0.15 * n)
                rms *= 0.8 + 0.2 * n
            top.append(f'{x + c},{mid - pk * amp:.1f}')
            bot.append(f'{x + c},{mid + pk * amp:.1f}')
            rt.append(f'{x + c},{mid - rms * amp:.1f}')
            rb.append(f'{x + c},{mid + rms * amp:.1f}')
        out.append(f'<polygon points="{" ".join(top)} {" ".join(reversed(bot))}" fill="{pk_ink}"/>')
        if band in ('near', 'close'):
            out.append(f'<polygon points="{" ".join(rt)} {" ".join(reversed(rb))}" fill="{rms_ink}"/>')
        out.append(rect(x, round(mid), w, 1, mix(ink, t['base'], 0.4)))
        if chans == 2 and ch == 0:
            out.append(rect(x, round(y + ch_h), w, 1, mix(t['base'], ink, 0.25)))
        if chans == 2 and band == 'close':
            out.append(text(x + 4, mid - amp + 9, 'LR'[ch], 9, t['ink3'], 600))
    return ''.join(out)


def automation(t, clip, x, y, w, h, ppb, band, state):
    start, bars, label = clip
    pts = S.auto_points(label, bars)
    ink = t['auto'] if state != 'mute' else mix(t['base'], t['ink3'], 0.45)
    pad = 4

    def Y(v):
        return y + pad + (1 - v) * (h - 2 * pad)
    line = [f'{x + c},{Y(S.auto_value(pts, c / ppb)):.1f}' for c in range(int(w) + 1)]
    out = [f'<polygon points="{x},{y + h} {" ".join(line)} {x + w},{y + h}" fill="{ink}" fill-opacity="{0.10 if t["name"] == "Night" else 0.12}"/>',
           f'<polyline points="{" ".join(line)}" fill="none" stroke="{ink}" stroke-width="{1.2 if band == "far" else 1.5}" stroke-linejoin="round"/>']
    if band == 'far':
        return ''.join(out)
    r = 2 if band == 'default' else 3
    for i, (b, v, k) in enumerate(pts):
        px, py = x + b * ppb, Y(v)
        if band != 'default':
            out.append(f'<circle cx="{px:.1f}" cy="{py:.1f}" r="{r + 1}" fill="{t["base"]}"/>')
        out.append(f'<circle cx="{px:.1f}" cy="{py:.1f}" r="{r}" fill="{ink}"/>')
        if band == 'close' and i + 1 < len(pts):
            b1 = pts[i + 1][0]
            mb = (b + b1) / 2
            hx, hy = x + mb * ppb, Y(S.auto_value(pts, mb))
            out.append(f'<rect x="{hx - 2.5:.1f}" y="{hy - 2.5:.1f}" width="5" height="5" transform="rotate(45 {hx:.1f} {hy:.1f})" '
                       f'fill="none" stroke="{ink}" stroke-width="1"/>')
        if band == 'close':
            hz = 120 * (2 ** (v * 7.2))
            lab = f'{hz / 1000:.1f} kHz' if hz >= 1000 else f'{hz:.0f} Hz'
            out.append(text(px + 6, py - 4 if v < 0.8 else py + 12, lab, 9, t['ink2'], 500))
    return ''.join(out)


def clip(t, lane, clip_, x, y, w, h, ppb, band, state='', uid='c'):
    name, hue, kind = lane
    out = [f'<clipPath id="{uid}"><rect x="{x}" y="{y}" width="{w}" height="{h}" rx="3"/></clipPath>']
    b, body_col = body(t, hue, x, y, w, h, state)
    out.append(b)
    out.append(f'<g clip-path="url(#{uid})">')
    cy, ch = y + 1, h - 2
    strip = h >= 40
    if strip:          # the name strip: the clip's name, quiet, its content below
        out.append(rect(x, y + 1, w, HEAD - 1, mix(body_col, t['base'], 0.35 if t['name'] == 'Night' else 0.2)))
        out.append(text(x + 5, y + 10.5, clip_[2], 10, mix(HUES[hue], t['ink1'], 0.55) if t['name'] == 'Night' else t['ink1'], 500))
        cy, ch = y + HEAD + 1, h - HEAD - 2
    if kind in ('melody', 'chords', 'drums'):
        out.append(midi(t, kind, hue, clip_, x + 1, cy, w - 2, ch, ppb, band, state))
    elif kind in ('audio', 'audio2'):
        out.append(audio(t, kind, hue, clip_, x + 1, cy, w - 2, ch, ppb, band, state))
    else:
        out.append(automation(t, clip_, x + 1, cy, w - 2, ch, ppb, band, state))
    # a name tag over content in short lanes: when selected, or when the clip is wide enough
    if not strip and (state == 'sel' or (band in ('near', 'close') and w >= 80)):
        lab = clip_[2]
        lw = tw(lab, 10, 500) + 8
        out.append(rect(x + 2, y + 2, lw, 12, mix(body_col, t['base'], 0.45), 2))
        out.append(text(x + 6, y + 11, lab, 10, mix(HUES[hue], t['ink1'], 0.55) if t['name'] == 'Night' else t['ink1'], 500))
    if state == 'mute':
        for k in range(-int(h), int(w), 6):
            out.append(f'<path d="M{x + k} {y + h}L{x + k + h} {y}" stroke="{t["ink4"]}" stroke-opacity="0.35" stroke-width="1"/>')
    out.append('</g>')
    return ''.join(out)
