"""New — **Glissando**, the figure of light: neon-futuristic, our own.

Drawn with light:
- **The body** is a near-black suit.
- **The light lines:** **lapis neon** (a periwinkle lapis, not cyan) runs along the outer edge of each limb, rings glow
  at the elbows and knees, and lines mark the collar and the belt.
- **The chest:** the logo's five vermilion bars, lit by the level.
- **The head:** a sleek dark head with two **gold** glowing eyes and gold headphones.
- **At night** each line is a halo, a line and a pale core.
- **On paper** the halo goes and the line stays, sharp lapis on the dark suit.

Fast hands and feet leave **light trails** (afterimages that fade within a quarter beat).

It dances like light: tutting (geometric arm positions snapped on the half beat, the joints
flashing on the beat) and a glide. Its signature, **Halo**: its hands draw a full circle of
light around itself; on the beat the circle drops to the floor as a ring and fades, and it
lands in a crouch.

Inspired by the genre (light-line sci-fi, synthwave), not by any film. There is no disc, no
vehicle, no helmet and no circuit layout from one; the colour scheme is the cast's lapis,
gold and vermilion; the motif is ours (the logo's five bars, edge lines, joint rings).
"""
import math
from body import Props, solve, place
from moves import Style, MOVES as ALL_MOVES, steps
from rig import (Pose, add, sub, mul, rot, length, lerp, snap_time, beat_down, cos1, smooth,
                 ease_in_out, ease_out, ease_out_back, clamp01)
from svg import Doc, tube_d, rrect_d, curve_d, f

NAME = "Glissando"
PROPS = Props(leg=0.49, thigh=0.256, shin=0.244, torso=0.27, neck=0.040, head=0.080,
              shoulder=0.094, hipw=0.050, stance=0.066, upper=0.160, fore=0.155, foot=0.10)
STYLE = Style(bounce=0.06, sway=0.07, step=0.18, lag=0.08, hop=0.12, squash=0.04, snap=0.7,
              lean=1.0, lift_step=0.03, chest=-0.05)
MOVES = {"groove": "gl_tut", "steps": "gl_glide", "jump": "gl_halo"}
TRAILS = True
HALO = (0.0, -0.07, 0.25)        # the Halo's centre (from the neck) and radius, units
TRAIL_STEP, TRAIL_N = 0.035, 6


def lp(a, b, t):
    return (lerp(a[0], b[0], t), lerp(a[1], b[1], t))


# tutting: geometric arm positions, by angle and bend (as rig_dance's arm/bend)
TUT = [((90, 90), (90, 90)),      # both forearms up: a frame
       ((90, -90), (30, 120)),    # one across the chest, the other up at the side
       ((150, 0), (90, -90)),     # a diagonal line up, a forearm across
       ((30, 120), (90, -90))]


def tut(b, st):
    """Two beats of tutting: a geometric arm shape on each half beat, snapped with an
    overshoot; the knees bounce on the beat; the joints flash on the beat."""
    bs = snap_time(b, 0.5, move=0.30, amount=st.snap, overshoot=2.0)
    p = Pose()
    ph = b % 1.0
    k0 = int(math.floor(bs / 0.5)) % 4
    t = (bs / 0.5) - math.floor(bs / 0.5)
    a0, a1 = TUT[k0], TUT[(k0 + 1) % 4]
    p.fk = tuple((lerp(a0[i][0], a1[i][0], t), lerp(a0[i][1], a1[i][1], t)) for i in range(2))
    p.hip = (0.05 * cos1(b / 2), st.bounce * beat_down(ph, 2.0))
    p.chest = (st.chest * cos1(b / 2), 0.0)
    p.lean = 3.0 * cos1(b / 2)
    p.tilt = 8.0 * cos1((b - st.lag) / 2)
    p.nod = 8.0 * beat_down((b - st.lag) % 1.0) - 3.0
    p.accent = beat_down(ph, 6)
    p.level = 0.5 + 0.45 * beat_down(ph, 3.0)
    p.eyes = "open"
    return p


