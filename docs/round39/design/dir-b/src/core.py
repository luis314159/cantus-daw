"""dir-b, Graphite and gold: the tokens, colour maths and a pixel-snapping SVG canvas.

Every sheet is drawn in *logical* units (the window is 1280x720) and emitted in physical
pixels at a scale `s` (1 or 1.5): rectangles and hairlines are rounded to the pixel grid at
that scale, so 1.5x is drawn for 1.5x, not blown up from 1x.
"""
import base64
import os
import re
import subprocess

EDGE = r"C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe"

FONT_DIR = r"F:/projects/daw-r39/fonts/sofia"
OUT_BUILD = r"F:/projects/daw-r39/tmp/dir-b"
HERE = os.path.dirname(os.path.abspath(__file__))
OUT_DOCS = os.path.normpath(os.path.join(HERE, ".."))

# ---- colour -------------------------------------------------------------------------


def hx(c):
    c = c.lstrip("#")
    return tuple(int(c[i:i + 2], 16) for i in (0, 2, 4))


def to_hex(t):
    return "#%02X%02X%02X" % tuple(max(0, min(255, round(v))) for v in t)


def mix(a, b, t):
    """`a` moved toward `b` by t (0..1)."""
    a, b = hx(a), hx(b)
    return to_hex(tuple(a[i] + (b[i] - a[i]) * t for i in range(3)))


def lum(c):
    def ch(v):
        v /= 255
        return v / 12.92 if v <= 0.03928 else ((v + 0.055) / 1.055) ** 2.4
    r, g, b = hx(c)
    return 0.2126 * ch(r) + 0.7152 * ch(g) + 0.0722 * ch(b)


def contrast(a, b):
    la, lb = sorted((lum(a), lum(b)), reverse=True)
    return (la + 0.05) / (lb + 0.05)


# ---- tokens -------------------------------------------------------------------------
# Graphite: R = G = B + 1 at most — neither brown nor blue. Gold from the logo's playhead
# (#E9B64C), teal from its patina (#3E8E7E), ember from its wave (#E07A2E).

NIGHT = dict(
    name="Night",
    well="#111111", canvas="#181818", lane_alt="#1C1C1B", panel="#1E1E1D",
    chrome="#242423", raised="#2B2B2A", hover="#323231",
    key="#1A1A19", key_edge="#0B0B0B", key_lip="#2E2E2D",
    line_soft="#262625", line="#323230", line_strong="#44433F",
    ink1="#ECEAE4", ink2="#B3AFA7", ink3="#8E8A83", ink4="#4B4A46",
    gold="#E6B450", gold_hi="#F5D58E", gold_ink="#1A1406", gold_tint="#E6B450",
    teal="#5CC2A9", red="#EE5A45", ember="#E5813F",
    meter_lo="#5CC2A9", meter_mid="#D9C25A", meter_hi="#EE5A45",
    shadow="#000000",
)

PAPER = dict(
    name="Paper",
    well="#FCFBF9", canvas="#F5F4F1", lane_alt="#EEEDE9", panel="#ECEAE6",
    chrome="#E4E2DD", raised="#FAF9F7", hover="#DCDAD4",
    key="#D9D6D0", key_edge="#BDB9B0", key_lip="#ECEAE5",
    line_soft="#E3E1DC", line="#D2CFC8", line_strong="#B4B0A6",
    ink1="#1D1C1A", ink2="#4A4741", ink3="#67635C", ink4="#B1ADA4",
    gold="#7C5307", gold_hi="#E3AE45", gold_ink="#1D1C1A", gold_tint="#D9A13A",
    teal="#136E5C", red="#CF3A27", ember="#B85A1C",
    meter_lo="#2E9C82", meter_mid="#C79A1E", meter_hi="#D23C2A",
    shadow="#5A5040",
)

# The user's track colours: lit only in a stripe and in the content; bodies muted.
HUES = dict(
    kick="#E3713B", clap="#E5A23E", hats="#CDBA4C", perc="#DC5A5C", bass="#9C7DE2",
    chords="#4EA8D8", pluck="#5BBE79", lead="#D968B2", choir="#7092DB", drums="#C98C52",
)


