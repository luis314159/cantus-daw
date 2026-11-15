"""Onda O3 — **the surfer**: a surfer riding its own waveform.

The floor is a **wave**: a lapis swell across the stage, its crest a gold waveform line,
rolling with the beat. A surfer rides it on a cream board with a vermilion stripe and a gold
fin; the board rides the wave's height and tilts with its slope. The surfer:
- is laid-back cool: sunglasses, messy dark hair with gold tips, a cream tank, vermilion
  board shorts, a small gold pendant;
- stands sideways on the board, knees bent, arms out for balance.

It carves (weight side to side, the arms swinging), walks the board, and its signature,
**Square set**: the wave turns **square** and the surfer hops each step on the beat; then it
turns into a **saw** — the surfer slides down the ramp and pops an aerial, landing on the
drop — and the sea calms back into a sine.
"""
import math
from body import Props, solve, place
from moves import Style, MOVES as ALL_MOVES, steps
from rig import (Pose, add, sub, mul, rot, length, lerp, beat_down, cos1, smooth, ease_out,
                 ease_out_back, clamp01, arc)
from svg import Doc, tube_d, rrect_d, curve_d, f

NAME = "Onda"
PROPS = Props(leg=0.36, thigh=0.186, shin=0.178, torso=0.23, neck=0.03, head=0.074,
              shoulder=0.078, hipw=0.045, stance=0.105, upper=0.140, fore=0.135, foot=0.10)
STYLE = Style(bounce=0.10, sway=0.06, step=0.10, lag=0.12, hop=0.12, squash=0.05)
MOVES = {"groove": "o3_carve", "steps": "o3_walk", "jump": "o3_set"}
CUT = 0.012
WX = 0.48          # the wave's half width
K = 1.2            # cycles across the stage


def carve(b, st):
    p = Pose()
    ph = b % 1.0
    side = math.sin(math.pi * b)
    p.hip = (0.05 * side, 0.20 + 0.05 * beat_down(ph, 2.0))
    p.lean = -8.0 * side
    p.hands = ((-1.0, 0.10 + 0.25 * side), (1.0, 0.10 - 0.25 * side))
    p.tilt = 6.0 * side
    p.feet = ((-0.05, 0.0), (0.05, 0.0))
    p.eyes = "cool"
    p.accent = beat_down(ph, 6)
    return p


def walk(b, st):
    p = carve(b, st)
    q = steps(b, st)
    p.hip = (q.hip[0] * 1.2, 0.16)
    p.feet = tuple((q.feet[i][0] * 1.2 + (-0.05 if i == 0 else 0.05), q.feet[i][1]) for i in range(2))
    p.hands = ((-0.95, -0.05), (0.95, -0.05))
    p.eyes = "happy" if (b % 2.0) > 1.0 else "cool"
    return p


def square_set(b, st):
    """Two beats: the wave goes square and the surfer hops each half beat; on the second
    beat a saw — a slide down the ramp, an aerial, the landing on the drop."""
    bb = b % 2.0
    p = carve(b, st)
    if bb < 1.0:
        p.shape = ease_out_back(clamp01(bb / 0.15), 1.6)
        p.lift = 0.10 * arc((bb * 2) % 1.0)
        p.hands = ((-0.9, -0.6), (0.9, -0.6))
        p.eyes = "wide"
        p.sig = 0.0
    else:
        u = bb - 1.0
        p.sig = 1.0 if u < 0.7 else 0.0
        p.shape = 0.0
        air = clamp01((u - 0.25) / 0.5)
        p.lift = 0.22 * arc(air) if 0.25 < u < 0.75 else 0.0
        p.point = 360.0 * smooth(air) if 0.25 < u < 0.75 else 0.0      # the board's spin
        p.hip = (0.0, 0.26 if u < 0.25 else 0.12)
        p.hands = ((-0.6, -1.0), (0.6, -1.0)) if 0.25 < u < 0.75 else ((-1.0, 0.1), (1.0, 0.1))
        p.eyes = "happy"
    p.accent = 1.0 if bb < 1.0 else beat_down(b % 1.0, 6)
    return p


ALL_MOVES.update({"o3_carve": (carve, 2.0), "o3_walk": (walk, 4.0), "o3_set": (square_set, 2.0)})


