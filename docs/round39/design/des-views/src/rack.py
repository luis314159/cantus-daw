"""des-views (round 39, Slate) — the channel rack: one row per channel (a kit's pads indented
under it), each with its lamp, pan, volume, mixer track and name; steps that show their
velocity as a level inside the step; melodic channels as a small roll, audio as its
waveform; two bars a page, the pattern's eight bars as a map in the head."""
import math
import glyphs as G
import song_rack as S
from base import rect, text, well, mix, tw, lit_edge, HUES

ROW, KID = 30, 26
STEP_W, STEP_P, BEAT_GAP, BAR_GAP = 18, 20, 4, 12


def step_x(x0, i):
    """x of step i (0..31) in a two-bar page."""
    beat, bar = i // 4, i // 16
    return x0 + i * STEP_P + (beat - bar) * BEAT_GAP + bar * BAR_GAP


def page_w():
    return step_x(0, 31) + STEP_W


def mini_knob(t, cx, cy, val, bip=False, r=6.5):
    """A rack knob at 16 px: a recessed disc, the value as an arc on its rim, a short pointer."""
    a0, a1 = math.radians(-135), math.radians(135)
    av = a0 + (a1 - a0) * val
    out = f'<circle cx="{cx}" cy="{cy}" r="{r + 1}" fill="{t["edge"]}"/>'
    out += f'<circle cx="{cx}" cy="{cy}" r="{r}" fill="url(#kcap-{t["name"]})"/>'
    st = math.radians(0) if bip else a0

    def pt(a, rr):
        return cx + rr * math.sin(a), cy - rr * math.cos(a)
    if abs(av - st) > 0.02:
        (xa, ya), (xb, yb) = pt(min(st, av), r + 2.6), pt(max(st, av), r + 2.6)
        large = 1 if abs(av - st) > math.pi else 0
        out += (f'<path d="M{xa:.2f} {ya:.2f}A{r + 2.6} {r + 2.6} 0 {large} 1 {xb:.2f} {yb:.2f}" fill="none" '
                f'stroke="{t["ink3"]}" stroke-width="1.2" stroke-linecap="round"/>')
    (x1, y1), (x2, y2) = pt(av, r * 0.25), pt(av, r * 0.85)
    out += f'<path d="M{x1:.2f} {y1:.2f}L{x2:.2f} {y2:.2f}" stroke="{t["ink1"]}" stroke-width="1.4" stroke-linecap="round"/>'
    return out


def lamp(t, cx, cy, on):
    if on:
        return (f'<circle cx="{cx}" cy="{cy}" r="4.5" fill="{t["play"]}" fill-opacity="0.18"/>'
                f'<circle cx="{cx}" cy="{cy}" r="2.8" fill="{t["play"]}"/>')
    return f'<circle cx="{cx}" cy="{cy}" r="2.8" fill="none" stroke="{t["ink4"]}" stroke-width="1.2"/>'


def hue_col(t, name, k):
    h = HUES[name]
    if t['name'] == 'Paper':
        h = mix(h, '#20303f', 0.25)
    return mix(t['base'], h, k)


def steps(t, x0, y, h, name, hits, sel=False, muted=False):
    """hits: {step: velocity}. Off steps are wells, a beat's four a shade apart from the next
    beat's; on steps fill from the floor to their velocity in the channel's colour."""
    out = []
    night = t['name'] == 'Night'
    for i in range(32):
        x = step_x(x0, i)
        odd = (i // 4) % 2 == 1
        base = mix(t['well'], t['surface'], 0.75 if night else 0.0) if odd else t['well']
        if not night and odd:
            base = mix(t['well'], t['edge'], 0.35)
        out.append(rect(x, y, STEP_W, h, t['edge'], 3))
        out.append(rect(x + 0.5, y + 1, STEP_W - 1, h - 1.5, base, 2.5))
        v = hits.get(i)
        if v:
            k = 0.80 if not muted else 0.30
            lit = hue_col(t, name, k + (0.06 if sel else 0))
            dim = hue_col(t, name, 0.42 if not muted else 0.18)
            fh = round((h - 2) * (0.45 + 0.55 * v))
            out.append(rect(x + 1, y + 1, STEP_W - 2, h - 2, dim, 2))
            out.append(rect(x + 1, y + h - 1 - fh, STEP_W - 2, fh, lit, 2))
            out.append(rect(x + 2, y + h - 1 - fh, STEP_W - 4, 1, mix(lit, '#ffffff', 0.35 if night else 0.25)))
    return ''.join(out)


def roll(t, x0, y, h, name, notes, sel=False):
    """A melodic channel: its pattern as a small roll over the same page, pitch ranked."""
    W = page_w()
    out = [rect(x0, y, W, h, t['edge'], 3), rect(x0 + 0.5, y + 1, W - 1, h - 1.5, t['well'], 2.5)]
    for k in range(1, 8):
        xx = step_x(x0, k * 4) - BEAT_GAP / 2 - (BAR_GAP - BEAT_GAP) / 2 * (k == 4)
        out.append(rect(round(xx), y + 3, 1, h - 6, mix(t['well'], t['ink4'], 0.6 if k == 4 else 0.3)))
    ps = sorted({p for _, _, p in notes})
    lo, hi = ps[0], ps[-1]
    span = max(1, hi - lo)
    ink = hue_col(t, name, 0.95 if sel else 0.78)
    for q, ln, p in notes:
        i0 = int(q * 4)
        xa = step_x(x0, i0) + (q * 4 - i0) * STEP_P
        i1 = min(31, int((q + ln) * 4 - 1e-6))
        xb = step_x(x0, i1) + ((q + ln) * 4 - i1) * STEP_P - 2
        yy = y + h - 4 - (p - lo) / span * (h - 9) - 2.5
        out.append(rect(round(xa, 1), round(yy, 1), max(2, round(xb - xa, 1)), 3, ink, 1))
    return ''.join(out)


def wave(t, x0, y, h, name, kind):
    """An audio channel: its sample's waveform over the page, the time it plays."""
    W = page_w()
    out = [rect(x0, y, W, h, t['edge'], 3), rect(x0 + 0.5, y + 1, W - 1, h - 1.5, t['well'], 2.5)]
    ink = hue_col(t, name, 0.72)
    mid = y + h / 2
    for i in range(int(W) - 6):
        beat = i / (W / 8)
        if kind == 'perc':
            ph = beat % 4
            a = 0.05
            for k, p in enumerate((0, 0.75, 1.5, 2.0, 2.75, 3.5)):
                d = ph - p
                if d >= 0:
                    a = max(a, (0.95 if k in (0, 3) else 0.62) * math.exp(-d * 3.5))
            a *= 0.55 + 0.45 * abs(math.sin(i * 2.1) * math.sin(i * 0.53 + 1))
        else:
            sw = 0.42 + 0.30 * math.sin(math.pi * min(1, ((beat / 4) % 1) * 1.15)) ** 0.8
            a = sw * (0.70 + 0.30 * abs(math.sin(i * 0.9) * math.sin(i * 0.37)))
        hh = max(1, a * (h - 6))
        out.append(rect(x0 + 3 + i, round(mid - hh / 2, 1), 1, round(hh, 1), ink))
    return ''.join(out)
