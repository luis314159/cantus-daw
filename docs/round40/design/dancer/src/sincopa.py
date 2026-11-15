"""Direction C, v2 — **Síncopa**, the figure in values: an editorial figure with swagger.

v1 was friendly and square (a boxy hoodie, a smile, a polite groove). v2 keeps the idea — no
ink line, every piece two flat values, the logo's lapis, gold and vermilion as clothes —
and gives it attitude: taller (about 8 heads), an open oversized vermilion bomber over a
cream tee, lapis cargo trousers with a strap that swings, chunky high-tops, gold headphones
worn round the neck, long braids in a high ponytail with gold beads that swing late, a gold
hoop, half-lidded eyes and a smirk. It dances with swagger: hips that hit and overshoot,
shoulders bouncing on the off-beat, the chin up; and its signature, **Shades**: on the drop
it slides its sunglasses on, flicks the hand away and freezes, leaning back.
"""
import math
from body import Props, solve, place
from moves import Style, MOVES as ALL_MOVES, steps_human, _hands, _lerp_pair
from rig import (Pose, add, sub, mul, rot, length, lerp, snap_time, beat_down, cos1, sin1,
                 smooth, ease_out_back, ease_in_out, clamp01)
from svg import Doc, tube_d, rrect_d, curve_d, f

NAME = "Síncopa"
PROPS = Props(leg=0.50, thigh=0.262, shin=0.248, torso=0.268, neck=0.040, head=0.080,
              shoulder=0.092, hipw=0.050, stance=0.070, upper=0.158, fore=0.152, foot=0.10)
HS = 1.08
STYLE = Style(bounce=0.06, sway=0.13, step=0.17, arms=1.0, lag=0.14, hop=0.16, squash=0.05,
              snap=0.40, lean=1.2, open=(0.56, 0.95), chest=-0.07, lift_step=0.04)
MOVES = {"groove": "sinc_swagger", "steps": "sinc_step", "jump": "sinc_shades"}
CUT = 0.011


def lp(a, b, t):
    return (lerp(a[0], b[0], t), lerp(a[1], b[1], t))


# ------------------------------------------------------------------ its moves
def swagger(b, st):
    """Two beats: the hips hit a side on each beat and overshoot (snap), the chest answers
    the other way, a shoulder pops on the off-beat; leaning back, chin up, the hands loose
    and swinging against the hips."""
    b = snap_time(b, 0.5, move=0.45, amount=st.snap, overshoot=2.2)
    p = Pose()
    ph = b % 1.0
    side = cos1(b / 2.0)                       # +1 on beat 1 (hips right), −1 on beat 2
    down = beat_down(ph, 2.0)
    p.hip = (st.sway * side, st.bounce * down + 0.02)
    p.chest = (st.chest * side, 0.02 * down)
    p.lean = -3.0 - 3.0 * side * 0.4
    pop = beat_down((ph + 0.5) % 1.0, 3.0)     # on the off-beat
    p.roll = 9.0 * pop * (1 if side > 0 else -1)
    p.nod = -5.0 + 8.0 * beat_down((b - st.lag) % 1.0, 2.0)
    p.tilt = 8.0 - 5.0 * cos1((b - st.lag) / 2.0)
    sw = 0.14 * side
    k = int(math.floor(b)) % 2
    snap_ = beat_down(ph, 2.5) if k == 1 else 0.0       # beat 2: a finger snap at the chest
    p.hands = ((-0.54 - sw, 0.94), lp((0.54 - sw, 0.94), (0.30, 0.44), snap_))
    p.claps = snap_
    p.feet = ((0.0, 0.0), (0.0, 0.0))
    p.toe = (6.0 * max(0.0, -side), 6.0 * max(0.0, side))
    p.knee = (1.0, 1.0)
    p.eyes = "open"
    p.accent = beat_down(ph, 8)
    p.point = 35.0 * cos1((b - 0.3) / 2.0)
    p.level = 0.5 + 0.4 * down
    return p


def step(b, st):
    """Four beats of a smooth step-touch: the shoulder leads, the near arm reaches out on
    the close, the head turns after it (the hit lands, then settles)."""
    p = steps_human(snap_time(b, 0.5, move=0.5, amount=0.3), st)
    p.lean = p.lean * 0.6 - 2.0
    p.nod -= 4.0
    p.eyes = "open"
    p.point = 35.0 * math.sin(math.pi * (b - 0.3) / 2.0)
    return p


