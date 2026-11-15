"""New — **Metrónomo**, the serious one: a wooden metronome with a butler's dignity.

A tall wooden pyramid (two values of wood and a lighter grain) on a plinth, a gold finial on
top, a gold tempo scale down its front. Its **pendulum** swings in front of it, the gold
weight on the rod, and it is never off the beat: each extreme lands exactly on a beat.

A butler's composure:
- a white wing collar with a black bow tie, white gloves;
- one hand carries a small silver tray, the other conducts in a tiny gesture;
- a gold monocle on a chain, half-lidded eyes, a raised brow, a curled moustache;
- white spats on short legs.

It is not Tic (the old family's metronome robot): Tic is a robot; Metrónomo is wood and
manners.

It barely moves — that is the joke: a small bow on the one, exact side steps.

Signature, **Doble tiempo**: the pendulum suddenly runs at double, then quadruple time (still
landing on the beats). He loses his composure — eyes wide, the monocle drops on its chain,
the arms flail. On the beat he freezes, straightens his bow tie, the monocle goes back, the
pendulum returns to time.
"""
import math
from body import Props, solve, place
from moves import Style, MOVES as ALL_MOVES, steps
from rig import (Pose, add, sub, mul, rot, length, lerp, snap_time, beat_down, cos1, smooth,
                 ease_in, ease_out, ease_out_back, clamp01)
from svg import Doc, tube_d, rrect_d, curve_d, f

NAME = "Metrónomo"
PROPS = Props(leg=0.14, thigh=0.075, shin=0.070, torso=0.36, neck=0.0, head=0.10,
              shoulder=0.100, hipw=0.060, stance=0.080, upper=0.130, fore=0.125, foot=0.10)
STYLE = Style(bounce=0.03, sway=0.0, step=0.18, lag=0.0, hop=0.06, squash=0.03, snap=1.0, lift_step=0.04)
MOVES = {"groove": "met_time", "steps": "met_step", "jump": "met_double"}
CUT = 0.012
SWING = 30.0          # the pendulum's extreme, degrees
TRAY = (-0.85, 0.20)  # the tray hand (neck-relative, A units)


def pendulum(b, mult=1.0):
    """The extreme exactly on each beat (cos at the beat), like a real metronome."""
    return SWING * math.cos(math.pi * b * mult)


def keep_time(b, st):
    """Two beats, composed: a small bow on the one, the free hand conducting a tiny two
    (down on the beat, up between), the tray steady, the pendulum exact."""
    p = Pose()
    ph = b % 1.0
    k = int(math.floor(b)) % 2
    p.shape = pendulum(b)
    bow = beat_down(ph, 3.0) if k == 0 else 0.0
    p.nod = 8.0 * bow
    p.lean = 2.0 * bow
    p.hip = (0.0, 0.02 * bow)
    p.hands = (TRAY, (0.70, 0.30 + 0.25 * beat_down(ph, 2.0)))
    p.eyes = "open"
    p.accent = beat_down(ph, 8)
    return p


def step(b, st):
    p = steps(b, st)
    p.shape = pendulum(b)
    p.hands = (TRAY, (0.70, 0.40))
    p.lean = 0.0
    p.eyes = "open"
    return p


def double(b, st):
    """Two beats: from the half beat the pendulum runs at ×2, then ×4 (its extremes still on
    the beats); he loses his composure — wide eyes, the monocle drops, arms flailing, the
    tray wobbling — and on beat 2 he freezes, straightens his tie, the monocle goes back."""
    p = Pose()
    bb = b % 2.0
    if bb < 1.0:
        mult = 1.0 if bb < 0.25 else (2.0 if bb < 0.6 else 4.0)
        p.shape = pendulum(b, mult)
        panic = smooth((bb - 0.2) / 0.4)
        f_ = math.sin(bb * math.pi * 8)
        p.hands = ((TRAY[0] - 0.1 * f_ * panic, TRAY[1] - 0.6 * panic * abs(f_)),
                   (0.85, 0.30 - 1.2 * panic * abs(math.sin(bb * math.pi * 6))))
        p.lean = 8.0 * panic * math.sin(bb * math.pi * 10)
        p.hip = (0.03 * panic * math.sin(bb * math.pi * 12), 0.0)
        p.eyes = "wide" if panic > 0.3 else "open"
        p.sig = panic                       # the monocle falls
        p.mouth = 0.8 * panic
    else:
        t = bb - 1.0
        p.shape = pendulum(b)
        settle = smooth(t / 0.8)
        tie = math.sin(math.pi * clamp01(t / 0.6))
        p.hands = (TRAY, (lerp(0.85, 0.12, tie), lerp(0.30, 0.20, tie)))
        p.lean = 0.0
        p.nod = -6.0 * tie
        p.eyes = "closed" if t < 0.6 else "open"
        p.sig = 1.0 - settle
        p.mouth = 0.0
    p.accent = beat_down(b % 1.0, 8)
    return p


