"""Slate (dir-a; copied unchanged from r39/dir-a d280cd13 as des-views' foundation) — tokens, colour helpers and SVG primitives for the design sheets.

Everything here is a design source: it emits SVG text that Edge renders to PNG. Colours are
resolved to literal hex per theme, so every sheet is deterministic.
"""

FONT_DIR = "file:///F:/projects/daw-r39/fonts/barlow-semi-condensed/"
FACE = "Slate Sans"

# ---------------------------------------------------------------- colour helpers

def rgb(h):
    h = h.lstrip('#')
    return tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))


def hexc(c):
    return '#%02x%02x%02x' % tuple(max(0, min(255, round(v))) for v in c)


def mix(a, b, t):
    """a → b by t (0 = a, 1 = b)."""
    A, B = rgb(a), rgb(b)
    return hexc([A[i] + (B[i] - A[i]) * t for i in range(3)])


def lum(h):
    def ch(v):
        v /= 255
        return v / 12.92 if v <= 0.04045 else ((v + 0.055) / 1.055) ** 2.4
    r, g, b = rgb(h)
    return 0.2126 * ch(r) + 0.7152 * ch(g) + 0.0722 * ch(b)


def contrast(a, b):
    la, lb = sorted((lum(a), lum(b)), reverse=True)
    return (la + 0.05) / (lb + 0.05)


# ---------------------------------------------------------------- the two themes

NIGHT = dict(
    name='Night',
    gutter='#14191e',     # between panels: value, not a border
    base='#1d2329',       # panel body, arrangement lanes
    lane2='#20272d',      # alternate bars in the grid
    surface='#262d34',    # bars, headers, strips
    raised='#2b3239',     # popovers, lifted pills
    well='#1a1f25',       # inside of a key or field: darker than its bar
    well_hi='#20262d',    # a key under the pointer
    edge='#11151a',       # the 1-px dark edge of a well
    lip='#2e363e',        # the 1-px light lip under a well
    line='#323b44',       # hairlines (used sparingly)
    grid_bar='#2a3239',
    grid_beat='#22292f',
    ink1='#e3e8ed',
    ink2='#b4bdc6',
    ink3='#909ca8',       # 4.5:1 or more on every surface it sits on
    ink4='#47515b',
    accent='#5cafe9',
    accent_ink='#0d1822',
    play='#5dce8a',
    rec='#ee5c5a',
    solo='#ebb64d',
    clip='#ff5d50',
    auto='#b48df4',
    meter_lo='#5dce8a',
    meter_mid='#c9d36a',
    meter_hi='#ebb64d',
    shadow='rgba(0,0,0,0.55)',
    tint_rest=0.19, tint_sel=0.30, note_rest=0.62, note_sel=0.86, stripe_rest=0.72,
    head_sel=0.13,
)

PAPER = dict(
    name='Paper',
    gutter='#c5cdd5',
    base='#f4f6f8',
    lane2='#eef1f4',
    surface='#e4e9ee',
    raised='#fbfcfd',
    well='#d6dde3',
    well_hi='#dde3e8',
    edge='#b9c3cc',
    lip='#eef2f5',
    line='#c8d0d8',
    grid_bar='#d9dfe5',
    grid_beat='#e8ecf0',
    ink1='#17202a',
    ink2='#38444f',
    ink3='#556170',       # 4.5:1 or more on every surface it sits on
    ink4='#a9b3bd',
    accent='#1b64b2',
    accent_ink='#ffffff',
    play='#1f8f55',
    rec='#d33b3b',
    solo='#a86e06',
    clip='#d6322b',
    auto='#7a4fd6',
    meter_lo='#2fa868',
    meter_mid='#9aa62c',
    meter_hi='#c98a12',
    shadow='rgba(40,55,70,0.22)',
    tint_rest=0.17, tint_sel=0.30, note_rest=0.70, note_sel=0.92, stripe_rest=0.70,
    head_sel=0.14,
)

# The user's colours (the song's), one per track, as the user picked them. Each theme
# spends them at its own strength: a stripe and a tint at rest, lit when selected.
HUES = {
    'Kick': '#e8794f', 'Clap': '#e6a54a', 'Hats': '#d2c25c', 'Percussion': '#e0606f',
    'Bass': '#9a80ee', 'Chords': '#3fc0c8', 'Pluck': '#5fc584', 'Lead': '#e06aa8',
    'Choir': '#6c93ea', 'Drum bus': '#9aa6b2', 'Reverb': '#86a9c8', 'Delay': '#93b5a3',
}


def hue_for(t, name, k):
    """The user's colour spent at strength k against the theme's lanes."""
    h = HUES[name]
    if t['name'] == 'Paper':
        # on paper the colour must darken to read: pull it toward ink before tinting
        h = mix(h, '#20303f', 0.18)
    return mix(t['base'], h, k)


def note_ink(t, name, sel=False):
    h = HUES[name]
    if t['name'] == 'Paper':
        h = mix(h, '#14202c', 0.42 if not sel else 0.30)
        return mix(t['base'], h, t['note_sel'] if sel else t['note_rest'])
    if sel:
        return mix(h, '#ffffff', 0.22)
    return mix(t['base'], h, t['note_rest'])


# ---------------------------------------------------------------- SVG primitives

def esc(s):
    return s.replace('&', '&amp;').replace('<', '&lt;').replace('>', '&gt;')


