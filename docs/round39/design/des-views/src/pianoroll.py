"""des-views (round 39, Slate) — the piano roll, whole: its head, the ruler, the keyboard,
the note grid, notes (at rest, selected, under the pointer, muted, ghosts of other clips and
tracks) and the velocity lane.

The window shows Neon Tide's Chords track, clip "Stabs" (bars 41-48), from bar 40.3 so the
neighbouring clip ("Pads") shows as ghosts in the darkened outside.
"""
import glyphs as G
import keys as K
from base import rect, text, well, mix, tw, lit_edge, HUES
from topbar import joined, key

HUE = HUES['Chords']
PROG = [(57, 60, 64), (53, 57, 60), (48, 52, 55), (55, 59, 62)]   # Am F C G, one a bar

# layout of the 1280 x 720 window (logical px)
HEAD_Y, HEAD_H = 46, 28
RUL_Y, RUL_H = 74, 24
GRID_Y = 98
KW = 72                     # the keyboard column
RH = 12                     # one row (pitch)
TOP, NROWS = 77, 40         # F5 down to D2
GRID_B = GRID_Y + RH * NROWS            # 578
GX0, GX1 = KW, 1268         # the grid; a 12-px vertical scroll at its right
HS_Y = GRID_B                           # horizontal scroll, 8 px
SPL_Y = HS_Y + 8                        # the splitter, 6 px
VEL_Y = SPL_Y + 6                       # 592
VEL_B = 698
PPB = 56                    # pixels per beat
T0 = (40.5 - 1) * 4         # view starts at bar 40, beat 3
CLIP = ((41 - 1) * 4, (49 - 1) * 4)


def bx(T):
    return GX0 + (T - T0) * PPB


def row_y(p, top=TOP, y0=GRID_Y, rh=RH):
    return y0 + (top - p) * rh


# ------------------------------------------------------------------ the song's notes
def stabs():
    """(T start, length, pitch, velocity, flags) — flags: 'sel', 'mute', 'hover'."""
    out = []
    rhythm = ((0, 0.5), (0.75, 0.5), (1.5, 0.75), (2.5, 0.5), (3.25, 0.5))
    for bar in range(41, 49):
        ch = PROG[(bar - 1) % 4]
        for k, (q, ln) in enumerate(rhythm):
            for j, p in enumerate(ch + (ch[0] + 12,)):
                v = (0.96 if k == 0 else 0.80) - 0.05 * ((k + bar) % 3)   # a programmed stab: one velocity a chord
                flags = ''
                if bar == 42 and k in (2, 3):
                    flags = 'sel'
                if bar == 44 and k == 4:
                    flags = 'mute'
                if bar == 43 and k == 1 and j == 1:
                    flags = 'hover'
                out.append(((bar - 1) * 4 + q, ln, p, v, flags))
    return out


def ghosts():
    """Other clips and tracks shown under the grid: the neighbouring Pads clip (this track),
    the Bass and the Lead (other tracks, chosen in the head's Ghosts menu)."""
    g = []
    # Pads, bar 40 (G chord, whole bar) — this track's previous clip
    for p in (55, 59, 62, 67):
        g.append(((40 - 1) * 4, 4, p, 'own'))
    for bar in range(40, 47):
        ch = PROG[(bar - 1) % 4]
        root = ch[0] - 24 + (12 if ch[0] < 50 else 0)
        if bar >= 41:
            for q, ln, dp in ((0, 0.75, 0), (1, 0.5, 0), (1.5, 0.5, 12), (2.5, 0.5, 0), (3, 0.5, 7), (3.5, 0.5, 0)):
                g.append(((bar - 1) * 4 + q, ln, root + dp, 'other'))
    phrase = [[(0, 1.5, 76), (1.5, 0.5, 74), (2, 1, 72), (3, 1, 74)],
              [(0, 2.5, 72), (3, 0.5, 69), (3.5, 0.5, 71)],
              [(0, 1, 72), (1, 1, 76), (2, 1.5, 79), (3.5, 0.5, 77)],
              [(0, 3, 76), (3, 0.5, 74)]]
    for bar in range(41, 47):
        for q, ln, p in phrase[(bar - 41) % 4]:
            g.append(((bar - 1) * 4 + q, ln, p, 'other'))
    return g


