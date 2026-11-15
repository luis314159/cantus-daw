"""Metrónomo M1 — **Clic**, the digital metronome: crisp precision that breaks loose.

A sleek slab of a device that wears the theme (graphite at night, white on paper):
- **the screen** is its face: its eyes are gold LED segments, its BPM a live 7-segment
  readout below them ("125");
- **a beat light** on a short stem on top blinks gold on every beat and vermilion on the one;
- **a round lapis TAP pad** on its front lights with the beat;
- straight robotic limbs with square joints, clicker mitts, block feet.

It keeps perfect time like a machine: tick-tock poses snapped on the eighths, the slab
tilting one way then the other.

Signature, **Tempo ramp**: the readout counts up (125 → 250 → 500) while its ticks
accelerate; on the drop it **breaks loose** — a free, fluid, joyful beat of dancing, the
screen showing ♪ — and snaps back to 125 on the next beat.
"""
import math
from body import Props, solve, place
from moves import Style, MOVES as ALL_MOVES, steps, _hands
from rig import (Pose, add, sub, mul, rot, length, lerp, snap_time, beat_down, cos1, sin1, smooth,
                 ease_out_back, clamp01)
from svg import Doc, tube_d, rrect_d, curve_d, f

NAME = "Clic"
PROPS = Props(leg=0.36, thigh=0.186, shin=0.178, torso=0.30, neck=0.0, head=0.10,
              shoulder=0.150, hipw=0.060, stance=0.075, upper=0.140, fore=0.140, foot=0.11)
STYLE = Style(bounce=0.05, sway=0.0, step=0.20, lag=0.0, hop=0.10, squash=0.04, snap=1.0, lift_step=0.06)
MOVES = {"groove": "m1_tick", "steps": "m1_step", "jump": "m1_ramp"}
CUT = 0.012

TICK = [((20, 90), (92, 0)), ((92, 0), (20, 90)), ((150, 0), (30, -60)), ((30, -60), (150, 0))]


def _fk_keys(keys, b, period, snap=1.0):
    bs = snap_time(b, 0.5, move=0.22, amount=snap, overshoot=2.2)
    n = len(keys)
    k0 = int(math.floor(bs / 0.5)) % n
    t = bs / 0.5 - math.floor(bs / 0.5)
    a0, a1 = keys[k0], keys[(k0 + 1) % n]
    return tuple((lerp(a0[i][0], a1[i][0], t), lerp(a0[i][1], a1[i][1], t)) for i in range(2)), bs


def tick(b, st):
    """Two beats of tick-tock: a robot arm shape on each eighth, snapped; the slab tilts one
    way then the other; the knees dip on the beat."""
    p = Pose()
    p.fk, bs = _fk_keys(TICK, b, 2.0)
    side = 1.0 if int(math.floor(bs / 0.5)) % 2 == 0 else -1.0
    p.lean = 5.0 * side
    p.hip = (0.0, st.bounce * beat_down(b % 1.0, 3.0))
    p.eyes = "open"
    p.accent = beat_down(b % 1.0, 8)
    p.shape = 125.0                                 # the BPM on its screen
    p.level = 0.5
    return p


def step(b, st):
    p = steps(b, st)
    p.fk, bs = _fk_keys(TICK[:2], b, 1.0)
    p.lean = 0.0
    p.eyes = "open"
    p.accent = beat_down(b % 1.0, 8)
    p.shape = 125.0
    return p


def ramp(b, st):
    """Two beats: the ticks accelerate (time warped) while the readout counts up to 500; on
    the drop it breaks loose — a free, fluid beat — and snaps back to 125."""
    bb = b % 2.0
    if bb < 1.0:
        warp = bb + 3.0 * bb * bb
        p = tick(warp, st)
        p.shape = 125.0 * (1 + 3.0 * bb * bb)
        p.eyes = "wide" if bb > 0.6 else "open"
        p.hip = (0.015 * math.sin(bb * 60) * bb, p.hip[1])
        p.sig = 0.0
    else:
        t = bb - 1.0
        p = Pose()
        a = 2 * math.pi * t
        loose = math.sin(math.pi * clamp01(t / 0.9))
        p.hands = ((-0.8 + 0.2 * math.sin(a * 2), -0.6 * loose + 0.6 * (1 - loose)),
                   (0.8 + 0.2 * math.cos(a * 2), -0.9 * loose + 0.6 * (1 - loose)))
        p.lean = 14.0 * math.sin(a) * loose
        p.hip = (0.08 * math.sin(a) * loose, 0.06 * loose)
        p.feet = ((0.0, 0.0), (0.18 * loose, -0.16 * loose))
        p.tilt = 12.0 * math.sin(a * 2) * loose
        p.eyes = "happy"
        p.sig = loose                               # the screen shows notes
        p.shape = 125.0
    p.accent = beat_down(b % 1.0, 8)
    return p


