"""Metrónomo M2 — **Péndulo**, the acrobat: the pendulum is her whole body.

A gymnast in a one-hand handstand on a small block with a gold pivot pin: her body is a
metronome's (inverted) pendulum, swinging side to side with each extreme exactly on a beat.
- **Her outfit:** a lapis leotard with a vermilion stripe, the metronome's **gold weight**
  as a disc on her belly, a gold headband.
- **Her face:** upside down and smiling.
- **The rhythm:** her legs split open on the beat; the free arm balances out to the side.

Tempo as athletics. Signature, **Doble tiempo**: the swings go double-time; then she swings
over into a full **flip** and sticks an upright landing on the drop (arms in a V, a puff of
chalk), and rolls back into the handstand.
"""
import math
from body import Props
from moves import Style, MOVES as ALL_MOVES
from rig import (Pose, add, sub, mul, rot, length, lerp, beat_down, cos1, smooth, ease_in_out,
                 ease_out, ease_out_back, clamp01, arc)
from svg import Doc, tube_d, rrect_d, curve_d, f

NAME = "Péndulo"
PROPS = Props()
STYLE = Style()
MOVES = {"groove": "m2_swing", "steps": "m2_scissor", "jump": "m2_flip"}
CUT = 0.012
PIVOT = (0.0, -0.165)
ARM, TORSO, LEG = 0.15, 0.25, 0.38
SWING = 22.0


def swing(b, st):
    """Two beats: she swings on her hand like a pendulum — the extremes on the beats — her
    legs splitting open on each beat, the free arm balancing against the swing."""
    p = Pose()
    p.shape = SWING * math.cos(math.pi * b)
    p.point = 34.0 * beat_down(b % 1.0, 2.0) + 6.0
    p.hands = ((-1.0, 0.0), (0.0, 0.0))
    p.lean = -p.shape * 1.2                       # the free arm, against the swing
    p.eyes = "happy" if beat_down(b % 1.0, 3.0) > 0.5 else "open"
    p.accent = beat_down(b % 1.0, 8)
    return p


def scissor(b, st):
    p = Pose()
    p.shape = 10.0 * math.cos(math.pi * b)
    sc = math.sin(math.pi * b)
    p.point = 22.0 + 20.0 * sc
    p.toe = (40.0 * max(0.0, sc), 40.0 * max(0.0, -sc))  # one knee bends, then the other
    p.lean = -30.0 * sc
    p.eyes = "open"
    p.accent = beat_down(b % 1.0, 8)
    return p


def flip(b, st):
    """Two beats: double-time swings; from the half beat a big swing over — a full flip —
    sticking an upright landing on the drop (arms in a V, chalk); held; then she rolls back
    into the handstand for the next bar."""
    p = Pose()
    bb = b % 2.0
    if bb < 0.55:
        p.shape = 30.0 * math.cos(2 * math.pi * b)
        p.point = 20.0
        p.sig = 0.0
        p.eyes = "wide"
    elif bb < 1.0:
        u = (bb - 0.55) / 0.45
        p.sig = ease_in_out(u)
        p.shape = 30.0 * (1 - u)
        p.point = 40.0 * math.sin(math.pi * u)
        p.lift = 0.16 * arc(u)
        p.eyes = "happy"
    elif bb < 1.65:
        t = bb - 1.0
        p.sig = 1.0
        p.squash = 0.10 * (1 - ease_out(clamp01(t / 0.25)))
        p.landed = max(0.0, 1 - t * 3)
        p.eyes = "happy"
        p.mouth = 0.6
    else:
        u = (bb - 1.65) / 0.35
        p.sig = 1.0 - ease_in_out(u)
        p.lift = 0.06 * arc(u)
        p.eyes = "open"
    p.accent = beat_down(b % 1.0, 8)
    return p


ALL_MOVES.update({"m2_swing": (swing, 2.0), "m2_scissor": (scissor, 4.0), "m2_flip": (flip, 2.0)})

PAL = {"wood": "#9A6A44", "wood_s": "#734B2E", "rim_w": "#C9A27A", "panel": "#5E3D25", "skin": "#B97A57", "skin_s": "#94603F", "suit": "#4664C0", "suit_s": "#2E4794", "stripe": "#E0693B",
       "hair": "#211A1A", "hair_l": "#5A443C", "block": "#2B3239", "rim": "#76818D", "eye": "#1A1414",
       "lip": "#7E4232", "gold": "#E2B85A", "gold_s": "#B38A35"}


def _unit(deg):
    r = math.radians(deg)
    return (math.cos(r), math.sin(r))


