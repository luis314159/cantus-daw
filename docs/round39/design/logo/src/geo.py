"""Shared geometry and metal shading for the round-39 logo variants (#10 enriched).

Everything is plain SVG: paths, linear/radial gradients, at most one drop shadow.
Every colour carries a `data-role` so `kit/logo.rs` can tint it (tile, wave, playhead,
neutral) exactly like the ten logos it already ships.
"""
import math

# Light comes from the upper left (screen coordinates, y down).
LIGHT = (-0.55, -0.835)

# Metal ramps: brightness 0..1 -> colour. Gold is the playhead's #E2B84A at 0.62;
# silver keeps #10's tick colour #C9D3E6 at 0.72.
GOLD = [(0.0, "#4E3108"), (0.22, "#7E5512"), (0.42, "#B5852A"), (0.62, "#E2B84A"),
        (0.80, "#F4D77C"), (0.92, "#FFEDB4"), (1.0, "#FFF9E2")]
SILVER = [(0.0, "#36435F"), (0.22, "#56658A"), (0.45, "#8C9AB8"), (0.72, "#C9D3E6"),
          (0.88, "#E6ECF7"), (1.0, "#FFFFFF")]
NAVY = "#16295E"


def hexrgb(h):
    h = h.lstrip("#")
    return tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))


def rgbhex(c):
    return "#%02X%02X%02X" % tuple(max(0, min(255, round(v))) for v in c)


def ramp(stops, t):
    t = max(0.0, min(1.0, t))
    for (a, ca), (b, cb) in zip(stops, stops[1:]):
        if t <= b:
            u = (t - a) / (b - a) if b > a else 0
            x, y = hexrgb(ca), hexrgb(cb)
            return rgbhex([x[i] + (y[i] - x[i]) * u for i in range(3)])
    return stops[-1][1]


def norm(v):
    l = math.hypot(*v) or 1.0
    return (v[0] / l, v[1] / l)


def lit(n, tilt=0.8):
    """Brightness of a facet whose outward normal (in the picture plane) is `n`.
    `tilt` is how steep the facet is: 0 = flat (always 0.5), 1 = edge-on."""
    n = norm(n)
    return 0.5 + 0.5 * tilt * (n[0] * LIGHT[0] + n[1] * LIGHT[1])


def pts(ps):
    return " ".join("%.2f,%.2f" % p for p in ps)


class Defs:
    """Collects gradient definitions and hands out ids."""

    def __init__(self, prefix):
        self.prefix = prefix
        self.items = []
        self.n = 0

    def add(self, body):
        self.items.append(body)

    def lin(self, p0, p1, stops, role):
        """A user-space linear gradient from p0 to p1; stops = [(offset, colour[, opacity])]."""
        self.n += 1
        gid = "%s%d" % (self.prefix, self.n)
        s = "".join(
            '<stop offset="%.3f" stop-color="%s"%s data-role="%s"/>'
            % (o, c, (' stop-opacity="%.3f"' % st[2]) if len(st) > 2 else "", role)
            for st in stops for o, c in [st[:2]])
        self.items.append(
            '<linearGradient id="%s" gradientUnits="userSpaceOnUse" x1="%.2f" y1="%.2f" '
            'x2="%.2f" y2="%.2f">%s</linearGradient>' % (gid, p0[0], p0[1], p1[0], p1[1], s))
        return "url(#%s)" % gid

    def svg(self):
        return "<defs>" + "".join(self.items) + "</defs>"


def facet(defs, poly, n, metal, role, tilt=0.8, sheen=0.14, along=None):
    """One flat facet of cut metal: its colour from its normal, with a soft sheen
    along `along` (two points) so it reads as polished, not painted."""
    b = lit(n, tilt)
    if along is None:
        return '<polygon points="%s" fill="%s" data-role="%s"/>' % (pts(poly), ramp(metal, b), role)
    g = defs.lin(along[0], along[1],
                 [(0.0, ramp(metal, b + sheen)), (0.55, ramp(metal, b)),
                  (1.0, ramp(metal, b - sheen * 0.8))], role)
    return '<polygon points="%s" fill="%s"/>' % (pts(poly), g)


def polar(c, r, a_deg):
    a = math.radians(a_deg)
    return (c[0] + r * math.cos(a), c[1] + r * math.sin(a))