def clip_tones(t, hue):
    """(body, name band, content, edge) for a clip of `hue` in theme `t`."""
    if t["name"] == "Night":
        return (mix(t["canvas"], hue, 0.17), mix(t["canvas"], hue, 0.30),
                mix(hue, "#FFFFFF", 0.22), mix(t["canvas"], hue, 0.42))
    return (mix(t["canvas"], hue, 0.15), mix(t["canvas"], hue, 0.27),
            mix(hue, "#000000", 0.48), mix(t["canvas"], hue, 0.50))


# ---- type ---------------------------------------------------------------------------

UI = "SofiaSemi"      # Sofia Sans Semi Condensed — labels, names, values
NARROW = "SofiaCond"  # Sofia Sans Condensed — strip names, dense places
CLOCK = "SofiaXC"     # Sofia Sans Extra Condensed — the clock, the tempo
WIDE = "Sofia"        # Sofia Sans — headings on sheets

_FACES = [(UI, "SofiaSansSemiCondensed[wght].ttf"), (NARROW, "SofiaSansCondensed[wght].ttf"),
          (CLOCK, "SofiaSansExtraCondensed[wght].ttf"), (WIDE, "SofiaSans[wght].ttf")]


def font_css():
    out = []
    for fam, fn in _FACES:
        with open(os.path.join(FONT_DIR, fn), "rb") as f:
            b = base64.b64encode(f.read()).decode()
        out.append("@font-face{font-family:'%s';src:url(data:font/ttf;base64,%s) "
                   "format('truetype');font-weight:1 1000;}" % (fam, b))
    return "\n".join(out)


def esc(s):
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


# ---- canvas -------------------------------------------------------------------------


