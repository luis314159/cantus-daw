"""The four variants at app-icon sizes (1024, 512, 256): one drawing per size, each
with the detail that size can hold. Run: python large.py <outdir>."""
import math
import sys
from geo import (c_outline, c_thickness, gilded_c2, lit, GOLD, SILVER, Defs, bead, bead_def, c_groove, c_polygon, c_profile,
                 cross_facets, cross_outline, gilded_c, keel_tick, polar, pts, ramp,
                 rrect_path, rrect_points)

C = (468.0, 512.0)  # the C's centre (#10's, a little left so the cross has room)
ANG = [55 + 25 * k for k in range(11)]  # #10's ticks, kept as ornament
AMP = [20, 35, 47.5, 30, 55, 40, 55, 30, 47.5, 35, 20]
# The continuous C: outer radius, belly and terminal weights, terminals at +-AE degrees.
CR, CTMAX, CTMIN, AE, CFLARE = 262.0, 96.0, 58.0, 50.0, 0.0
TF = c_thickness(CTMAX, CTMIN)
TILE = (100, 100, 824, 824, 186)
# The cross (playhead): #10's, a touch heavier so it can carry a bevel.
X = dict(cx=746.0, cy=346.0, h=20.0, ytop=256.0, ybot=768.0, xl=676.0, xr=816.0)

VARIANTS = ["relicario", "astrolabio", "roseton", "grabado"]


def head(size, defs, extra=""):
    return ('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024" width="%d" '
            'height="%d">' % (size, size) + defs.svg() + extra)


def base_defs(defs, size):
    sd = 22 if size >= 512 else 16
    defs.add('<filter id="sh" x="-20%%" y="-20%%" width="140%%" height="140%%"><feDropShadow '
             'dx="0" dy="18" stdDeviation="%d" flood-color="#000000" flood-opacity="0.35" '
             'data-role="neutral"/></filter>' % sd)
    defs.add('<filter id="emb" x="-30%" y="-30%" width="160%" height="160%">'
             '<feGaussianBlur stdDeviation="7"/></filter>')
    defs.add('<radialGradient id="bg" cx="0.34" cy="0.28" r="0.95">'
             '<stop offset="0" stop-color="#2B4796" data-role="tile"/>'
             '<stop offset="0.5" stop-color="#16295E" data-role="tile"/>'
             '<stop offset="1" stop-color="#091433" data-role="tile"/></radialGradient>')
    defs.add('<linearGradient id="gloss" x1="0" y1="0" x2="0" y2="1">'
             '<stop offset="0" stop-color="#FFFFFF" stop-opacity="0.16" data-role="neutral"/>'
             '<stop offset="0.42" stop-color="#FFFFFF" stop-opacity="0" data-role="neutral"/>'
             '</linearGradient>')
    defs.add('<linearGradient id="rim" x1="0" y1="0" x2="1" y2="1">' + "".join(
        '<stop offset="%.2f" stop-color="%s" data-role="playhead"/>' % (o, ramp(GOLD, b))
        for o, b in [(0, 0.95), (0.22, 0.7), (0.45, 0.4), (0.62, 0.86), (0.8, 0.5),
                     (1, 0.25)]) + '</linearGradient>')
    defs.add('<linearGradient id="leaf" gradientUnits="userSpaceOnUse" x1="230" y1="250" '
             'x2="700" y2="790">' + "".join(
                 '<stop offset="%.2f" stop-color="%s" data-role="wave"/>' % (o, ramp(GOLD, b))
                 for o, b in [(0, 0.97), (0.2, 0.8), (0.42, 0.55), (0.6, 0.86), (0.78, 0.6),
                              (1, 0.36)]) + '</linearGradient>')
    defs.add('<linearGradient id="srim" x1="0" y1="0" x2="1" y2="1">' + "".join(
        '<stop offset="%.2f" stop-color="%s" data-role="wave"/>' % (o, ramp(SILVER, b))
        for o, b in [(0, 0.98), (0.3, 0.72), (0.55, 0.45), (0.75, 0.85), (1, 0.35)])
        + '</linearGradient>')
    defs.add('<radialGradient id="knob" cx="0.38" cy="0.32" r="0.72">' + "".join(
        '<stop offset="%.2f" stop-color="%s" data-role="wave"/>' % (o, ramp(GOLD, b))
        for o, b in [(0, 1.0), (0.25, 0.86), (0.6, 0.55), (0.85, 0.3), (1, 0.42)])
        + '</radialGradient>')
    defs.add('<clipPath id="tileclip"><path d="%s"/></clipPath>' % rrect_path(*TILE))
    defs.add(bead_def(GOLD, "playhead"))
    defs.add(bead_def(SILVER, "wave"))