def keel_tick(defs, c, a, r0, r1, w, metal, role, tip=None, tilt=0.85, arch=False):
    """A radial bar cut with a ridge down its middle (two long facets), pointed at both
    ends (`tip` = point length). With `arch`, the outer end is a pointed arch and the
    inner end round, like a lancet window."""
    tip = w * 0.5 if tip is None else tip
    d = norm((math.cos(math.radians(a)), math.sin(math.radians(a))))
    p = (-d[1], d[0])
    h = w / 2

    def at(r, s):
        return (c[0] + d[0] * r + p[0] * s, c[1] + d[1] * r + p[1] * s)

    out = []
    for side in (1, -1):
        if arch:
            # Round inner end, straight flank, then an equilateral pointed arch.
            body = [at(r0 + h - h * math.cos(t), side * h * math.sin(t))
                    for t in [math.pi / 2 * k / 6 for k in range(7)]]
            sh = r1 - w * 0.866
            vc = -side * h
            arc = [at(sh + w * math.sin(th), vc + side * w * math.cos(th))
                   for th in [math.radians(60) * k / 8 for k in range(9)]]
            poly = body + arc
            n = (p[0] * side, p[1] * side)
            out.append(facet(defs, poly, n, metal, role, tilt,
                             along=(at(r0, 0), at(r1, 0))))
        else:
            poly = [at(r0, 0), at(r0 + tip, side * h), at(r1 - tip, side * h), at(r1, 0)]
            n = (p[0] * side, p[1] * side)
            out.append(facet(defs, poly, n, metal, role, tilt,
                             along=(at(r0, 0), at(r1, 0))))
    return "".join(out)


def rod_tick(defs, c, a, r0, r1, w, metal, role):
    """A round-capped rod with a cylindrical sheen across it (light on its upper-left
    flank), for the plainer variant."""
    d = norm((math.cos(math.radians(a)), math.sin(math.radians(a))))
    p = (-d[1], d[0])
    s = 1 if (p[0] * LIGHT[0] + p[1] * LIGHT[1]) > 0 else -1
    m = ((r0 + r1) / 2)
    mid = (c[0] + d[0] * m, c[1] + d[1] * m)
    a0 = (mid[0] + p[0] * s * w / 2, mid[1] + p[1] * s * w / 2)
    a1 = (mid[0] - p[0] * s * w / 2, mid[1] - p[1] * s * w / 2)
    b = lit(d, 0.35)
    g = defs.lin(a0, a1, [(0.0, ramp(metal, b + 0.28)), (0.35, ramp(metal, b + 0.12)),
                          (0.7, ramp(metal, b - 0.12)), (1.0, ramp(metal, b - 0.26))], role)
    x0, y0 = c[0] + d[0] * r0, c[1] + d[1] * r0
    x1, y1 = c[0] + d[0] * r1, c[1] + d[1] * r1
    return ('<line x1="%.2f" y1="%.2f" x2="%.2f" y2="%.2f" stroke="%s" stroke-width="%.2f" '
            'stroke-linecap="round"/>' % (x0, y0, x1, y1, g, w))


def cross_facets(defs, cx, cy, h, ytop, ybot, xl, xr, metal, role, flare=1.0, tilt=0.9,
                 two=False):
    """The gold cross (the playhead), cut as a bevelled bar: a ridge down every arm's
    middle, hips to each end and valleys into the four inner corners, the straight
    skeleton of the outline. `flare` > 1 widens the ends (a cross pattee's hint).
    With `two`, each arm is only split along its ridge (a plainer two-facet cut)."""
    e = h * flare
    C = (cx, cy)
    # Ridge ends: as far in from each end as the end's half width.
    Rt, Rb = (cx, ytop + e), (cx, ybot - e)
    Rl, Rr = (xl + e, cy), (xr - e, cy)
    il, ir, it, ib = (cx - h, cy - h), (cx + h, cy - h), (cx + h, cy + h), (cx - h, cy + h)
    # Outline corners at the ends.
    tl, tr = (cx - e, ytop), (cx + e, ytop)
    bl, br = (cx - e, ybot), (cx + e, ybot)
    lt, lb = (xl, cy - e), (xl, cy + e)
    rt, rb = (xr, cy - e), (xr, cy + e)
    U, D, L, R = (0, -1), (0, 1), (-1, 0), (1, 0)
    F = []
    if two:
        Rt, Rb, Rl, Rr = (cx, ytop), (cx, ybot), (xl, cy), (xr, cy)
        F += [([tl, Rt, C, il], L, (Rt, C)), ([tr, Rt, C, ir], R, (Rt, C)),
              ([bl, Rb, C, ib], L, (C, Rb)), ([br, Rb, C, it], R, (C, Rb)),
              ([lt, Rl, C, il], U, (Rl, C)), ([lb, Rl, C, ib], D, (Rl, C)),
              ([rt, Rr, C, ir], U, (C, Rr)), ([rb, Rr, C, it], D, (C, Rr))]
    else:
        F += [([tl, tr, Rt], U, None), ([tl, Rt, C, il], L, (Rt, C)),
              ([tr, Rt, C, ir], R, (Rt, C)),
              ([rt, rb, Rr], R, None), ([ir, C, Rr, rt], U, (C, Rr)),
              ([it, C, Rr, rb], D, (C, Rr)),
              ([bl, br, Rb], D, None), ([it, C, Rb, br], R, (C, Rb)),
              ([ib, C, Rb, bl], L, (C, Rb)),
              ([lt, lb, Rl], L, None), ([ib, C, Rl, lb], D, (Rl, C)),
              ([il, C, Rl, lt], U, (Rl, C))]
    out = [facet(defs, poly, n, metal, role, tilt, along=al) for poly, n, al in F]
    return "".join(out)