def draw(pose, th, cut_color=None, trails=None, floor=True, face=True):
    c = PAL
    room = cut_color or th["base"]
    d = Doc(cut_color=room, cut=CUT)
    s = clamp01(pose.sig)
    if floor:
        d.raw(f'<ellipse cx="0" cy="0.010" rx="0.16" ry="0.024" fill="{th["shadow"]}" opacity="0.5"/>')
        if pose.landed > 0.05:
            for sd in (-1, 1):
                r = 0.03 + 0.06 * (1 - pose.landed)
                d.raw(f'<ellipse cx="{f(sd * (0.10 + 0.07 * (1 - pose.landed)))}" cy="-0.10" rx="{f(r)}" ry="{f(r * 0.6)}" '
                      f'fill="{th["ink3"]}" opacity="{f(pose.landed * 0.8)}"/>')
    # her stage: a small wooden metronome housing (its pendulum is her), gold ticks, the pivot
    house = [(-0.150, 0.0), (0.150, 0.0), (0.085, -0.150), (-0.085, -0.150)]
    hd = "M" + " L".join(f"{f(x)},{f(y)}" for x, y in house) + " Z"
    d.path(hd, c["rim_w"], cut=True, extra=f' stroke="{c["rim_w"]}" stroke-width="0.018" stroke-linejoin="round"')
    d.cel(hd, c["wood"], c["wood_s"], off=(-0.012, -0.006), cut=False)
    d.path("M" + " L".join(f"{f(x)},{f(y)}" for x, y in [(-0.040, -0.020), (0.040, -0.020), (0.028, -0.135), (-0.028, -0.135)]) + " Z",
           c["panel"], cut=False)
    for n in range(5):
        y = -0.035 - n * 0.022
        d.line([(-0.022 + n * 0.002, y), (0.022 - n * 0.002, y)], 0.004, c["gold"], cut=False)
    d.circle(PIVOT, 0.018, c["gold"], cut=False)
    # the body's axis: V points from the hips to the shoulders (down in the handstand, up standing)
    th0 = pose.shape
    U = (math.sin(math.radians(th0)), -math.cos(math.radians(th0)))      # up from the hand
    M0 = add(PIVOT, mul(U, ARM + TORSO / 2))
    M1 = (0.0, PIVOT[1] - (LEG + TORSO / 2))
    M = add((lerp(M0[0], M1[0], s), lerp(M0[1], M1[1], s)), (0.0, -pose.lift))
    va = math.degrees(math.atan2(-U[1], -U[0]))                          # handstand: toward the pivot
    va = va - 180.0 * s                                                  # flipping over to upright
    V = _unit(va)
    sq = pose.squash
    N = (-V[1], V[0])
    # in the handstand the supporting shoulder is right over the pivot: shift the body sideways
    M = add(M, mul(N, 0.045 * (1 - s)))
    S = add(M, mul(V, TORSO / 2 * (1 - sq)))
    H = sub(M, mul(V, TORSO / 2 * (1 - sq)))
    # legs: along −V, split by the beat
    split = pose.point
    feet = []
    for k, sd in enumerate((-1, 1)):
        dirv = _unit(va + 180 + sd * split / 2)
        bend = pose.toe[k] if pose.toe else 0.0
        knee = add(add(H, mul(N, sd * 0.03)), mul(dirv, LEG * 0.5))
        dir2 = _unit(va + 180 + sd * split / 2 + sd * bend)
        foot = add(knee, mul(dir2, LEG * 0.5))
        feet.append(foot)
        d.path(tube_d([add(H, mul(N, sd * 0.03)), knee, foot], [0.074, 0.058, 0.044]), c["skin"], cut=True)
        d.ellipse(foot, 0.030, 0.020, c["skin_s"], deg=va + sd * split / 2)
    # the leotard: shoulders to hips, a vermilion stripe, the gold weight
    d.cel(tube_d([S, H], [0.128, 0.110]), c["suit"], c["suit_s"], off=(-0.010, -0.008), cut=True)
    d.line([add(S, mul(N, 0.046)), add(H, mul(N, 0.040))], 0.012, c["stripe"], cut=False)
    wc = add(M, mul(V, -0.020))
    wt = [add(wc, rot((-0.034, -0.026), va + 90)), add(wc, rot((0.034, -0.026), va + 90)),
          add(wc, rot((0.022, 0.026), va + 90)), add(wc, rot((-0.022, 0.026), va + 90))]
    d.path("M" + " L".join(f"{f(x)},{f(y)}" for x, y in wt) + " Z", c["gold"], cut=True)
    d.line([add(wc, rot((-0.020, 0.0), va + 90)), add(wc, rot((0.020, 0.0), va + 90))], 0.006, c["gold_s"], cut=False)
    # arms: the support arm to the pivot (handstand) or up in a V (standing); the free arm balances
    free = _unit(va + 90 + pose.lean)
    sup_end = (lerp(PIVOT[0], add(S, mul(_unit(va - 35), ARM * 1.9))[0], s),
               lerp(PIVOT[1], add(S, mul(_unit(va - 35), ARM * 1.9))[1], s))
    free_end = (lerp(add(S, mul(free, ARM * 1.9))[0], add(S, mul(_unit(va + 35), ARM * 1.9))[0], s),
                lerp(add(S, mul(free, ARM * 1.9))[1], add(S, mul(_unit(va + 35), ARM * 1.9))[1], s))
    for k, end in enumerate((sup_end, free_end)):
        sh = add(S, mul(N, (-1 if k == 0 else 1) * 0.045))
        d.path(tube_d([sh, end], [0.056, 0.044]), c["skin"], cut=True)
        d.circle(end, 0.026, c["skin_s"], cut=False)
    # the head: beyond the shoulders along V (upside down in the handstand), bun, gold band
    hc = add(add(S, mul(V, 0.088)), mul(N, 0.030 * (1 - s)))
    bun = add(hc, mul(V, 0.075))
    d.circle(bun, 0.040, c["hair"], cut=True)
    d.circle(hc, 0.070, c["skin"], cut=True)
    up = va + 90       # the face's up is along V
    H_ = lambda x, y: add(hc, rot((x, y), up))
    cap = [H_(0.070 * math.cos(math.radians(a)), 0.070 * math.sin(math.radians(a))) for a in range(180, 361, 15)]
    d.path(curve_d(cap + [H_(0.068, -0.012), H_(-0.068, -0.012)], closed=True), c["hair"], cut=False)
    d.line([H_(-0.068, -0.022), H_(0.0, -0.040), H_(0.068, -0.022)], 0.014, c["gold"], cut=False, smooth=True)
    if face:
        _face(d, H_, pose, c)
    return d.svg()


