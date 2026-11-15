"""SVG primitives for the dancer sheets. Every shape is opaque (ADR 0090 §2), lines are
round-capped strokes here and **ribbons** in the rig (one fill each); a shape's `cut` is
the rig's outline (the shape grown, filled first) in the room's colour."""
import math


def f(v):
    return f"{v:.4f}".rstrip("0").rstrip(".")


def pts(points):
    return " ".join(f"{f(x)},{f(y)}" for x, y in points)


def line_d(points):
    return "M" + " L".join(f"{f(x)},{f(y)}" for x, y in points)


def curve_d(points, closed=False):
    """A smooth line through the points (Catmull-Rom, as `spline_into`)."""
    n = len(points)
    if n < 3:
        return line_d(points)
    P = list(points)
    d = f"M{f(P[0][0])},{f(P[0][1])}"
    rng = range(n) if closed else range(n - 1)
    for i in rng:
        p0 = P[(i - 1) % n] if (closed or i > 0) else P[0]
        p1 = P[i]
        p2 = P[(i + 1) % n]
        p3 = P[(i + 2) % n] if (closed or i + 2 < n) else P[-1]
        c1 = (p1[0] + (p2[0] - p0[0]) / 6, p1[1] + (p2[1] - p0[1]) / 6)
        c2 = (p2[0] - (p3[0] - p1[0]) / 6, p2[1] - (p3[1] - p1[1]) / 6)
        d += f" C{f(c1[0])},{f(c1[1])} {f(c2[0])},{f(c2[1])} {f(p2[0])},{f(p2[1])}"
    return d + (" Z" if closed else "")


def tube_d(points, widths):
    """A tapered tube along a polyline, round at both ends (as `tube_into`/`limb_into`)."""
    n = len(points)
    L, R = [], []
    for i in range(n):
        a = points[max(i - 1, 0)]
        b = points[min(i + 1, n - 1)]
        dx, dy = b[0] - a[0], b[1] - a[1]
        ln = math.hypot(dx, dy) or 1e-6
        nx, ny = -dy / ln, dx / ln
        h = widths[i] / 2
        L.append((points[i][0] + nx * h, points[i][1] + ny * h))
        R.append((points[i][0] - nx * h, points[i][1] - ny * h))
    h0, h1 = widths[0] / 2, widths[-1] / 2
    d = f"M{f(L[0][0])},{f(L[0][1])}"
    for p in L[1:]:
        d += f" L{f(p[0])},{f(p[1])}"
    d += f" A{f(h1)},{f(h1)} 0 0 0 {f(R[-1][0])},{f(R[-1][1])}"
    for p in reversed(R[:-1]):
        d += f" L{f(p[0])},{f(p[1])}"
    d += f" A{f(h0)},{f(h0)} 0 0 0 {f(L[0][0])},{f(L[0][1])} Z"
    return d


def bezier_pts(a, c, b, n=10):
    out = []
    for i in range(n + 1):
        t = i / n
        u = 1 - t
        out.append((u * u * a[0] + 2 * u * t * c[0] + t * t * b[0],
                    u * u * a[1] + 2 * u * t * c[1] + t * t * b[1]))
    return out


def rrect_d(cx, cy, w, h, r, deg=0.0):
    """A rounded rectangle centred on (cx, cy), turned by `deg`."""
    r = min(r, w / 2, h / 2)
    x0, y0 = -w / 2, -h / 2
    k = 0.5523 * r
    P = []
    d = (f"M{f(x0 + r)},{f(y0)} H{f(x0 + w - r)} C{f(x0 + w - r + k)},{f(y0)} {f(x0 + w)},{f(y0 + r - k)} {f(x0 + w)},{f(y0 + r)}"
         f" V{f(y0 + h - r)} C{f(x0 + w)},{f(y0 + h - r + k)} {f(x0 + w - r + k)},{f(y0 + h)} {f(x0 + w - r)},{f(y0 + h)}"
         f" H{f(x0 + r)} C{f(x0 + r - k)},{f(y0 + h)} {f(x0)},{f(y0 + h - r + k)} {f(x0)},{f(y0 + h - r)}"
         f" V{f(y0 + r)} C{f(x0)},{f(y0 + r - k)} {f(x0 + r - k)},{f(y0)} {f(x0 + r)},{f(y0)} Z")
    return d, f"translate({f(cx)},{f(cy)}) rotate({f(deg)})"