def cross_outline(cx, cy, h, ytop, ybot, xl, xr, flare=1.0):
    e = h * flare
    return [(cx - e, ytop), (cx + e, ytop), (cx + h, cy - h), (xr, cy - e), (xr, cy + e),
            (cx + h, cy + h), (cx + e, ybot), (cx - e, ybot), (cx - h, cy + h),
            (xl, cy + e), (xl, cy - e), (cx - h, cy - h)]


def rrect_path(x, y, w, h, r):
    return ("M%.2f,%.2f H%.2f A%.2f,%.2f 0 0 1 %.2f,%.2f V%.2f A%.2f,%.2f 0 0 1 %.2f,%.2f "
            "H%.2f A%.2f,%.2f 0 0 1 %.2f,%.2f V%.2f A%.2f,%.2f 0 0 1 %.2f,%.2fZ"
            % (x + r, y, x + w - r, r, r, x + w, y + r, y + h - r, r, r, x + w - r, y + h,
               x + r, r, r, x, y + h - r, y + r, r, r, x + r, y))


def rrect_points(x, y, w, h, r, step):
    """Points every `step` along a rounded rectangle's outline (for a bead row)."""
    segs = []
    per = 2 * (w - 2 * r) + 2 * (h - 2 * r) + 2 * math.pi * r
    n = max(4, int(per / step))
    out = []
    for k in range(n):
        s = per * k / n
        for (kind, a, b) in [
            ("l", (x + r, y), (x + w - r, y)), ("a", (x + w - r, y + r), -90),
            ("l", (x + w, y + r), (x + w, y + h - r)), ("a", (x + w - r, y + h - r), 0),
            ("l", (x + w - r, y + h), (x + r, y + h)), ("a", (x + r, y + h - r), 90),
            ("l", (x, y + h - r), (x, y + r)), ("a", (x + r, y + r), 180)]:
            L = math.dist(a, b) if kind == "l" else math.pi * r / 2
            if s <= L:
                if kind == "l":
                    t = s / L if L else 0
                    out.append((a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t))
                else:
                    ang = math.radians(b + 90 * s / L)
                    out.append((a[0] + r * math.cos(ang), a[1] + r * math.sin(ang)))
                break
            s -= L
    return out


def bead(defs, p, r, metal, role):
    """A pearl of gold: a lit dome (radial gradient) with a dark seat."""
    gid = "bd%s" % role
    return ('<circle cx="%.2f" cy="%.2f" r="%.2f" fill="url(#%s)"/>' % (p[0], p[1], r, gid))


def bead_def(metal, role):
    return ('<radialGradient id="bd%s" cx="0.36" cy="0.32" r="0.75">'
            '<stop offset="0" stop-color="%s" data-role="%s"/>'
            '<stop offset="0.45" stop-color="%s" data-role="%s"/>'
            '<stop offset="1" stop-color="%s" data-role="%s"/></radialGradient>'
            % (role, ramp(metal, 1.0), role, ramp(metal, 0.66), role, ramp(metal, 0.2), role))


# ---- the continuous C (owner, 2026-11-09: "que la C sea continua") --------------------