# ------------------------------------------------------------------ colours of notes
def note_cols(t, v, flags, hue=HUE):
    """fill, edge, label ink for one note."""
    night = t['name'] == 'Night'
    h = hue if night else mix(hue, '#14202c', 0.38)
    if flags == 'mute':
        return mix(t['base'], h, 0.10), mix(t['base'], h, 0.55), t['ink3']
    if flags == 'sel':
        f = mix(h, '#ffffff', 0.30) if night else mix(t['base'], h, 1.0)
        return f, t['ink1'], ('#0b1a20' if night else '#ffffff')
    k = (0.50 + 0.22 * v) if night else (0.58 + 0.30 * v)
    f = mix(t['base'], h, k)
    return f, mix(f, t['gutter'], 0.55 if night else 0.35), ('#0b1a20' if night else '#ffffff')


def note(t, x, y, w, h, v, flags, label=None, hue=HUE):
    fill, edge, ink = note_cols(t, v, flags, hue)
    out = []
    if flags == 'mute':
        out.append(rect(x + 0.5, y + 0.5, w - 1, h - 1, fill, 2, f' stroke="{edge}" stroke-width="1"'))
    else:
        out.append(rect(x, y, w, h, edge, 2))
        out.append(rect(x + 1, y + 1, w - 2, h - 2, fill, 1.5))
        # the attack: the note's first pixel column a step brighter, so repeated notes part
        out.append(rect(x + 1, y + 1, 1, h - 2, mix(fill, '#ffffff', 0.35 if t['name'] == 'Night' else 0.0) if t['name'] == 'Night' else mix(fill, '#000000', 0.18)))
        if flags == 'sel':
            out.append(rect(x - 0.5, y - 0.5, w + 1, h + 1, 'none', 2.5, f' stroke="{t["ink1"]}" stroke-width="1"'))
        if flags == 'hover':
            out.append(rect(x - 0.5, y - 0.5, w + 1, h + 1, 'none', 2.5, f' stroke="{mix(fill, "#ffffff", 0.6)}" stroke-width="1"'))
    if label and w >= 20 and h >= 10:
        out.append(text(x + 4, y + h / 2 + 3.2, label, 9, ink if flags != 'mute' else t['ink3'], 600))
    return ''.join(out)


# ------------------------------------------------------------------ the grid
def row_fills(t):
    """White-key rows sit on the panel's base; black-key rows a step down, toward the gutter."""
    night = t['name'] == 'Night'
    return t['base'], mix(t['base'], t['gutter'], 0.62 if night else 0.30)