def colours(th):
    night = th["name"] == "Night"
    return {"sea": "#4664C0" if night else "#3550A8", "sea_s": "#2E4794" if night else "#24397A",
            "crest": "#6A86DA" if night else "#5B79D6", "foam": "#F2C766" if night else "#E2B85A",
            "board": "#EDE6DA", "board_s": "#C9C0B2", "stripe": "#E0693B", "fin": "#E2B85A",
            "skin": "#B97A57", "skin_s": "#94603F", "tank": "#EDE6DA", "tank_s": "#C9C0B2",
            "shorts": "#E0693B", "shorts_s": "#B24E29", "hair": "#2A1E1B", "tips": "#E2B85A",
            "glass": "#14191E", "gold": "#E2B85A", "lip": "#7E4232"}


def _wave(x, shape, saw):
    if saw:
        return 2.0 * ((x + 0.25) % 1.0) - 1.0
    s = math.sin(2 * math.pi * x)
    return math.copysign(abs(s) ** max(0.05, 1.0 - shape * 0.97), s)


def wave_y(x, pose):
    amp = 0.075 + 0.020 * pose.accent
    return -0.15 - amp * _wave(x / (2 * WX) * K - pose.t * 0.5, pose.shape, pose.sig > 0.5)


def _local(origin, deg):
    return lambda x, y: add(origin, rot((x, y), deg))


def draw(pose, th, cut_color=None, trails=None, floor=True, face=True):
    C = colours(th)
    room = cut_color or th["base"]
    d = Doc(cut_color=room, cut=CUT)
    # the sea: a swell across the stage, a lighter crest band, a gold waveform for a crest
    xs = [-WX + 2 * WX * i / 60 for i in range(61)]
    top = [(x, wave_y(x, pose)) for x in xs]
    sea = top + [(WX, 0.012), (-WX, 0.012)]
    sd = "M" + " L".join(f"{f(x)},{f(y)}" for x, y in sea) + " Z"
    d.raw(f'<path d="{sd}" fill="{room}" stroke="{room}" stroke-width="{f(2 * CUT)}" stroke-linejoin="round"/>')
    d.cel(sd, C["sea"], C["sea_s"], off=(-0.010, -0.010), cut=False)
    band = top + [(x, y + 0.030) for x, y in reversed(top)]
    d.path("M" + " L".join(f"{f(x)},{f(y)}" for x, y in band) + " Z", C["crest"], cut=False)
    d.line(top, 0.010, C["foam"], cut=False)
    for k in range(5):                       # a few little swell lines in the water
        y0 = -0.03 - 0.018 * k
        d.line([(-WX + 0.06 + 0.17 * k, y0), (-WX + 0.12 + 0.17 * k, y0 - 0.008)], 0.006, C["crest"], cut=False)
    # the board rides the wave's height and slope
    y0 = wave_y(0.0, pose)
    dy = (wave_y(0.02, pose) - wave_y(-0.02, pose)) / 0.04
    ang = max(-28.0, min(28.0, math.degrees(math.atan(dy)) * 0.8))
    if pose.shape > 0.5 or pose.sig > 0.5:
        ang *= 0.4
    bc = (0.0, y0 - 0.020 - pose.lift)
    spin = pose.point
    sx = math.cos(math.radians(spin))
    d.raw(f'<g transform="translate({f(bc[0])},{f(bc[1])}) rotate({f(ang)}) scale(1,{f(max(0.15, abs(sx)))})">')
    dd, tr = rrect_d(0, 0, 0.46, 0.040, 0.020, 0)
    d.cel(dd, C["board"] if sx >= 0 else C["board_s"], C["board_s"], off=(-0.006, -0.006), cut=True, transform=tr)
    d.line([(-0.20, 0.0), (0.20, 0.0)], 0.008, C["stripe"], cut=False)
    d.path("M-0.15,0.018 L-0.12,0.018 L-0.15,0.050 Z", C["fin"], cut=False)
    d.raw("</g>")
    # the surfer, standing on the board (its frame)
    d.raw(f'<g transform="translate(0,{f(y0 - 0.040)}) rotate({f(ang)})">')
    _surfer(d, pose, C, face)
    d.raw("</g>")
    return d.svg()


