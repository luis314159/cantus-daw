"""Síncopa S1 — **graphic**: a bold, simplified human in strong geometric shapes.

Compás's clarity with clothes, hair and a face. Every piece is one flat value with no
shading at all. Limbs are bars of one width with round ends; the head is a disc; the hair is
a blunt **vermilion bob** (the silhouette people remember); the face is two dots, two short
brows and one line. Like Compás it wears the theme:
- the jacket is lapis (a step lighter on Night);
- the wide trousers are the theme's ink 1 (light at night, near-black on paper);
- white sneakers with a gold sole; gold hoops.

Pieces are cut from each other by the room's colour. It dances with graphic snaps (hits on
the half beat). Signature, **Spin and point**: a paper-cut spin (the figure turns as a flat
card: its width runs through the cosine, its back shown in the turn), landing on the beat
into a disco point.
"""
import math
from dataclasses import replace
from body import Props, solve, place
from moves import Style, MOVES as ALL_MOVES, steps_human
from rig import (Pose, add, sub, mul, rot, length, lerp, snap_time, beat_down, cos1, smooth,
                 ease_in_out, ease_out_back, clamp01)
from svg import Doc, tube_d, rrect_d, curve_d, f
import sincopa as v2

NAME = "Síncopa S1"
PROPS = Props(leg=0.45, thigh=0.232, shin=0.222, torso=0.265, neck=0.030, head=0.092,
              shoulder=0.088, hipw=0.050, stance=0.064, upper=0.150, fore=0.145, foot=0.10)
STYLE = Style(bounce=0.07, sway=0.12, step=0.17, lag=0.06, hop=0.12, squash=0.05, snap=0.75,
              lean=1.2, open=(0.56, 0.95), chest=-0.06, lift_step=0.06)
MOVES = {"groove": "s1_groove", "steps": "s1_step", "jump": "s1_spin"}
CUT = 0.015
W_LEG, W_ARM = 0.090, 0.070


def lp(a, b, t):
    return (lerp(a[0], b[0], t), lerp(a[1], b[1], t))


def groove(b, st):
    p = v2.swagger(b, st)
    p.eyes = "open"
    return p


def step(b, st):
    p = steps_human(snap_time(b, 0.5, move=0.3, amount=0.7), st)
    p.eyes = "happy" if 0.9 < (b % 2.0) < 1.4 else "open"
    return p


def spin(b, st):
    """Two beats: a dip with the arms in; a full turn as a flat card over most of a beat;
    landing on beat 2 into a disco point, held with a bounce."""
    p = Pose()
    bb = b % 2.0
    dip = smooth(bb / 0.3) * (1 - smooth((bb - 0.3) / 0.2))
    turn = ease_in_out(clamp01((bb - 0.3) / 0.7))
    land = ease_out_back(clamp01((bb - 1.0) / 0.25), 2.0)
    p.sig = turn if bb < 1.0 else 0.0
    p.hip = (0.0, 0.06 * dip + 0.05 * beat_down(bb % 1.0, 3.0) * (1 if bb >= 1.0 else 0))
    p.lift = 0.03 * math.sin(math.pi * turn) if bb < 1.0 else 0.0
    tuck = ((-0.12, 0.44), (0.12, 0.44))
    point = ((-0.40, 0.78), (0.66, -0.92))
    rest = ((-0.56, 0.95), (0.56, 0.95))
    h = v2.lp(rest[0], tuck[0], smooth(bb / 0.3)), v2.lp(rest[1], tuck[1], smooth(bb / 0.3))
    p.hands = (lp(h[0], point[0], land), lp(h[1], point[1], land))
    p.feet = ((0.0, 0.0), (0.14 * land, 0.0))
    p.toe = (0.0, 14.0 * land)
    p.lean = 7.0 * land
    p.tilt = -10.0 * land
    p.eyes = "happy" if bb < 1.0 else "cool"
    p.accent = beat_down(b % 1.0, 8)
    return p


ALL_MOVES.update({"s1_groove": (groove, 2.0), "s1_step": (step, 4.0), "s1_spin": (spin, 2.0)})