def shades(b, st):
    """Two beats, its signature on the drop: the right hand comes up over the eyes and the
    sunglasses are on; the hand flicks away with an overshoot; it freezes, leaning back, a
    hand on the hip and the other pointing down and out; then it breathes."""
    p = Pose()
    bb = b % 2.0
    up = smooth(bb / 0.40)
    flick = ease_out_back(clamp01((bb - 0.45) / 0.30), 2.0)
    freeze = ease_out_back(clamp01((bb - 0.75) / 0.30), 1.6)
    out = smooth((bb - 1.75) / 0.25)
    face_pt = (0.10, -0.66)
    rest_r, rest_l = (0.54, 0.94), (-0.54, 0.94)
    right = lp(rest_r, face_pt, up)
    right = lp(right, (0.98, -0.30), flick)
    right = lp(right, (0.92, 0.58), freeze)
    right = lp(right, rest_r, out)
    left = lp(rest_l, (-0.38, 0.80), freeze * (1 - out))
    p.hands = (left, right)
    p.elbow = (1.0, 1.0)
    p.hip = (lerp(0.0, -0.10, freeze) * (1 - out), 0.03 + 0.03 * freeze)
    p.lean = lerp(-2.0, -11.0, freeze) * (1 - out) + 2.0 * up * (1 - flick)
    p.chest = (0.05 * freeze * (1 - out), 0.0)
    p.nod = 14.0 * up * (1 - flick) - 10.0 * freeze * (1 - out)
    p.tilt = 10.0 * freeze * (1 - out)
    p.feet = ((-0.16 * freeze * (1 - out), 0.0), (0.08 * freeze * (1 - out), 0.0))
    p.toe = (0.0, 14.0 * freeze * (1 - out))
    p.eyes = "cool" if bb >= 0.42 else "open"
    p.sig = 1.0 if bb >= 0.42 else 0.0
    p.accent = beat_down(b % 1.0, 8)
    p.level = 0.6 + 0.4 * freeze
    p.point = -40.0 * flick + 30.0 * freeze * math.sin(bb * 6)
    return p


ALL_MOVES.update({"sinc_swagger": (swagger, 2.0), "sinc_step": (step, 4.0),
                  "sinc_shades": (shades, 2.0)})

PAL = {
    "skin": "#C08463", "skin_s": "#9C6548",
    "hair": "#1E1717", "hair_l": "#5A443C", "braid": "#3E2E29", "braid_l": "#7A5E52",
    "jacket": "#E0693B", "jacket_s": "#B24E29", "jacket_d": "#8E3D20",
    "tee": "#EDE6DA", "tee_s": "#C9C0B2",
    "pants": "#4664C0", "pants_s": "#2E4794",
    "strap": "#2B3239",
    "shoe": "#F3F5F7", "shoe_s": "#BAC3CC", "sole": "#2B3239",
    "gold": "#E2B85A", "gold_s": "#B38A35",
    "eye": "#1E1717", "lid": "#8A5640", "lip": "#7E4232", "glass": "#14191E",
}


def _local(origin, deg):
    return lambda x, y: add(origin, rot((x, y), deg))


