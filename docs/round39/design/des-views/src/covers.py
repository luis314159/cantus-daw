"""des-views — the pack covers, drawn per pack, per size (14 px in the tree, 40 in a row,
120 on the Packs page). Each pack has its own motif, made from what it holds; the cover is a
deep tone of the pack's colour with the motif in a lighter tone of the same colour. No
words on the cover: the name sits beside or under it, in the type of the interface."""
import math
from base import mix

PACKS = {
    # name: (colour, motif, what it holds, count, size, licence)
    'Cantus Starter Drums': ('#e8794f', 'rings', 'Drums', 142, '38 MB', 'CC0'),
    'AFR Kicks 2024': ('#e0606f', 'kick', 'Kicks', 64, '21 MB', 'CC BY 4.0'),
    'Choir Voices': ('#6c93ea', 'vowels', 'Vocals', 40, '96 MB', 'CC BY 4.0'),
    'Lo-fi Keys': ('#e6a54a', 'keys', 'Instrument', 61, '140 MB', 'CC0'),
    'Analog Bass': ('#9a80ee', 'square', 'Presets', 48, '1 MB', 'CC0'),
    'Field Recordings': ('#5fc584', 'contour', 'Textures', 77, '310 MB', 'CC BY 4.0'),
    'Aula Spaces': ('#3fc0c8', 'decay', 'Impulse responses', 36, '54 MB', 'CC0'),
    'Street Percussion': ('#d2c25c', 'dots', 'Loops', 52, '88 MB', 'CC BY 3.0'),
}

_N = [0]