def glide(b, st):
    p = steps(b, st)
    bs = snap_time(b, 0.5, move=0.3, amount=0.7)
    keys = [((40, 0), (140, 0)), ((140, 0), (40, 0))]
    k = int(math.floor(bs)) % 2
    t = smooth(bs % 1.0)
    a0, a1 = keys[k], keys[(k + 1) % 2]
    p.fk = tuple((lerp(a0[i][0], a1[i][0], t), lerp(a0[i][1], a1[i][1], t)) for i in range(2))
    p.accent = beat_down(b % 1.0, 6)
    p.level = 0.5 + 0.4 * beat_down(b % 1.0, 3.0)
    p.eyes = "open"
    return p


def halo(b, st):
    """Two beats, its signature: the hands start low together and sweep up round both
    sides, drawing a circle of light round the figure that closes overhead on beat 2; on
    that beat the circle drops to the floor as a ring and fades, the figure lands in a
    crouch, one hand to the floor."""
    p = Pose()
    bb = b % 2.0
    draw_ = ease_in_out(clamp01(bb / 1.0))
    after = clamp01((bb - 1.0) / 1.0)
    p.sig = draw_ if bb < 1.0 else 1.0 + after       # 0…1 drawing, 1…2 dropping
    ang = math.radians(180 * draw_)                   # 0: hands low, 180: overhead
    A = PROPS.arm
    cx, cy, R = HALO                                  # the circle, in units from the neck
    hl = ((cx - R * math.sin(ang)) / A, (cy + R * math.cos(ang)) / A)
    hr = ((cx + R * math.sin(ang)) / A, (cy + R * math.cos(ang)) / A)
    if bb < 1.0:
        p.hands = (hl, hr)
        p.lean = 0.0
        p.nod = -10.0 * draw_
        p.hip = (0.0, 0.02 - 0.03 * draw_)
        p.eyes = "open"
    else:
        land = ease_out_back(clamp01(after / 0.25), 1.6)
        rise = smooth((after - 0.6) / 0.4)
        p.hands = (lp((-0.10, -1.0), (-0.40, 1.25), land * (1 - rise)), lp((0.10, -1.0), (0.85, 0.20), land * (1 - rise)))
        p.hip = (0.0, 0.26 * land * (1 - rise))
        p.feet = ((-0.10 * land * (1 - rise), 0.0), (0.16 * land * (1 - rise), -0.02))
        p.lean = -6.0 * land * (1 - rise)
        p.nod = 14.0 * land * (1 - rise) - 6.0 * rise
        p.eyes = "wide" if after < 0.3 else "open"
    p.accent = 1.0 if 1.0 <= bb < 1.15 else beat_down(b % 1.0, 6) * 0.6
    p.level = 1.0 if bb >= 1.0 else 0.4 + 0.6 * draw_
    return p


ALL_MOVES.update({"gl_tut": (tut, 2.0), "gl_glide": (glide, 4.0), "gl_halo": (halo, 2.0)})


def colours(th):
    """Lapis neon (a periwinkle lapis at night, never the films' cyan), gold eyes and
    headphones, the logo's vermilion bars. Every colour >= 78 from a chroma key."""
    night = th["name"] == "Night"
    return {"suit": "#0B0E12", "line": "#8FAAF5" if night else "#5B79D6", "core": "#E6EEFF",
            "halo": "#6F8FE8", "gold": "#F2C766" if night else "#9C711C", "eye": "#F2C766",
            "eye_core": "#FFF4D6", "bars": "#F0784A", "rim": "#2B3239", "night": night}


def _local(origin, deg):
    return lambda x, y: add(origin, rot((x, y), deg))


def _path(pts, smooth_=False):
    return curve_d(pts) if smooth_ else "M" + " L".join(f"{f(x)},{f(y)}" for x, y in pts)


