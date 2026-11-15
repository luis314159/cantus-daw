"""New — **Voz**, the microphone: a crooner, smooth and showy.

Its head is a vintage microphone's capsule — a grille of fine bars over a metal body, a gold
ring at its base — under a pork-pie hat with a gold band. It wears the evening: a dinner
jacket that is white at night and black on paper (it inverts with the theme, as the inks do),
a gold bow tie, a vermilion pocket square, slim trousers, two-tone spectator shoes. It moves
like a crooner: a hand on the heart, then out to the crowd; a finger-snapping slide; and its
signature, **Hat tip**: on the drop it tips its hat with a bow, then flings both arms out —
ta-da — and holds it.
"""
import math
from body import Props, solve, place
from moves import Style, MOVES as ALL_MOVES, steps, _hands, _lerp_pair
from rig import (Pose, add, sub, mul, rot, length, lerp, snap_time, beat_down, cos1, sin1,
                 smooth, ease_out_back, ease_in_out, clamp01)
from svg import Doc, tube_d, rrect_d, curve_d, f

NAME = "Voz"
PROPS = Props(leg=0.49, thigh=0.255, shin=0.240, torso=0.262, neck=0.050, head=0.088,
              shoulder=0.090, hipw=0.044, stance=0.058, upper=0.155, fore=0.150, foot=0.10)
STYLE = Style(bounce=0.05, sway=0.07, step=0.16, arms=1.0, lag=0.16, hop=0.10, squash=0.04,
              snap=0.30, lean=1.0, lift_step=0.03, chest=-0.04)
MOVES = {"groove": "voz_croon", "steps": "voz_slide", "jump": "voz_hattip"}
CUT = 0.011


def lp(a, b, t):
    return (lerp(a[0], b[0], t), lerp(a[1], b[1], t))


def croon(b, st):
    """Two beats: on beat 1 the right hand to the heart, on beat 2 out to the crowd, palm
    up, the head back; the left hand in the pocket; a smooth sway."""
    b = snap_time(b, 0.5, move=0.5, amount=st.snap)
    p = Pose()
    ph = b % 1.0
    k = int(math.floor(b)) % 2
    h0, h1, t = _hands({0.0: ((-0.36, 0.86), (0.04, 0.44)), 1.0: ((-0.36, 0.86), (0.96, 0.10))}, b, 2.0)
    p.hands = _lerp_pair(h0, h1, ease_in_out(t))
    side = cos1(b / 2.0)
    p.hip = (0.06 * side, st.bounce * beat_down(ph, 2.0))
    p.chest = (st.chest * side, 0.0)
    p.lean = -2.0 - 4.0 * (1 - side) / 2
    p.nod = 6.0 * side - 4.0
    p.tilt = -6.0 * side
    p.toe = (0.0, 10.0 * max(0.0, -side))
    p.eyes = "closed" if side > 0.3 else "happy"
    p.level = 0.5 + 0.4 * beat_down(ph, 2.0)
    p.accent = beat_down(ph, 8)
    return p


def slide(b, st):
    """Four beats: a smooth slide, the feet barely lifting, both hands snapping fingers at
    shoulder height, one up on each beat."""
    p = steps(b, st)
    bs = snap_time(b, 0.5, move=0.35, amount=0.6)
    a, bb = ((-0.58, 0.34), (0.58, 0.34)), None
    up_l, up_r = ((-0.66, 0.06), (0.58, 0.34)), ((-0.58, 0.34), (0.66, 0.06))
    h0, h1, t = _hands({0.0: up_r, 0.5: a, 1.0: up_l, 1.5: a}, bs, 2.0)
    p.hands = _lerp_pair(h0, h1, t)
    p.claps = beat_down(b % 1.0, 8)
    p.eyes = "open"
    p.tilt = 8.0 * math.sin(math.pi * b / 2.0)
    p.lean *= 0.6
    return p


