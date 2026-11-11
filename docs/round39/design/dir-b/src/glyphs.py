"""dir-b glyphs, drawn for the physical size they are shown at.

Grammar ("engraved graphite"):
- A glyph sits in a box of side P physical pixels — about a third of its key for the
  transport, about 45 % for tools and windows — with air all round.
- Solid parts are filled in ink-2; the edge that faces the light (top, top-left) is a
  1-px line of ink-1 — light from above on a dark engraving. Below 10 px the edge is
  dropped (it would be noise) and the body takes ink-1.
- Strokes: 1 px under P 12, 1.5 px from 12, 2 px from 18; ends round, joins round.
- On: the body turns gold and the edge gold-hi. Off: ink-3, no edge.
- Every vertex snaps to the pixel grid at its size (fills to whole pixels, odd strokes to
  half pixels); small sizes have their own simpler drawing (gear, metronome, windows).
"""


class Pen:
    def __init__(self, c, cx, cy, size, ink1, ink2, on=False, off=False):
        self.c = c
        s = c.s
        P = max(5, round(size * s))
        self.P = P
        self.x0 = round(cx * s - P / 2)
        self.y0 = round(cy * s - P / 2)
        t = c.t
        if on:
            ink1, ink2 = t["gold_hi"], t["gold"]
        if off:
            ink1 = ink2 = t["ink3"]
        self.small = P < 10
        self.i1 = ink1                            # the lit edge
        self.i2 = ink1 if self.small else ink2    # the body (small: ink-1, no edge)
        self.edge = not self.small and not off
        self.sw = 1.0 if P < 12 else (1.5 if P < 18 else 2.0)

    # snapping
    def X(self, n):
        return self.x0 + round(n * self.P)

    def Y(self, n):
        return self.y0 + round(n * self.P)

    def _h(self, v):
        if self.sw == 1.0:
            return round(v - 0.5) + 0.5      # odd width: on half pixels
        if self.sw == 2.0:
            return float(round(v))           # even width: on whole pixels
        return round(v * 2) / 2

    def Xh(self, n):
        return self._h(self.x0 + n * self.P)

    def Yh(self, n):
        return self._h(self.y0 + n * self.P)

    def raw(self, s):
        self.c.add(s)

    def poly(self, pts, fill=None, op=None):
        fill = fill or self.i2
        d = "M" + " L".join("%d %d" % (self.X(x), self.Y(y)) for x, y in pts) + "Z"
        a = ' fill-opacity="%.2f"' % op if op else ""
        self.raw('<path d="%s" fill="%s"%s/>' % (d, fill, a))

    def line(self, pts, color=None, sw=None, op=None, cap="round"):
        color = color or self.i2
        sw = sw or self.sw
        d = "M" + " L".join("%.1f %.1f" % (self.Xh(x), self.Yh(y)) for x, y in pts)
        a = ' stroke-opacity="%.2f"' % op if op else ""
        self.raw('<path d="%s" fill="none" stroke="%s" stroke-width="%.1f" '
                 'stroke-linecap="%s" stroke-linejoin="round"%s/>' % (d, color, sw, cap, a))

    def curve(self, d_norm, color=None, sw=None, fill="none"):
        """A path with normalised numbers (M/L/C/Q/Z only)."""
        import re
        color = color or self.i2
        sw = sw or self.sw
        toks = re.findall(r"[MLCQZ]|-?\d*\.?\d+", d_norm)
        out, axis = [], 0
        for tk in toks:
            if tk in "MLCQZ":
                out.append(tk)
                axis = 0
            else:
                v = float(tk)
                out.append("%.2f" % (self.x0 + v * self.P if axis == 0 else self.y0 + v * self.P))
                axis ^= 1
        st = ' stroke="%s" stroke-width="%.1f" stroke-linecap="round" stroke-linejoin="round"' % (
            color, sw) if color != "none" else ""
        self.raw('<path d="%s" fill="%s"%s/>' % (" ".join(out), fill, st))

    def dot(self, nx, ny, nr, fill=None, stroke=None, sw=None):
        fill = fill or self.i2
        cx, cy = self.x0 + nx * self.P, self.y0 + ny * self.P
        r = max(1.0, nr * self.P)
        st = ' stroke="%s" stroke-width="%.1f"' % (stroke, sw or self.sw) if stroke else ""
        self.raw('<circle cx="%.2f" cy="%.2f" r="%.2f" fill="%s"%s/>' % (cx, cy, r, fill, st))

    def box(self, x0, y0, x1, y1, fill=None, rx=0.0):
        fill = fill or self.i2
        X0, Y0, X1, Y1 = self.X(x0), self.Y(y0), self.X(x1), self.Y(y1)
        r = ' rx="%.1f"' % rx if rx else ""
        self.raw('<rect x="%d" y="%d" width="%d" height="%d"%s fill="%s"/>' % (
            X0, Y0, max(1, X1 - X0), max(1, Y1 - Y0), r, fill))
        return X0, Y0, X1, Y1

    def top_edge(self, X0, X1, Y):
        if self.edge:
            self.raw('<rect x="%d" y="%d" width="%d" height="1" fill="%s"/>' % (
                X0, Y, X1 - X0, self.i1))