def _face(d, H, pose, c):
    e = pose.eyes
    for s in (-1, 1):
        x = s * 0.026
        if e == "happy":
            d.line([H(x - 0.012, 0.010), H(x, -0.002), H(x + 0.012, 0.010)], 0.007, c["eye"], cut=False)
        elif e in ("closed", "sleepy"):
            d.line([H(x - 0.012, 0.006), H(x + 0.012, 0.006)], 0.006, c["eye"], cut=False)
        elif e == "cool":
            dd, tr = rrect_d(*H(x, 0.004), 0.034, 0.018, 0.006, 0)
            d.path(dd, c["eye"], cut=False, transform=tr)
        else:
            d.ellipse(H(x, 0.004), 0.009 * (1.3 if e == "wide" else 1), 0.012 * (1.3 if e == "wide" else 1), c["eye"])
    if pose.mouth > 0.3 or e == "wide":
        d.ellipse(H(0.0, 0.040), 0.012, 0.010, c["lip"])
    else:
        d.line([H(-0.018, 0.036), H(0.0, 0.046), H(0.018, 0.036)], 0.006, c["lip"], cut=False, smooth=True)


def key_poses():
    def at(fn, b):
        p = fn(b, STYLE)
        p.t = b
        return p
    return [("Handstand", Pose(shape=0, point=8, lean=0, eyes="open")), ("Swing", at(swing, 0.0)),
            ("Swing back", at(swing, 1.0)), ("Scissor", at(scissor, 0.5)), ("Double", at(flip, 0.25)),
            ("Over", at(flip, 0.75)), ("Flip", at(flip, 0.88)), ("Stuck it", at(flip, 1.1)),
            ("Back down", at(flip, 1.82))]


def mood_items():
    return [("Neutral", Pose(eyes="open", sig=1.0)), ("Happy", Pose(eyes="happy", sig=1.0)),
            ("Excited", Pose(eyes="wide", sig=1.0, mouth=0.8)), ("Calm", Pose(eyes="closed", sig=1.0)),
            ("Surprised", Pose(eyes="wide", sig=1.0)), ("Sleepy", Pose(eyes="sleepy", sig=1.0)),
            ("Cool", Pose(eyes="cool", sig=1.0)), ("Upside down", Pose(eyes="happy", sig=0.0))]


MOOD_FOCUS = -0.82
MOOD_SIZE = 320