def hattip(b, st):
    """Two beats, its signature: the hand to the brim, the hat tipped with a bow; then the
    hat back on and both arms flung out — ta-da — leaning back, a knee popped; held."""
    p = Pose()
    bb = b % 2.0
    reach = smooth(bb / 0.35)
    tip = smooth((bb - 0.35) / 0.35) * (1 - smooth((bb - 0.85) / 0.2))
    tada = ease_out_back(clamp01((bb - 0.95) / 0.30), 2.2)
    out = smooth((bb - 1.8) / 0.2)
    brim = (0.30, -0.95)
    right = lp((0.42, 0.90), brim, reach)
    right = lp(right, (0.48, -1.12), tip)
    right = lp(right, (1.02, -0.30), tada)
    right = lp(right, (0.42, 0.90), out)
    left = lp((-0.42, 0.90), (-0.24, 0.72), reach * (1 - tada))
    left = lp(left, (-1.02, -0.20), tada * (1 - out))
    p.hands = (left, right)
    p.sig = tip
    p.nod = 22.0 * tip - 10.0 * tada * (1 - out)
    p.lean = -9.0 * tada * (1 - out)
    p.hip = (0.04 * tada * (1 - out), 0.02 + 0.05 * tip)
    p.chest = (0.0, 0.05 * tip)
    p.feet = ((-0.06 * tada, 0.0), (0.10 * tada, 0.0))
    p.toe = (0.0, 22.0 * tada * (1 - out))
    p.knee = (1.0, -1.0)
    p.eyes = "closed" if tip > 0.3 else ("happy" if tada > 0.3 else "open")
    p.level = 0.5 + 0.5 * tada * (1 - out)
    p.accent = beat_down(b % 1.0, 8)
    return p


ALL_MOVES.update({"voz_croon": (croon, 2.0), "voz_slide": (slide, 4.0), "voz_hattip": (hattip, 2.0)})


def colours(th):
    night = th["name"] == "Night"
    return {
        "jacket": "#E9E4DA" if night else "#232A31", "jacket_s": "#BDB4A4" if night else "#14191E",
        "lapel": "#D2CABB" if night else "#323A42", "shirt": "#14191E" if night else "#F3F5F7",
        "pants": "#59636E" if night else "#2B3239", "pants_s": "#3E4752" if night else "#1D2329",
        "grille": "#C3CBD3" if night else "#6A7682", "grille_s": "#8A96A2" if night else "#47515B",
        "bar": "#7D8996" if night else "#3A434C", "body": "#3A434C" if night else "#2B3239",
        "hat": "#2B3239", "hat_rim": "#76818D" if night else "#2B3239",
        "eye": "#14191E" if night else "#F3F5F7",
        "shoe_a": "#F3F5F7", "shoe_b": "#14191E",
        "skin": "#B9825F",
    }


def _local(origin, deg):
    return lambda x, y: add(origin, rot((x, y), deg))


