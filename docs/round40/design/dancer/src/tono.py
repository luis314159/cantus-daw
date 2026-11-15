"""Direction B, v2 — **Tono**, the knob: Slate's parts with an attitude.

v1 was cute (a big round head, pill eyes, floppy cables). v2 keeps the idea — a knob for a
head, a key for a body, the scale of dots lit by the level, the gold pointer — and makes it
cool: a smaller head on a lean robot body with straight limbs, an **LED visor** with gold
segments for eyes (no mouth: its chest's five bars are its voice), a jack cable slung round
its neck like a scarf with a gold plug that swings late, high-top fader-cap sneakers. It
dances with control: a hard head nod with fist hits, a mechanical glide, and its signature,
**Turn it up**: on the drop it reaches up and turns its own knob, three clicks with an
overshoot each, the dots filling to the top.
"""
import math
from body import Props, solve, place
from moves import Style, MOVES as ALL_MOVES, steps, _hands, _lerp_pair
from rig import (Pose, add, sub, mul, rot, length, lerp, snap_time, beat_down, cos1,
                 ease_out_back, smooth, clamp01)
from svg import Doc, tube_d, rrect_d, curve_d, f

NAME = "Tono"
PROPS = Props(leg=0.30, thigh=0.158, shin=0.152, torso=0.25, neck=0.035, head=0.165,
              shoulder=0.105, hipw=0.052, stance=0.068, upper=0.16, fore=0.155, foot=0.11)
STYLE = Style(bounce=0.10, sway=0.06, step=0.22, arms=1.0, lag=0.14, hop=0.18, squash=0.08,
              snap=0.45, lean=1.0, lift_step=0.02)
MOVES = {"groove": "tono_nod", "steps": "tono_glide", "jump": "tono_turnup"}
CUT = 0.013


def lp(a, b, t):
    return (lerp(a[0], b[0], t), lerp(a[1], b[1], t))


# ------------------------------------------------------------------ its moves
def nod(b, st):
    """Two beats: leaning back, knees bouncing, the head nodding hard on the beat; a fist
    drops across the body on each beat, one hand then the other."""
    b = snap_time(b, 0.5, move=0.4, amount=st.snap)
    p = Pose()
    ph = b % 1.0
    k = int(math.floor(b)) % 2
    down = beat_down(ph, 2.0)
    p.hip = (0.03 * cos1(b / 2), st.bounce * down)
    p.lean = -5.0 + 3.0 * cos1(b / 2)
    p.chest = (0.02 * cos1(b / 2), 0.04 * down)
    p.nod = 16.0 * beat_down((b - st.lag) % 1.0, 2.0) - 6.0
    p.tilt = 9.0 + 4.0 * cos1(b / 2)
    hit = beat_down(ph, 3.0)
    p.hands = (lp((-0.48, 0.95), (-0.10, 0.62), hit if k == 1 else 0.0),
               lp((0.48, 0.95), (0.10, 0.62), hit if k == 0 else 0.0))
    p.toe = (0.0, 12.0 * beat_down((ph + 0.5) % 1.0, 2.0))
    p.eyes = "open"
    p.accent = beat_down(ph, 8)
    p.level = 0.5 + 0.45 * down
    p.point = 26 * cos1((b - 0.2) / 2)
    return p


def glide(b, st):
    """Four beats: the step-touch as a glide — the feet slide, barely lifted — with robot
    arms (elbows held at a right angle, forearms pumping on the half beat, snapped)."""
    p = steps(b, Style(**{**st.__dict__, "snap": 0.0}))
    bs = snap_time(b, 0.5, move=0.3, amount=0.8)
    up = ((-0.62, 0.30), (0.62, 0.62))
    dn = ((-0.62, 0.62), (0.62, 0.30))
    h0, h1, t = _hands({0.0: up, 0.5: dn, 1.0: up, 1.5: dn}, bs, 2.0)
    p.hands = _lerp_pair(h0, h1, t)
    p.elbow = (1.0, 1.0)
    p.lean *= -0.6
    p.tilt = 10.0 * math.sin(math.pi * (b % 4.0) / 2.0) + 6
    p.eyes = "cool" if 1.0 <= (b % 4.0) < 1.5 or 3.0 <= (b % 4.0) < 3.5 else "open"
    p.level = 0.55 + 0.35 * beat_down(b % 1.0, 3)
    p.point = 22 * math.sin(math.pi * (b - st.lag) / 2.0)
    return p