# ---- transport ----------------------------------------------------------------------

def play(p):
    p.poly([(0.14, 0.0), (0.14, 1.0), (0.98, 0.5)])
    if p.edge:  # the upper facet catches the light
        p.raw('<path d="M%d %d L%d %d" stroke="%s" stroke-width="1" stroke-linecap="round"/>'
              % (p.X(0.14) + 0.5, p.Y(0.0) + 0.5, p.X(0.98) - 1, p.Y(0.5), p.i1))


def pause(p):
    a = p.box(0.12, 0.0, 0.40, 1.0)
    b = p.box(0.60, 0.0, 0.88, 1.0)
    p.top_edge(a[0], a[2], a[1])
    p.top_edge(b[0], b[2], b[1])


def stop(p):
    a = p.box(0.12, 0.12, 0.88, 0.88, rx=max(0.5, p.P * 0.06))
    p.top_edge(a[0] + 1, a[2] - 1, a[1])


def record(p, lit=False):
    t = p.c.t
    col = t["red"] if lit else _mix(t["red"], t["ink3"], 0.30)
    p.dot(0.5, 0.5, 0.46, fill=col)
    if lit:
        p.dot(0.5, 0.5, 0.46, fill="none", stroke=t["red"], sw=1)
    elif p.edge:
        p.raw('<path d="M%.1f %.1f Q%.1f %.1f %.1f %.1f" fill="none" stroke="%s" '
              'stroke-opacity="0.55" stroke-width="1" stroke-linecap="round"/>' % (
                  p.x0 + p.P * 0.22, p.y0 + p.P * 0.36, p.x0 + p.P * 0.5, p.y0 + p.P * 0.02,
                  p.x0 + p.P * 0.78, p.y0 + p.P * 0.36, _mix(col, "#FFFFFF", 0.45)))


def to_start(p):
    a = p.box(0.04, 0.0, 0.18, 1.0)
    p.top_edge(a[0], a[2], a[1])
    p.poly([(0.96, 0.0), (0.96, 1.0), (0.24, 0.5)])
    if p.edge:
        p.raw('<path d="M%d %d L%d %d" stroke="%s" stroke-width="1" stroke-linecap="round"/>'
              % (p.X(0.96) - 0.5, p.Y(0.0) + 0.5, p.X(0.26) + 1, p.Y(0.5), p.i1))


def loop(p):
    # a rounded loop, open at the top right, one arrow riding it
    p.curve("M0.62 0.18 L0.30 0.18 C0.10 0.18 0.02 0.30 0.02 0.50 C0.02 0.70 0.12 0.82 "
            "0.32 0.82 L0.68 0.82 C0.88 0.82 0.98 0.70 0.98 0.50 C0.98 0.36 0.93 0.27 0.84 0.22")
    p.poly([(0.56, 0.0), (0.80, 0.18), (0.56, 0.36)], fill=p.i1 if p.edge else p.i2)


def metronome(p):
    if p.P < 12:
        p.line([(0.20, 0.96), (0.38, 0.06), (0.62, 0.06), (0.80, 0.96), (0.20, 0.96)])
        p.line([(0.5, 0.80), (0.80, 0.18)], color=p.i1)
        return
    p.line([(0.16, 0.96), (0.36, 0.04), (0.64, 0.04), (0.84, 0.96), (0.16, 0.96)])
    p.line([(0.24, 0.70), (0.76, 0.70)], sw=1)
    p.line([(0.5, 0.70), (0.84, 0.10)], color=p.i1)
    p.dot(0.70, 0.35, 0.09, fill=p.i1)