def cover(x, y, s, pack, r=None):
    hue, motif = PACKS[pack][0], PACKS[pack][1]
    _N[0] += 1
    gid = f'cv{_N[0]}'
    r = r if r is not None else (3 if s < 24 else 6 if s < 80 else 8)
    deep, mid = mix(hue, '#0b0f14', 0.62), mix(hue, '#0b0f14', 0.30)
    ink = mix(hue, '#ffffff', 0.30)
    out = [f'<defs><linearGradient id="{gid}" x1="0" y1="0" x2="1" y2="1">'
           f'<stop offset="0" stop-color="{mid}"/><stop offset="1" stop-color="{deep}"/></linearGradient>'
           f'<clipPath id="{gid}c"><rect x="{x}" y="{y}" width="{s}" height="{s}" rx="{r}"/></clipPath></defs>',
           f'<rect x="{x}" y="{y}" width="{s}" height="{s}" rx="{r}" fill="url(#{gid})"/>',
           f'<g clip-path="url(#{gid}c)">']
    sw = max(1, s / 40)                 # the motif's line weight grows with the cover
    small = s < 24

    def P(fx, fy):
        return f'{x + fx * s:.2f} {y + fy * s:.2f}'

    if motif == 'rings':                # a drum head seen from above, off centre
        n = 2 if small else 5
        for k in range(n):
            rr = s * (0.20 + k * (0.42 if small else 0.16))
            out.append(f'<circle cx="{x + s * 0.68:.2f}" cy="{y + s * 0.70:.2f}" r="{rr:.2f}" fill="none" '
                       f'stroke="{ink}" stroke-opacity="{0.9 - k * 0.15:.2f}" stroke-width="{sw * (1.6 if k == 0 else 1):.2f}"/>')
        out.append(f'<circle cx="{x + s * 0.68:.2f}" cy="{y + s * 0.70:.2f}" r="{s * 0.07:.2f}" fill="{ink}"/>')
    elif motif == 'kick':               # one kick: a fast sweep down, decaying
        pts = []
        for i in range(0, 121):
            f = i / 120
            ph = 2 * math.pi * (2.2 * f + 3.0 * (1 - math.exp(-f * 6)))
            a = math.exp(-f * 3.2) * 0.36
            pts.append(P(0.06 + f * 0.9, 0.55 - a * math.sin(ph)))
        out.append(f'<path d="M{" L".join(pts)}" fill="none" stroke="{ink}" stroke-width="{sw * 1.6:.2f}" stroke-linejoin="round"/>')
        if not small:
            out.append(f'<path d="M{P(0.06, 0.82)} L{P(0.94, 0.82)}" stroke="{ink}" stroke-opacity="0.35" stroke-width="{sw:.2f}"/>')
    elif motif == 'vowels':             # three open mouths: arcs widening upward
        for k in range(2 if small else 4):
            w = 0.22 + k * 0.17
            yy = 0.78 - k * 0.15
            out.append(f'<path d="M{P(0.5 - w, yy - 0.02)} Q{P(0.5, yy + 0.18)} {P(0.5 + w, yy - 0.02)}" fill="none" '
                       f'stroke="{ink}" stroke-opacity="{1 - k * 0.18:.2f}" stroke-width="{sw * 1.4:.2f}" stroke-linecap="round"/>')
    elif motif == 'keys':               # a keyboard in perspective, from the corner
        for k in range(4 if small else 9):
            fx = 0.10 + k * (0.24 if small else 0.11)
            out.append(f'<path d="M{P(fx, 1.05)} L{P(fx + 0.30, 0.30)}" stroke="{ink}" stroke-opacity="0.75" stroke-width="{sw:.2f}"/>')
            if not small and k % 7 not in (2, 6):
                out.append(f'<path d="M{P(fx + 0.17, 0.62)} L{P(fx + 0.27, 0.37)}" stroke="{ink}" stroke-width="{sw * 3:.2f}" stroke-linecap="round"/>')
    elif motif == 'square':             # a square wave whose edges round as it decays
        d = f'M{P(0.04, 0.5)}'
        for k in range(3 if small else 5):
            x0 = 0.04 + k * (0.30 if small else 0.18)
            hw = (0.15 if small else 0.09)
            a = 0.26 * (0.88 ** k)
            d += f' L{P(x0, 0.5 - a)} L{P(x0 + hw, 0.5 - a)} L{P(x0 + hw, 0.5 + a)} L{P(x0 + 2 * hw, 0.5 + a)} L{P(x0 + 2 * hw, 0.5)}'
        out.append(f'<path d="{d}" fill="none" stroke="{ink}" stroke-width="{sw * 1.5:.2f}" stroke-linejoin="round"/>')
    elif motif == 'contour':            # a map's contour lines
        for k in range(3 if small else 7):
            rx, ry = 0.14 + k * 0.10, 0.09 + k * 0.075
            out.append(f'<ellipse cx="{x + s * 0.38:.2f}" cy="{y + s * 0.42:.2f}" rx="{s * rx:.2f}" ry="{s * ry:.2f}" '
                       f'transform="rotate(-18 {x + s * 0.38:.2f} {y + s * 0.42:.2f})" fill="none" stroke="{ink}" '
                       f'stroke-opacity="{0.95 - k * 0.09:.2f}" stroke-width="{sw:.2f}"/>')
    elif motif == 'decay':              # an impulse response: spikes decaying into air
        n = 6 if small else 26
        for k in range(n):
            f = k / n
            a = math.exp(-f * 3.4) * (0.62 if k == 0 else 0.40 * (0.6 + 0.4 * abs(math.sin(k * 2.3))))
            xx = 0.10 + f * 0.82
            out.append(f'<path d="M{P(xx, 0.78)} L{P(xx, 0.78 - a)}" stroke="{ink}" stroke-width="{sw * (1.6 if k == 0 else 1.1):.2f}" stroke-linecap="round"/>')
    elif motif == 'dots':               # a groove: hits on a grid, accents larger
        cols = 4 if small else 8
        for i in range(cols):
            for j in range(2 if small else 4):
                on = (i * 3 + j * 5) % 7 in (0, 2, 3)
                rr = (0.07 if small else 0.035) * (1.5 if (i + j) % 4 == 0 else 1)
                out.append(f'<circle cx="{x + s * (0.12 + i * 0.76 / (cols - 1)):.2f}" cy="{y + s * (0.3 + j * (0.4 if small else 0.15)):.2f}" '
                           f'r="{s * rr:.2f}" fill="{ink}" fill-opacity="{0.95 if on else 0.28}"/>')
    # a hairline of light on the cover's upper edge
    out.append(f'</g><rect x="{x + 0.5}" y="{y + 0.5}" width="{s - 1}" height="{s - 1}" rx="{r}" fill="none" '
               f'stroke="#ffffff" stroke-opacity="0.10" stroke-width="1"/>')
    return ''.join(out)