def turnup(b, st):
    """Two beats, its signature on the drop: the right hand reaches up to its own pointer;
    it turns the knob in three clicks (each with an overshoot), leaning back, the dots
    filling to the top and the visor scanning; then it lets go."""
    p = Pose()
    bb = b % 2.0
    reach = smooth(bb / 0.45)
    letgo = smooth((bb - 1.6) / 0.4)
    clicks = sum(ease_out_back(clamp01((bb - (0.5 + n * 0.34)) / 0.16), 2.4) / 3.0 for n in range(3))
    p.sig = clicks * (1 - letgo)
    p.lean = -4.0 - 8.0 * p.sig
    bump = beat_down((bb * 3) % 1.0, 4.0) if 0.5 < bb < 1.6 else 0.0
    p.hip = (0.04 * p.sig, 0.05 + 0.05 * bump)
    p.feet = ((-0.06, 0.0), (0.12, 0.0))
    p.toe = (0.0, 18.0 * p.sig)
    rest_r = (0.48, 0.95)
    A, R = PROPS.arm, PROPS.head + 0.03
    a = math.radians(-55 + 70 * p.sig)            # the grip travels round the rim as it turns
    grip = (R * math.cos(a) / A, (-(PROPS.neck + PROPS.head) + R * math.sin(a)) / A)
    hand_r = lp(lp(rest_r, grip, reach), rest_r, letgo)
    p.hands = (lp((-0.48, 0.95), (-0.95, 0.30), reach * (1 - letgo)), hand_r)
    p.tilt = -6.0 - 6.0 * p.sig
    p.nod = -8.0 * p.sig
    p.eyes = "cool" if p.sig > 0.2 else "open"
    p.level = max(0.4, p.sig)
    p.point = 0.0
    p.accent = beat_down(b % 1.0, 8)
    return p


ALL_MOVES.update({"tono_nod": (nod, 2.0), "tono_glide": (glide, 4.0), "tono_turnup": (turnup, 2.0)})


# ------------------------------------------------------------------ colours
def colours(th):
    night = th["name"] == "Night"
    return {
        "knob": "#2F373F" if night else "#FBFCFD",
        "knob_lip": "#3B444D" if night else "#FFFFFF",
        "rim": "#76818D" if night else "#7A8692",
        "body": "#2B3239" if night else "#E4E9EE",
        "body_lip": "#3A434C" if night else "#FBFCFD",
        "edge": "#0F1317" if night else th["base"],
        "well": "#14191E" if night else "#2B3239",
        "slot": "#0D1014" if night else "#1D2329",
        "limb": "#7D8996" if night else "#6A7682",
        "joint": "#59636E" if night else "#556170",
        "fist": th["ink2"] if night else "#38444F",
        "shoe": "#434C56" if night else "#FBFCFD",
        "sole": "#0D1014" if night else "#38444F",
        "cable": th["verm"],
        "dot_off": "#3A434C" if night else "#C5CDD5",
        "led": "#F2C766",
    }


