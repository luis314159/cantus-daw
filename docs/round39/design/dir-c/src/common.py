"""Studio ink (round 39, dir-c): the shared drawing helpers, tokens and the renderer.

Everything is drawn in logical pixels (1280x720 for the window) and snapped to the
physical grid of the scale it is rendered at, so a hairline is one physical pixel at 1x
and at 1.5x alike (the kit would do the same). No shape here comes from the kit, an icon
set or FL Studio: every path is drawn for this sheet.
"""
import math
import os
import subprocess

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.normpath(os.path.join(HERE, ".."))
FONTS = "file:///F:/projects/daw-r39/fonts/ibm-plex/"
EDGE = r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"

SANS = "PlexC"   # IBM Plex Sans Condensed 1.3 (OFL 1.1)
MONO = "PlexM"   # IBM Plex Mono 2.3 (OFL 1.1)

FONT_CSS = "".join(
    f'@font-face{{font-family:{fam};src:url("{FONTS}{file}");font-weight:{w}}}'
    for fam, file, w in [
        (SANS, "IBMPlexSansCondensed-ExtraLight.ttf", 200),
        (SANS, "IBMPlexSansCondensed-Light.ttf", 300),
        (SANS, "IBMPlexSansCondensed-Regular.ttf", 400),
        (SANS, "IBMPlexSansCondensed-Medium.ttf", 500),
        (SANS, "IBMPlexSansCondensed-SemiBold.ttf", 600),
        (MONO, "IBMPlexMono-Light.ttf", 300),
        (MONO, "IBMPlexMono-Regular.ttf", 400),
        (MONO, "IBMPlexMono-Medium.ttf", 500),
    ]
)

# ---------------------------------------------------------------- colour maths

def hx(c):
    c = c.lstrip("#")
    return tuple(int(c[i:i + 2], 16) for i in (0, 2, 4))


def to_hex(rgb):
    return "#" + "".join(f"{max(0, min(255, round(v))):02X}" for v in rgb)


def mix(a, b, t):
    """a toward b by t (0 = a, 1 = b)."""
    A, B = hx(a), hx(b)
    return to_hex(tuple(A[i] + (B[i] - A[i]) * t for i in range(3)))


def lum(c):
    def ch(v):
        v /= 255
        return v / 12.92 if v <= 0.03928 else ((v + 0.055) / 1.055) ** 2.4
    r, g, b = hx(c)
    return 0.2126 * ch(r) + 0.7152 * ch(g) + 0.0722 * ch(b)


def contrast(a, b):
    la, lb = lum(a), lum(b)
    hi, lo = max(la, lb), min(la, lb)
    return (hi + 0.05) / (lo + 0.05)


def desat(c, t):
    """Pull a colour toward its own grey by t."""
    r, g, b = hx(c)
    y = 0.299 * r + 0.587 * g + 0.114 * b
    return to_hex((r + (y - r) * t, g + (y - g) * t, b + (y - b) * t))


# ---------------------------------------------------------------- the SVG builder