def colours(th):
    night = th["name"] == "Night"
    return {"jacket": "#6A86DA" if night else "#2E4794", "inner": "#EDE6DA",
            "pants": th["ink1"], "hair": "#F0784A" if night else "#D2541A",
            "skin": "#B97A57", "shoe": "#F3F5F7", "sole": "#E2B85A" if night else "#B8882C",
            "eye": "#14191E", "gold": th["gold"]}


def _local(origin, deg):
    return lambda x, y: add(origin, rot((x, y), deg))


def draw(pose, th, cut_color=None, trails=None, floor=True, face=True):
    j = solve(pose, PROPS)
    P = lambda p: place(p, j)
    C = colours(th)
    room = cut_color or th["base"]
    d = Doc(cut_color=room, cut=CUT)
    gx = (P(j.feet[0])[0] + P(j.feet[1])[0]) / 2
    out = []
    if floor:
        k = max(0.0, 1.0 - pose.lift * 3.0)
        out.append(f'<ellipse cx="{f(gx)}" cy="0.010" rx="{f(0.16 * k + 0.05)}" ry="{f(0.024 * k + 0.008)}" '
                   f'fill="{th["shadow"]}" opacity="0.5"/>')
    turn = math.cos(2 * math.pi * pose.sig)
    back = turn < 0
    # legs and torso: one silhouette, flat
    T = PROPS.torso
    L = _local(P(j.neck), j.torso_deg)
    legs = [[P(j.hips[i]), P(j.knees[i]), add(P(j.feet[i]), (0, -0.040))] for i in range(2)]
    jacket = [L(-0.050, -0.004), L(-0.112, 0.014), L(-0.120, 0.060), L(-0.104, T + 0.020),
              L(0.104, T + 0.020), L(0.120, 0.060), L(0.112, 0.014), L(0.050, -0.004)]
    jd = "M" + " L".join(f"{f(x)},{f(y)}" for x, y in jacket) + " Z"
    for leg in legs:
        d.line(leg, W_LEG + 2 * CUT, room, cut=False, cap="butt")
    d.raw(f'<path d="{jd}" fill="{room}" stroke="{room}" stroke-width="{f(0.03 + 2 * CUT)}" stroke-linejoin="round"/>')
    for leg in legs:
        d.line(leg, W_LEG, C["pants"], cut=False, cap="butt")
    d.raw(f'<path d="{jd}" fill="{C["jacket"]}" stroke="{C["jacket"]}" stroke-width="0.03" stroke-linejoin="round"/>')
    if not back:
        d.path("M" + " L".join(f"{f(x)},{f(y)}" for x, y in (L(-0.036, -0.004), L(0.036, -0.004), L(0.0, 0.105))) + " Z",
               C["inner"], cut=False)
    # sneakers: a half capsule on a gold sole
    for i, s in enumerate((-1, 1)):
        foot = P(j.feet[i])
        ang = -s * j.toes[i] * 0.6
        c = (foot[0] + s * 0.020, foot[1] - 0.026)
        dd, tr = rrect_d(c[0], c[1] + 0.020, 0.130, 0.016, 0.008, ang)
        d.path(dd, C["sole"], cut=False, transform=tr)
        dd, tr = rrect_d(c[0], c[1], 0.120, 0.050, 0.025, ang)
        d.path(dd, C["shoe"], cut=False, transform=tr)
    # head: neck bar, disc, the vermilion bob, hoops, the face
    hc = P(j.head)
    H = _local(hc, j.head_deg)
    d.line([L(0, 0.0), H(0, 0.06)], 0.044, C["skin"], cut=False, cap="butt")
    R = PROPS.head
    d.circle(hc, R, C["skin"], cut=True)
    arc = [H(0.108 * math.cos(math.radians(a)), -0.006 + 0.108 * math.sin(math.radians(a))) for a in range(180, 361, 12)]
    if back:
        bob = arc + [H(0.108, 0.080), H(-0.108, 0.080)]
    else:   # a sharp A-line bob: longer at the front, a side-swept fringe
        bob = arc + [H(0.110, 0.086), H(0.068, 0.068), H(0.064, -0.006), H(0.036, -0.036),
                     H(-0.020, -0.050), H(-0.068, -0.032), H(-0.072, 0.054), H(-0.104, 0.074)]
    d.path("M" + " L".join(f"{f(x)},{f(y)}" for x, y in bob) + " Z", C["hair"], cut=False)
    for s in (-1, 1):
        if not back or True:
            hx, hy = H(s * 0.080, 0.090)
            d.raw(f'<circle cx="{f(hx)}" cy="{f(hy)}" r="0.013" fill="none" stroke="{C["gold"]}" stroke-width="0.006"/>')
    if face and not back:
        _face(d, H, pose, C)
    # arms: bars in the jacket's colour, cut where they cross, skin hands
    for i in range(2):
        sh, el, ha = P(j.shoulders[i]), P(j.elbows[i]), P(j.hands[i])
        free = add(sh, mul(sub(el, sh), 0.55))
        d.line([free, el, ha], W_ARM + 2 * CUT, room, cut=False)
        d.line([sh, el, ha], W_ARM, C["jacket"], cut=False)
        d.circle(ha, 0.032, C["skin"], cut=False)
    body = "".join(out[1:]) if False else d.svg()
    cx = P(j.pelvis)[0]
    k = max(0.22, abs(turn))
    spun = f'<g transform="translate({f(cx)},0) scale({f(k)},1) translate({f(-cx)},0)">{body}</g>'
    return (out[0] if out else "") + spun