def draw(pose, th, cut_color=None, trails=None, floor=True, face=True):
    j = solve(pose, PROPS)
    P = lambda p: place(p, j)
    C = colours(th)
    room = cut_color or th["base"]
    d = Doc(cut_color=room, cut=CUT)
    gx = (P(j.feet[0])[0] + P(j.feet[1])[0]) / 2
    if floor:
        k = max(0.0, 1.0 - pose.lift * 3.0)
        d.raw(f'<ellipse cx="{f(gx)}" cy="0.008" rx="{f(0.15 * k + 0.05)}" ry="{f(0.022 * k + 0.008)}" '
              f'fill="{th["shadow"]}" opacity="0.5"/>')
    # slim trousers, spectator shoes
    for i, s in enumerate((-1, 1)):
        hip, knee, foot = P(j.hips[i]), P(j.knees[i]), P(j.feet[i])
        top = add(add(hip, mul(sub(P(j.pelvis), hip), 0.3)), (0, -0.03))
        d.cel(tube_d([top, knee, add(foot, (0, -0.040))], [0.084, 0.068, 0.064]), C["pants"], C["pants_s"],
              off=(-0.011, -0.004), cut=False)
    for i, s in enumerate((-1, 1)):
        foot = P(j.feet[i])
        ang = -s * j.toes[i] * 0.6
        base = add(foot, (s * 0.024, -0.012))
        up = [(-0.050, 0.004), (-0.046, -0.030), (-0.010, -0.040), (0.040, -0.026), (0.070, -0.004), (0.068, 0.006)]
        d.path(curve_d([add(base, rot((x * s, y), ang)) for x, y in up], closed=True), C["shoe_b"], cut=False)
        up2 = [(-0.044, -0.002), (-0.040, -0.028), (-0.008, -0.036), (0.020, -0.028), (0.022, -0.002)]
        d.path(curve_d([add(base, rot((x * s, y), ang)) for x, y in up2], closed=True), C["shoe_a"], cut=False)
    # the dinner jacket: shirt, two halves with lapels, a pocket square, a gold bow tie
    T = PROPS.torso
    L = _local(P(j.neck), j.torso_deg)
    hl = T + 0.040
    d.path(curve_d([L(-0.050, 0.0), L(0.050, 0.0), L(0.050, 0.15), L(-0.050, 0.15)], closed=True), C["shirt"], cut=False)
    for s in (-1, 1):
        half = [L(s * 0.030, -0.004), L(s * 0.092, 0.006), L(s * 0.116, 0.048), L(s * 0.110, 0.140),
                L(s * 0.098, 0.230), L(s * 0.104, hl), L(s * 0.010, hl + 0.012), L(s * 0.004, 0.180),
                L(s * 0.026, 0.070)]
        d.cel(curve_d(half, closed=True), C["jacket"], C["jacket_s"], off=(-0.014, -0.010), cut=False)
        lap = [L(s * 0.030, -0.002), L(s * 0.060, 0.030), L(s * 0.040, 0.050), L(s * 0.008, 0.172)]
        d.path("M" + " L".join(f"{f(x)},{f(y)}" for x, y in lap) + " Z", C["lapel"], cut=False)
    d.path("M" + " L".join(f"{f(x)},{f(y)}" for x, y in [L(-0.088, 0.075), L(-0.058, 0.075), L(-0.068, 0.096)]) + " Z",
           th["verm"], cut=False)
    d.path(curve_d([L(-0.034, 0.000), L(0.0, 0.014), L(0.034, 0.000), L(0.034, 0.030), L(0.0, 0.016),
                    L(-0.034, 0.030)], closed=True), th["gold"] if th["name"] == "Night" else "#C99A3A", cut=False)
    # the microphone's body up to the gold ring, the capsule, the hat
    hc = P(j.head)
    H = _local(hc, j.head_deg)
    d.path(tube_d([L(0, 0.004), H(0, 0.105)], [0.040, 0.050]), C["body"], cut=False)
    _capsule(d, H, pose, C, th, face)
    _hat(d, H, pose, C, th)
    # sleeves, shirt cuffs, white gloves
    for i, s in enumerate((-1, 1)):
        sh, el, ha = P(j.shoulders[i]), P(j.elbows[i]), P(j.hands[i])
        sh = add(sh, mul(sub(P(j.neck), sh), 0.15))
        fdir = sub(ha, el)
        fl = length(fdir) or 1e-6
        fdir = mul(fdir, 1 / fl)
        wrist = sub(ha, mul(fdir, 0.014))
        free = add(sh, mul(sub(el, sh), 0.6))
        d.line([free, el, wrist], 0.064 + 2 * CUT, room, cut=False)
        d.cel(tube_d([sh, el, wrist], [0.074, 0.064, 0.058]), C["jacket"], C["jacket_s"], off=(-0.010, -0.009), cut=False)
        d.path(tube_d([sub(wrist, mul(fdir, 0.006)), add(wrist, mul(fdir, 0.010))], [0.056, 0.054]), C["shirt"], cut=False)
        d.ellipse(add(ha, mul(fdir, 0.018)), 0.030, 0.033, "#BAC3CC", deg=math.degrees(math.atan2(fdir[1], fdir[0])))
        d.ellipse(add(ha, mul(fdir, 0.014)), 0.027, 0.030, "#F3F5F7", deg=math.degrees(math.atan2(fdir[1], fdir[0])))
    return d.svg()