def glow(d, pts, C, w=0.010, colour=None, core=None, halo=None, opacity=1.0, smooth_=False):
    """A neon line: at night a translucent halo, the line, a pale core; on paper the line
    alone, sharp (a halo on paper would read as a smudge)."""
    colour = colour or C["line"]
    dd = _path(pts, smooth_)
    if C["night"]:
        d.raw(f'<path d="{dd}" fill="none" stroke="{halo or C["halo"]}" stroke-width="{f(w * 3.2)}" '
              f'stroke-linecap="round" stroke-linejoin="round" opacity="{f(0.24 * opacity)}"/>')
    d.raw(f'<path d="{dd}" fill="none" stroke="{colour}" stroke-width="{f(w * (1.0 if C["night"] else 1.15))}" '
          f'stroke-linecap="round" stroke-linejoin="round" opacity="{f(opacity)}"/>')
    if C["night"]:
        d.raw(f'<path d="{dd}" fill="none" stroke="{core or C["core"]}" stroke-width="{f(w * 0.38)}" '
              f'stroke-linecap="round" stroke-linejoin="round" opacity="{f(opacity)}"/>')


def ring(d, c, r, C, w=0.008, colour=None, opacity=1.0):
    pts = [add(c, (r * math.cos(math.radians(a)), r * math.sin(math.radians(a)))) for a in range(0, 361, 30)]
    glow(d, pts, C, w, colour=colour, opacity=opacity, smooth_=True)


def joint(d, c, r, side, C, opacity):
    """A short arc of light on a joint's outer side."""
    a0 = 0 if side > 0 else 180
    pts = [add(c, (r * math.cos(math.radians(a0 + a)), r * math.sin(math.radians(a0 + a)))) for a in range(-60, 61, 20)]
    glow(d, pts, C, 0.007, opacity=opacity, smooth_=True)


def edge(pts, widths, side):
    """The polyline along a tube's outer edge (the side away from the body, `side` −1/+1)."""
    out = []
    n = len(pts)
    for i in range(n):
        a, b = pts[max(i - 1, 0)], pts[min(i + 1, n - 1)]
        dx, dy = b[0] - a[0], b[1] - a[1]
        ln = math.hypot(dx, dy) or 1e-6
        nx, ny = -dy / ln, dx / ln
        if nx * side < 0:
            nx, ny = -nx, -ny
        h = widths[i] / 2 - 0.009
        out.append((pts[i][0] + nx * h, pts[i][1] + ny * h))
    return out