def tile(body_under=""):
    x, y, w, h, r = TILE
    return ('<g filter="url(#sh)"><path d="%s" fill="url(#bg)"/></g>' % rrect_path(x, y, w, h, r)
            + '<g clip-path="url(#tileclip)">' + body_under + '</g>')


def finish():
    x, y, w, h, r = TILE
    p = rrect_path(x, y, w, h, r)
    return ('<path d="%s" fill="url(#gloss)"/>' % p +
            '<path d="%s" fill="none" stroke="#FFFFFF" stroke-opacity="0.13" stroke-width="3" '
            'data-role="neutral"/>' % rrect_path(x + 1.5, y + 1.5, w - 3, h - 3, r - 1.5) +
            '</svg>')


def engraving(size, rays=True, rings=True):
    """Basse-taille: the silver under the navy enamel, engraved, showing through."""
    out = []
    if rays and size >= 512:
        n = 120 if size >= 1024 else 72
        for k in range(n):
            a = 360 * k / n
            p0, p1 = polar(C, 40, a), polar(C, 760, a)
            out.append('<line x1="%.1f" y1="%.1f" x2="%.1f" y2="%.1f"/>' % (*p0, *p1))
        g = ('<g stroke="#7E9BE0" stroke-opacity="%.2f" stroke-width="%.1f" data-role="tile">'
             % (0.10 if size >= 1024 else 0.09, 2.2 if size >= 1024 else 3.2))
        out = [g] + out + ['</g>']
    if rings:
        step = 26 if size >= 1024 else (36 if size >= 512 else 60)
        sw = 2.0 if size >= 1024 else (3.0 if size >= 512 else 5.0)
        out.append('<g fill="none" stroke="#8FA8E8" stroke-opacity="0.08" stroke-width="%.1f" '
                   'data-role="tile">' % sw)
        r = step * 2
        while r < 760:
            out.append('<circle cx="%.1f" cy="%.1f" r="%d"/>' % (*C, r))
            r += step
        out.append('</g>')
    return "".join(out)


def rim(size, inset=30, sw=7, beads=True, pearl="wave"):
    x, y, w, h, r = TILE
    o = ['<path d="%s" fill="none" stroke="url(#rim)" stroke-width="%.1f"/>'
         % (rrect_path(x + inset, y + inset, w - 2 * inset, h - 2 * inset, r - inset),
            sw if size >= 512 else max(sw, 9))]
    if beads and size >= 512:
        bi = inset + (20 if size >= 1024 else 24)
        step, br = (24, 5.5) if size >= 1024 else (34, 7.5)
        for p in rrect_points(x + bi, y + bi, w - 2 * bi, h - 2 * bi, r - bi, step):
            o.append(bead(None, p, br, GOLD, pearl))
    return "".join(o)


# Each variant's letter weight (belly, terminals): Roseton the most uncial, Grabado the
# most even.
WEIGHT = {"relicario": (96.0, 58.0), "astrolabio": (96.0, 58.0), "roseton": (112.0, 50.0),
          "grabado": (86.0, 66.0)}
CUR = {"v": "relicario"}


def tf():
    return c_thickness(*WEIGHT[CUR["v"]])


def profile(size):
    tmax, tmin = WEIGHT[CUR["v"]]
    return c_profile(C, CR, tmax, tmin, AE, 0.0, step=2.0 if size >= 512 else 3.0)


def the_c(defs, size, groove=True, punch=False, scale_marks=False):
    prof = profile(size)
    bevel = 13 if size >= 512 else 16
    o = [gilded_c2(C, CR, tf(), AE, bevel, "url(#leaf)", GOLD, "wave",
                   step=2.0 if size >= 512 else 3.0)]
    if scale_marks:
        o.append(limb_marks(prof, size))
    if groove:
        o.append(c_groove(C, prof, 0.5, 3.0 if size >= 1024 else (4.0 if size >= 512 else 7.0)))
    if punch and size >= 512:
        # Punchwork: a row of struck dots along the face, as on a gilded panel.
        pr = [p for p in prof if prof[0][0] + 10 <= p[0] <= prof[-1][0] - 10]
        dots = []
        a = pr[0][0]
        for ang, ro, ri in pr:
            if ang >= a:
                for kk in (0.3, 0.7):
                    q = polar(C, ro - (ro - ri) * kk, ang)
                    dots.append("M%.1f,%.1f m-3.2,0 a3.2,3.2 0 1 0 6.4,0 a3.2,3.2 0 1 0 -6.4,0"
                                % q)
                a = ang + (6 if size >= 1024 else 9)
        o.append('<path d="%s" fill="#5A3A08" fill-opacity="0.55" data-role="neutral"/>'
                 % "".join(dots))
    return "".join(o)