class Svg:
    """A list of SVG elements in logical pixels, snapped to the physical grid of `s`."""

    def __init__(self, w, h, s=1.0):
        self.w, self.h, self.s = w, h, s
        self.parts = []
        self.defs = []
        self._id = 0

    def uid(self, p="g"):
        self._id += 1
        return f"{p}{self._id}"

    # snapping: a coordinate lands on a physical pixel edge
    def sn(self, v):
        return round(v * self.s) / self.s

    @property
    def px(self):
        """One physical pixel, in logical units."""
        return 1.0 / self.s

    def add(self, el):
        self.parts.append(el)

    def rect(self, x, y, w, h, fill, rx=0, op=None, stroke=None, sw=None, snap=True, extra=""):
        if snap:
            x0, y0 = self.sn(x), self.sn(y)
            x1, y1 = self.sn(x + w), self.sn(y + h)
            if w > 0 and x1 == x0:
                x1 = x0 + self.px
            if h > 0 and y1 == y0:
                y1 = y0 + self.px
            x, y, w, h = x0, y0, x1 - x0, y1 - y0
        a = f'<rect x="{x:.3f}" y="{y:.3f}" width="{w:.3f}" height="{h:.3f}" fill="{fill}"'
        if rx:
            a += f' rx="{rx}"'
        if op is not None:
            a += f' opacity="{op}"'
        if stroke:
            a += f' stroke="{stroke}" stroke-width="{sw or self.px:.3f}"'
        self.add(a + f" {extra}/>")

    def box(self, x, y, w, h, fill, line, rx=0, lw=None):
        """A filled box with a crisp inner hairline border (drawn inside its bounds)."""
        lw = lw or self.px
        x0, y0, x1, y1 = self.sn(x), self.sn(y), self.sn(x + w), self.sn(y + h)
        if fill:
            self.add(f'<rect x="{x0:.3f}" y="{y0:.3f}" width="{x1 - x0:.3f}" height="{y1 - y0:.3f}" rx="{rx}" fill="{fill}"/>')
        if line:
            h2 = lw / 2
            self.add(
                f'<rect x="{x0 + h2:.3f}" y="{y0 + h2:.3f}" width="{x1 - x0 - lw:.3f}" height="{y1 - y0 - lw:.3f}"'
                f' rx="{max(0, rx - h2):.3f}" fill="none" stroke="{line}" stroke-width="{lw:.3f}"/>'
            )

    def hline(self, x1, x2, y, color, op=None, t=None):
        t = t or self.px
        self.rect(x1, y, x2 - x1, t, color, op=op)

    def vline(self, x, y1, y2, color, op=None, t=None):
        t = t or self.px
        self.rect(x, y1, t, y2 - y1, color, op=op)

    def path(self, d, fill="none", stroke=None, sw=None, op=None, cap="round", join="round", extra=""):
        a = f'<path d="{d}" fill="{fill}"'
        if stroke:
            a += f' stroke="{stroke}" stroke-width="{sw or self.px:.3f}" stroke-linecap="{cap}" stroke-linejoin="{join}"'
        if op is not None:
            a += f' opacity="{op}"'
        self.add(a + f" {extra}/>")

    def circle(self, cx, cy, r, fill="none", stroke=None, sw=None, op=None):
        a = f'<circle cx="{cx:.3f}" cy="{cy:.3f}" r="{r:.3f}" fill="{fill}"'
        if stroke:
            a += f' stroke="{stroke}" stroke-width="{sw or self.px:.3f}"'
        if op is not None:
            a += f' opacity="{op}"'
        self.add(a + "/>")

    def text(self, x, y, s, size=12, fill="#fff", w=400, fam=SANS, anchor="start", ls=0,
             op=None, tnum=True, caps=False, extra=""):
        s = str(s)
        if caps:
            s = s.upper()
        s = s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
        st = "font-feature-settings:'tnum' 1;" if tnum else ""
        a = (f'<text x="{x:.2f}" y="{y:.2f}" font-family="{fam}" font-size="{size}" font-weight="{w}"'
             f' fill="{fill}" text-anchor="{anchor}" style="{st}letter-spacing:{ls}px"')
        if op is not None:
            a += f' opacity="{op}"'
        self.add(a + f" {extra}>{s}</text>")

    def raw(self, s):
        self.add(s)

    def group(self, inner, transform="", extra=""):
        self.add(f'<g transform="{transform}" {extra}>' + "".join(inner) + "</g>")

    def svg(self):
        return (
            f'<svg xmlns="http://www.w3.org/2000/svg" width="{self.w}" height="{self.h}" '
            f'viewBox="0 0 {self.w} {self.h}" text-rendering="geometricPrecision">'
            f'<defs>{"".join(self.defs)}</defs>{"".join(self.parts)}</svg>'
        )


def page(body, bg, w, h, extra_css=""):
    return (
        "<!doctype html><html><head><meta charset='utf-8'><style>"
        + FONT_CSS
        + f"html,body{{margin:0;padding:0;background:{bg};width:{w}px;height:{h}px;overflow:hidden}}"
        + "svg{display:block}"
        + extra_css
        + "</style></head><body>"
        + body
        + "</body></html>"
    )


def render(html_path, png_path, w, h, scale):
    """Edge headless: w x h CSS pixels at `scale` device pixels per CSS pixel."""
    url = "file:///" + html_path.replace("\\", "/")
    cmd = [
        EDGE, "--headless=new", "--disable-gpu", "--hide-scrollbars", "--disable-lcd-text",
        f"--force-device-scale-factor={scale}", f"--window-size={w},{h}",
        "--virtual-time-budget=4000", f"--screenshot={png_path}", url,
    ]
    subprocess.run(cmd, check=True, capture_output=True, timeout=180)
    with open(png_path, "rb") as f:
        assert f.read(4) == b"\x89PNG", png_path


def write(path, text):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write(text)