def rect(x, y, w, h, fill, r=0, extra=''):
    rr = f' rx="{r}"' if r else ''
    return f'<rect x="{x}" y="{y}" width="{w}" height="{h}"{rr} fill="{fill}"{extra}/>'


def hline(x1, x2, y, col, w=1):
    return f'<path d="M{x1} {y + 0.5}H{x2}" stroke="{col}" stroke-width="{w}"/>'


def vline(x, y1, y2, col, w=1):
    return f'<path d="M{x + 0.5} {y1}V{y2}" stroke="{col}" stroke-width="{w}"/>'


def text(x, y, s, size=12, col='#fff', weight=400, anchor='start', extra='', track=0):
    ls = f' letter-spacing="{track}"' if track else ''
    return (f'<text x="{x}" y="{y}" font-size="{size}" font-weight="{weight}" fill="{col}"'
            f' text-anchor="{anchor}"{ls}{extra}>{esc(s)}</text>')


def well(t, x, y, w, h, r=4, hi=False, fill=None):
    """A key or field: a flat well a shade darker than its bar, a 1-px dark edge on top
    and a 1-px light lip at the bottom — recessed, never a glossy cap."""
    f = fill or (t['well_hi'] if hi else t['well'])
    out = [rect(x, y, w, h, t['lip'], r)]                 # lip shows 1 px under
    out.append(rect(x, y, w, h - 1, t['edge'], r))        # edge
    out.append(rect(x + 1, y + 1, w - 2, h - 2, f, max(0, r - 1)))
    return ''.join(out)


def font_css():
    faces = [(200, 'ExtraLight'), (300, 'Light'), (400, 'Regular'), (500, 'Medium'),
             (600, 'SemiBold'), (700, 'Bold')]
    css = ''.join(
        f"@font-face{{font-family:'{FACE}';font-weight:{w};"
        f"src:url({FONT_DIR}BarlowSemiCondensed-{n}.ttf)}}" for w, n in faces)
    return css + (f"svg text{{font-family:'{FACE}';font-variant-numeric:tabular-nums;"
                  "font-kerning:normal}")


def page(svg, w, h, bg='#000', extra_css=''):
    return (f'<!doctype html><html><head><meta charset="utf-8"><style>{font_css()}'
            f'html,body{{margin:0;padding:0;background:{bg};width:{w}px;height:{h}px;'
            f'overflow:hidden}}svg{{display:block}}{extra_css}</style></head><body>'
            f'{svg}</body></html>')


# ---------------------------------------------------------------- text width (estimate)
_NARROW = set("iljtfr.,:;'!|·I ")
_WIDE = set("mwMW%")


def tw(s, size, weight=400):
    """A close estimate of Barlow Semi Condensed's advance, for layout only."""
    w = 0
    for ch in s:
        if ch in _NARROW:
            w += 0.25 if ch != ' ' else 0.22
        elif ch in _WIDE:
            w += 0.74
        elif ch.isdigit():
            w += 0.50
        elif ch.isupper():
            w += 0.56
        else:
            w += 0.5
    return w * size * (1.0 + (weight - 400) / 2000)


# layout of the 1280 x 720 logical window
TOP_H = 44
INSP_X = 1034
ARR_Y = 46
TOOL_H = 28
RULER_Y = ARR_Y + TOOL_H          # 74
RULER_H = 24
TRK_Y = RULER_Y + RULER_H         # 98
ROW_H = 30
HEAD_W = 176
BAR_W = 44
FIRST_BAR = 37
MIX_Y = 378
MIX_HEAD = 28
STRIP_Y = MIX_Y + MIX_HEAD        # 406
STATUS_Y = 700


def lit_edge(t, x, y, w):
    """The 1-px lit top edge of a bar: light from above, the only gradient Slate keeps."""
    c = mix(t['surface'], t['ink1'], 0.07) if t['name'] == 'Night' else '#f7f9fb'
    return rect(x, y, w, 1, c)


def defs(t):
    """Gradients for the few objects that carry relief: a knob's cap, a fader's cap, the
    strips' faint light from above."""
    n = t['name'] == 'Night'
    k = (mix(t['raised'], t['ink1'], 0.16), t['raised'], mix(t['raised'], t['edge'], 0.35)) if n \
        else ('#ffffff', '#f6f8fa', '#dfe5ea')
    f = ('#4b5560', '#3a434d', '#323a43') if n else ('#ffffff', '#f3f6f8', '#e1e6eb')
    s = (mix(t['surface'], t['ink1'], 0.035), t['surface']) if n else ('#e9edf1', t['surface'])
    return ('<defs>'
            f'<radialGradient id="kcap-{t["name"]}" cx="38%" cy="30%" r="80%"><stop offset="0" stop-color="{k[0]}"/>'
            f'<stop offset="0.55" stop-color="{k[1]}"/><stop offset="1" stop-color="{k[2]}"/></radialGradient>'
            f'<linearGradient id="fcap-{t["name"]}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{f[0]}"/>'
            f'<stop offset="0.5" stop-color="{f[1]}"/><stop offset="1" stop-color="{f[2]}"/></linearGradient>'
            f'<linearGradient id="strip-{t["name"]}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{s[0]}"/>'
            f'<stop offset="0.35" stop-color="{s[1]}"/></linearGradient>'
            '</defs>')