ALL_MOVES.update({"m1_tick": (tick, 2.0), "m1_step": (step, 4.0), "m1_ramp": (ramp, 2.0)})


def colours(th):
    night = th["name"] == "Night"
    return {"body": "#2B3239" if night else "#E4E9EE", "body_l": "#3A434C" if night else "#FBFCFD",
            "rim": "#76818D" if night else "#7A8692", "screen": "#0D1014", "led": "#F2C766",
            "led_dim": "#3A3220", "limb": "#7D8996" if night else "#6A7682", "joint": "#59636E",
            "pad": "#5B79D6", "pad_lit": "#A9BFFA", "bulb_off": "#3A434C" if night else "#C5CDD5",
            "verm": th["verm"], "gold": th["gold"], "foot": "#14191E" if night else "#38444F"}


SEG = {"0": "abcdef", "1": "bc", "2": "abged", "3": "abgcd", "4": "fgbc", "5": "afgcd", "6": "afgedc",
       "7": "abc", "8": "abcdefg", "9": "abcfgd", "-": "g"}


def digit(d, ch, x, y, w, h, col, thick):
    """A 7-segment digit with its top-left at (x, y)."""
    pts = {"a": [(x, y), (x + w, y)], "b": [(x + w, y), (x + w, y + h / 2)], "c": [(x + w, y + h / 2), (x + w, y + h)],
           "d": [(x, y + h), (x + w, y + h)], "e": [(x, y + h / 2), (x, y + h)], "f": [(x, y), (x, y + h / 2)],
           "g": [(x, y + h / 2), (x + w, y + h / 2)]}
    for s in SEG.get(ch, ""):
        d.line(pts[s], thick, col, cut=False, cap="butt")


def draw(pose, th, cut_color=None, trails=None, floor=True, face=True):
    j = solve(pose, PROPS)
    P = lambda p: place(p, j)
    C = colours(th)
    room = cut_color or th["base"]
    d = Doc(cut_color=room, cut=CUT)
    gx = (P(j.feet[0])[0] + P(j.feet[1])[0]) / 2
    if floor:
        d.raw(f'<ellipse cx="{f(gx)}" cy="0.010" rx="0.20" ry="0.028" fill="{th["shadow"]}" opacity="0.5"/>')
    # legs: straight segments, square joints, block feet
    for i, s in enumerate((-1, 1)):
        hip, knee, foot = P(j.hips[i]), P(j.knees[i]), P(j.feet[i])
        d.line([hip, knee, add(foot, (0, -0.03))], 0.044, C["limb"], cut=False, cap="butt")
        dd, tr = rrect_d(knee[0], knee[1], 0.050, 0.050, 0.010, j.torso_deg)
        d.path(dd, C["joint"], cut=False, transform=tr)
        dd, tr = rrect_d(foot[0] + s * 0.016, foot[1] - 0.026, 0.120, 0.052, 0.012, -s * j.toes[i] * 0.5)
        d.path(dd, C["foot"], cut=False, transform=tr)
    # the slab, drawn in its own frame
    c = add(P(j.pelvis), rot((0.0, -0.215), j.torso_deg))
    sq = j.squash
    d.raw(f'<g transform="translate({f(c[0])},{f(c[1])}) rotate({f(j.torso_deg)}) scale({f(1 + 0.6 * sq)},{f(1 - sq)})">')
    for w_, col, cut in ((0.012, C["rim"], True), (0.0, C["body_l"], False)):
        dd, tr = rrect_d(0, 0, 0.31 + w_, 0.50 + w_, 0.055, 0)
        d.path(dd, col, cut=cut, transform=tr)
    dd, tr = rrect_d(0, 0.006, 0.306, 0.488, 0.050, 0)
    d.path(dd, C["body"], cut=False, transform=tr)
    dd, tr = rrect_d(0, -0.135, 0.250, 0.170, 0.030, 0)
    d.path(dd, C["screen"], cut=False, transform=tr)
    if face:
        _eyes(d, pose, C)
    _readout(d, pose, C)
    lit = pose.accent > 0.4
    d.circle((0, 0.080), 0.068, C["rim"], cut=False)
    d.circle((0, 0.080), 0.060, C["pad_lit"] if lit else C["pad"], cut=False)
    d.raw(f'<text x="0" y="0.092" font-size="0.032" font-weight="600" text-anchor="middle" fill="{C["screen"]}">TAP</text>')
    for s in (-1, 1):
        dd, tr = rrect_d(s * 0.112, 0.080, 0.040, 0.034, 0.008, 0)
        d.path(dd, C["joint"], cut=False, transform=tr)
        d.line([(s * 0.112 - 0.010, 0.080), (s * 0.112 + 0.010, 0.080)], 0.005, C["body_l"], cut=False, cap="butt")
        if s > 0:
            d.line([(s * 0.112, 0.070), (s * 0.112, 0.090)], 0.005, C["body_l"], cut=False, cap="butt")
    # the beat light: gold on every beat, vermilion on the one
    d.line([(0, -0.250), (0, -0.300)], 0.016, C["rim"], cut=True)
    one = int(math.floor(pose.t + 1e-6)) % 4 == 0
    bulb = (C["verm"] if one else C["gold"]) if pose.accent > 0.3 else C["bulb_off"]
    if pose.accent > 0.3:
        d.raw(f'<circle cx="0" cy="-0.322" r="{f(0.034 + 0.030 * pose.accent)}" fill="{bulb}" opacity="0.25"/>')
    d.circle((0, -0.322), 0.034, bulb)
    d.raw("</g>")
    # arms: straight segments, square joints, clicker mitts
    for i in range(2):
        sh, el, ha = P(j.shoulders[i]), P(j.elbows[i]), P(j.hands[i])
        d.line([sh, el, ha], 0.040, C["limb"], cut=True, cap="butt")
        for pnt in (sh, el):
            dd, tr = rrect_d(pnt[0], pnt[1], 0.046, 0.046, 0.010, j.torso_deg)
            d.path(dd, C["joint"], cut=False, transform=tr)
        fd = sub(ha, el)
        ang = math.degrees(math.atan2(fd[1], fd[0]))
        dd, tr = rrect_d(ha[0], ha[1], 0.060, 0.048, 0.014, ang)
        d.path(dd, C["foot"], cut=True, transform=tr)
    return d.svg()