class Canvas:
    """SVG in logical units, emitted in physical pixels at scale `s`."""

    def __init__(self, w, h, s=1.0, t=NIGHT):
        self.w, self.h, self.s, self.t = w, h, s, t
        self.parts = []
        self.defs = []
        self._id = 0

    # physical rounding
    def p(self, v):
        return round(v * self.s)

    def uid(self, stem="g"):
        self._id += 1
        return "%s%d" % (stem, self._id)

    def add(self, s):
        self.parts.append(s)

    def rect(self, x, y, w, h, fill, rx=0, stroke=None, sw=1, op=None, extra=""):
        x0, y0, x1, y1 = self.p(x), self.p(y), self.p(x + w), self.p(y + h)
        a = ' fill-opacity="%.3f"' % op if op is not None else ""
        r = ' rx="%.2f"' % (rx * self.s) if rx else ""
        if stroke:
            # a 1-px (physical) border inside the box, on half pixels
            k = max(1, round(sw * self.s)) if sw >= 1 else sw * self.s
            hk = k / 2
            self.add('<rect x="%.1f" y="%.1f" width="%.1f" height="%.1f"%s fill="%s"%s '
                     'stroke="%s" stroke-width="%.2f"%s/>' % (
                         x0 + hk, y0 + hk, x1 - x0 - k, y1 - y0 - k, r,
                         fill or "none", a, stroke, k, extra))
        else:
            self.add('<rect x="%d" y="%d" width="%d" height="%d"%s fill="%s"%s%s/>' % (
                x0, y0, x1 - x0, y1 - y0, r, fill, a, extra))

    def hline(self, x0, x1, y, color, w=1, op=None):
        k = max(1, round(w * self.s))
        yy = self.p(y) + k / 2
        a = ' stroke-opacity="%.3f"' % op if op is not None else ""
        self.add('<line x1="%d" y1="%.1f" x2="%d" y2="%.1f" stroke="%s" stroke-width="%d"%s/>'
                 % (self.p(x0), yy, self.p(x1), yy, color, k, a))

    def vline(self, x, y0, y1, color, w=1, op=None):
        k = max(1, round(w * self.s))
        xx = self.p(x) + k / 2
        a = ' stroke-opacity="%.3f"' % op if op is not None else ""
        self.add('<line x1="%.1f" y1="%d" x2="%.1f" y2="%d" stroke="%s" stroke-width="%d"%s/>'
                 % (xx, self.p(y0), xx, self.p(y1), color, k, a))

    def text(self, x, y, s, size=12, fill=None, fam=UI, wt=500, anchor="start", ls=0,
             op=None, caps=False, tnum=False, extra=""):
        fill = fill or self.t["ink1"]
        st = "font-family:'%s';font-size:%.2fpx;font-weight:%d;" % (fam, size * self.s, wt)
        if ls:
            st += "letter-spacing:%.2fpx;" % (ls * self.s)
        feats = []
        if tnum:
            feats.append("'tnum' 1")
        if caps:
            feats.append("'c2sc' 1, 'smcp' 1")
        if feats:
            st += "font-feature-settings:%s;" % ",".join(feats)
        a = ' fill-opacity="%.3f"' % op if op is not None else ""
        self.add('<text x="%.2f" y="%.2f" style="%s" fill="%s" text-anchor="%s"%s%s>%s</text>'
                 % (x * self.s, y * self.s, st, fill, anchor, a, extra, esc(s)))

    def path(self, d, fill="none", stroke=None, sw=1.0, op=None, cap="round", join="round",
             extra=""):
        """`d` in logical units; numbers are scaled here."""
        st = ' stroke="%s" stroke-width="%.2f" stroke-linecap="%s" stroke-linejoin="%s"' % (
            stroke, sw * self.s, cap, join) if stroke else ""
        a = ' fill-opacity="%.3f"' % op if op is not None and fill != "none" else ""
        if op is not None and stroke:
            a += ' stroke-opacity="%.3f"' % op
        self.add('<path d="%s" fill="%s"%s%s%s/>' % (self.sd(d), fill, st, a, extra))

    def sd(self, d):
        # no arc commands in our paths: every number is a coordinate
        s = self.s
        return re.sub(r"-?\d*\.?\d+", lambda m: "%.2f" % (float(m.group()) * s), d)

    def circle(self, cx, cy, r, fill="none", stroke=None, sw=1.0, op=None):
        st = ' stroke="%s" stroke-width="%.2f"' % (stroke, sw * self.s) if stroke else ""
        a = ' opacity="%.3f"' % op if op is not None else ""
        self.add('<circle cx="%.2f" cy="%.2f" r="%.2f" fill="%s"%s%s/>' % (
            cx * self.s, cy * self.s, r * self.s, fill, st, a))

    def lingrad(self, stops, x1=0, y1=0, x2=0, y2=1):
        i = self.uid("lg")
        st = "".join('<stop offset="%.3f" stop-color="%s" stop-opacity="%.3f"/>' % s
                     for s in stops)
        self.defs.append('<linearGradient id="%s" x1="%s" y1="%s" x2="%s" y2="%s">%s'
                         '</linearGradient>' % (i, x1, y1, x2, y2, st))
        return "url(#%s)" % i

    def group(self, inner, extra=""):
        self.add("<g %s>%s</g>" % (extra, inner))

    def svg(self):
        W, H = self.p(self.w), self.p(self.h)
        return ('<svg xmlns="http://www.w3.org/2000/svg" width="%d" height="%d" '
                'viewBox="0 0 %d %d" text-rendering="geometricPrecision">'
                '<defs>%s</defs>%s</svg>' % (W, H, W, H, "".join(self.defs), "".join(self.parts)))


def page(svg, bg="#000"):
    return ('<!doctype html><html><head><meta charset="utf-8"><style>%s\n'
            'html,body{margin:0;padding:0;background:%s;}svg{display:block}</style></head>'
            '<body>%s</body></html>' % (font_css(), bg, svg))


def write_build(name, html):
    os.makedirs(OUT_BUILD, exist_ok=True)
    p = os.path.join(OUT_BUILD, name + ".html")
    with open(p, "w", encoding="utf-8") as f:
        f.write(html)
    return p


def render(name, html, w, h, out_png):
    """Write the page and screenshot it with Edge headless at device scale 1 (the SVG is
    already in physical pixels)."""
    src = write_build(name, html)
    out_png = os.path.abspath(out_png)
    os.makedirs(os.path.dirname(out_png), exist_ok=True)
    prof = os.path.join(OUT_BUILD, "edge-profile")
    cmd = [EDGE, "--headless=new", "--disable-gpu", "--hide-scrollbars",
           "--force-device-scale-factor=1", "--user-data-dir=" + prof,
           "--virtual-time-budget=3000", "--window-size=%d,%d" % (w, h),
           "--screenshot=" + out_png, "file:///" + src.replace("\\", "/")]
    subprocess.run(cmd, check=True, capture_output=True, timeout=120)
    return out_png