def c_profile(c, R, tmax, tmin, ae, flare=0.0, fl_deg=14.0, step=2.0, shift=0.0):
    """A Lombardic-style C round centre `c`: the outer edge a circle of radius R, the
    stroke thick in the belly (left) and thinner toward the terminals at +-`ae` degrees
    (screen angles, y down), each terminal swelling by `flare` over its last `fl_deg`.
    `shift` moves the counter right (more contrast). Returns [(angle, r_out, r_in)]."""
    out = []
    a = ae
    while True:
        s = (1 - math.cos(math.radians(a))) / 2  # 0 at the opening, 1 at the belly
        t = tmin + (tmax - tmin) * s ** 1.3
        d = min(a - ae, 360 - ae - a)
        if d < fl_deg:
            t += flare * (1 - d / fl_deg) ** 2
        out.append((a, R, R - t - shift * math.cos(math.radians(a)) * 0))
        if a >= 360 - ae:
            break
        a = min(a + step, 360 - ae)
    return out


def c_polygon(c, prof, k0=0.0, k1=1.0):
    """The band between fractions k0 and k1 of the stroke (0 = outer edge, 1 = inner)."""
    o = [polar(c, ro - (ro - ri) * k0, a) for a, ro, ri in prof]
    i = [polar(c, ro - (ro - ri) * k1, a) for a, ro, ri in reversed(prof)]
    return o + i


def gilded_c(defs, c, prof, bevel, face_fill, metal, role, tilt=0.85, end_bevel=True):
    """The C cut like a raised gilded letter: a flat polished face (`face_fill`, a
    gradient) between two bevels whose shade follows their normals all the way round,
    in thin flat slices over a base so no seam shows."""
    o = ['<polygon points="%s" fill="%s" data-role="%s"/>'
         % (pts(c_polygon(c, prof)), ramp(metal, 0.5), role)]
    for (a0, ro0, ri0), (a1, ro1, ri1) in zip(prof, prof[1:]):
        am = (a0 + a1) / 2
        n = (math.cos(math.radians(am)), math.sin(math.radians(am)))
        e = 0.35  # overlap in degrees so slices close up
        A0, A1 = a0 - e, a1 + e
        outer = [polar(c, ro0, A0), polar(c, ro1, A1), polar(c, ro1 - bevel, A1),
                 polar(c, ro0 - bevel, A0)]
        inner = [polar(c, ri0 + bevel, A0), polar(c, ri1 + bevel, A1), polar(c, ri1, A1),
                 polar(c, ri0, A0)]
        o.append('<polygon points="%s" fill="%s" data-role="%s"/>'
                 % (pts(outer), ramp(metal, lit(n, tilt)), role))
        o.append('<polygon points="%s" fill="%s" data-role="%s"/>'
                 % (pts(inner), ramp(metal, lit((-n[0], -n[1]), tilt)), role))
    # The face, inset by the bevel, and the two end cuts.
    face = [(a, ro - bevel, ri + bevel) for a, ro, ri in prof]
    if end_bevel:
        da = math.degrees(bevel / prof[0][1]) * 1.1
        face = [(a, ro, ri) for a, ro, ri in face if prof[0][0] + da <= a <= prof[-1][0] - da]
        for (a, ro, ri), sgn in ((prof[0], -1), (prof[-1], 1)):
            # end facet: normal along the tangent, pointing out of the stroke
            tng = (-math.sin(math.radians(a)) * sgn, math.cos(math.radians(a)) * sgn)
            p = [polar(c, ro, a), polar(c, ri, a), polar(c, ri + bevel, a - sgn * da),
                 polar(c, ro - bevel, a - sgn * da)]
            o.append('<polygon points="%s" fill="%s" data-role="%s"/>'
                     % (pts(p), ramp(metal, lit(tng, tilt)), role))
    o.append('<polygon points="%s" fill="%s"/>' % (pts(c_polygon(c, face)), face_fill))
    return "".join(o)


def c_groove(c, prof, k=0.5, sw=3.0, dark="#4A2F06", light="#FFF3C8", inset_deg=6):
    """An engraved line along the face: a dark cut with a lit lip beside it."""
    pr = [p for p in prof if prof[0][0] + inset_deg <= p[0] <= prof[-1][0] - inset_deg]

    def line(kk, dr):
        ps = [polar(c, ro - (ro - ri) * kk + dr, a) for a, ro, ri in pr]
        return "M" + " L".join("%.2f,%.2f" % p for p in ps)
    return ('<path d="%s" fill="none" stroke="%s" stroke-opacity="0.55" stroke-width="%.1f" '
            'stroke-linecap="round" data-role="neutral"/>' % (line(k, 0), dark, sw) +
            '<path d="%s" fill="none" stroke="%s" stroke-opacity="0.6" stroke-width="%.1f" '
            'stroke-linecap="round" data-role="wave"/>' % (line(k, -sw * 0.9), light, sw * 0.6))