class Doc:
    """Collects SVG elements; `cut` draws a shape grown by `c` in the room's colour first
    (the rig's outline-as-a-fill), so a later piece separates from an earlier one."""

    ids = 0

    def __init__(self, cut_color=None, cut=0.0):
        self.out = []
        self.cut_color = cut_color
        self.cut = cut

    def raw(self, s):
        self.out.append(s)

    def path(self, d, fill, cut=True, transform=None, extra=""):
        tr = f' transform="{transform}"' if transform else ""
        if cut and self.cut_color and self.cut > 0:
            self.out.append(f'<path d="{d}"{tr} fill="{self.cut_color}" stroke="{self.cut_color}" '
                            f'stroke-width="{f(self.cut * 2)}" stroke-linejoin="round"/>')
        self.out.append(f'<path d="{d}"{tr} fill="{fill}"{extra}/>')

    def line(self, points, width, color, cut=True, smooth=False, cap="round", extra=""):
        d = curve_d(points) if smooth else line_d(points)
        if cut and self.cut_color and self.cut > 0:
            self.out.append(f'<path d="{d}" fill="none" stroke="{self.cut_color}" '
                            f'stroke-width="{f(width + self.cut * 2)}" stroke-linecap="{cap}" stroke-linejoin="round"/>')
        self.out.append(f'<path d="{d}" fill="none" stroke="{color}" stroke-width="{f(width)}" '
                        f'stroke-linecap="{cap}" stroke-linejoin="round"{extra}/>')

    def circle(self, c, r, fill, cut=True):
        if cut and self.cut_color and self.cut > 0:
            self.out.append(f'<circle cx="{f(c[0])}" cy="{f(c[1])}" r="{f(r + self.cut)}" fill="{self.cut_color}"/>')
        self.out.append(f'<circle cx="{f(c[0])}" cy="{f(c[1])}" r="{f(r)}" fill="{fill}"/>')

    def ellipse(self, c, rx, ry, fill, cut=False, deg=0.0, extra=""):
        tr = f' transform="rotate({f(deg)} {f(c[0])} {f(c[1])})"' if deg else ""
        if cut and self.cut_color and self.cut > 0:
            self.out.append(f'<ellipse cx="{f(c[0])}" cy="{f(c[1])}" rx="{f(rx + self.cut)}" ry="{f(ry + self.cut)}"{tr} fill="{self.cut_color}"/>')
        self.out.append(f'<ellipse cx="{f(c[0])}" cy="{f(c[1])}" rx="{f(rx)}" ry="{f(ry)}"{tr} fill="{fill}"{extra}/>')

    def cel(self, d, base, shade, off=(-0.010, -0.012), cut=True, transform=None):
        """A piece in two values (the rig's hard cel, ADR 0105): the shade, then the base
        moved toward the key light (upper left) and clipped to the piece, leaving a band of
        shade along the lower right."""
        Doc.ids += 1
        cid = f"c{Doc.ids}"
        tr = f' transform="{transform}"' if transform else ""
        moved = f"translate({f(off[0])},{f(off[1])})" + (f" {transform}" if transform else "")
        self.path(d, shade, cut=cut, transform=transform)
        self.out.append(f'<clipPath id="{cid}"><path d="{d}"{tr}/></clipPath>'
                        f'<g clip-path="url(#{cid})"><path d="{d}" fill="{base}" transform="{moved}"/></g>')

    def svg(self):
        return "\n".join(self.out)


def wrap(inner, x, y, scale):
    """Place a character drawn in its units at (x, y) on the ground, `scale` px a unit."""
    return f'<g transform="translate({f(x)},{f(y)}) scale({f(scale)})">{inner}</g>'