def grid(t, x0, x1, y0, rh, top, n, t0, ppb, clip=None, snap=0.25, hover_row=None, uid='g'):
    """The note grid: rows by key colour, lines for bars, beats and the snap, the octave
    line under each C. clip=(T0, T1) darkens what lies outside the edited clip."""
    white, black = row_fills(t)
    night = t['name'] == 'Night'
    out = [f'<clipPath id="{uid}"><rect x="{x0}" y="{y0}" width="{x1 - x0}" height="{n * rh}"/></clipPath>',
           f'<g clip-path="url(#{uid})">', rect(x0, y0, x1 - x0, n * rh, white)]
    for i in range(n):
        p = top - i
        y = y0 + i * rh
        if K.is_black(p):
            out.append(rect(x0, y, x1 - x0, rh, black))
        if p == hover_row:
            out.append(rect(x0, y, x1 - x0, rh, t['accent'], 0, ' fill-opacity="0.07"'))
        if p % 12 == 0:      # the octave: a line under every C
            out.append(rect(x0, y + rh - 1, x1 - x0, 1, mix(t['grid_bar'], t['ink4'], 0.35 if night else 0.2)))
        elif p % 12 == 5:    # E|F, the other seam between two white rows
            out.append(rect(x0, y + rh - 1, x1 - x0, 1, mix(white, t['grid_bar'], 0.55)))
    # vertical lines: snap, beat, bar
    T = t0 - (t0 % snap)
    while bx_any(T, x0, t0, ppb) <= x1:
        x = round(bx_any(T, x0, t0, ppb))
        if x >= x0:
            if abs(T % 4) < 1e-6:
                out.append(rect(x, y0, 1, n * rh, mix(t['grid_bar'], t['ink4'], 0.45 if night else 0.35)))
            elif abs(T % 1) < 1e-6:
                out.append(rect(x, y0, 1, n * rh, t['grid_bar']))
            elif ppb * snap >= 6:
                out.append(rect(x, y0, 1, n * rh, mix(white, t['grid_beat'], 0.9) if night else t['grid_beat']))
        T += snap
    out.append('</g>')
    return ''.join(out)


def bx_any(T, x0, t0, ppb):
    return x0 + (T - t0) * ppb


def outside(t, x0, x1, y0, h, clip, t0, ppb):
    """Outside the edited clip: the grid stays, a veil of the gutter over it."""
    out = ''
    a = bx_any(clip[0], x0, t0, ppb)
    b = bx_any(clip[1], x0, t0, ppb)
    veil = ' fill-opacity="0.55"' if t['name'] == 'Night' else ' fill-opacity="0.42"'
    if a > x0:
        out += rect(x0, y0, min(a, x1) - x0, h, t['gutter'], 0, veil)
    if b < x1:
        out += rect(max(b, x0), y0, x1 - max(b, x0), h, t['gutter'], 0, veil)
    return out


def ghost_note(t, x, y, w, h, kind):
    night = t['name'] == 'Night'
    if kind == 'own':      # this track's other clip: its hue, faint
        c = mix(t['base'], HUE if night else mix(HUE, '#14202c', 0.38), 0.20 if night else 0.22)
    else:                  # another track: no hue at all, a soft slate shape
        c = mix(t['base'], t['ink3'], 0.24 if night else 0.22)
    return rect(x, y + 1, max(2, w - 1), h - 2, c, 2)


def notes_layer(t, x0, x1, y0, rh, top, n, t0, ppb, uid='nl', show_ghosts=True):
    out = [f'<clipPath id="{uid}"><rect x="{x0}" y="{y0}" width="{x1 - x0}" height="{n * rh}"/></clipPath>',
           f'<g clip-path="url(#{uid})">']
    if show_ghosts:
        for T, ln, p, kind in ghosts():
            if top - n < p <= top:
                x = bx_any(T, x0, t0, ppb)
                out.append(ghost_note(t, x, y0 + (top - p) * rh, ln * ppb, rh, kind))
    for T, ln, p, v, fl in stabs():
        if top - n < p <= top:
            x = bx_any(T, x0, t0, ppb)
            out.append(note(t, round(x) + 1, y0 + (top - p) * rh, round(ln * ppb) - 1, rh, v, fl, K.name(p)))
    out.append('</g>')
    return ''.join(out)


# ------------------------------------------------------------------ head, ruler, lane
def dot_toggle(t, x, y, label, on, w=None, h=20):
    """A word that switches: a well with a small lamp and the word — the lamp is the state."""
    w = w or round(tw(label, 11) + 26)
    out = well(t, x, y, w, h, 4)
    c = t['accent'] if on else mix(t['well'], t['ink3'], 0.55)
    out += f'<circle cx="{x + 10}" cy="{y + h / 2}" r="2.5" fill="{c}"/>'
    out += text(x + 18, y + h / 2 + 4, label, 11, t['ink1'] if on else t['ink2'])
    return out, w