# ------------------------------------------------------------------ drawing
def draw(pose, th, cut_color=None, trails=None, floor=True, face=True):
    j = solve(pose, PROPS)
    P = lambda p: place(p, j)
    C = colours(th)
    room = cut_color or th["base"]
    d = Doc(cut_color=room, cut=CUT)
    gx = (P(j.feet[0])[0] + P(j.feet[1])[0]) / 2
    if floor:
        k = max(0.0, 1.0 - pose.lift * 3.0)
        d.raw(f'<ellipse cx="{f(gx)}" cy="0.01" rx="{f(0.19 * k + 0.05)}" ry="{f(0.026 * k + 0.008)}" '
              f'fill="{th["shadow"]}" opacity="0.55"/>')
    # legs: straight robot segments with knee caps
    for i in range(2):
        hip, knee, foot = P(j.hips[i]), P(j.knees[i]), P(j.feet[i])
        ankle = add(foot, (0, -0.05))
        d.line([hip, knee, ankle], 0.048, C["limb"], cut=False)
        d.circle(knee, 0.030, C["joint"], cut=False)
    # high-top fader-cap sneakers: a sole, the cap, a gold stripe
    for i, s in enumerate((-1, 1)):
        foot = P(j.feet[i])
        ang = -s * j.toes[i] * 0.6
        c = (foot[0] + s * 0.016, foot[1] - 0.040)
        dd, tr = rrect_d(c[0], c[1] + 0.028, 0.136, 0.026, 0.012, ang)
        d.path(dd, C["sole"], transform=tr)
        dd, tr = rrect_d(c[0], c[1] - 0.006, 0.120, 0.072, 0.022, ang)
        d.path(dd, C["shoe"], cut=False, transform=tr)
        a = add(c, rot((-0.040 * s, 0.012), ang))
        b = add(c, rot((0.020 * s, -0.022), ang))
        d.line([a, b], 0.011, th["gold"], cut=False)
    # the body: a key — cut, rim, lip, surface; a dark well with the five bars
    top, bot = P(j.neck), P(j.pelvis)
    mid = mul(add(top, bot), 0.5)
    h = length(sub(top, bot)) + 0.05
    deg = j.torso_deg
    w = 0.235 * (1 + j.squash * 0.25)
    for ww, col, cut in ((0.020, C["edge"], True), (0.008, C["rim"], False)):
        dd, tr = rrect_d(mid[0], mid[1], w + ww, h + ww, 0.050 + ww / 2, deg)
        d.path(dd, col, cut=cut, transform=tr)
    dd, tr = rrect_d(mid[0], mid[1], w, h, 0.046, deg)
    d.path(dd, C["body_lip"], cut=False, transform=tr)
    dd, tr = rrect_d(mid[0], mid[1] + 0.006, w - 0.004, h - 0.012, 0.043, deg)
    d.path(dd, C["body"], cut=False, transform=tr)
    well_c = add(mid, rot((0.0, 0.006), deg))
    dd, tr = rrect_d(well_c[0], well_c[1], 0.160, 0.090, 0.018, deg)
    d.path(dd, C["well"], cut=False, transform=tr)
    for n, x in enumerate((-0.052, -0.026, 0.0, 0.026, 0.052)):
        shape = (0.4, 0.7, 1.0, 0.7, 0.4)[n]
        lvl = 0.25 + 0.75 * pose.level * (0.7 + 0.3 * math.sin(n * 1.7 + pose.level * 9))
        bh = 0.064 * shape * lvl + 0.010
        c = add(well_c, rot((x, 0.0), deg))
        dd, tr = rrect_d(c[0], c[1], 0.014, bh, 0.007, deg)
        d.path(dd, th["verm"], cut=False, transform=tr)
    _head(d, j, P, pose, th, C, face)
    _cable(d, j, P, pose, th, C, room)
    # arms: straight segments, elbow joints, fists; cut where they cross the body
    for i in range(2):
        sh, el, ha = P(j.shoulders[i]), P(j.elbows[i]), P(j.hands[i])
        free = add(sh, mul(sub(el, sh), 0.5))
        d.line([free, el, ha], 0.046 + 2 * CUT, room, cut=False)
        d.line([sh, el, ha], 0.046, C["limb"], cut=False)
        d.circle(el, 0.027, C["joint"], cut=False)
        d.circle(sh, 0.030, C["joint"], cut=False)
        d.circle(ha, 0.040, C["edge"], cut=False)
        d.circle(ha, 0.034, C["fist"], cut=False)
    return d.svg()


def _cable(d, j, P, pose, th, C, room):
    """A jack cable slung round the neck: across the collar, over the right shoulder, its
    gold plug hanging and swinging late."""
    L = lambda x, y: add(P(j.neck), rot((x, y), j.torso_deg))
    sw = 0.0012 * max(-40.0, min(40.0, pose.point)) - 0.02 * math.sin(math.radians(pose.lean))
    pts = [L(-0.075, 0.012), L(-0.030, 0.052), L(0.030, 0.050), L(0.085, 0.010),
           L(0.120 + sw * 0.4, 0.075), L(0.128 + sw, 0.150), L(0.122 + sw * 1.6, 0.205)]
    d.line(pts, 0.020, C["cable"], cut=True, smooth=True)
    a, z = pts[-2], pts[-1]
    ang = math.degrees(math.atan2(z[1] - a[1], z[0] - a[0])) - 90
    tip = add(z, rot((0, 0.028), ang))
    dd, tr = rrect_d(tip[0], tip[1], 0.030, 0.052, 0.008, ang)
    d.path(dd, th["gold"], cut=True, transform=tr)
    d.line([add(z, rot((0, 0.050), ang)), add(z, rot((0, 0.066), ang))], 0.010, th["gold"], cut=False)