def _surfer(d, pose, C, face):
    j = solve(pose, PROPS)
    P = lambda p: place(p, j)
    for i, s in enumerate((-1, 1)):
        hip, knee, foot = P(j.hips[i]), P(j.knees[i]), P(j.feet[i])
        d.path(tube_d([hip, knee, foot], [0.060, 0.050, 0.046]), C["skin"], cut=True)
        d.ellipse(add(foot, (s * 0.018, 0.006)), 0.034, 0.014, C["skin_s"])
        mid = add(hip, mul(sub(knee, hip), 0.75))
        top = add(add(hip, mul(sub(P(j.pelvis), hip), 0.3)), (0, -0.02))
        d.cel(tube_d([top, mid], [0.090, 0.076]), C["shorts"], C["shorts_s"], off=(-0.010, -0.004), cut=False)
    T = PROPS.torso
    L = _local(P(j.neck), j.torso_deg)
    tank = [L(-0.050, -0.004), L(-0.080, 0.010), L(-0.082, 0.120), L(-0.080, T + 0.010),
            L(0.080, T + 0.010), L(0.082, 0.120), L(0.080, 0.010), L(0.050, -0.004)]
    d.cel(curve_d(tank, closed=True), C["tank"], C["tank_s"], off=(-0.012, -0.008), cut=True)
    d.line([L(-0.030, 0.000), L(0.0, 0.040), L(0.030, 0.000)], 0.005, C["gold"], cut=False, smooth=True)
    d.path("M" + " L".join(f"{f(x)},{f(y)}" for x, y in [L(-0.010, 0.040), L(0.010, 0.040), L(0.0, 0.066)]) + " Z",
           C["gold"], cut=False)
    hc = P(j.head)
    H = _local(hc, j.head_deg)
    d.path(tube_d([L(0, 0.0), H(0, 0.05)], [0.036, 0.036]), C["skin_s"], cut=False)
    d.circle(hc, PROPS.head, C["skin"], cut=True)
    # messy hair with gold tips
    hair = [H(-0.078, 0.010), H(-0.090, -0.040), H(-0.070, -0.080), H(-0.040, -0.110), H(-0.020, -0.090),
            H(0.000, -0.122), H(0.030, -0.095), H(0.060, -0.112), H(0.070, -0.070), H(0.092, -0.050),
            H(0.078, 0.000), H(0.060, -0.030), H(0.020, -0.045), H(-0.030, -0.040), H(-0.060, -0.020)]
    d.path(curve_d(hair, closed=True), C["hair"], cut=False)
    for (x, y) in ((-0.040, -0.108), (0.000, -0.120), (0.058, -0.108)):
        d.circle(H(x, y), 0.010, C["tips"], cut=False)
    if face:
        _face(d, H, pose, C)
    for i, s in enumerate((-1, 1)):
        sh, el, ha = P(j.shoulders[i]), P(j.elbows[i]), P(j.hands[i])
        d.path(tube_d([sh, el, ha], [0.044, 0.038, 0.034]), C["skin"], cut=True)
        d.circle(ha, 0.024, C["skin_s"], cut=False)


def _face(d, H, pose, C):
    e = pose.eyes
    if e == "cool":
        for s in (-1, 1):
            dd, tr = rrect_d(*H(s * 0.026, 0.004), 0.042, 0.022, 0.009, 0)
            d.path(dd, C["glass"], cut=False, transform=tr)
        d.line([H(-0.008, 0.000), H(0.008, 0.000)], 0.005, C["glass"], cut=False)
        d.line([H(-0.040, -0.002), H(-0.032, 0.008)], 0.004, C["gold"], cut=False)
    else:
        for s in (-1, 1):
            if e == "happy":
                d.line([H(s * 0.026 - 0.011, 0.008), H(s * 0.026, -0.002), H(s * 0.026 + 0.011, 0.008)], 0.006, C["glass"], cut=False)
            else:
                big = 1.3 if e == "wide" else 1.0
                d.ellipse(H(s * 0.026, 0.004), 0.008 * big, 0.011 * big, C["glass"])
    d.line([H(-0.016, 0.040), H(0.0, 0.048), H(0.018, 0.036)], 0.006, C["lip"], cut=False, smooth=True)


def key_poses():
    def at(fn, b):
        p = fn(b, STYLE)
        p.t = b
        return p
    return [("Cruise", at(carve, 0.5)), ("Carve", at(carve, 1.5)), ("Walk", at(walk, 1.0)), ("Nose", at(walk, 2.5)),
            ("Square!", at(square_set, 0.1)), ("Hop", at(square_set, 0.25)), ("Saw", at(square_set, 1.15)),
            ("Aerial", at(square_set, 1.5)), ("Landed", at(square_set, 1.85))]


def mood_items():
    base = dict(hip=(0.0, 0.20), hands=((-1.0, 0.1), (1.0, 0.1)))
    return [("Neutral", Pose(eyes="open", **base)), ("Happy", Pose(eyes="happy", **base)),
            ("Excited", Pose(eyes="wide", **base)), ("Calm", Pose(eyes="happy", **base)),
            ("Surprised", Pose(eyes="wide", shape=1.0, **base)), ("Sleepy", Pose(eyes="happy", **base)),
            ("Cool", Pose(eyes="cool", **base)), ("Stoked", Pose(eyes="happy", accent=1.0, **base))]


MOOD_FOCUS = -0.82
MOOD_SIZE = 380