def _eyes(d, pose, C):
    """LED segments in the screen."""
    e = "happy" if pose.sig > 0.2 else pose.eyes
    led = C["led"]
    for s in (-1, 1):
        x, y = s * 0.052, -0.168
        if e == "happy":
            d.line([(x - 0.020, y + 0.010), (x, y - 0.012), (x + 0.020, y + 0.010)], 0.012, led, cut=False)
        elif e in ("closed", "sleepy"):
            d.line([(x - 0.020, y + 0.012), (x + 0.020, y + 0.012)], 0.010, led if e == "closed" else C["led_dim"], cut=False)
        elif e == "wide":
            d.raw(f'<rect x="{f(x - 0.018)}" y="{f(y - 0.022)}" width="0.036" height="0.044" fill="none" stroke="{led}" stroke-width="0.010"/>')
        elif e == "cool":
            if s < 0:
                d.line([(-0.085, y), (0.085, y)], 0.014, led, cut=False, cap="butt")
        else:
            d.line([(x, y - 0.022), (x, y + 0.020)], 0.013, led, cut=False, cap="butt")


def _readout(d, pose, C):
    """The BPM in 7-segment digits (an unlit 888 behind them); notes when it breaks loose."""
    w, h, gap = 0.020, 0.036, 0.034
    if pose.sig > 0.3:
        for k, x in enumerate((-0.040, 0.0, 0.040)):
            y = -0.082 - 0.010 * math.sin(pose.t * 12 + k)
            d.circle((x, y + 0.010), 0.008, C["led"], cut=False)
            d.line([(x + 0.007, y + 0.010), (x + 0.007, y - 0.018)], 0.004, C["led"], cut=False)
        return
    txt = f"{int(round(pose.shape)):3d}"[-3:]
    for k in range(3):
        x = -0.052 + k * gap
        digit(d, "8", x, -0.110, w, h, C["led_dim"], 0.005)
        if txt[k] != " ":
            digit(d, txt[k], x, -0.110, w, h, C["led"], 0.005)


def key_poses():
    def at(fn, b):
        p = fn(b, STYLE)
        p.t = b
        return p
    return [("Idle", Pose(eyes="open", shape=125, fk=((10, 0), (10, 0)))), ("Tick", at(tick, 0.0)),
            ("Tock", at(tick, 0.5)), ("Up", at(tick, 1.0)), ("Step", at(step, 1.0)),
            ("Ramp 250", at(ramp, 0.55)), ("Ramp 500", at(ramp, 0.97)), ("Loose!", at(ramp, 1.35)),
            ("Loose", at(ramp, 1.7))]


def mood_items():
    return [("Neutral", Pose(eyes="open", shape=125)), ("Happy", Pose(eyes="happy", shape=125)),
            ("Excited", Pose(eyes="wide", shape=250, accent=1.0)), ("Calm", Pose(eyes="closed", shape=90)),
            ("Surprised", Pose(eyes="wide", shape=125)), ("Sleepy", Pose(eyes="sleepy", shape=60)),
            ("Cool", Pose(eyes="cool", shape=125)), ("Loose", Pose(eyes="happy", sig=1.0, t=0.3))]


MOOD_FOCUS = -0.62
MOOD_SIZE = 320
