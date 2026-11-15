"""Onda O2 — **the wave-jelly**: a chunky creature whose outline is the waveform.

A solid, wobbly body in two values of lapis (theme-aware), its top edge the live waveform
travelling across it, a gold trace running just inside that edge like a scope line on its
forehead. Big eyes, a wide mouth, stubby arms, little feet. It jiggles: squash and stretch,
the sides bulging on the beat.

Signature, **Shape shift**:
- its top snaps into a **square** wave — crenellations like a crown — eyes wide, arms up;
- then a **saw** — a row of shark teeth — with a wicked grin;
- then it melts back into a sine with a jiggle.
"""
import math
from body import Props
from moves import Style, MOVES as ALL_MOVES, steps
from rig import (Pose, add, sub, mul, rot, length, lerp, beat_down, cos1, smooth, ease_out,
                 ease_out_back, clamp01, arc)
from svg import Doc, curve_d, rrect_d, f

NAME = "Onda"
PROPS = Props()
STYLE = Style(bounce=0.08, sway=0.08, step=0.22, lag=0.10, hop=0.14, squash=0.12)
MOVES = {"groove": "o2_jiggle", "steps": "o2_hop", "jump": "o2_shift"}
CUT = 0.013
W, H = 0.46, 0.56


def jiggle(b, st):
    p = Pose()
    ph = b % 1.0
    k = int(math.floor(b)) % 2
    p.squash = 0.10 * beat_down(ph, 3.0) - 0.05 * beat_down((ph + 0.5) % 1.0, 3.0)
    p.lean = 8.0 * cos1(b / 2.0)
    p.accent = beat_down(ph, 4.0)
    up = beat_down(ph, 1.6)
    p.hands = ((-1.0, 0.3 - 1.2 * up * (k == 0)), (1.0, 0.3 - 1.2 * up * (k == 1)))
    p.eyes = "happy" if ph < 0.2 else "open"
    p.mouth = 0.6 * beat_down(ph, 3.0)
    return p


def hop(b, st):
    q = steps(b, st)
    p = jiggle(b, st)
    ph = b % 1.0
    p.hip = (q.hip[0] * 1.4, 0.0)
    p.lift = 0.06 * arc(clamp01((ph + 0.5) % 1.0))
    p.lean = -q.lean * 1.5
    return p


def shift(b, st):
    """Two beats: square (a crown, eyes wide, arms up), then saw (shark teeth, a grin),
    then a sine again with a jiggle."""
    p = Pose()
    bb = b % 2.0
    sq = ease_out_back(clamp01(bb / 0.15), 1.8) * (1 - smooth((bb - 0.9) / 0.1))
    p.shape = sq
    p.sig = 1.0 if 1.0 <= bb < 1.6 else 0.0
    p.accent = 1.0 if bb < 1.6 else beat_down(b % 1.0, 4.0)
    if bb < 1.0:
        p.hands = ((-0.7, -1.1), (0.7, -1.1))
        p.eyes = "wide"
        p.mouth = 0.5
        p.squash = -0.08 * sq
    elif bb < 1.6:
        p.hands = ((-1.0, 0.0), (1.0, 0.0))
        p.eyes = "cool"
        p.mouth = 0.0
        p.squash = 0.04
        p.lean = 6.0
    else:
        t = (bb - 1.6) / 0.4
        p.squash = 0.12 * math.sin(t * math.pi * 3) * (1 - t)
        p.hands = ((-1.0, 0.3), (1.0, 0.3))
        p.eyes = "happy"
    return p


ALL_MOVES.update({"o2_jiggle": (jiggle, 2.0), "o2_hop": (hop, 4.0), "o2_shift": (shift, 2.0)})