def knobs(prof, size):
    """The terminals end in gilded knobs, as a torc's do: a collar, then a polished ball
    a little wider than the stroke. They keep the C from reading as a crescent."""
    o = []
    for (a0, ro, ri), sgn in ((prof[0], -1), (prof[-1], 1)):
        t = ro - ri
        rm = (ro + ri) / 2
        r = t * 0.56
        ang = a0 + sgn * math.degrees((r * 0.78) / rm)
        p = polar(C, rm, ang)
        q0, q1 = polar(C, ro + 2, a0 + sgn * 1.2), polar(C, ri - 2, a0 + sgn * 1.2)
        o.append('<line x1="%.1f" y1="%.1f" x2="%.1f" y2="%.1f" stroke="#3B2606" '
                 'stroke-opacity="0.65" stroke-width="%d" stroke-linecap="round" '
                 'data-role="neutral"/>' % (*q0, *q1, 9 if size >= 512 else 12))
        o.append('<circle cx="%.1f" cy="%.1f" r="%.1f" fill="#3B2606" fill-opacity="0.55" '
                 'data-role="neutral"/>' % (p[0] + 2, p[1] + 4, r + 2))
        o.append('<circle cx="%.1f" cy="%.1f" r="%.1f" fill="url(#knob)"/>' % (*p, r))
    return "".join(o)


def limb_marks(prof, size):
    """Astrolabio: a scale engraved across the C's face, longer every 30 degrees."""
    o = []
    step = 6 if size >= 1024 else (10 if size >= 512 else 15)
    for ang, ro, ri in prof:
        k = round(ang)
        if k % step or not (prof[0][0] + 8 <= ang <= prof[-1][0] - 8):
            continue
        major = (k - 60) % 30 == 0
        q0 = polar(C, ro - 15, ang)
        q1 = polar(C, ro - 15 - (ro - ri - 30) * (0.62 if major else 0.32), ang)
        w = (5 if major else 3) if size >= 512 else (8 if major else 6)
        o.append('<line x1="%.1f" y1="%.1f" x2="%.1f" y2="%.1f" stroke-width="%d"/>'
                 % (*q0, *q1, w))
    return ('<g stroke="#4A2F06" stroke-opacity="0.6" stroke-linecap="round" '
            'data-role="neutral">' + "".join(o) + '</g>')


def cast_shadow(variant):
    prof = profile(1024)
    return ('<g filter="url(#emb)" fill="#000000" fill-opacity="0.55" data-role="neutral" '
            'transform="translate(0,10)"><polygon points="%s"/><polygon points="%s"/></g>'
            % (pts(c_outline(C, CR, tf(), AE)[0]), pts(cross_outline(
                X["cx"], X["cy"], X["h"], X["ytop"], X["ybot"], X["xl"], X["xr"],
                1.35 if variant == "roseton" else 1.0))))


def cross(defs, variant):
    if variant == "grabado":
        return cross_facets(defs, metal=GOLD, role="playhead", two=True, **X)
    flare = 1.35 if variant == "roseton" else 1.0
    return cross_facets(defs, metal=GOLD, role="playhead", flare=flare, **X)


def jewel(size):
    """Relicario: a pearl set where the arms cross, in a dark collet."""
    p = (X["cx"], X["cy"])
    return ('<circle cx="%.1f" cy="%.1f" r="21" fill="url(#rim)"/>'
            '<circle cx="%.1f" cy="%.1f" r="17" fill="#3B2606" fill-opacity="0.6" '
            'data-role="neutral"/>' % (*p, *p) + bead(None, p, 15, SILVER, "wave"))


def dial(size):
    """Astrolabio: inside the C's counter, a silver dial carrying #10's ticks, small,
    with a pin at the centre."""
    o = []
    cc = (C[0] + 14, C[1])
    sw = 3 if size >= 512 else 6
    o.append('<circle cx="%.1f" cy="%.1f" r="128" fill="none" stroke="url(#srim)" '
             'stroke-width="%d"/>' % (*cc, sw))
    if size >= 512:
        g = []
        for k in range(72):
            a = 5 * k
            r1 = 128 - (12 if k % 6 == 0 else 6)
            g.append('<line x1="%.1f" y1="%.1f" x2="%.1f" y2="%.1f"/>'
                     % (*polar(cc, 128, a), *polar(cc, r1, a)))
        o.append('<g stroke="url(#srim)" stroke-width="2.5">' + "".join(g) + '</g>')
    d = Defs("dl")
    for a, amp in zip(ANG, AMP):
        o.append(keel_tick(d, cc, a, 80 - amp * 0.5, 80 + amp * 0.5 + 6, 15 if size >= 512 else 20,
                           SILVER, "wave", tip=7))
    o.insert(0, d.svg())
    o.append(bead(None, cc, 18, GOLD, "playhead"))
    o.append(bead(None, cc, 7, SILVER, "wave"))
    return "".join(o)