def draw(pose, th, cut_color=None, trails=None, floor=True, face=True):
    j = solve(pose, PROPS)
    P = lambda p: place(p, j)
    C = colours(th)
    d = Doc()
    gx = (P(j.feet[0])[0] + P(j.feet[1])[0]) / 2
    if floor:
        k = max(0.0, 1.0 - pose.lift * 3.0)
        d.raw(f'<ellipse cx="{f(gx)}" cy="0.008" rx="{f(0.16 * k + 0.05)}" ry="{f(0.022 * k + 0.008)}" '
              f'fill="{th["shadow"]}" opacity="0.55"/>')
        if C["night"]:   # the light on the floor
            d.raw(f'<ellipse cx="{f(gx)}" cy="0.006" rx="{f(0.13 * k + 0.03)}" ry="{f(0.014 * k + 0.004)}" '
                  f'fill="{C["halo"]}" opacity="{f(0.10 + 0.12 * pose.accent)}"/>')
    # the Halo: drawn by the hands, then dropped to the floor as a ring
    nk = P(j.neck)
    A = PROPS.arm
    if 0 < pose.sig <= 1.0:
        ang = 180 * pose.sig
        cx, cy, R = HALO
        for s in (-1, 1):
            pts = [add(nk, (cx + s * R * math.sin(math.radians(a)), cy + R * math.cos(math.radians(a))))
                   for a in [ang * i / 16 for i in range(17)]]
            glow(d, pts, C, 0.012, smooth_=True)
    elif pose.sig > 1.0:
        t = pose.sig - 1.0
        cx, c0, R = HALO
        cy = lerp(nk[1] + c0, -0.01, ease_out(clamp01(t / 0.35)))
        rx = R * (1 + 0.9 * t)
        ry = lerp(R, rx * 0.14, ease_out(clamp01(t / 0.35)))
        pts = [(gx + rx * math.cos(math.radians(a)), cy + ry * math.sin(math.radians(a))) for a in range(0, 361, 15)]
        glow(d, pts, C, 0.012 * (1 - t * 0.6), opacity=max(0.0, 1 - t), smooth_=True)
    # light trails: where the hands and feet just were
    if trails:
        for i in range(2):
            for limb in ("hands", "feet"):
                now = P(getattr(j, limb)[i])
                path = [now] + [place(getattr(t_, limb)[i], t_) for t_ in trails]
                travel = sum(length(sub(path[n], path[n + 1])) for n in range(len(path) - 1))
                if travel < 0.08:
                    continue
                for n in range(len(path) - 1):
                    glow(d, [path[n], path[n + 1]], C, 0.014 * (1 - n / len(path)),
                         opacity=0.85 * (1 - n / (len(path) - 1)))
    T = PROPS.torso
    L = _local(nk, j.torso_deg)
    suit = C["suit"]
    legs, arms = [], []
    # the suit: legs, shoes, torso, neck, head
    for i, s in enumerate((-1, 1)):
        hip, knee, foot = P(j.hips[i]), P(j.knees[i]), P(j.feet[i])
        top = add(add(hip, mul(sub(P(j.pelvis), hip), 0.3)), (0, -0.03))
        pts, ws = [top, knee, add(foot, (0, -0.040))], [0.078, 0.064, 0.058]
        d.path(tube_d(pts, ws), suit, cut=False)
        legs.append((pts, ws, s, knee))
    for i, s in enumerate((-1, 1)):
        foot = P(j.feet[i])
        ang = -s * j.toes[i] * 0.6
        base = add(foot, (s * 0.022, -0.016))
        up = [(-0.056, 0.0), (-0.050, -0.044), (-0.016, -0.060), (0.034, -0.050), (0.066, -0.018), (0.066, 0.0)]
        d.path(curve_d([add(base, rot((x * s, y), ang)) for x, y in up], closed=True), suit, cut=False)
        glow(d, [add(base, rot((-0.050 * s, -0.002), ang)), add(base, rot((0.062 * s, -0.002), ang))], C, 0.008)
    torso = [L(-0.040, -0.004), L(-0.108, 0.010), L(-0.120, 0.050), L(-0.090, 0.170), L(-0.084, T),
             L(-0.098, T + 0.040), L(0.098, T + 0.040), L(0.084, T), L(0.090, 0.170), L(0.120, 0.050),
             L(0.108, 0.010), L(0.040, -0.004)]
    d.path(curve_d(torso, closed=True), suit, cut=False)
    hc = P(j.head)
    H = _local(hc, j.head_deg)
    d.path(tube_d([L(0, 0.0), H(0, 0.06)], [0.040, 0.040]), suit, cut=False)
    head = [H(-0.064, -0.010), H(-0.058, -0.064), H(-0.020, -0.090), H(0.026, -0.088), H(0.062, -0.058),
            H(0.066, 0.004), H(0.050, 0.056), H(0.0, 0.080), H(-0.050, 0.056)]
    d.path(curve_d(head, closed=True), suit, cut=False)
    # the light on the suit
    for pts, ws, s, knee in legs:
        glow(d, edge(pts, ws, s), C, 0.009)
        joint(d, knee, 0.030, s, C, 0.6 + 0.4 * pose.accent)
    for s in (-1, 1):
        glow(d, [L(s * 0.100, 0.022), L(s * 0.112, 0.060), L(s * 0.084, 0.170), L(s * 0.078, T)], C, 0.009, smooth_=True)
    glow(d, [L(-0.044, 0.004), L(0.0, 0.030), L(0.044, 0.004)], C, 0.008, smooth_=True)
    glow(d, [L(-0.084, T + 0.006), L(0.084, T + 0.006)], C, 0.008)
    for n, x in enumerate((-0.036, -0.018, 0.0, 0.018, 0.036)):
        shape = (0.45, 0.75, 1.0, 0.75, 0.45)[n]
        hgt = 0.050 * shape * (0.35 + 0.65 * pose.level)
        glow(d, [L(x, 0.120 - hgt / 2), L(x, 0.120 + hgt / 2)], C, 0.008, colour=C["bars"], core="#FFE1D2",
             halo=C["bars"])
    # head: a gold headphone band and ring, lapis eyes
    band = [H(0.074 * math.cos(math.radians(a)), -0.008 + 0.090 * math.sin(math.radians(a))) for a in range(200, 341, 20)]
    glow(d, band, C, 0.007, colour=C["gold"], core="#FFF4D6", halo="#E2B85A", smooth_=True)
    glow(d, [H(-0.056, 0.040), H(-0.066, -0.006), H(-0.058, -0.052)], C, 0.006, opacity=0.8, smooth_=True)
    for s in (-1, 1):
        dd, tr = rrect_d(*H(s * 0.068, 0.006), 0.024, 0.044, 0.010, 0)
        d.path(dd, C["gold"], cut=False, transform=tr)
    if face:
        _eyes(d, H, pose, C)
    # arms over the torso: suit, then their light
    for i, s in enumerate((-1, 1)):
        sh, el, ha = P(j.shoulders[i]), P(j.elbows[i]), P(j.hands[i])
        sh = add(sh, mul(sub(nk, sh), 0.12))
        pts, ws = [sh, el, ha], [0.068, 0.056, 0.048]
        d.raw(f'<path d="{tube_d(pts, [w + 0.014 for w in ws])}" fill="{C["rim"]}"/>')
        d.path(tube_d(pts, ws), suit, cut=False)
        fd = sub(ha, el)
        fl = length(fd) or 1e-6
        hand = add(ha, mul(fd, 0.026 / fl))
        d.circle(hand, 0.030, suit, cut=False)
        glow(d, edge(pts, ws, s), C, 0.008)
        joint(d, el, 0.027, s, C, 0.6 + 0.4 * pose.accent)
    return d.svg()