def follow(p):
    p.line([(0.30, 0.0), (0.30, 1.0)], color=p.i1)
    p.line([(0.48, 0.5), (0.96, 0.5)])
    p.poly([(0.72, 0.26), (1.0, 0.5), (0.72, 0.74)])


# ---- edit ---------------------------------------------------------------------------

def magnet(p):
    w = 0.22 if p.P >= 12 else 0.26
    p.curve("M0.14 0.06 L0.14 0.52 C0.14 0.80 0.30 0.96 0.50 0.96 C0.70 0.96 0.86 0.80 "
            "0.86 0.52 L0.86 0.06", sw=max(p.sw, p.P * w * 0.55))
    # pole tips, the lit faces
    p.box(0.02, 0.0, 0.27, 0.22, fill=p.i1 if p.edge else p.i2)
    p.box(0.73, 0.0, 0.98, 0.22, fill=p.i1 if p.edge else p.i2)


def undo(p, mirror=False):
    m = (lambda x: 1 - x) if mirror else (lambda x: x)
    d = "M%.2f 0.86 C%.2f 0.48 %.2f 0.34 %.2f 0.34 L%.2f 0.34" % (
        m(0.90), m(0.90), m(0.70), m(0.50), m(0.30))
    p.curve(d)
    p.poly([(m(0.02), 0.34), (m(0.36), 0.04), (m(0.36), 0.64)], fill=p.i1 if p.edge else p.i2)


def redo(p):
    undo(p, mirror=True)


def select(p):
    p.poly([(0.20, 0.0), (0.20, 0.86), (0.40, 0.68), (0.56, 1.0), (0.70, 0.93),
            (0.55, 0.62), (0.82, 0.62)])
    if p.edge:
        p.raw('<path d="M%.1f %.1f L%.1f %.1f" stroke="%s" stroke-width="1"/>' % (
            p.X(0.20) + 0.5, p.Y(0.0) + 1, p.X(0.20) + 0.5, p.Y(0.80), p.i1))


def draw(p):
    # a pencil at 45°: body outline, a lit ferrule, a filled tip
    p.curve("M0.70 0.06 L0.94 0.30 L0.34 0.90 L0.10 0.66 Z")
    p.line([(0.58, 0.18), (0.82, 0.42)], color=p.i1, sw=1)
    p.poly([(0.10, 0.66), (0.34, 0.90), (0.02, 0.98)])


def slice_(p):
    p.curve("M0.92 0.04 L0.40 0.56 L0.56 0.72 C0.80 0.52 0.96 0.28 0.92 0.04 Z",
            fill=p.i2, color=p.i2)
    p.line([(0.92, 0.04), (0.40, 0.56)], color=p.i1, sw=1)
    p.line([(0.32, 0.64), (0.06, 0.94)], sw=max(p.sw, 2.0 if p.P >= 12 else 1.5))


def erase(p):
    p.curve("M0.58 0.06 L0.96 0.44 L0.52 0.88 L0.14 0.50 Z", fill=p.i2, color=p.i2, sw=1)
    p.curve("M0.14 0.50 L0.36 0.72", color=p.c.t["panel"], sw=max(1.0, p.P * 0.08))
    p.line([(0.30, 0.96), (0.96, 0.96)], color=p.i1, sw=1)


def mute_tool(p):
    p.poly([(0.04, 0.34), (0.22, 0.34), (0.46, 0.10), (0.46, 0.90), (0.22, 0.66), (0.04, 0.66)])
    p.line([(0.64, 0.34), (0.94, 0.66)], color=p.i1)
    p.line([(0.94, 0.34), (0.64, 0.66)], color=p.i1)


def zoom(p):
    p.dot(0.40, 0.40, 0.32, fill="none", stroke=p.i2)
    if p.edge:
        p.curve("M0.20 0.36 C0.22 0.24 0.30 0.18 0.40 0.17", color=p.i1, sw=1)
    p.line([(0.64, 0.64), (0.94, 0.94)], sw=max(p.sw, 2.0 if p.P >= 12 else 1.5))


def _mix(a, b, t):
    from core import mix
    return mix(a, b, t)
