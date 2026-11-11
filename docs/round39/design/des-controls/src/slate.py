"""des-controls — Slate's tokens and the drawing primitives of the control sheets.

The tokens are Slate's (dir-a, chosen by the owner 2026-11-09) with the few this sheet adds:
`mod` (a modulator drives the value), `scrim` (behind a dialog), the button fills and the
focus ring. Everything is drawn in *physical* pixels: each control takes `s` (1 or 1.5)
and looks up its own measurements per scale, so 150 % is drawn for itself, not scaled.
"""

FONT_DIR = "file:///F:/projects/daw-r39/fonts/barlow-semi-condensed/"
FACE = "Slate Sans"


def rgb(h):
    h = h.lstrip('#')
    return tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))


def hexc(c):
    return '#%02x%02x%02x' % tuple(max(0, min(255, round(v))) for v in c)


def mix(a, b, k):
    A, B = rgb(a), rgb(b)
    return hexc([A[i] + (B[i] - A[i]) * k for i in range(3)])


def lum(h):
    def ch(v):
        v /= 255
        return v / 12.92 if v <= 0.04045 else ((v + 0.055) / 1.055) ** 2.4
    r, g, b = rgb(h)
    return 0.2126 * ch(r) + 0.7152 * ch(g) + 0.0722 * ch(b)


def contrast(a, b):
    la, lb = sorted((lum(a), lum(b)), reverse=True)
    return (la + 0.05) / (lb + 0.05)


NIGHT = dict(
    name='Night', gutter='#14191e', base='#1d2329', surface='#262d34', raised='#2f373f',
    well='#1a1f25', well_hi='#222930', edge='#11151a', lip='#2e363e', line='#323b44',
    ink1='#e3e8ed', ink2='#a6b0ba', ink3='#7a8692', ink4='#47515b',
    accent='#5cafe9', accent_ink='#0d1822',
    play='#5dce8a', rec='#ee5c5a', solo='#ebb64d', clip='#ff5d50', auto='#b48df4',
    meter_lo='#5dce8a', meter_mid='#c9d36a', meter_hi='#ebb64d',
    # added by des-controls
    mod='#ee86c8',            # a modulator drives the value (orchid: apart from auto's violet)
    scrim='rgba(8,11,14,0.62)',
    cap_hi='#4a545e', cap_mid='#353e47', cap_lo='#272e35',   # knob caps, lit from above
    fcap_hi='#8a95a1', fcap_mid='#66707b', fcap_lo='#525b65', # fader and slider caps: the strip's brightest neutral
    danger='#c8443f', danger_ink='#ffffff',
    primary_ink='#0d1822',
    shadow_op=0.50,
)

PAPER = dict(
    name='Paper', gutter='#c5cdd5', base='#f4f6f8', surface='#e4e9ee', raised='#fbfcfd',
    well='#d6dde3', well_hi='#ccd4db', edge='#b9c3cc', lip='#eef2f5', line='#c8d0d8',
    ink1='#17202a', ink2='#4a5867', ink3='#6a7684', ink4='#a9b3bd',
    accent='#1f6ec2', accent_ink='#ffffff',
    play='#1f8f55', rec='#d33b3b', solo='#a86e06', clip='#d6322b', auto='#7a4fd6',
    meter_lo='#2fa868', meter_mid='#9aa62c', meter_hi='#c98a12',
    mod='#b83a8a',
    scrim='rgba(34,46,58,0.34)',
    cap_hi='#ffffff', cap_mid='#f5f7f9', cap_lo='#dde3e9',
    fcap_hi='#ffffff', fcap_mid='#f3f6f8', fcap_lo='#dae1e7',
    danger='#c8352f', danger_ink='#ffffff',
    primary_ink='#ffffff',
    shadow_op=0.18,
)

THEMES = {'night': NIGHT, 'paper': PAPER}


def esc(s):
    return s.replace('&', '&amp;').replace('<', '&lt;').replace('>', '&gt;')


def f(v):
    """A coordinate, short."""
    return ('%.2f' % v).rstrip('0').rstrip('.')


def rect(x, y, w, h, fill, r=0, extra=''):
    rr = f' rx="{f(r)}"' if r else ''
    return f'<rect x="{f(x)}" y="{f(y)}" width="{f(w)}" height="{f(h)}"{rr} fill="{fill}"{extra}/>'


def frame(x, y, w, h, col, r=0, sw=1, extra=''):
    """A stroked rectangle whose 1-px line sits on whole pixels."""
    o = sw / 2
    rr = f' rx="{f(max(0, r - o))}"' if r else ''
    return (f'<rect x="{f(x + o)}" y="{f(y + o)}" width="{f(w - sw)}" height="{f(h - sw)}"{rr} '
            f'fill="none" stroke="{col}" stroke-width="{f(sw)}"{extra}/>')


def line(x1, y1, x2, y2, col, sw=1, cap='round', extra=''):
    return (f'<path d="M{f(x1)} {f(y1)}L{f(x2)} {f(y2)}" stroke="{col}" stroke-width="{f(sw)}" '
            f'stroke-linecap="{cap}" fill="none"{extra}/>')


def path(d, col, sw=1, fill='none', extra=''):
    return (f'<path d="{d}" stroke="{col}" stroke-width="{f(sw)}" fill="{fill}" '
            f'stroke-linecap="round" stroke-linejoin="round"{extra}/>')


