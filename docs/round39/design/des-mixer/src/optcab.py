"""des-mixer — the routing cables, designed: three directions on one busy desk.

Kick → Drum bus (output), Kick → Reverb (send −12), Chords → Reverb (send −16), Chords →
Delay (send −22), Kick → Bass's compressor (sidechain key), Drum bus → Master (a bus to
the master), Reverb → Master (a return). A track's route to the master is implied (its foot
says "Master"); a strip that receives (bus, return) draws its way to the master so a chain
reads end to end. Drag Drum bus → Kick would loop: refused.

A — patch cables: one cubic per cable, vertical tangents at both sockets (it hangs from the
    jacks), sag by span; a darker under-stroke of its own colour; width by kind and level.
B — A, plus a travelling gleam while audio flows (speed and brightness by level; at rest
    nothing moves).
C — subway: orthogonal routes with rounded corners in a channel under the strips, one lane
    per kind, sub-lanes where same-kind routes overlap.
"""
import data  # noqa: F401
import strip as ST
from base import rect, text, mix, tw, HUES
from sheets import song

H = 360           # strips drawn at this height, only their foot shows
SHOW = 112        # how much of the strip's foot is visible
BAND = 50         # the channel under the strips
NAMES = ['Kick', 'Bass', 'Chords', 'Drum bus', 'Reverb', 'Delay']
ROUTES = [  # (from, to, kind, level dB)
    ('Kick', 'Drum bus', 'out', 0), ('Kick', 'Reverb', 'send', -12), ('Kick', 'Bass', 'key', 0),
    ('Chords', 'Reverb', 'send', -16), ('Chords', 'Delay', 'send', -22),
    ('Drum bus', 'Master', 'out', 0), ('Reverb', 'Master', 'out', 0),
]


def tok(t):
    """The cable tokens (derived from Slate's; no new base colour except the mixes)."""
    n = t['name'] == 'Night'
    return dict(out=t['accent'],
                send=mix(t['accent'], t['ink1'], 0.45) if n else mix(t['accent'], t['base'], 0.38),
                key=mix(t['solo'], t['rec'], 0.25) if n else mix(t['solo'], '#ff9a2e', 0.40),
                refused=t['rec'],
                channel=mix(t['base'], t['gutter'], 0.40),
                hole=t['edge'] if n else mix(t['well'], t['edge'], 0.6))


def width(kind, db):
    if kind == 'out':
        return 2.0
    if kind == 'send':
        return round(1.0 + 1.6 * max(0.0, min(1.0, (db + 36) / 36)), 2)
    return 1.5


def under(t, col):
    return mix(col, t['edge'], 0.55) if t['name'] == 'Night' else mix(col, t['ink1'], 0.28)


def bez(sx, tx, y, ty=None):
    """The hanging cable: P0 at the source jack, P1 straight below it, P2 straight below the
    target, P3 at the target jack. k grows with the span (a long cable sags more)."""
    ty = y if ty is None else ty
    k = min(58.0, 16 + 0.15 * abs(tx - sx))
    return f'M{sx:.1f} {y:.1f}C{sx:.1f} {y + k:.1f} {tx:.1f} {ty + k * (1 if ty == y else 0.55):.1f} {tx:.1f} {ty:.1f}'


def subway(sx, tx, y, ly, r=4):
    d = 1 if tx > sx else -1
    return (f'M{sx:.1f} {y:.1f}V{ly - r:.1f}Q{sx:.1f} {ly:.1f} {sx + d * r:.1f} {ly:.1f}'
            f'H{tx - d * r:.1f}Q{tx:.1f} {ly:.1f} {tx:.1f} {ly - r:.1f}V{y:.1f}')


def lanes(routes, pos):
    """C: one lane per kind; inside a kind, the shortest free sub-lane (interval colouring)."""
    base = {'out': 14, 'send': 26, 'key': 40}
    taken = {}
    out = {}
    for r in sorted(routes, key=lambda r: abs(pos[r[1]][0] - pos[r[0]][1])):
        a, b = sorted((pos[r[0]][1], pos[r[1]][0]))
        k = 0
        while any(not (b < x0 - 6 or a > x1 + 6) for x0, x1 in taken.get((r[2], k), [])):
            k += 1
        taken.setdefault((r[2], k), []).append((a, b))
        out[r] = base[r[2]] + 4 * k
    return out


def path_d(style, sx, tx, ys, route, ln):
    if style == 'C':
        return subway(sx, tx, ys, ys + ln[route])
    return bez(sx, tx, ys)


def cable(t, d, col, w, dash=False, op=1.0, pulse=None):
    da = ' stroke-dasharray="4 3"' if dash else ''
    o = f' opacity="{op}"' if op < 1 else ''
    s = (f'<g{o}><path d="{d}" fill="none" stroke="{under(t, col)}" stroke-width="{w + 2}" stroke-linecap="round"{da}/>'
         f'<path d="{d}" fill="none" stroke="{col}" stroke-width="{w}" stroke-linecap="round"{da}/>')
    if pulse is not None:      # B: a gleam every 64 px, its brightness by level
        lvl, off = pulse
        gl = mix(col, '#ffffff', 0.6) if t['name'] == 'Night' else mix(col, '#ffffff', 0.7)
        s += (f'<path d="{d}" fill="none" stroke="{gl}" stroke-width="{w}" stroke-linecap="round" '
              f'stroke-dasharray="7 57" stroke-dashoffset="{off}" opacity="{0.35 + 0.65 * lvl:.2f}"/>')
    return s + '</g>'


def socket(t, cx, cy, kind, col=None, halo=None, bad=False):
    """A jack: a dark hole with a ring; the output carries a dot in its hole. A connected jack
    takes its cable's colour; a valid drop target gets a halo; a refused one a red ring."""
    T = tok(t)
    ring = col or mix(t['ink4'], t['ink3'], 0.5)
    out = ''
    if halo:
        out += f'<circle cx="{cx}" cy="{cy}" r="8" fill="{halo}" opacity="0.22"/>'
    out += f'<circle cx="{cx}" cy="{cy}" r="4" fill="{T["hole"]}" stroke="{T["refused"] if bad else ring}" stroke-width="1.25"/>'
    if kind == 'out':
        out += f'<circle cx="{cx}" cy="{cy}" r="1.5" fill="{col or t["ink3"]}"/>'
    if bad:
        out += f'<path d="M{cx - 2} {cy - 2}L{cx + 2} {cy + 2}" stroke="{T["refused"]}" stroke-width="1.2"/>'
    return out


def plug(t, x, y, col):
    return (f'<circle cx="{x}" cy="{y}" r="3.5" fill="{col}" stroke="{under(t, col)}" stroke-width="1"/>'
            f'<circle cx="{x - 1}" cy="{y - 1}" r="1" fill="#ffffff" opacity="0.5"/>')


def pointer(t, x, y):
    return (f'<path d="M{x} {y}V{y + 13}L{x + 3.5} {y + 9.5}L{x + 6} {y + 15}L{x + 8} {y + 14}L{x + 5.5} {y + 8.5}H{x + 10}Z" '
            f'fill="#ffffff" stroke="#10151a" stroke-width="1" stroke-linejoin="round"/>')


def tip(t, x, y, s, col=None):
    w = tw(s, 10) + 14
    return (rect(x - w / 2, y, w, 18, t['raised'], 4) + rect(x - w / 2, y, w, 1, mix(t['raised'], t['ink1'], 0.08)) +
            text(x, y + 12.5, s, 10, col or t['ink1'], 500, 'middle'))