# ------------------------------------------------------------------ drawing
def draw(pose, th, cut_color=None, trails=None, floor=True, face=True):
    j = solve(pose, PROPS)
    P = lambda p: place(p, j)
    c = PAL
    room = cut_color or th["base"]
    d = Doc(cut_color=room, cut=CUT)
    gx = (P(j.feet[0])[0] + P(j.feet[1])[0]) / 2
    if floor:
        k = max(0.0, 1.0 - pose.lift * 3.0)
        d.raw(f'<ellipse cx="{f(gx)}" cy="0.008" rx="{f(0.17 * k + 0.05)}" ry="{f(0.024 * k + 0.008)}" '
              f'fill="{th["shadow"]}" opacity="0.5"/>')
    T = PROPS.torso
    L = _local(P(j.neck), j.torso_deg)
    hc = P(j.head)
    Hraw = _local(hc, j.head_deg)
    H = lambda x, y: Hraw(x * HS, y * HS - 0.010)
    sw = 0.0009 * max(-45.0, min(45.0, pose.point)) - 0.0015 * pose.lean
    _braids(d, H, sw, c)
    # trousers: wide lapis cargo, a pocket flap, a strap that swings
    for i, s in enumerate((-1, 1)):
        hip, knee, foot = P(j.hips[i]), P(j.knees[i]), P(j.feet[i])
        top = add(add(hip, mul(sub(P(j.pelvis), hip), 0.25)), (0, -0.03))
        ankle = add(foot, (0, -0.060))
        d.cel(tube_d([top, knee, ankle], [0.106, 0.088, 0.100]), c["pants"], c["pants_s"],
              off=(-0.013, -0.004), cut=False)
        flap = add(top, mul(sub(knee, top), 0.55))
        dd, tr = rrect_d(flap[0] + s * 0.030, flap[1], 0.040, 0.034, 0.006, j.torso_deg)
        d.path(dd, c["pants_s"], cut=False, transform=tr)
    sa = L(0.070, T + 0.035)
    sz = add(sa, (0.008 + sw * 1.4, 0.085))
    d.line([sa, add(sa, (0.005 + sw * 0.6, 0.045)), sz], 0.016, c["strap"], cut=False, smooth=True)
    d.circle(sz, 0.011, c["gold"], cut=False)
    # high-top sneakers
    for i, s in enumerate((-1, 1)):
        foot = P(j.feet[i])
        ang = -s * j.toes[i] * 0.5
        base = add(foot, (s * 0.020, -0.016))
        dd, tr = rrect_d(base[0], base[1] + 0.006, 0.134, 0.026, 0.012, ang)
        d.path(dd, c["sole"], cut=False, transform=tr)
        up = [(-0.058, 0.0), (-0.054, -0.050), (-0.026, -0.080), (0.020, -0.074), (0.060, -0.030), (0.062, 0.0)]
        pts = [add(base, rot((x * s, y), ang)) for x, y in up]
        d.cel(curve_d(pts, closed=True), c["shoe"], c["shoe_s"], off=(-0.006, -0.008), cut=False)
        d.circle(add(base, rot((-s * 0.040, -0.062), ang)), 0.011, c["gold"], cut=False)
        d.line([add(base, rot((-s * 0.030, -0.014), ang)), add(base, rot((s * 0.026, -0.046), ang))],
               0.010, c["gold"], cut=False)
    # the tee, then the open bomber's two halves over it
    hl = T + 0.035
    tee = [L(-0.050, 0.0), L(0.050, 0.0), L(0.064, hl - 0.012), L(-0.064, hl - 0.012)]
    d.cel(curve_d(tee, closed=True), c["tee"], c["tee_s"], off=(-0.008, -0.006), cut=False)
    d.line([L(-0.03, 0.150), L(0.03, 0.150)], 0.010, c["strap"], cut=False)
    d.line([L(-0.03, 0.170), L(0.03, 0.170)], 0.006, th["verm"] if False else c["jacket"], cut=False)
    for s in (-1, 1):
        half = [L(s * 0.032, -0.006), L(s * 0.100, 0.006), L(s * 0.130, 0.050), L(s * 0.126, 0.130),
                L(s * 0.114, 0.210), L(s * 0.120, hl), L(s * 0.044, hl), L(s * 0.036, 0.170),
                L(s * 0.030, 0.070)]
        d.cel(curve_d(half, closed=True), c["jacket"], c["jacket_s"], off=(-0.016, -0.012), cut=False)
        hem = [L(s * 0.044, hl - 0.026), L(s * 0.119, hl - 0.026), L(s * 0.120, hl), L(s * 0.044, hl)]
        d.path("M" + " L".join(f"{f(x)},{f(y)}" for x, y in hem) + " Z", c["jacket_s"], cut=False)
        d.line([L(s * 0.032, -0.004), L(s * 0.036, 0.060), L(s * 0.040, 0.150)], 0.016, c["jacket_d"], cut=False)
    # neck, gold headphones round it
    d.path(tube_d([L(0, 0.004), H(0, 0.04)], [0.046, 0.046]), c["skin_s"], cut=False)
    d.line([L(-0.062, -0.018), L(-0.040, 0.022), L(0.0, 0.034), L(0.040, 0.022), L(0.062, -0.018)],
           0.014, c["gold_s"], cut=False, smooth=True)
    for s in (-1, 1):
        cx, cy = L(s * 0.074, -0.020)
        dd, tr = rrect_d(cx, cy, 0.034, 0.050, 0.013, j.torso_deg + s * 18)
        d.cel(dd, c["gold"], c["gold_s"], off=(-0.006, -0.006), cut=False, transform=tr)
    _head(d, H, pose, c, face)
    # sleeves: puffy vermilion, ribbed cuffs, cut from the jacket where they cross it
    for i, s in enumerate((-1, 1)):
        sh, el, ha = P(j.shoulders[i]), P(j.elbows[i]), P(j.hands[i])
        sh = add(sh, mul(sub(P(j.neck), sh), 0.12))
        fdir = sub(ha, el)
        fl = length(fdir) or 1e-6
        fdir = mul(fdir, 1 / fl)
        wrist = sub(ha, mul(fdir, 0.012))
        free = add(sh, mul(sub(el, sh), 0.6))
        d.line([free, el, wrist], 0.074 + 2 * CUT, room, cut=False)
        d.cel(tube_d([sh, el, wrist], [0.086, 0.074, 0.066]), c["jacket"], c["jacket_s"],
              off=(-0.011, -0.010), cut=False)
        cuff = sub(wrist, mul(fdir, 0.028))
        d.path(tube_d([cuff, wrist], [0.064, 0.060]), c["jacket_s"], cut=False)
        d.ellipse(add(ha, mul(fdir, 0.018)), 0.028, 0.031, c["skin"],
                  deg=math.degrees(math.atan2(fdir[1], fdir[0])))
    return d.svg()