def _capsule(d, H, pose, C, th, face):
    """The capsule: a pill of grille over a gold ring; fine bars; eyes on the grille."""
    w, h = 0.150, 0.215
    cx, cy = H(0, -0.010)
    deg = math.degrees(math.atan2(H(1, 0)[1] - H(0, 0)[1], H(1, 0)[0] - H(0, 0)[0]))
    dd, tr = rrect_d(cx, cy, w, h, w / 2, deg)
    d.cel(dd, C["grille"], C["grille_s"], off=(-0.012, -0.008), cut=True, transform=tr)
    for n in range(-7, 8):
        y = n * 0.0125
        half = (w / 2 - 0.012) * math.sqrt(max(0.0, 1 - max(0.0, abs(y) - (h / 2 - w / 2)) ** 2 / (w / 2) ** 2))
        if half > 0.012:
            d.line([H(-half, -0.010 + y), H(half, -0.010 + y)], 0.0045, C["bar"], cut=False)
    rx, ry = H(0, 0.100)
    dd, tr = rrect_d(rx, ry, 0.136, 0.030, 0.012, deg)
    d.cel(dd, "#E2B85A", "#B38A35", off=(-0.006, -0.004), cut=False, transform=tr)
    if face:
        e = pose.eyes
        for s in (-1, 1):
            c = H(s * 0.032, 0.008)
            if e == "happy":
                d.line([H(s * 0.032 - 0.014, 0.014), H(s * 0.032, 0.000), H(s * 0.032 + 0.014, 0.014)], 0.010, C["eye"], cut=False)
            elif e in ("closed", "sleepy"):
                d.line([H(s * 0.032 - 0.014, 0.006), H(s * 0.032, 0.014), H(s * 0.032 + 0.014, 0.006)], 0.009, C["eye"], cut=False)
            elif e == "cool":
                dd2, tr2 = rrect_d(*H(s * 0.032, 0.006), 0.050, 0.026, 0.010, deg)
                d.path(dd2, "#14191E", cut=False, transform=tr2)
            elif e == "wink" and s > 0:
                d.line([H(s * 0.032 - 0.014, 0.008), H(s * 0.032 + 0.014, 0.008)], 0.009, C["eye"], cut=False)
            else:
                big = 1.3 if e == "wide" else 1.0
                d.ellipse(c, 0.012 * big, 0.017 * big, C["eye"], deg=deg)
        if e == "cool":
            d.line([H(-0.010, 0.002), H(0.010, 0.002)], 0.006, "#14191E", cut=False)
            d.line([H(-0.050, -0.002), H(-0.040, 0.010)], 0.004, th["gold"], cut=False)


def _hat(d, H, pose, C, th):
    """A pork-pie hat with a gold band; tipped (lifted and turned) by the signature."""
    k = pose.sig
    piv = H(0.110, -0.112)
    T = lambda x, y: rot(add(H(x, y), (0.0, -0.06 * k)), -22 * k, piv)
    deg = math.degrees(math.atan2(H(1, 0)[1] - H(0, 0)[1], H(1, 0)[0] - H(0, 0)[0])) - 22 * k
    for (x, y, w, h, r, col) in ((0.0, -0.118, 0.236, 0.026, 0.013, "brim"), (0.0, -0.150, 0.150, 0.060, 0.012, "crown")):
        cx, cy = T(x, y)
        dd, tr = rrect_d(cx, cy, w + 0.010, h + 0.010, r + 0.004, deg)
        d.path(dd, C["hat_rim"], cut=True, transform=tr)
        dd, tr = rrect_d(cx, cy, w, h, r, deg)
        d.path(dd, C["hat"], cut=False, transform=tr)
    cx, cy = T(0.0, -0.130)
    dd, tr = rrect_d(cx, cy, 0.150, 0.014, 0.004, deg)
    d.path(dd, th["gold"], cut=False, transform=tr)


def key_poses():
    return [
        ("Idle", Pose(eyes="open", hip=(0.04, 0.02), lean=-2, tilt=-4, hands=((-0.36, 0.86), (0.42, 0.90)),
                      feet=((0, 0), (0.06, 0)), toe=(0, 12))),
        ("To the heart", croon(0.0, STYLE)),
        ("To the crowd", croon(1.0, STYLE)),
        ("Snap", slide(0.0, STYLE)),
        ("Slide", slide(1.5, STYLE)),
        ("Brim", hattip(0.35, STYLE)),
        ("Hat tip", hattip(0.62, STYLE)),
        ("Ta-da", hattip(1.4, STYLE)),
        ("Cool", Pose(eyes="cool", lean=-5, tilt=8, hands=((-0.36, 0.86), (0.40, 0.42)), feet=((0, 0), (0.08, 0)))),
    ]


def mood_items():
    return [("Neutral", Pose(eyes="open")), ("Happy", Pose(eyes="happy")),
            ("Excited", Pose(eyes="happy", nod=-10, tilt=-8)), ("Calm", Pose(eyes="closed", tilt=6)),
            ("Surprised", Pose(eyes="wide")), ("Sleepy", Pose(eyes="sleepy", nod=14, tilt=12)),
            ("Cool", Pose(eyes="cool", tilt=-6)), ("Hat tip", Pose(eyes="closed", sig=1.0, nod=16))]


MOOD_SIZE = 380   # the sheet's face close-ups
MOOD_FOCUS = -0.90
