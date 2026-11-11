"""des-mixer — style D, "Jack": real 1/4" patch cables on the mixer.

Every strip's foot carries two jacks on its bottom edge (input at the centre −11, output at
the centre +11): a round nut, the hole. A connected jack holds a plug seen from the side,
hanging below the edge: the metal collar, a knurled metal barrel, a band in the cable's kind
colour, a rubber strain-relief boot with three ribs. The cable leaves the boot and hangs on
a catenary to the other plug. Layers, back to front: a two-pass soft shadow on the desk, a
darker edge, the body, a narrow highlight (a tube lit from above), and for a sidechain key a
spiral of darker marks (the dashed meaning of A/B, kept on a real cable).
"""
import math
from base import mix

PLUG_LEN = 22      # collar 4 + barrel 7 + band 1.5 + boot 9.5 (logical px)


def tok(t):
    n = t['name'] == 'Night'
    return dict(
        out=t['accent'], send=mix(t['accent'], t['ink1'], 0.45) if n else mix(t['accent'], t['base'], 0.38),
        key=mix(t['solo'], t['rec'], 0.25) if n else mix(t['solo'], '#ff9a2e', 0.40), refused=t['rec'],
        metal_hi='#f1f4f7', metal_mid='#a3adb7' if n else '#b4bdc6', metal_lo='#4a535d' if n else '#6c7782',
        boot='#1b2026' if n else '#2c353e', boot_hi='#323a43' if n else '#46515c',
        nut='#5d6772' if n else '#9aa5b0', hole='#0b0e11',
        shadow='#000000' if n else '#1c2a38', shadow_a=(0.30, 0.12) if n else (0.18, 0.07),
        channel=mix(t['base'], t['gutter'], 0.40))


def metal_grad(t, gid):
    T = tok(t)
    return (f'<linearGradient id="{gid}" x1="0" x2="1" y1="0" y2="0">'
            f'<stop offset="0" stop-color="{T["metal_lo"]}"/><stop offset="0.32" stop-color="{T["metal_hi"]}"/>'
            f'<stop offset="0.6" stop-color="{T["metal_mid"]}"/><stop offset="1" stop-color="{T["metal_lo"]}"/></linearGradient>'
            f'<linearGradient id="{gid}-boot" x1="0" x2="1" y1="0" y2="0">'
            f'<stop offset="0" stop-color="{T["boot"]}"/><stop offset="0.35" stop-color="{T["boot_hi"]}"/>'
            f'<stop offset="1" stop-color="{T["boot"]}"/></linearGradient>')


def jack(t, cx, y, empty=True, halo=None, bad=False):
    """The jack on the strip's bottom edge: a round nut with its hole (an empty jack shows it)."""
    T = tok(t)
    out = ''
    if halo:
        out += f'<circle cx="{cx}" cy="{y}" r="10" fill="{halo}" opacity="0.25"/>'
    out += f'<ellipse cx="{cx}" cy="{y}" rx="6" ry="3" fill="{T["nut"]}"/>'
    out += f'<ellipse cx="{cx}" cy="{y - 0.6}" rx="5" ry="2" fill="{mix(T["nut"], "#ffffff", 0.25)}"/>'
    if empty:
        out += f'<ellipse cx="{cx}" cy="{y}" rx="3" ry="1.4" fill="{T["hole"]}"/>'
    if bad:
        out += f'<ellipse cx="{cx}" cy="{y}" rx="7" ry="3.8" fill="none" stroke="{T["refused"]}" stroke-width="1.3"/>'
    return out


def plug_seated(t, cx, y, band, gid, boot=None):
    """A plug in its jack, seen from the side, hanging below the edge (y = the nut)."""
    T = tok(t)
    out = f'<rect x="{cx - 2.5}" y="{y + 1}" width="5" height="4" fill="url(#{gid})"/>'                 # collar
    out += f'<rect x="{cx - 3.5}" y="{y + 5}" width="7" height="7" rx="1" fill="url(#{gid})"/>'         # barrel
    for k in range(3):                                                                                     # knurl
        out += f'<rect x="{cx - 3.5}" y="{y + 6.5 + 2 * k}" width="7" height="0.6" fill="{T["metal_lo"]}" opacity="0.55"/>'
    out += f'<rect x="{cx - 3.5}" y="{y + 12}" width="7" height="1.5" fill="{band}"/>'                    # kind band
    bt = f'url(#{gid}-boot)' if not boot else boot
    out += (f'<path d="M{cx - 3.3} {y + 13.5}L{cx + 3.3} {y + 13.5}L{cx + 2.1} {y + PLUG_LEN}L{cx - 2.1} {y + PLUG_LEN}Z" '
            f'fill="{bt}"/>')                                                                              # boot
    for k in range(3):                                                                                     # ribs
        yy = y + 15.5 + 2.2 * k
        hw = 3.1 - 0.35 * k
        out += f'<rect x="{cx - hw}" y="{yy}" width="{2 * hw}" height="0.7" fill="#000000" opacity="0.35"/>'
    return out