def circle(cx, cy, r, fill, extra=''):
    return f'<circle cx="{f(cx)}" cy="{f(cy)}" r="{f(r)}" fill="{fill}"{extra}/>'


def text(x, y, s, size=12, col='#fff', weight=400, anchor='start', extra=''):
    return (f'<text x="{f(x)}" y="{f(y)}" font-size="{f(size)}" font-weight="{weight}" '
            f'fill="{col}" text-anchor="{anchor}"{extra}>{esc(s)}</text>')


def well(t, x, y, w, h, r=4, fill=None):
    """Slate's recessed well: a 1-px dark edge, the fill, a 1-px light lip under it."""
    fl = fill or t['well']
    return (rect(x, y, w, h, t['lip'], r) + rect(x, y, w, h - 1, t['edge'], r)
            + rect(x + 1, y + 1, w - 2, h - 2, fl, max(0, r - 1)))


def popover(t, x, y, w, h, r=6, fid='pop'):
    """A raised sheet over the workspace: one soft shadow, a 1-px edge."""
    edge = mix(t['raised'], t['ink1'], 0.09) if t['name'] == 'Night' else t['line']
    return (f'<rect x="{f(x)}" y="{f(y)}" width="{f(w)}" height="{f(h)}" rx="{f(r)}" '
            f'fill="{t["raised"]}" filter="url(#{fid}-{t["name"]})"/>' + frame(x, y, w, h, edge, r))


_NARROW = set("iljtfr.,:;'!|·I ()[]")
_WIDE = set("mwMW%")


def tw(s, size, weight=400):
    """A close estimate of Barlow Semi Condensed's advance (layout only)."""
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


def defs(t):
    """Gradients and the popover shadow, one set per theme."""
    n = t['name']
    so = t['shadow_op']
    return ('<defs>'
            f'<radialGradient id="cap-{n}" cx="40%" cy="28%" r="78%">'
            f'<stop offset="0" stop-color="{t["cap_hi"]}"/><stop offset="0.55" stop-color="{t["cap_mid"]}"/>'
            f'<stop offset="1" stop-color="{t["cap_lo"]}"/></radialGradient>'
            f'<linearGradient id="fcap-{n}" x1="0" y1="0" x2="0" y2="1">'
            f'<stop offset="0" stop-color="{t["fcap_hi"]}"/><stop offset="0.5" stop-color="{t["fcap_mid"]}"/>'
            f'<stop offset="1" stop-color="{t["fcap_lo"]}"/></linearGradient>'
            f'<filter id="pop-{n}" x="-30%" y="-30%" width="160%" height="170%">'
            f'<feDropShadow dx="0" dy="6" stdDeviation="9" flood-color="#000" flood-opacity="{so}"/></filter>'
            f'<filter id="tip-{n}" x="-30%" y="-40%" width="160%" height="200%">'
            f'<feDropShadow dx="0" dy="3" stdDeviation="5" flood-color="#000" flood-opacity="{so * 0.9}"/></filter>'
            f'<filter id="dlg-{n}" x="-20%" y="-20%" width="140%" height="150%">'
            f'<feDropShadow dx="0" dy="14" stdDeviation="22" flood-color="#000" flood-opacity="{so * 1.1}"/></filter>'
            '</defs>')


def font_css():
    faces = [(200, 'ExtraLight'), (300, 'Light'), (400, 'Regular'), (500, 'Medium'),
             (600, 'SemiBold'), (700, 'Bold')]
    css = ''.join(
        f"@font-face{{font-family:'{FACE}';font-weight:{w};"
        f"src:url({FONT_DIR}BarlowSemiCondensed-{nm}.ttf)}}" for w, nm in faces)
    return css + (f"svg text{{font-family:'{FACE}';font-variant-numeric:tabular-nums;"
                  "font-kerning:normal}")


def page(svg, w, h, bg):
    return (f'<!doctype html><html><head><meta charset="utf-8"><style>{font_css()}'
            f'html,body{{margin:0;padding:0;background:{bg};width:{w}px;height:{h}px;'
            f'overflow:hidden}}svg{{display:block}}</style></head><body>{svg}</body></html>')


class Sheet:
    """A sheet drawn at scale `s`: layout in logical units, P() turns them to pixels."""

    def __init__(self, t, s, w, h, title, sub):
        self.t, self.s, self.w, self.h = t, s, w, h
        self.parts = []
        self.title, self.sub = title, sub

    def P(self, v):
        return round(v * self.s)

    def add(self, svg):
        self.parts.append(svg)

    def head(self, x, y, s, sub=''):
        t, k = self.t, self.s
        self.add(text(self.P(x), self.P(y), s, 15 * k, t['ink1'], 500))
        if sub:
            self.add(text(self.P(x) + tw(s, 15 * k, 500) + self.P(14), self.P(y), sub, 11 * k, t['ink3']))

    def label(self, x, y, s, anchor='start', col=None, size=11, weight=400):
        self.add(text(self.P(x), self.P(y), s, size * self.s, col or self.t['ink3'], weight, anchor))

    def html(self):
        t, k = self.t, self.s
        W, H = self.P(self.w), self.P(self.h)
        top = (rect(0, 0, W, H, t['base'])
               + text(self.P(24), self.P(34), self.title, 20 * k, t['ink1'], 500)
               + text(self.P(24), self.P(52), self.sub, 11 * k, t['ink3']))
        svg = (f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" '
               f'viewBox="0 0 {W} {H}">{defs(t)}{top}{"".join(self.parts)}</svg>')
        return page(svg, W, H, t['base'])
