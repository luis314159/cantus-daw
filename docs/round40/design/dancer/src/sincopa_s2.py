"""Síncopa S2 — **the sidekick**: feature-animation proportions, a face that acts.

The character design:
- **Proportions:** a big head (about 3.5 heads tall), big hands and feet, long thin legs.
- **Face:** huge eyes with pupils and a highlight, thick brows, and a wide mouth that grins
  with teeth, shouts and smirks.
- **Hair:** a curly puff with a gold headband that bounces after the head.
- **Clothes:** an oversized vermilion tee, lapis shorts, white tube socks with lapis stripes,
  chunky white sneakers with gold laces.
- **Shading:** two flat values per piece (the rig's hard cel), no lines.

It dances like an animated sidekick: big anticipation, squash and stretch, the arms pumping
over the head, the hair and hands following through. Signature, **Hype jump**: a deep crouch,
a stretched launch, a star in the air with a shout, a squashed landing with dust.
"""
import math
from body import Props, solve, place
from moves import Style, MOVES as ALL_MOVES, steps, _hands, _lerp_pair
from rig import (Pose, add, sub, mul, rot, length, lerp, beat_down, cos1, smooth, ease_out,
                 ease_in_out, ease_out_back, clamp01, arc)
from svg import Doc, tube_d, rrect_d, curve_d, f

NAME = "Síncopa S2"
PROPS = Props(leg=0.40, thigh=0.205, shin=0.198, torso=0.20, neck=0.018, head=0.148,
              shoulder=0.082, hipw=0.042, stance=0.072, upper=0.168, fore=0.162, foot=0.14)
STYLE = Style(bounce=0.14, sway=0.08, step=0.20, arms=1.2, lag=0.14, hop=0.30, squash=0.14,
              snap=0.0, lean=1.4, lift_step=0.12)
MOVES = {"groove": "s2_bop", "steps": "s2_shuffle", "jump": "s2_hype"}
CUT = 0.012


def lp(a, b, t):
    return (lerp(a[0], b[0], t), lerp(a[1], b[1], t))


def bop(b, st):
    """Two beats: a big bounce, a fist pumped over the head on each beat (one, then the
    other), the other arm swinging low; a heel kicks back; the mouth cheers on the beat."""
    p = Pose()
    ph = b % 1.0
    k = int(math.floor(b)) % 2
    down = beat_down(ph, 1.8)
    up = beat_down((ph + 0.5) % 1.0, 2.0)
    p.hip = (0.04 * cos1(b / 2), st.bounce * down)
    p.squash = 0.08 * beat_down(ph, 4.0) - 0.05 * up
    p.lean = 6.0 * cos1(b / 2)
    p.nod = 14.0 * beat_down((b - st.lag) % 1.0) - 6.0
    p.tilt = -8.0 * cos1(b / 2)
    pump = smooth(1.0 - abs(ph - 0.0) * 2.2) if ph < 0.5 else smooth((ph - 0.55) * 2.2)
    hi, lo = (0.62, -1.25), (0.46, 0.88)
    r = lp(lo, hi, beat_down(ph, 1.2)) if k == 0 else lo
    l = lp((-lo[0], lo[1]), (-hi[0], hi[1]), beat_down(ph, 1.2)) if k == 1 else (-lo[0], lo[1])
    p.hands = (l, r)
    p.feet = ((0.0, -0.10 * up if k == 1 else 0.0), (0.0, -0.10 * up if k == 0 else 0.0))
    p.mouth = 0.9 * beat_down(ph, 3.0)
    p.eyes = "happy" if p.mouth > 0.4 else "open"
    p.accent = beat_down(ph, 8)
    p.point = 30 * cos1((b - st.lag) / 2)
    return p