def _face(d, H, pose, C):
    e = pose.eyes
    eye = C["eye"]
    for s in (-1, 1):
        c = H(s * 0.032, 0.010)
        if e == "happy":
            d.line([H(s * 0.032 - 0.012, 0.016), H(s * 0.032, 0.004), H(s * 0.032 + 0.012, 0.016)], 0.008, eye, cut=False)
        elif e in ("closed", "sleepy"):
            d.line([H(s * 0.032 - 0.012, 0.010), H(s * 0.032 + 0.012, 0.012)], 0.008, eye, cut=False)
        elif e == "wink" and s > 0:
            d.line([H(s * 0.032 - 0.012, 0.010), H(s * 0.032 + 0.012, 0.010)], 0.008, eye, cut=False)
        elif e != "cool":
            d.circle(c, 0.011 * (1.35 if e == "wide" else 1.0), eye, cut=False)
    if e == "cool":
        a, b = H(-0.064, 0.008), H(0.064, 0.008)
        d.line([a, b], 0.028, eye, cut=False, cap="butt")
        d.line([H(-0.050, 0.002), H(-0.034, 0.002)], 0.005, C["gold"], cut=False)
    if pose.mouth > 0.25 or e == "wide":
        d.circle(H(0.0, 0.052), 0.012, eye, cut=False)
    elif e in ("happy", "wink"):
        d.line([H(-0.020, 0.046), H(0.0, 0.058), H(0.020, 0.046)], 0.007, eye, cut=False, smooth=True)
    else:
        d.line([H(-0.016, 0.052), H(0.018, 0.048)], 0.007, eye, cut=False)


def key_poses():
    st = STYLE
    return [("Idle", Pose(eyes="open", hip=(0.05, 0.02), lean=-3, tilt=6, hands=((-0.56, 0.95), (0.38, 0.80)))),
            ("Groove", groove(0.0, st)), ("Pop", groove(0.5, st)), ("Step", step(1.0, st)),
            ("Dip", spin(0.25, st)), ("Spin", spin(0.62, st)), ("Back", spin(0.80, st)),
            ("Point", spin(1.3, st)), ("Lean back", Pose(hip=(0.06, 0.03), lean=-9, eyes="cool",
                                                         hands=((-0.40, 0.80), (0.42, 0.80)), feet=((-0.12, 0), (0.08, 0))))]


def mood_items():
    return [("Neutral", Pose(eyes="open")), ("Happy", Pose(eyes="happy")), ("Excited", Pose(eyes="happy", mouth=0.8)),
            ("Calm", Pose(eyes="closed", tilt=6)), ("Surprised", Pose(eyes="wide")),
            ("Sleepy", Pose(eyes="sleepy", nod=14, tilt=12)), ("Cool", Pose(eyes="cool", tilt=-6)),
            ("Wink", Pose(eyes="wink", tilt=8))]


MOOD_SIZE = 420