ALL_MOVES.update({"met_time": (keep_time, 2.0), "met_step": (step, 4.0), "met_double": (double, 2.0)})


def colours(th):
    night = th["name"] == "Night"
    return {"wood": "#9A6A44", "wood_s": "#734B2E", "grain": "#B07E54", "panel": "#5E3D25",
            "rim": "#C9A27A" if night else "#5E3D25", "white": "#F3F5F7", "white_s": "#BAC3CC",
            "ink": "#14191E", "steel": "#C3CBD3", "gold": th["gold"], "gold_l": "#E2B85A"}


def _local(origin, deg):
    return lambda x, y: add(origin, rot((x, y), deg))


def _poly(pts):
    return "M" + " L".join(f"{f(x)},{f(y)}" for x, y in pts) + " Z"


def draw(pose, th, cut_color=None, trails=None, floor=True, face=True):
    j = solve(pose, PROPS)
    P = lambda p: place(p, j)
    C = colours(th)
    room = cut_color or th["base"]
    d = Doc(cut_color=room, cut=CUT)
    gx = (P(j.feet[0])[0] + P(j.feet[1])[0]) / 2
    if floor:
        d.raw(f'<ellipse cx="{f(gx)}" cy="0.010" rx="0.22" ry="0.030" fill="{th["shadow"]}" opacity="0.5"/>')
    # short legs: white spats, black shoes
    for i, s in enumerate((-1, 1)):
        hip, knee, foot = P(j.hips[i]), P(j.knees[i]), P(j.feet[i])
        d.path(tube_d([hip, knee, add(foot, (0, -0.03))], [0.060, 0.056, 0.064]), C["ink"], cut=False)
        dd, tr = rrect_d(foot[0] + s * 0.010, foot[1] - 0.040, 0.070, 0.034, 0.010, 0)
        d.path(dd, C["white"], cut=False, transform=tr)
        dd, tr = rrect_d(foot[0] + s * 0.024, foot[1] - 0.012, 0.110, 0.030, 0.014, 0)
        d.path(dd, C["ink"], cut=False, transform=tr)
    L = _local(P(j.pelvis), j.torso_deg)
    # the plinth and the wooden pyramid (two values and a grain), a gold finial
    dd, tr = rrect_d(*L(0.0, -0.010), 0.38, 0.075, 0.016, j.torso_deg)
    d.cel(dd, C["wood"], C["wood_s"], off=(-0.010, -0.006), cut=True, transform=tr)
    pyr = [L(-0.160, -0.045), L(0.160, -0.045), L(0.080, -0.700), L(-0.080, -0.700)]
    d.path(_poly(pyr), C["rim"], cut=True, extra=f' stroke="{C["rim"]}" stroke-width="0.024" stroke-linejoin="round"')
    d.cel(_poly(pyr), C["wood"], C["wood_s"], off=(-0.020, -0.006), cut=False)
    for x in (-0.110, 0.095):
        d.line([L(x, -0.08), L(x * 0.55, -0.66)], 0.006, C["grain"], cut=False)
    d.circle(L(0.0, -0.730), 0.026, C["gold"], cut=True)
    # the tempo scale (a dark panel with gold ticks)
    d.path(_poly([L(-0.055, -0.105), L(0.055, -0.105), L(0.034, -0.460), L(-0.034, -0.460)]), C["panel"], cut=False)
    for n in range(9):
        y = -0.13 - n * 0.038
        w = 0.030 - n * 0.002
        d.line([L(-w, y), L(w, y)], 0.004, C["gold_l"], cut=False)
    # collar and bow tie
    d.path(_poly([L(-0.090, -0.505), L(-0.006, -0.470), L(-0.050, -0.450)]), C["white"], cut=False)
    d.path(_poly([L(0.090, -0.505), L(0.006, -0.470), L(0.050, -0.450)]), C["white"], cut=False)
    d.path(_poly([L(-0.040, -0.490), L(0.0, -0.476), L(-0.040, -0.462)]), C["ink"], cut=False)
    d.path(_poly([L(0.040, -0.490), L(0.0, -0.476), L(0.040, -0.462)]), C["ink"], cut=False)
    d.circle(L(0.0, -0.476), 0.010, C["ink"], cut=False)
    if face:
        _face(d, L, pose, C)
    # the pendulum: pivot, steel rod, gold weight — exact on the beat
    piv = L(0.0, -0.110)
    ang = math.radians(pose.shape + j.torso_deg)
    tip = add(piv, (0.37 * math.sin(ang), -0.37 * math.cos(ang)))
    d.line([piv, tip], 0.013, C["steel"], cut=True)
    wt = add(piv, (0.25 * math.sin(ang), -0.25 * math.cos(ang)))
    dd, tr = rrect_d(wt[0], wt[1], 0.050, 0.040, 0.008, pose.shape + j.torso_deg)
    d.path(dd, C["gold_l"], cut=True, transform=tr)
    d.circle(piv, 0.018, C["gold_l"], cut=False)
    # arms: white sleeves of the shirt? no — black tail-coat sleeves, white gloves; a silver tray
    for i, s in enumerate((-1, 1)):
        sh, el, ha = P(j.shoulders[i]), P(j.elbows[i]), P(j.hands[i])
        d.path(tube_d([sh, el, ha], [0.050, 0.044, 0.040]), C["ink"], cut=True)
        d.ellipse(ha, 0.032, 0.030, C["white_s"], cut=True)
        d.ellipse(add(ha, (-0.003, -0.003)), 0.028, 0.026, C["white"])
    tray_c = add(P(j.hands[0]), (0.0, -0.040))
    tilt = 10.0 * math.sin(pose.t * 20) * pose.sig
    d.ellipse(tray_c, 0.075, 0.014, C["white_s"], cut=True, deg=tilt)
    d.ellipse(add(tray_c, (0.0, -0.002)), 0.070, 0.010, C["steel"], deg=tilt)
    cup = add(tray_c, rot((0.010, -0.024), tilt))
    dd, tr = rrect_d(cup[0], cup[1], 0.030, 0.030, 0.006, tilt)
    d.path(dd, C["white"], transform=tr)
    return d.svg()