def _braids(d, H, sw, c):
    """Long braids in a high ponytail: up from the crown, over, down behind the right
    shoulder, swinging late; braid marks along it, gold beads at the end."""
    k = HS
    p0 = H(0.020, -0.092)
    pts = [p0, add(p0, (0.026 * k, -0.022 * k))]
    for dx, dy, f_ in ((0.046, 0.030, 0.5), (0.020, 0.085, 1.0), (0.006, 0.090, 1.4), (-0.004, 0.088, 1.8)):
        pts.append(add(pts[-1], ((dx + sw * f_) * k, dy * k)))
    d.path(tube_d(pts, [0.036 * k, 0.040 * k, 0.036 * k, 0.032 * k, 0.026 * k, 0.018 * k]), c["braid"], cut=False)
    for n in range(1, len(pts) - 1):
        a, b = pts[n], pts[n + 1]
        for t in (0.3, 0.75):
            m = (lerp(a[0], b[0], t), lerp(a[1], b[1], t))
            dirv = sub(b, a)
            ln = length(dirv) or 1e-6
            nx, ny = -dirv[1] / ln, dirv[0] / ln
            w = 0.012 * k
            d.line([add(m, (nx * w, ny * w)), add(m, (dirv[0] / ln * 0.010, dirv[1] / ln * 0.010)),
                    add(m, (-nx * w, -ny * w))], 0.007, c["braid_l"], cut=False)
    d.circle(pts[1], 0.016 * k, c["gold"], cut=False)
    end = pts[-1]
    d.circle(end, 0.012 * k, c["gold"], cut=False)
    d.circle(add(pts[-2], mul(sub(end, pts[-2]), 0.55)), 0.011 * k, c["gold"], cut=False)


def _head(d, H, pose, c, face):
    head = [H(-0.060, -0.010), H(-0.058, -0.050), H(-0.034, -0.080), H(0.0, -0.086),
            H(0.034, -0.080), H(0.058, -0.050), H(0.060, -0.010), H(0.050, 0.034),
            H(0.022, 0.066), H(0.0, 0.070), H(-0.022, 0.066), H(-0.050, 0.034)]
    d.cel(curve_d(head, closed=True), c["skin"], c["skin_s"], off=(-0.012, -0.004), cut=False)
    hair = [H(-0.064, 0.004), H(-0.068, -0.048), H(-0.046, -0.084), H(-0.006, -0.096),
            H(0.036, -0.090), H(0.062, -0.060), H(0.066, -0.010), H(0.058, 0.008),
            H(0.052, -0.034), H(0.022, -0.054), H(-0.024, -0.054), H(-0.052, -0.036)]
    d.cel(curve_d(hair, closed=True), c["hair_l"], c["hair"], off=(-0.008, -0.010), cut=False)
    d.raw(f'<circle cx="{f(H(0.064, 0.036)[0])}" cy="{f(H(0.064, 0.036)[1])}" r="{f(0.011 * HS)}" '
          f'fill="none" stroke="{c["gold"]}" stroke-width="{f(0.005 * HS)}"/>')
    if face:
        _face(d, H, pose, c)