def c_thickness(tmax, tmin):
    def t(a):
        s = (1 - math.cos(math.radians(a))) / 2
        return tmin + (tmax - tmin) * s ** 1.3
    return t


def _bound(c, rf, xlim):
    """The angle in (0, 89) where the curve r(a) reaches x = xlim (upper-right quadrant
    mirrored: we work with a in degrees below the opening's axis)."""
    lo, hi = 0.0, 89.0
    for _ in range(50):
        m = (lo + hi) / 2
        x = c[0] + rf(m) * math.cos(math.radians(m))
        if x > xlim:
            lo = m
        else:
            hi = m
    return (lo + hi) / 2


def c_outline(c, R, tf, ae, io=0.0, ii=0.0, xi=0.0, step=2.0):
    """The C's outline with its ends cut upright at x0 = c.x + R cos(ae): the outer
    edge at radius R - io, the inner at R - t(a) + ii, both cut at x0 - xi."""
    x0 = c[0] + R * math.cos(math.radians(ae)) - xi
    ro = lambda a: R - io  # noqa: E731
    ri = lambda a: R - tf(a) + ii  # noqa: E731
    ao, ai = _bound(c, ro, x0), _bound(c, ri, x0)

    def arc(rf, a0, a1):
        n = max(2, int(abs(a1 - a0) / step))
        return [polar(c, rf(a0 + (a1 - a0) * k / n), a0 + (a1 - a0) * k / n) for k in range(n + 1)]
    return arc(ro, ao, 360 - ao) + arc(ri, 360 - ai, ai), (ao, ai, x0)


def gilded_c2(c, R, tf, ae, bevel, face_fill, metal, role, tilt=0.85, step=2.0, cid="cclip"):
    """The continuous C as a raised gilded letter with upright-cut ends: a polished face
    between bevels whose shade follows their normals all the way round."""
    full, (ao, ai, x0) = c_outline(c, R, tf, ae, step=step)
    o = ['<clipPath id="%s"><polygon points="%s"/></clipPath><g clip-path="url(#%s)">'
         % (cid, pts(full), cid),
         '<polygon points="%s" fill="%s" data-role="%s"/>' % (pts(full), ramp(metal, 0.5), role)]

    def strips(rf, rg, a0, a1, sign):
        n = max(2, int(abs(a1 - a0) / step))
        for k in range(n):
            u0 = a0 + (a1 - a0) * k / n - 0.3
            u1 = a0 + (a1 - a0) * (k + 1) / n + 0.3
            am = (u0 + u1) / 2
            nn = (sign * math.cos(math.radians(am)), sign * math.sin(math.radians(am)))
            q = [polar(c, rf(u0), u0), polar(c, rf(u1), u1), polar(c, rg(u1), u1),
                 polar(c, rg(u0), u0)]
            o.append('<polygon points="%s" fill="%s" data-role="%s"/>'
                     % (pts(q), ramp(metal, lit(nn, tilt)), role))
    strips(lambda a: R, lambda a: R - bevel, ao, 360 - ao, 1)
    strips(lambda a: R - tf(a), lambda a: R - tf(a) + bevel, ai, 360 - ai, -1)
    # The upright end cuts face right: one facet each.
    face, _ = c_outline(c, R, tf, ae, bevel, bevel, bevel * 0.8, step=step)
    for sgn in (-1, 1):
        yo = c[1] + sgn * (R * math.sin(math.radians(ao)))
        yi = c[1] + sgn * ((R - tf(ai)) * math.sin(math.radians(ai)))
        q = [(x0, yo), (x0, yi), (x0 - bevel * 0.8, yi - sgn * bevel),
             (x0 - bevel * 0.8, yo + sgn * bevel)]
        o.append('<polygon points="%s" fill="%s" data-role="%s"/>'
                 % (pts(q), ramp(metal, lit((1, 0.1 * sgn), tilt)), role))
    o.append('<polygon points="%s" fill="%s"/>' % (pts(face), face_fill))
    o.append('</g>')
    return "".join(o)