def _eyes(d, H, pose, C):
    e = pose.eyes
    for s in (-1, 1):
        x = s * 0.027
        if e == "happy":
            pts = [H(x - 0.014, 0.006), H(x, -0.006), H(x + 0.014, 0.006)]
        elif e in ("closed", "sleepy"):
            pts = [H(x - 0.014, 0.004), H(x + 0.014, 0.004)]
        elif e == "wink" and s > 0:
            pts = [H(x - 0.014, 0.002), H(x + 0.014, 0.002)]
        elif e == "wide":
            pts = [H(x, -0.004), H(x, 0.008)]
        elif e == "cool":
            pts = [H(-0.048, 0.0), H(0.048, 0.0)] if s < 0 else None
        else:
            pts = [H(x - 0.014 * s, 0.003), H(x + 0.012 * s, -0.003)]
        if pts:
            w = 0.012 if e == "wide" else (0.006 if e == "sleepy" else 0.009)
            glow(d, pts, C, w, colour=C["eye"], core=C["eye_core"], halo=C["eye"],
                 opacity=0.5 if e == "sleepy" else 1.0)


def key_poses():
    st = STYLE
    return [("Idle", Pose(eyes="open", hip=(0.04, 0.02), lean=-2, tilt=6, hands=((-0.52, 0.96), (0.44, 0.90)),
                          feet=((0, 0), (0.06, 0)), toe=(0, 10), level=0.4)),
            ("Tut 1", tut(0.0, st)), ("Tut 2", tut(0.5, st)), ("Tut 3", tut(1.0, st)), ("Glide", glide(1.0, st)),
            ("Draw", halo(0.5, st)), ("Close", halo(0.98, st)), ("Drop", halo(1.12, st)), ("Land", halo(1.35, st))]


def mood_items():
    return [("Neutral", Pose(eyes="open")), ("Happy", Pose(eyes="happy")), ("Excited", Pose(eyes="wide", level=1.0)),
            ("Calm", Pose(eyes="closed", tilt=6)), ("Surprised", Pose(eyes="wide")),
            ("Sleepy", Pose(eyes="sleepy", nod=14, tilt=12, level=0.1)), ("Cool", Pose(eyes="cool", tilt=-6)),
            ("Wink", Pose(eyes="wink", tilt=8))]


MOOD_SIZE = 420