def head(t, y=HEAD_Y, W=1280, tool=1):
    h = HEAD_H
    out = [rect(0, y, W, h, t['surface']), lit_edge(t, 0, y, W)]
    out.append(text(12, y + 18, 'Piano roll', 13, t['ink1'], 500))
    # the clip being edited: the track's colour, its name, the clip — a menu of the track's clips
    x = 86
    out.append(well(t, x, y + 4, 128, 20, 4))
    out.append(rect(x + 7, y + 10, 3, 8, HUE, 1))
    out.append(text(x + 15, y + 18, 'Chords', 12, t['ink1'], 500))
    cx = x + 15 + tw('Chords', 12, 500) + 5
    out.append(G.chevron_right(cx + 2, y + 14, 2.5, t['ink3'], 1))
    out.append(text(cx + 9, y + 18, 'Stabs', 12, t['ink2']))
    out.append(G.chevron_down(x + 128 - 11, y + 14, 2.5, t['ink3'], 1.2))
    # the tools: a radio set, the chosen one a lifted neutral pill
    tools = [G.select, G.draw, G.slice_, G.erase, G.mute_tool, G.zoom]
    kx, ky, kw, kh = 228, y + 4, 26, 20
    out.append(joined(t, kx, ky, [kw] * len(tools), kh))
    out.append(rect(kx + tool * kw + 2, ky + 2, kw - 4, kh - 4, t['raised'], 3))
    for i, g in enumerate(tools):
        out.append(g(kx + i * kw + kw / 2, ky + kh / 2, 20, t['ink1'] if i == tool else t['ink2'], t['ink3']))
    gx = kx + kw * len(tools) + 16
    out.append(text(gx, y + 18, 'Grid', 11, t['ink3']))
    out.append(well(t, gx + 26, ky, 54, kh))
    out.append(text(gx + 34, ky + 14, '1/16', 11, t['ink1']))
    out.append(G.chevron_down(gx + 26 + 54 - 10, ky + 10, 2.5, t['ink3'], 1.2))
    sx = gx + 26 + 54 + 6
    out.append(key(t, sx, ky, 26, kh, on=True))
    out.append(G.magnet(sx + 13, ky + kh / 2, 20, t['accent'], mix(t['accent'], t['well'], 0.4)))
    # ghosts and scale: words that switch, what they show beside them in ink 3
    gx2 = sx + 26 + 16
    s, w = dot_toggle(t, gx2, ky, 'Ghosts', True)
    out.append(s)
    out.append(text(gx2 + w + 7, y + 18, 'Pads · Bass · Lead', 11, t['ink3']))
    sc = gx2 + w + 7 + tw('Pads · Bass · Lead', 11) + 16
    s, w = dot_toggle(t, sc, ky, 'Scale', False)
    out.append(s)
    out.append(text(sc + w + 7, y + 18, 'A minor', 11, t['ink3']))
    # right: what is selected, said once
    r = W - 12
    out.append(text(r, y + 18, 'F3 – F4  ·  velocity 89 – 102', 11, t['ink3'], 400, 'end'))
    out.append(text(r - tw('F3 – F4  ·  velocity 89 – 102', 11) - 8, y + 18, '8 notes selected', 11, t['ink1'], 500, 'end'))
    return ''.join(out)


