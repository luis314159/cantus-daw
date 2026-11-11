"""Slate (dir-a) — Cantus's mark drawn small, for each place it is shown at.

The idea of the user's logo #2 ("C de onda con playhead-cruz"): a dark tile, a C made of a
waveform's spokes open to the right, and a playhead with its crossbar where the C opens.
Below 48 px the original's glow turns to blur; this redraws the idea for each size:
- 16 px: the C is one solid arc, the playhead one pixel wide; no cap.
- 20 / 24 px: the C is a thick arc cut into teeth (the spokes, read as a comb); the
  playhead two pixels on whole pixels; the cap from 24.
- 30 px and up: real spokes of alternating length, as the original.
Every vertical and horizontal edge sits on whole pixels.
"""
import math

WAVE = ('#ffd98a', '#e07a2e', '#9c300c')
HEAD = ('#fff6d6', '#e9b64c')
TILE = ('#2c1810', '#160c07', '#0b0604')

SPEC = {
    # n: (radius, C centre x, ri, ro, mode, teeth/spokes, spoke width, playhead x, width)
    16: (4, 7, 2.2, 5.4, 'arc', 0, 0, 11, 1),
    20: (4, 8, 2.6, 6.4, 'teeth', 8, 0, 14, 2),
    24: (5, 10, 3.2, 7.6, 'teeth', 9, 0, 17, 2),
    30: (7, 12, 4.2, 9.4, 'spokes', 11, 1.5, 21, 2),
    32: (7, 13, 4.4, 10.0, 'spokes', 11, 1.6, 23, 2),
    36: (8, 15, 5.0, 11.2, 'spokes', 13, 1.7, 26, 3),
    48: (10, 20, 6.6, 15.0, 'spokes', 17, 2.0, 34, 3),
}


def _defs(uid, n, wave=None):
    WV = wave or WAVE
    return (f'<radialGradient id="lt{uid}" cx="32%" cy="26%" r="90%"><stop offset="0" stop-color="{TILE[0]}"/>'
            f'<stop offset="0.6" stop-color="{TILE[1]}"/><stop offset="1" stop-color="{TILE[2]}"/></radialGradient>'
            f'<linearGradient id="lw{uid}" gradientUnits="userSpaceOnUse" x1="{n * 0.2}" y1="{n * 0.2}" x2="{n * 0.7}" y2="{n * 0.85}">'
            f'<stop offset="0" stop-color="{WV[0]}"/><stop offset="0.45" stop-color="{WV[1]}"/><stop offset="1" stop-color="{WV[2]}"/></linearGradient>'
            f'<linearGradient id="lh{uid}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{HEAD[0]}"/>'
            f'<stop offset="1" stop-color="{HEAD[1]}"/></linearGradient>')


def mark(n, x=0, y=0, uid=None, wave=None):
    """The mark at n physical pixels, its top-left at (x, y)."""
    uid = uid or f'{n}_{x}_{y}'.replace('.', '_')
    r, cx, ri, ro, mode, k, sw, phx, lw = SPEC[n]
    cy = n / 2
    a0, a1 = 52, 308          # the C opens to the right, toward the playhead
    out = [f'<g transform="translate({x} {y})"><defs>{_defs(uid, n, wave)}</defs>']
    out.append(f'<rect width="{n}" height="{n}" rx="{r}" fill="url(#lt{uid})"/>')
    if n >= 24:   # the tile's lit top edge
        out.append(f'<rect x="{r}" y="0" width="{n - 2 * r}" height="1" fill="#ffffff" opacity="0.10"/>')
    wave = f'url(#lw{uid})'
    if mode in ('arc', 'teeth'):
        rm = (ri + ro) / 2
        th = ro - ri
        p0 = (cx + rm * math.cos(math.radians(a0)), cy + rm * math.sin(math.radians(a0)))
        p1 = (cx + rm * math.cos(math.radians(a1)), cy + rm * math.sin(math.radians(a1)))
        d = f'M{p0[0]:.2f} {p0[1]:.2f}A{rm:.2f} {rm:.2f} 0 1 1 {p1[0]:.2f} {p1[1]:.2f}'
        dash = ''
        if mode == 'teeth':
            L = math.radians(a1 - a0) * rm
            seg = L / k
            dash = f' stroke-dasharray="{seg * 0.62:.2f} {seg * 0.38:.2f}"'
        out.append(f'<path d="{d}" fill="none" stroke="{wave}" stroke-width="{th:.2f}" stroke-linecap="butt"{dash}/>')
    else:
        for i in range(k):
            a = math.radians(a0 + (a1 - a0) * i / (k - 1))
            wob = 1.0 if i % 2 == 0 else 0.84
            x0, y0 = cx + ri * math.cos(a), cy + ri * math.sin(a)
            x1, y1 = cx + ro * wob * math.cos(a), cy + ro * wob * math.sin(a)
            out.append(f'<path d="M{x0:.2f} {y0:.2f}L{x1:.2f} {y1:.2f}" stroke="{wave}" stroke-width="{sw}" stroke-linecap="round"/>')
    head = f'url(#lh{uid})'
    top, bot = round(n * 0.22), round(n * 0.79)
    out.append(f'<rect x="{phx}" y="{top}" width="{lw}" height="{bot - top}" fill="{head}"/>')
    bw = lw + 2 * max(2, round(n * 0.1))
    by = round(n * 0.34)
    out.append(f'<rect x="{phx - (bw - lw) // 2}" y="{by}" width="{bw}" height="{lw}" fill="{head}"/>')
    if n >= 24:
        c = phx + lw / 2
        h = max(2, round(n * 0.08))
        out.append(f'<path d="M{phx - 2} {top}H{phx + lw + 2}L{c} {top + h}Z" fill="{head}"/>')
    out.append('</g>')
    return ''.join(out)