def colours(th):
    night = th["name"] == "Night"
    return {"body": "#6A86DA" if night else "#3550A8", "shade": "#4A64B8" if night else "#24397A",
            "trace": "#F2C766" if night else "#E2B85A", "eye": "#FFFFFF", "pupil": "#14191E",
            "mouth": "#14191E", "tongue": "#E0693B", "foot": "#2E4794" if night else "#1D2A5C",
            "tooth": "#FFFFFF"}


def _wave(x, shape, saw):
    if saw:
        return 2.0 * ((x + 0.25) % 1.0) - 1.0
    s = math.sin(2 * math.pi * x)
    return math.copysign(abs(s) ** max(0.05, 1.0 - shape * 0.97), s)


def top_y(x, pose):
    amp = 0.040 + 0.030 * pose.accent + 0.015 * pose.shape
    return -H - amp * _wave((x + W / 2) / W * 2.0 + 0.25 - pose.t * 0.5, pose.shape, pose.sig > 0.5)


def draw(pose, th, cut_color=None, trails=None, floor=True, face=True):
    C = colours(th)
    room = cut_color or th["base"]
    d = Doc(cut_color=room, cut=CUT)
    bx = pose.hip[0] * 0.47
    sq = pose.squash
    lean = math.radians(pose.lean)

    def T(x, y):     # squash about the floor, a lean as a shear, the hop
        return (bx + x * (1 + 0.6 * sq) - y * math.sin(lean) * 0.6, y * (1 - sq) - pose.lift)

    if floor:
        k = max(0.0, 1.0 - pose.lift * 3.0)
        d.raw(f'<ellipse cx="{f(bx)}" cy="0.010" rx="{f(0.24 * k + 0.04)}" ry="{f(0.028 * k + 0.006)}" '
              f'fill="{th["shadow"]}" opacity="0.5"/>')
    # feet
    for s in (-1, 1):
        fx, fy = T(s * 0.10, -0.010)
        d.ellipse((fx, fy), 0.060, 0.030, C["foot"], cut=True)
    # the body: rounded sides and bottom, the waveform for a top
    xs = [-W / 2 + W * i / 48 for i in range(49)]
    top = [T(x, top_y(x, pose)) for x in xs]
    bulge = 0.012 * pose.accent
    right = [T(W / 2 + bulge, -0.30), T(W / 2 + 0.005, -0.12), T(W / 2 - 0.040, -0.035), T(W / 2 - 0.10, -0.018)]
    left = [T(-W / 2 + 0.10, -0.018), T(-W / 2 + 0.040, -0.035), T(-W / 2 - 0.005, -0.12), T(-W / 2 - bulge, -0.30)]
    outline = top + right + left
    dd = "M" + " L".join(f"{f(x)},{f(y)}" for x, y in outline) + " Z"
    d.raw(f'<path d="{dd}" fill="{room}" stroke="{room}" stroke-width="{f(2 * CUT)}" stroke-linejoin="round"/>')
    d.cel(dd, C["body"], C["shade"], off=(-0.018, -0.014), cut=False)
    # the gold trace just inside the top edge, like a scope line on its forehead
    inner = [T(x, top_y(x, pose) + 0.040) for x in xs[3:-3]]
    d.line(inner, 0.012, C["trace"], cut=False)
    if face:
        _face(d, T, pose, C)
    # stubby arms toward the hands
    for i, s in enumerate((-1, 1)):
        hx, hy = pose.hands[i]
        ang = math.atan2(hy, hx * s)
        base = T(s * (W / 2 - 0.01), -0.24)
        tip = add(base, (s * 0.13 * math.cos(ang), 0.13 * math.sin(ang)))
        d.line([base, tip], 0.070, C["body"], cut=True)
        d.circle(tip, 0.042, C["shade"], cut=False)
        d.circle(add(tip, (-0.004, -0.004)), 0.036, C["body"], cut=False)
    return d.svg()