def _face(d, H, pose, c):
    e = pose.eyes
    for s in (-1, 1):
        ex, ey = H(s * 0.025, 0.006)
        if e == "happy":
            d.line([H(s * 0.025 - 0.011, 0.010), H(s * 0.025, 0.000), H(s * 0.025 + 0.011, 0.010)],
                   0.006, c["eye"], cut=False, smooth=True)
        elif e in ("closed", "sleepy"):
            d.line([H(s * 0.025 - 0.011, 0.004), H(s * 0.025, 0.010), H(s * 0.025 + 0.011, 0.004)],
                   0.006, c["eye"], cut=False, smooth=True)
        elif e == "wink" and s > 0:
            d.line([H(s * 0.025 - 0.011, 0.006), H(s * 0.025 + 0.011, 0.006)], 0.006, c["eye"], cut=False)
        elif e != "cool":
            big = 1.35 if e == "wide" else 1.0
            d.ellipse((ex, ey), 0.0095 * big, 0.012 * big, c["eye"])
            if e != "wide":   # half-lidded: the upper lid low and level — confident
                d.path(rrect_d(ex, ey - 0.008, 0.024, 0.012, 0.002)[0], c["skin"], cut=False,
                       transform=rrect_d(ex, ey - 0.008, 0.024, 0.012, 0.002)[1])
                d.line([H(s * 0.025 - 0.012, 0.0015), H(s * 0.025 + 0.012, 0.0005)], 0.005, c["eye"], cut=False)
        lift = 0.006 if e == "wide" else 0.0
        d.line([H(s * 0.012, -0.018 - lift), H(s * 0.038, -0.024 - lift + (0.004 if e == "open" else 0))],
               0.0065, c["hair"], cut=False)
    if e == "cool":
        for s in (-1, 1):
            cx, cy = H(s * 0.026, 0.004)
            dd, tr = rrect_d(cx, cy, 0.044, 0.024, 0.010, 0)
            d.path(dd, c["glass"], cut=False, transform=tr)
        d.line([H(-0.008, -0.002), H(0.008, -0.002)], 0.006, c["glass"], cut=False)
        d.line([H(-0.042, -0.004), H(-0.032, 0.008)], 0.004, c["gold"], cut=False)
        d.line([H(0.012, -0.004), H(0.020, 0.006)], 0.004, c["gold"], cut=False)
    d.line([H(0.004, 0.018), H(0.008, 0.030)], 0.006, c["skin_s"], cut=False)
    if pose.mouth > 0.25:
        d.ellipse(H(0.0, 0.046), 0.014, 0.006 + 0.010 * pose.mouth, c["lip"])
    elif e == "happy":
        d.line([H(-0.014, 0.042), H(0.0, 0.049), H(0.014, 0.042)], 0.0055, c["lip"], cut=False, smooth=True)
    else:          # the smirk: one corner up
        d.line([H(-0.013, 0.046), H(0.002, 0.047), H(0.015, 0.039)], 0.0055, c["lip"], cut=False, smooth=True)


def key_poses():
    return [
        ("Idle", Pose(eyes="open", hip=(0.05, 0.02), lean=-3, chest=(-0.04, 0), tilt=7, nod=-4,
                      hands=((-0.55, 0.94), (0.38, 0.80)), elbow=(1, 1), feet=((0, 0), (0.06, 0)), toe=(0, 10))),
        ("Swagger", swagger(0.0, STYLE)),
        ("Shoulder pop", swagger(0.5, STYLE)),
        ("Step", step(1.0, STYLE)),
        ("Shades on", shades(0.42, STYLE)),
        ("Flick", shades(0.68, STYLE)),
        ("Freeze", shades(1.3, STYLE)),
        ("Point", Pose(hip=(-0.07, 0.03), lean=6, hands=((-0.44, 0.42), (0.62, -0.95)),
                       feet=((0, 0), (0.14, 0)), toe=(0, 14), tilt=-8, nod=-6, eyes="cool")),
        ("Lean back", Pose(hip=(0.06, 0.03), lean=-9, chest=(-0.05, 0), hands=((-0.42, 0.40), (0.45, 0.42)),
                           feet=((-0.12, 0), (0.08, 0)), tilt=-10, nod=-8, eyes="open")),
    ]


def mood_items():
    return [("Neutral", Pose(eyes="open")), ("Happy", Pose(eyes="happy")),
            ("Excited", Pose(eyes="wide", mouth=0.7)), ("Calm", Pose(eyes="closed", tilt=6)),
            ("Surprised", Pose(eyes="wide", mouth=0.45)), ("Sleepy", Pose(eyes="sleepy", nod=14, tilt=12)),
            ("Cool", Pose(eyes="cool", tilt=-6)), ("Wink", Pose(eyes="wink", tilt=8))]


MOOD_SIZE = 520   # the sheet's face close-ups