def _head(d, j, P, pose, th, C, face):
    hc = P(j.head)
    R = PROPS.head
    hd = j.head_deg
    lit = int(round(clamp01(pose.level) * 11))
    for n in range(11):
        a = 135 + n * 27
        pnt = add(hc, rot((math.cos(math.radians(a)) * (R + 0.042), math.sin(math.radians(a)) * (R + 0.042)), hd))
        d.circle(pnt, 0.012 if n < lit else 0.010, th["gold"] if n < lit else C["dot_off"], cut=False)
    d.circle(hc, R + 0.010, C["edge"])
    d.circle(hc, R, C["rim"], cut=False)
    d.circle(hc, R - 0.011, C["knob_lip"], cut=False)
    d.circle(add(hc, (0.004, 0.006)), R - 0.015, C["knob"], cut=False)
    # the pointer: a gold fin through the rim, swinging late (turned by Turn it up)
    pa = -90 + pose.point * 0.5 + hd + pose.sig * 135  # fully turned: 5 o'clock, the top
    u = (math.cos(math.radians(pa)), math.sin(math.radians(pa)))
    d.line([add(hc, mul(u, R - 0.07)), add(hc, mul(u, R + 0.022))], 0.026, th["gold"], cut=True)
    if face:
        _visor(d, hc, R, hd, pose, th, C)


def _visor(d, hc, R, hd, pose, th, C):
    """The LED visor: a dark slot across the knob; gold segments are its eyes."""
    H = lambda x, y: add(hc, rot((x * R, y * R), hd))
    c = H(0.0, 0.12)
    dd, tr = rrect_d(c[0], c[1], R * 1.42, R * 0.40, R * 0.20, hd)
    d.path(dd, C["slot"], cut=False, transform=tr)
    e, led = pose.eyes, C["led"]
    if e == "cool":   # one bar the whole width, a bright scan running along it
        d.line([H(-0.55, 0.12), H(0.55, 0.12)], R * 0.12, th["gold"], cut=False)
        x = -0.45 + 0.9 * ((pose.t * 0.5) % 1.0)
        d.line([H(x - 0.08, 0.12), H(x + 0.08, 0.12)], R * 0.14, "#FFF1C9", cut=False)
        return
    for s in (-1, 1):
        cx = s * 0.30
        if e == "happy":
            d.line([H(cx - 0.16, 0.19), H(cx, 0.05), H(cx + 0.16, 0.19)], R * 0.10, led, cut=False)
        elif e in ("closed", "sleepy"):
            col = th["gold_deep"] if e == "sleepy" else led
            d.line([H(cx - 0.16, 0.16), H(cx + 0.16, 0.16)], R * 0.06, col, cut=False)
        elif e == "wide":
            dd, tr = rrect_d(*H(cx, 0.12), R * 0.24, R * 0.24, R * 0.05, hd)
            d.path(dd, led, cut=False, transform=tr)
        elif e == "wink" and s < 0:
            d.line([H(cx - 0.16, 0.16), H(cx + 0.16, 0.16)], R * 0.06, led, cut=False)
        else:     # a keen bar, its inner end a little higher: confident, not cute
            d.line([H(cx - 0.17 * s, 0.15), H(cx + 0.15 * s, 0.10)], R * 0.12, led, cut=False)


def key_poses():
    return [
        ("Idle", Pose(eyes="open", lean=-4, tilt=8, hip=(0.03, 0.02), level=0.35,
                      hands=((-0.48, 0.95), (0.46, 0.92)))),
        ("Nod", nod(0.0, STYLE)),
        ("Hit", nod(1.0, STYLE)),
        ("Glide", glide(1.0, STYLE)),
        ("Robot arms", glide(0.5, STYLE)),
        ("Reach", turnup(0.45, STYLE)),
        ("Turn it up", turnup(1.45, STYLE)),
        ("Cool", Pose(eyes="cool", lean=-6, tilt=10, hands=((-0.45, 0.42), (0.45, 0.42)),
                      feet=((-0.08, 0), (0.08, 0)), level=0.7)),
        ("Point", Pose(eyes="open", lean=5, tilt=-8, hands=((-0.42, 0.40), (1.0, -0.40)),
                       feet=((0, 0), (0.14, 0)), toe=(0, 16), level=0.8, point=30)),
    ]


def mood_items():
    return [("Neutral", Pose(eyes="open")), ("Happy", Pose(eyes="happy")),
            ("Excited", Pose(eyes="happy", level=1.0, point=30)), ("Calm", Pose(eyes="closed", tilt=6)),
            ("Surprised", Pose(eyes="wide")),
            ("Sleepy", Pose(eyes="sleepy", nod=14, tilt=12, level=0.1, point=-40)),
            ("Cool", Pose(eyes="cool", tilt=-6, t=0.3)), ("Wink", Pose(eyes="wink", tilt=8))]


MOOD_SIZE = 340   # the sheet's face close-ups