def _face(d, T, pose, C):
    e = pose.eyes
    for s in (-1, 1):
        c = T(s * 0.080, -0.305)
        if e == "happy":
            d.line([T(s * 0.080 - 0.040, -0.295), T(s * 0.080, -0.330), T(s * 0.080 + 0.040, -0.295)], 0.022, C["pupil"], cut=False)
        elif e in ("closed", "sleepy"):
            d.line([T(s * 0.080 - 0.040, -0.300), T(s * 0.080, -0.285), T(s * 0.080 + 0.040, -0.300)], 0.020, C["pupil"], cut=False)
        else:
            big = 1.2 if e == "wide" else 1.0
            d.ellipse(c, 0.048 * big, 0.058 * big, C["eye"])
            pc = add(c, (0.008 * s * 0, 0.010))
            d.circle(pc, 0.024 if e != "wide" else 0.018, C["pupil"], cut=False)
            d.circle(add(pc, (-0.008, -0.010)), 0.008, C["eye"], cut=False)
            if e == "cool":   # wicked brows for the saw
                d.line([T(s * 0.080 - 0.050, -0.390 + 0.020 * (s < 0) - 0.020 * 0), T(s * 0.080 + 0.050, -0.390 + 0.020 * (s > 0))],
                       0.020, C["pupil"], cut=False)
    if pose.sig > 0.5:    # the saw: a grin full of shark teeth
        pts = [T(-0.100, -0.170), T(0.100, -0.170), T(0.080, -0.120), T(-0.080, -0.120)]
        d.path("M" + " L".join(f"{f(x)},{f(y)}" for x, y in pts) + " Z", C["mouth"], cut=False)
        for k in range(5):
            x0 = -0.090 + k * 0.038
            tri = [T(x0, -0.170), T(x0 + 0.030, -0.170), T(x0 + 0.015, -0.142)]
            d.path("M" + " L".join(f"{f(x)},{f(y)}" for x, y in tri) + " Z", C["tooth"], cut=False)
    elif pose.mouth > 0.25:
        o = pose.mouth
        d.ellipse(T(0.0, -0.160), 0.070, 0.020 + 0.040 * o, C["mouth"])
        d.ellipse(T(0.0, -0.160 + 0.020 + 0.022 * o), 0.036, 0.014, C["tongue"])
    else:
        d.line([T(-0.080, -0.175), T(0.0, -0.140), T(0.080, -0.175)], 0.022, C["mouth"], cut=False, smooth=True)


def key_poses():
    def at(fn, b):
        p = fn(b, STYLE)
        p.t = b
        return p
    return [("Idle", Pose(eyes="open", hands=((-1, 0.3), (1, 0.3)), t=0.2)), ("Jiggle", at(jiggle, 0.0)),
            ("Wave", at(jiggle, 1.0)), ("Hop", at(hop, 0.5)), ("Square!", at(shift, 0.2)),
            ("Crown", at(shift, 0.7)), ("Saw", at(shift, 1.25)), ("Teeth", at(shift, 1.5)), ("Sine", at(shift, 1.8))]


def mood_items():
    return [("Neutral", Pose(eyes="open", hands=((-1, 0.3), (1, 0.3)))), ("Happy", Pose(eyes="happy", hands=((-1, 0.3), (1, 0.3)))),
            ("Excited", Pose(eyes="wide", mouth=0.8, accent=1.0, hands=((-1, -0.8), (1, -0.8)))),
            ("Calm", Pose(eyes="closed", hands=((-1, 0.4), (1, 0.4)))), ("Surprised", Pose(eyes="wide", mouth=0.5, hands=((-1, 0.3), (1, 0.3)))),
            ("Sleepy", Pose(eyes="sleepy", lean=8, hands=((-1, 0.6), (1, 0.6)))), ("Square", Pose(eyes="wide", shape=1.0, hands=((-0.7, -1.1), (0.7, -1.1)))),
            ("Saw", Pose(eyes="cool", sig=1.0, hands=((-1, 0.0), (1, 0.0))))]


MOOD_FULL = True