def shuffle(b, st):
    p = steps(b, st)
    ph = b % 1.0
    p.squash = 0.10 * beat_down(ph, 4.0)
    a = st.arms
    keys = {0.0: ((-0.85, 0.30), (0.85, 0.30)), 1.0: ((-0.30, 0.55), (0.80, -0.70)),
            2.0: ((-0.85, 0.30), (0.85, 0.30)), 3.0: ((-0.80, -0.70), (0.30, 0.55))}
    h0, h1, t = _hands(keys, b, 4.0)
    p.hands = _lerp_pair(h0, h1, ease_in_out(t))
    p.eyes = "happy" if (b % 2.0) > 0.8 else "open"
    p.mouth = 0.4 * beat_down(ph, 3.0)
    p.point = 30 * math.sin(math.pi * (b - st.lag) / 2.0)
    return p


def hype(b, st):
    """Two beats, its signature: crouch deep (squashed, arms swung back), launch stretched,
    a star in the air with a shout, a squashed landing with dust on the next beat."""
    p = Pose()
    bb = b % 2.0
    crouch = smooth(bb / 0.7)
    air = clamp01((bb - 0.75) / 1.0)
    inair = 0.75 <= bb < 1.75
    land = clamp01((bb - 1.75) / 0.25)
    if not inair and bb < 0.75:
        p.squash = 0.22 * crouch
        p.hip = (0.0, 0.30 * crouch)
        p.hands = ((-0.55, 0.95), (0.55, 0.95)) if crouch < 0.3 else ((-0.75, 0.75), (0.75, 0.75))
        p.nod = 18 * crouch
        p.eyes = "closed" if crouch > 0.6 else "open"
        p.mouth = 0.0
    elif inair:
        p.lift = st.hop * arc(air) ** 0.8
        p.squash = -0.16 * math.sin(math.pi * clamp01(air / 0.35)) + 0.0
        star = math.sin(math.pi * clamp01((air - 0.05) / 0.85))
        p.feet = ((-0.20 * star, -0.12 * star), (0.20 * star, -0.12 * star))
        p.hands = _lerp_pair(((-0.75, 0.75), (0.75, 0.75)), ((-0.90, -1.10), (0.90, -1.10)), ease_out(clamp01(air / 0.25)))
        p.nod = -14 * star
        p.eyes = "wide"
        p.mouth = 1.0 * star + 0.2
    else:
        p.squash = 0.18 * (1 - ease_out(land))
        p.hip = (0.0, 0.18 * (1 - ease_out(land)))
        p.hands = ((-1.0, 0.30), (1.0, 0.30))
        p.landed = 1 - land
        p.eyes = "happy"
        p.mouth = 0.6
    p.accent = beat_down(b % 1.0, 8)
    p.point = 40 * (p.lift * 4) if inair else -20 * crouch
    return p


ALL_MOVES.update({"s2_bop": (bop, 2.0), "s2_shuffle": (shuffle, 4.0), "s2_hype": (hype, 2.0)})

PAL = {"skin": "#8D5A3B", "skin_s": "#6E4329", "hair": "#2A1E1B", "hair_l": "#5C4238",
       "tee": "#E0693B", "tee_s": "#B24E29", "shorts": "#4664C0", "shorts_s": "#2E4794",
       "sock": "#F3F5F7", "shoe": "#F3F5F7", "shoe_s": "#BAC3CC", "sole": "#2B3239",
       "gold": "#E2B85A", "white": "#FFFFFF", "pupil": "#1A1414", "mouth": "#4A1E18",
       "tongue": "#D46A5A", "brow": "#2A1E1B"}


def _local(origin, deg):
    return lambda x, y: add(origin, rot((x, y), deg))