def _face(d, L, pose, C):
    e = pose.eyes
    k = C["ink"]
    ey = -0.590
    for s in (-1, 1):
        x = s * 0.036
        if e == "wide":
            d.circle(L(x, ey), 0.016, C["white"], cut=False)
            d.circle(L(x, ey), 0.007, k, cut=False)
        elif e in ("closed", "sleepy", "happy"):
            pts = ([L(x - 0.016, ey + 0.004), L(x, ey - 0.008), L(x + 0.016, ey + 0.004)] if e == "happy"
                   else [L(x - 0.016, ey), L(x, ey + 0.006), L(x + 0.016, ey)])
            d.line(pts, 0.006, k, cut=False, smooth=True)
        else:                       # composed: half-lidded
            d.ellipse(L(x, ey + 0.002), 0.012, 0.008, k)
            d.line([L(x - 0.016, ey - 0.004), L(x + 0.016, ey - 0.004)], 0.006, C["wood_s"], cut=False)
        brow = -0.020 if (s > 0 and e not in ("wide",)) else -0.014
        d.line([L(x - 0.018, ey + brow), L(x + 0.016, ey + brow - 0.004 * s)], 0.006, k, cut=False)
    # the monocle on the right eye, its chain; it falls with sig
    eye = L(0.036, ey)
    hang = L(0.060, -0.470)
    mc = (lerp(eye[0], hang[0], pose.sig), lerp(eye[1], hang[1], pose.sig))
    d.line([L(0.075, -0.495), mc], 0.003, C["gold_l"], cut=False)
    d.raw(f'<circle cx="{f(mc[0])}" cy="{f(mc[1])}" r="0.022" fill="none" stroke="{C["gold"]}" stroke-width="0.006"/>')
    # the moustache, curled; the mouth under it when it gasps
    for s in (-1, 1):
        d.line([L(0.0, -0.548), L(s * 0.030, -0.552), L(s * 0.054, -0.546), L(s * 0.060, -0.558)], 0.010, k,
               cut=False, smooth=True)
    if pose.mouth > 0.3:
        d.ellipse(L(0.0, -0.526), 0.012, 0.010 * pose.mouth + 0.004, k)


def key_poses():
    def at(fn, b):
        p = fn(b, STYLE)
        p.t = b
        return p
    return [("Composed", at(keep_time, 0.5)), ("The bow", at(keep_time, 0.0)), ("Tick", at(keep_time, 1.0)),
            ("Step", at(step, 1.0)), ("×2", at(double, 0.4)), ("×4", at(double, 0.75)),
            ("Lost it", at(double, 0.95)), ("The tie", at(double, 1.3)), ("Recomposed", at(double, 1.9))]


def mood_items():
    return [("Composed", Pose(eyes="open", shape=10)), ("Pleased", Pose(eyes="happy")), ("Alarmed", Pose(eyes="wide", mouth=0.8, sig=0.6)),
            ("Calm", Pose(eyes="closed")), ("Surprised", Pose(eyes="wide", mouth=0.5)), ("Sleepy", Pose(eyes="sleepy", nod=10)),
            ("Unamused", Pose(eyes="open", tilt=-6)), ("Monocle off", Pose(eyes="wide", sig=1.0))]


MOOD_FOCUS = -0.68
MOOD_SIZE = 380