def plug_free(t, cx, y, band, gid, boot=None):
    """A plug held in the hand (dragged): pointing up, its tip at y. Tip, ring, sleeve shown."""
    T = tok(t)
    dark = '#14181c'
    out = f'<path d="M{cx - 1.6} {y + 3}Q{cx - 1.6} {y} {cx} {y}Q{cx + 1.6} {y} {cx + 1.6} {y + 3}Z" fill="url(#{gid})"/>'  # tip
    out += f'<rect x="{cx - 1.6}" y="{y + 3}" width="3.2" height="1" fill="{dark}"/>'                       # insulator
    out += f'<rect x="{cx - 1.8}" y="{y + 4}" width="3.6" height="2.2" fill="url(#{gid})"/>'                # ring
    out += f'<rect x="{cx - 1.8}" y="{y + 6.2}" width="3.6" height="1" fill="{dark}"/>'                     # insulator
    out += f'<rect x="{cx - 1.9}" y="{y + 7.2}" width="3.8" height="8" fill="url(#{gid})"/>'                # sleeve
    out += f'<rect x="{cx - 1.1}" y="{y + 7.6}" width="0.6" height="7" fill="#ffffff" opacity="0.6"/>'      # highlight
    return out + plug_seated(t, cx, y + 14, band, gid, boot)


def catenary(sx, tx, yb, sag, n=28):
    """Points of y = yb + sag − a·(cosh((x − xm)/a) − 1), with a solved so the ends sit at yb."""
    L = abs(tx - sx)
    xm = (sx + tx) / 2
    lo, hi = 0.05, 1e5
    for _ in range(80):        # bisection on a: f(a) = a(cosh(L/2a) − 1) − sag, decreasing in a
        a = (lo + hi) / 2
        f = a * (math.cosh(min(L / (2 * a), 60)) - 1) - sag
        if f > 0:
            lo = a
        else:
            hi = a
    a = (lo + hi) / 2
    return [(sx + (tx - sx) * i / n, yb + sag - a * (math.cosh(((sx + (tx - sx) * i / n) - xm) / a) - 1)) for i in range(n + 1)]


def sag_for(span):
    return min(52.0, 12 + 0.16 * span)


def cable_d(sx, tx, yb, sag):
    """The cable's path: a quadratic blend out of each boot (vertical), the catenary between."""
    p = catenary(sx, tx, yb + 3, sag)
    d = f'M{sx:.1f} {yb:.1f}Q{sx:.1f} {yb + 7:.1f} {p[2][0]:.1f} {p[2][1]:.1f}'
    d += ''.join(f'L{x:.1f} {y:.1f}' for x, y in p[3:-2])
    d += f'Q{tx:.1f} {yb + 7:.1f} {tx:.1f} {yb:.1f}'
    return d


def free_d(sx, sy, tx, ty):
    """While dragging, the free cable is a cubic with vertical tangents (eases to the catenary on drop)."""
    k = min(60.0, 18 + 0.18 * abs(tx - sx))
    return f'M{sx:.1f} {sy:.1f}C{sx:.1f} {sy + k:.1f} {tx:.1f} {ty + k * 0.7:.1f} {tx:.1f} {ty:.1f}'


def cable(t, d, col, w, key=False, op=1.0, lift=False, gleam=None, cloth=False):
    T = tok(t)
    sa, sb = T['shadow_a']
    dx, dy = (3, 5) if lift else (1.5, 2.5)
    if lift:
        sa, sb = sa * 0.75, sb * 0.8
    o = f' opacity="{op}"' if op < 1 else ''
    s = f'<g{o}>'
    s += (f'<g transform="translate({dx},{dy})"><path d="{d}" fill="none" stroke="{T["shadow"]}" stroke-width="{w + 6}" '
          f'stroke-linecap="round" opacity="{sb}"/><path d="{d}" fill="none" stroke="{T["shadow"]}" stroke-width="{w + 3}" '
          f'stroke-linecap="round" opacity="{sa}"/></g>')
    edge = mix(col, t['edge'], 0.55) if t['name'] == 'Night' else mix(col, t['ink1'], 0.35)
    s += f'<path d="{d}" fill="none" stroke="{edge}" stroke-width="{w + 1.5}" stroke-linecap="round"/>'
    s += f'<path d="{d}" fill="none" stroke="{col}" stroke-width="{w}" stroke-linecap="round"/>'
    if cloth:      # a braid: two crossing dash layers, lighter and darker, offset by half a period
        s += (f'<path d="{d}" fill="none" stroke="{mix(col, "#ffffff", 0.35)}" stroke-width="{w}" stroke-dasharray="1.6 1.6" opacity="0.7"/>'
              f'<path d="{d}" fill="none" stroke="{mix(col, "#000000", 0.35)}" stroke-width="{w * 0.5}" stroke-dasharray="1.6 1.6" stroke-dashoffset="1.6" opacity="0.6"/>')
    if key:        # the key's spiral: short darker marks along the body
        s += f'<path d="{d}" fill="none" stroke="{mix(col, "#000000", 0.45)}" stroke-width="{w}" stroke-dasharray="1.4 3.2" opacity="0.8"/>'
    hi = mix(col, '#ffffff', 0.6)
    s += (f'<g transform="translate(-0.3,{-w * 0.22:.2f})"><path d="{d}" fill="none" stroke="{hi}" stroke-width="{w * 0.35:.2f}" '
          f'stroke-linecap="round" opacity="{0.85 if lift else 0.6}"/></g>')
    if gleam is not None:
        lvl, off = gleam
        s += (f'<path d="{d}" fill="none" stroke="{mix(col, "#ffffff", 0.75)}" stroke-width="{w}" stroke-linecap="round" '
              f'stroke-dasharray="8 56" stroke-dashoffset="{off}" opacity="{0.35 + 0.65 * lvl:.2f}"/>')
    return s + '</g>'


def width(kind, db):
    if kind == 'out':
        return 3.2
    if kind == 'send':
        return round(2.2 + 1.2 * max(0.0, min(1.0, (db + 36) / 36)), 2)
    return 2.8