def draw(pose, th, cut_color=None, trails=None, floor=True, face=True):
    j = solve(pose, PROPS)
    P = lambda p: place(p, j)
    c = PAL
    room = cut_color or th["base"]
    d = Doc(cut_color=room, cut=CUT)
    gx = (P(j.feet[0])[0] + P(j.feet[1])[0]) / 2
    if floor:
        k = max(0.0, 1.0 - pose.lift * 3.0)
        d.raw(f'<ellipse cx="{f(gx)}" cy="0.010" rx="{f(0.20 * k + 0.05)}" ry="{f(0.028 * k + 0.008)}" '
              f'fill="{th["shadow"]}" opacity="0.5"/>')
        if pose.landed > 0.05:
            for sd in (-1, 1):
                r = 0.03 + 0.06 * (1 - pose.landed)
                d.raw(f'<ellipse cx="{f(gx + sd * (0.16 + 0.08 * (1 - pose.landed)))}" cy="-0.014" rx="{f(r)}" '
                      f'ry="{f(r * 0.5)}" fill="{th["ink4"]}" opacity="{f(pose.landed)}"/>')
    # legs: long and thin, tube socks with lapis stripes, big sneakers
    for i, s in enumerate((-1, 1)):
        hip, knee, foot = P(j.hips[i]), P(j.knees[i]), P(j.feet[i])
        ankle = add(foot, (0, -0.055))
        d.path(tube_d([hip, knee, ankle], [0.046, 0.040, 0.038]), c["skin"], cut=False)
        sock0 = add(knee, mul(sub(ankle, knee), 0.35))
        d.path(tube_d([sock0, ankle], [0.050, 0.048]), c["sock"], cut=False)
        for t in (0.12, 0.26):
            a = add(sock0, mul(sub(ankle, sock0), t))
            dd = sub(ankle, sock0)
            ln = length(dd) or 1e-6
            nx, ny = -dd[1] / ln, dd[0] / ln
            d.line([add(a, (nx * 0.024, ny * 0.024)), add(a, (-nx * 0.024, -ny * 0.024))], 0.010, c["shorts"], cut=False, cap="butt")
    for i, s in enumerate((-1, 1)):
        foot = P(j.feet[i])
        ang = -s * j.toes[i] * 0.6
        base = add(foot, (s * 0.030, -0.018))
        dd, tr = rrect_d(base[0], base[1] + 0.008, 0.180, 0.030, 0.015, ang)
        d.path(dd, c["sole"], cut=False, transform=tr)
        up = [(-0.075, 0.0), (-0.070, -0.060), (-0.030, -0.085), (0.030, -0.070), (0.090, -0.034), (0.090, 0.0)]
        d.cel(curve_d([add(base, rot((x * s, y), ang)) for x, y in up], closed=True), c["shoe"], c["shoe_s"],
              off=(-0.008, -0.010), cut=False)
        for n in range(3):
            a = add(base, rot((s * (-0.010 + n * 0.022), -0.060 + n * 0.010), ang))
            d.line([a, add(a, rot((s * 0.020, 0.004), ang))], 0.010, c["gold"], cut=False)
    # shorts, then the oversized tee over their top
    T = PROPS.torso
    L = _local(P(j.neck), j.torso_deg)
    for i, s in enumerate((-1, 1)):
        hip, knee = P(j.hips[i]), P(j.knees[i])
        top = add(add(hip, mul(sub(P(j.pelvis), hip), 0.4)), (0, -0.03))
        end = add(hip, mul(sub(knee, hip), 0.72))
        d.cel(tube_d([top, end], [0.118, 0.104]), c["shorts"], c["shorts_s"], off=(-0.012, -0.004), cut=False)
    tee = [L(-0.040, -0.006), L(-0.105, 0.010), L(-0.122, 0.060), L(-0.112, T + 0.050),
           L(0.112, T + 0.050), L(0.122, 0.060), L(0.105, 0.010), L(0.040, -0.006)]
    d.cel(curve_d(tee, closed=True), c["tee"], c["tee_s"], off=(-0.016, -0.012), cut=False)
    d.path(curve_d([L(-0.040, -0.006), L(0.0, 0.040), L(0.040, -0.006), L(0.0, 0.016)], closed=True), c["skin_s"], cut=False)
    # the head: hair puff behind, ears, a big face, the gold headband
    hc = P(j.head)
    H = _local(hc, j.head_deg)
    bob = -0.012 * pose.accent + 0.0004 * pose.point
    puff = []
    for n in range(18):
        a = math.radians(n * 20)
        r = 1.0 if n % 2 == 0 else 0.90
        puff.append(H(0.190 * r * math.cos(a), -0.110 + bob + 0.150 * r * math.sin(a) * (1 + 0.08 * pose.accent)))
    d.cel(curve_d(puff, closed=True), c["hair_l"], c["hair"], off=(-0.016, -0.016), cut=True)
    for s in (-1, 1):
        d.circle(H(s * 0.140, 0.020), 0.032, c["skin_s"], cut=False)
    head = [H(-0.135, -0.040), H(-0.120, -0.110), H(-0.060, -0.150), H(0.0, -0.156), H(0.060, -0.150),
            H(0.120, -0.110), H(0.135, -0.040), H(0.132, 0.040), H(0.100, 0.110), H(0.040, 0.145),
            H(0.0, 0.150), H(-0.040, 0.145), H(-0.100, 0.110), H(-0.132, 0.040)]
    d.cel(curve_d(head, closed=True), c["skin"], c["skin_s"], off=(-0.016, -0.008), cut=False)
    fringe = [H(-0.130, -0.060), H(-0.090, -0.120), H(-0.030, -0.140), H(0.040, -0.138), H(0.100, -0.118),
              H(0.132, -0.064), H(0.090, -0.096), H(0.030, -0.104), H(-0.040, -0.100), H(-0.100, -0.084)]
    d.cel(curve_d(fringe, closed=True), c["hair_l"], c["hair"], off=(-0.008, -0.008), cut=False)
    d.line([H(-0.128, -0.080), H(-0.060, -0.122), H(0.0, -0.128), H(0.060, -0.122), H(0.128, -0.080)],
           0.024, c["gold"], cut=False, smooth=True)
    if face:
        _face(d, H, pose, c)
    # arms: tee sleeves, thin arms, big hands
    for i, s in enumerate((-1, 1)):
        sh, el, ha = P(j.shoulders[i]), P(j.elbows[i]), P(j.hands[i])
        sh = add(sh, mul(sub(P(j.neck), sh), 0.10))
        sleeve_end = add(sh, mul(sub(el, sh), 0.55))
        d.path(tube_d([add(sh, mul(sub(el, sh), 0.25)), el, ha], [0.040, 0.036, 0.034]), c["skin"], cut=True)
        d.cel(tube_d([sh, sleeve_end], [0.096, 0.086]), c["tee"], c["tee_s"], off=(-0.010, -0.008), cut=False)
        fd = sub(ha, el)
        fl = length(fd) or 1e-6
        fd = mul(fd, 1 / fl)
        hand = add(ha, mul(fd, 0.030))
        ang = math.degrees(math.atan2(fd[1], fd[0]))
        d.ellipse(hand, 0.052, 0.044, c["skin_s"], cut=True, deg=ang)
        d.ellipse(add(hand, (-0.004, -0.004)), 0.046, 0.039, c["skin"], deg=ang)
        thumb = add(hand, rot((-0.010, -s * 0.038), ang))
        d.circle(thumb, 0.017, c["skin"], cut=False)
    return d.svg()