def ruler(t, y=RUL_Y, x0=GX0, x1=GX1, t0=T0, ppb=PPB, head_T=None):
    h = RUL_H
    out = [rect(0, y, 1280, h, t['surface'])]
    # the keyboard column's corner: the zoom of the rows, as quiet words
    out.append(text(12, y + 16, 'Rows', 10, t['ink3']))
    out.append(well(t, 38, y + 3, 26, 18, 3))
    out.append(text(51, y + 15.5, '12', 10, t['ink2'], 500, 'middle'))
    out.append(f'<clipPath id="ruc"><rect x="{x0}" y="{y}" width="{x1 - x0}" height="{h}"/></clipPath><g clip-path="url(#ruc)">')
    # the clip's own span: a band in its colour along the foot, the outside veiled
    a, b = bx_any(CLIP[0], x0, t0, ppb), bx_any(CLIP[1], x0, t0, ppb)
    out.append(rect(a, y + h - 2, b - a, 2, mix(t['surface'], HUE, 0.75 if t['name'] == 'Night' else 0.85)))
    out.append(outside(t, x0, x1, y, h, CLIP, t0, ppb))
    # the loop: a quiet accent band on the numbers' half
    la, lb = bx_any(CLIP[0], x0, t0, ppb), bx_any(CLIP[1], x0, t0, ppb)
    # upper tier: the loop as a quiet accent band, the section's name in it
    out.append(rect(la, y + 2, lb - la, 10, mix(t['surface'], t['accent'], 0.13)))
    out.append(rect(la, y + 2, 1, 10, t['accent']))
    out.append(text(la + 5, y + 10, 'Drop', 9, t['ink2'], 600))
    # lower tier: bar numbers strong, beats quiet, sixteenths as ticks
    T = t0 - (t0 % 1)
    while bx_any(T, x0, t0, ppb) <= x1:
        x = round(bx_any(T, x0, t0, ppb))
        bar, beat = int(T // 4) + 1, int(T % 4) + 1
        if beat == 1:
            out.append(rect(x, y + 13, 1, 9, t['ink3']))
            out.append(text(x + 6, y + 21, str(bar), 10, t['ink1'], 600))
        else:
            out.append(rect(x, y + 16, 1, 6, t['ink4']))
            out.append(text(x + 4, y + 21, f'{bar}.{beat}', 9, t['ink3']))
        for q in (0.25, 0.5, 0.75):
            xq = round(bx_any(T + q, x0, t0, ppb))
            out.append(rect(xq, y + 19, 1, 3, mix(t['surface'], t['ink4'], 0.6)))
        T += 1
    if head_T is not None:
        px = bx_any(head_T, x0, t0, ppb)
        out.append(f'<path d="M{px - 4} {y + 14}H{px + 5}V{y + 18}L{px + 0.5} {y + 22.5}L{px - 4} {y + 18}Z" fill="{t["accent"]}"/>')
    out.append('</g>')
    return ''.join(out)


def vel_lane(t, y0=VEL_Y, y1=VEL_B, x0=GX0, x1=GX1, t0=T0, ppb=PPB):
    """Velocity: each note a stem in its colour from the floor, a short cap at its value;
    the note's length a hairline from the cap. Chords stack their caps; the selection lit."""
    night = t['name'] == 'Night'
    out = [rect(0, y0, KW, y1 - y0, t['surface']), rect(x0, y0, x1 - x0, y1 - y0, t['base'])]
    # the lane's head in the keyboard column: what it shows, a menu
    out.append(text(10, y0 + 17, 'Velocity', 11, t['ink1'], 500))
    out.append(G.chevron_down(10 + tw('Velocity', 11, 500) + 8, y0 + 13, 2.5, t['ink3'], 1.2))
    top, bot = y0 + 26, y1 - 6
    for val, lab in ((1.0, '127'), (0.5, '64')):
        yy = round(bot - (bot - top) * val)
        out.append(text(KW - 6, yy + 3.5, lab, 9, t['ink3'], 400, 'end'))
        out.append(rect(x0, yy, x1 - x0, 1, mix(t['base'], t['grid_bar'], 0.6)))
    out.append(f'<clipPath id="vlc"><rect x="{x0}" y="{y0}" width="{x1 - x0}" height="{y1 - y0}"/></clipPath><g clip-path="url(#vlc)">')
    T = t0 - (t0 % 1)
    while bx_any(T, x0, t0, ppb) <= x1:
        x = round(bx_any(T, x0, t0, ppb))
        out.append(rect(x, y0, 1, y1 - y0, mix(t['grid_bar'], t['ink4'], 0.45) if T % 4 == 0 else t['grid_beat']))
        T += 1
    out.append(outside(t, x0, x1, y0, y1 - y0, CLIP, t0, ppb))
    order = sorted(stabs(), key=lambda n: (n[4] == 'sel', n[4] == 'hover'))
    for T, ln, p, v, fl in order:
        x = round(bx_any(T, x0, t0, ppb)) + 1
        yy = round(bot - (bot - top) * v * 0.98)
        fill, edge, _ = note_cols(t, v, fl)
        stem = fill if fl != 'mute' else edge
        if fl == 'mute':
            out.append(rect(x + 0.5, yy, 0.01, bot - yy, 'none', 0, f' stroke="{stem}" stroke-width="1" stroke-dasharray="2 2"'))
            out.append(rect(x - 1.5, yy - 0.5, 5, 2, 'none', 1, f' stroke="{stem}" stroke-width="1"'))
            continue
        w = 2 if fl == 'sel' else 1
        out.append(rect(x, yy, w, bot - yy, stem))
        out.append(rect(x - 2, yy - 1, w + 5, 2, t['ink1'] if fl == 'sel' else stem, 1))
    out.append('</g>')
    return ''.join(out)


def scrolls(t):
    out = [rect(0, HS_Y, 1280, 8, t['surface'])]
    out.append(rect(GX0 + 210, HS_Y + 2, 280, 4, mix(t['surface'], t['ink3'], 0.55), 2))
    # the splitter: the gutter's colour, a grip in its middle
    out.append(rect(0, SPL_Y, 1280, 6, t['gutter']))
    for k in range(5):
        out.append(rect(1280 / 2 - 10 + k * 5, SPL_Y + 2, 2, 2, t['ink4'], 1))
    # vertical scroll beside the grid, its thumb where F5..D2 sits on the whole keyboard
    out.append(rect(GX1, GRID_Y, 1280 - GX1, GRID_B - GRID_Y, t['surface']))
    out.append(rect(GX1 + 4, GRID_Y + 170, 4, 120, mix(t['surface'], t['ink3'], 0.55), 2))
    return ''.join(out)


def status(t, hint):
    y = 700
    out = rect(0, y, 1280, 20, t['surface']) + lit_edge(t, 0, y, 1280)
    out += text(12, y + 14, hint[0], 11, t['ink1'])
    out += text(12 + tw(hint[0], 11) + 10, y + 14, hint[1], 11, t['ink3'])
    out += text(1280 - 12, y + 14, '48 kHz · 256 samples · 5.3 ms', 11, t['ink3'], 400, 'end')
    return out


def window(t, state):
    import topbar
    hover = 52
    out = [rect(0, 0, 1280, 720, t['gutter']), topbar.topbar(t, state), head(t), ruler(t, head_T=state.get('head_T', CLIP[0]))]
    out.append(grid(t, GX0, GX1, GRID_Y, RH, TOP, NROWS, T0, PPB, hover_row=hover))
    out.append(outside(t, GX0, GX1, GRID_Y, RH * NROWS, CLIP, T0, PPB))
    out.append(notes_layer(t, GX0, GX1, GRID_Y, RH, TOP, NROWS, T0, PPB))
    hp = state.get('head_T', CLIP[0])
    out.append(rect(round(bx(hp)), GRID_Y, 1, RH * NROWS, t['accent']))
    out.append(K.keyboard(t, 0, GRID_Y, KW, RH, TOP, NROWS, lit={hover: 'hover'}, uid='kbw'))
    out.append(scrolls(t))
    out.append(vel_lane(t))
    out.append(status(t, ('E3 · 43.1.4 · 1/8 · velocity 89', 'drag to move · drag its end to resize · Alt-drag to copy · double-click to delete')))
    return ''.join(out)