def diaper(size):
    """Roseton's ground: an illuminator's diaper, a fine gilded lattice with a dot in
    every lozenge, faint so the letter stays the subject."""
    if size < 512:
        return ""
    step = 64 if size >= 1024 else 80
    sw = 2 if size >= 1024 else 3
    o = []
    for k in range(-20, 21):
        o.append("M%d,%d L%d,%d" % (k * step, 0, k * step + 1100, 1100))
        o.append("M%d,%d L%d,%d" % (k * step + 1100, 0, k * step, 1100))
    dots = []
    for i in range(-2, 20):
        for j in range(-2, 40):
            x = i * step + (step / 2 if j % 2 else 0)
            y = j * step / 2
            dots.append("M%.1f,%.1f m-4,0 a4,4 0 1 0 8,0 a4,4 0 1 0 -8,0" % (x + step / 2, y))
    return ('<g clip-path="url(#tileclip)"><path d="%s" stroke="#E9C466" stroke-opacity="0.12" '
            'stroke-width="%d" fill="none" data-role="playhead"/><path d="%s" fill="#E9C466" '
            'fill-opacity="0.2" data-role="playhead"/></g>' % ("".join(o), sw, "".join(dots)))


def lancet(c, a, r0, r1, w):
    """A lancet's outline: a round foot and an equilateral pointed head."""
    d = (math.cos(math.radians(a)), math.sin(math.radians(a)))
    p = (-d[1], d[0])
    h = w / 2

    def at(r, s):
        return (c[0] + d[0] * r + p[0] * s, c[1] + d[1] * r + p[1] * s)
    foot = [at(r0 + h - h * math.cos(t), h * math.sin(t))
            for t in [math.radians(-90 + 15 * k) for k in range(13)]]
    sh = r1 - w * 0.866
    head = [at(sh + w * math.sin(th), -h + w * math.cos(th))
            for th in [math.radians(60) * k / 8 for k in range(9)]]
    back = [at(sh + w * math.sin(th), h - w * math.cos(th))
            for th in [math.radians(60) * k / 8 for k in range(7, -1, -1)]]
    return foot + head + back


def rose(size):
    """Roseton: a small rose window in the C's counter: eight lancets of navy glass in
    gilded frames round an eight-lobed rosette."""
    cc = (C[0] + 16, C[1])
    o = []
    fw = 5 if size >= 512 else 8
    o.append('<circle cx="%.1f" cy="%.1f" r="134" fill="#0C1A44" fill-opacity="0.55" '
             'stroke="url(#rim)" stroke-width="%d" data-role="tile"/>' % (*cc, fw + 1))
    for k in range(8):
        a = 22.5 + 45 * k
        o.append('<polygon points="%s" fill="#0F2156" stroke="url(#rim)" stroke-width="%d" '
                 'stroke-linejoin="round" data-role="tile"/>'
                 % (pts(lancet(cc, a, 54, 128, 26)), fw))
        if size >= 512:
            q = polar(cc, 112, a + 22.5)
            o.append(bead(None, q, 6, GOLD, "playhead"))
    for k in range(8):
        q = polar(cc, 30, 45 * k)
        o.append('<circle cx="%.1f" cy="%.1f" r="15" fill="#0C1A44" stroke="url(#rim)" '
                 'stroke-width="%d" data-role="tile"/>' % (*q, fw - 1))
    o.append(bead(None, cc, 17, GOLD, "playhead"))
    return "".join(o)


def build(variant, size):
    CUR["v"] = variant
    defs = Defs("g")
    base_defs(defs, size)
    body = []
    if variant == "relicario":
        under = engraving(size, rings=False)
        body += [rim(size, 30, 7, True, "wave"), cast_shadow(variant), the_c(defs, size),
                 cross(defs, variant), jewel(size)]
    elif variant == "astrolabio":
        under = engraving(size, rays=False)
        body += [rim(size, 30, 6, False), dial(size), cast_shadow(variant),
                 the_c(defs, size, groove=False, scale_marks=True), cross(defs, variant)]
    elif variant == "roseton":
        under = ""
        body += [diaper(size), rim(size, 30, 7, True, "playhead"), rose(size),
                 cast_shadow(variant), the_c(defs, size, punch=True), cross(defs, variant)]
    else:
        under = engraving(size, rays=False)
        body += [rim(size, 34, 5, False), cast_shadow(variant),
                 the_c(defs, size, groove=False), cross(defs, variant)]
    return head(size, defs) + tile(under) + "".join(body) + finish()


if __name__ == "__main__":
    out = sys.argv[1] if len(sys.argv) > 1 else "."
    for v in VARIANTS:
        for s in (1024, 512, 256):
            with open("%s/%s-%d.svg" % (out, v, s), "w", newline="\n") as f:
                f.write(build(v, s))