def _face(d, H, pose, c):
    e = pose.eyes
    lk = (0.0004 * pose.point, 0.0)
    for s in (-1, 1):
        ex, ey = 0.054 * s, -0.020
        if e in ("happy", "closed", "sleepy") or (e == "wink" and s > 0):
            up = e == "happy" or e == "wink"
            pts = ([H(ex - 0.030, ey + 0.012), H(ex, ey - 0.014), H(ex + 0.030, ey + 0.012)] if up else
                   [H(ex - 0.030, ey - 0.002), H(ex, ey + 0.016), H(ex + 0.030, ey - 0.002)])
            d.line(pts, 0.014, c["pupil"], cut=False, smooth=True)
        elif e == "cool":
            dd, tr = rrect_d(*H(ex, ey), 0.084, 0.052, 0.022, 0)
            d.path(dd, c["gold"], cut=False, transform=tr)
            dd, tr = rrect_d(*H(ex, ey), 0.072, 0.040, 0.018, 0)
            d.path(dd, c["pupil"], cut=False, transform=tr)
            d.line([H(ex - 0.024, ey - 0.008), H(ex - 0.010, ey - 0.012)], 0.006, "#FFFFFF", cut=False)
        else:
            big = 1.18 if e == "wide" else 1.0
            d.ellipse(H(ex, ey), 0.038 * big, 0.048 * big, c["white"])
            pr = 0.016 if e == "wide" else 0.021
            pc = H(ex + lk[0] + 0.004 * s * 0, ey + 0.006)
            d.circle(pc, pr, c["pupil"], cut=False)
            d.circle(add(pc, (-0.007, -0.008)), 0.006, c["white"], cut=False)
        lift = 0.016 if e in ("wide", "happy") else 0.0
        tilt = -0.010 * s if e == "cool" else 0.0
        d.line([H(ex - 0.034, -0.080 - lift + 0.004), H(ex, -0.090 - lift + tilt), H(ex + 0.034, -0.082 - lift - tilt)],
               0.016, c["brow"], cut=False, smooth=True)
    d.circle(H(0.006, 0.040), 0.016, c["skin_s"], cut=False)
    if pose.mouth > 0.25:
        o = pose.mouth
        top = 0.068
        shape = [H(-0.058, top), H(0.058, top), H(0.040, top + 0.050 * o + 0.012), H(0.0, top + 0.068 * o + 0.014),
                 H(-0.040, top + 0.050 * o + 0.012)]
        d.path(curve_d(shape, closed=True), c["mouth"], cut=False)
        d.path(curve_d([H(-0.050, top + 0.002), H(0.050, top + 0.002), H(0.044, top + 0.016), H(-0.044, top + 0.016)],
                       closed=True), c["white"], cut=False)
        d.ellipse(H(0.0, top + 0.050 * o + 0.004), 0.024, 0.010 * o + 0.004, c["tongue"])
    elif e == "cool":
        d.line([H(-0.034, 0.080), H(0.010, 0.084), H(0.040, 0.070)], 0.012, c["mouth"], cut=False, smooth=True)
    else:
        w = 0.050 if e in ("happy", "wink") else 0.036
        d.line([H(-w, 0.070), H(0.0, 0.090), H(w, 0.070)], 0.012, c["mouth"], cut=False, smooth=True)


def key_poses():
    st = STYLE
    return [("Idle", Pose(eyes="open", hip=(0.03, 0.02), tilt=-6, hands=((-0.60, 0.90), (0.60, 0.90)), toe=(0, 12))),
            ("Bop", bop(0.0, st)), ("Pump", bop(1.0, st)), ("Shuffle", shuffle(1.0, st)),
            ("Crouch", hype(0.6, st)), ("Launch", hype(0.82, st)), ("Hype!", hype(1.15, st)),
            ("Landing", hype(1.8, st)), ("Cool", Pose(eyes="cool", lean=-6, hands=((-0.40, 0.50), (0.85, -0.20)),
                                                       feet=((0, 0), (0.10, 0)), toe=(0, 14)))]


def mood_items():
    return [("Neutral", Pose(eyes="open")), ("Happy", Pose(eyes="happy")), ("Excited", Pose(eyes="wide", mouth=0.9)),
            ("Calm", Pose(eyes="closed", tilt=6)), ("Surprised", Pose(eyes="wide", mouth=0.5)),
            ("Sleepy", Pose(eyes="sleepy", nod=14, tilt=12)), ("Cool", Pose(eyes="cool", tilt=-6)),
            ("Wink", Pose(eyes="wink", tilt=8, mouth=0.0))]


MOOD_SIZE = 300
