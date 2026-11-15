"""Síncopa S3 — **noir**: a human drawn as a silhouette, mysterious and stylish.

One dark ink for the whole figure. A **gold rim light** runs down its lit side, a cool rim
down the other: that is what draws it on Night's dark room, while on Paper it is a sharp
silhouette. It wears:
- a hood (gold headphones over it);
- a long open coat whose tails swing after the body;
- slim trousers and sneakers with **gold soles**, which flash when it steps.

The face is barely there: two gold glints in the hood's dark. It moves smoothly — liquid,
behind the beat, the coat following. Signature, **Arm wave**: the arms rise and a wave runs
from one hand, through the shoulders, out of the other; then it turns its head and its eyes
light.
"""
import math
from body import Props, solve, place
from moves import Style, MOVES as ALL_MOVES, steps
from rig import (Pose, add, sub, mul, rot, length, lerp, beat_down, cos1, sin1, smooth,
                 ease_in_out, clamp01)
from svg import Doc, tube_d, rrect_d, curve_d, f

NAME = "Síncopa"
PROPS = Props(leg=0.50, thigh=0.262, shin=0.248, torso=0.27, neck=0.040, head=0.082,
              shoulder=0.094, hipw=0.050, stance=0.068, upper=0.158, fore=0.152, foot=0.10)
STYLE = Style(bounce=0.05, sway=0.10, step=0.17, lag=0.22, hop=0.10, squash=0.03, snap=0.0,
              lean=1.2, lift_step=0.02, chest=-0.08)
MOVES = {"groove": "s3_liquid", "steps": "s3_glide", "jump": "s3_wave"}
RIM = 0.011


def lp(a, b, t):
    return (lerp(a[0], b[0], t), lerp(a[1], b[1], t))


def liquid(b, st):
    """Two beats, smooth: the hips sway on a sine (no hits), the chest answers, the
    shoulders roll; the right hand draws a slow circle at the chest, the left hangs."""
    p = Pose()
    ph = b % 1.0
    s = math.sin(math.pi * b)                     # one sway each way over two beats
    p.hip = (st.sway * s, st.bounce * beat_down(ph, 1.5))
    p.chest = (st.chest * s, 0.02 * beat_down(ph, 1.5))
    p.roll = 8.0 * math.sin(math.pi * (b - st.lag))
    p.lean = -2.0 - 3.0 * s
    p.tilt = 8.0 * math.sin(math.pi * (b - st.lag * 1.5))
    p.nod = 6.0 * beat_down((b - st.lag) % 1.0) - 3.0
    a = 2 * math.pi * (b / 2.0)
    p.hands = ((-0.52, 0.96), (0.30 + 0.18 * math.cos(a), 0.50 + 0.14 * math.sin(a)))
    p.eyes = "open"
    p.point = 40 * math.sin(math.pi * (b - st.lag * 2))
    p.accent = beat_down(ph, 8)
    return p


def glide(b, st):
    p = steps(b, st)
    p.hands = ((-0.55, 0.95), (0.55, 0.95))
    p.point = 50 * math.sin(math.pi * (b - 0.4) / 2.0)
    p.eyes = "open"
    p.tilt = 6 * math.sin(math.pi * b / 2.0)
    return p


def pulse(u, c, w=0.13):
    return math.exp(-((u - c) / w) ** 2)


def wave(b, st):
    """Two beats, its signature: the arms rise to a line; a wave runs from the left hand
    through the left elbow and shoulder, the chest, the right shoulder and elbow, out of
    the right hand; then the arms fall and it turns its head to look — the eyes light."""
    p = Pose()
    bb = b % 2.0
    rise = smooth(bb / 0.3)
    fall = smooth((bb - 1.55) / 0.3)
    u = clamp01((bb - 0.3) / 1.2)
    l_bend = 70 * pulse(u, 0.06)
    l_up = 26 * pulse(u, 0.24)
    r_up = 26 * pulse(u, 0.74)
    r_bend = 70 * pulse(u, 0.92)
    out = 92 * rise * (1 - fall) + 8 * (1 - rise * (1 - fall))
    p.fk = ((out + l_up * (1 - fall), l_bend * (1 - fall)), (out + r_up * (1 - fall), r_bend * (1 - fall)))
    p.chest = (0.0, -0.04 * pulse(u, 0.5))
    p.roll = 12.0 * (pulse(u, 0.40) - pulse(u, 0.60))
    p.hip = (0.0, 0.03)
    p.tilt = 10.0 * fall
    p.nod = -6.0 * fall
    p.eyes = "wide" if bb > 1.6 else "open"
    p.point = 30 * (pulse(u, 0.5))
    p.accent = beat_down(b % 1.0, 8)
    return p


ALL_MOVES.update({"s3_liquid": (liquid, 2.0), "s3_glide": (glide, 4.0), "s3_wave": (wave, 2.0)})


def colours(th):
    night = th["name"] == "Night"
    return {"ink": "#0E1216", "gold": "#E2B85A" if night else "#C99A3A",
            "cool": "#8A96A2" if night else "#7A8692", "glint": "#F2C766"}


def _local(origin, deg):
    return lambda x, y: add(origin, rot((x, y), deg))


def _lit(d, path_d, C, transform=None, r=RIM):
    """A piece of the silhouette: its gold rim on the lit (left, upper) side, a cool rim on
    the other, then the ink — the rim survives only at the piece's edges."""
    tr = f" {transform}" if transform else ""
    d.raw(f'<path d="{path_d}" fill="{C["gold"]}" transform="translate({f(-r)},{f(-r * 0.35)}){tr}"/>')
    d.raw(f'<path d="{path_d}" fill="{C["cool"]}" transform="translate({f(r * 0.6)},{f(r * 0.1)}){tr}"/>')
    d.raw(f'<path d="{path_d}" fill="{C["ink"]}"{(" transform=" + chr(34) + transform + chr(34)) if transform else ""}/>')


def draw(pose, th, cut_color=None, trails=None, floor=True, face=True):
    j = solve(pose, PROPS)
    P = lambda p: place(p, j)
    C = colours(th)
    d = Doc()
    gx = (P(j.feet[0])[0] + P(j.feet[1])[0]) / 2
    if floor:
        k = max(0.0, 1.0 - pose.lift * 3.0)
        d.raw(f'<ellipse cx="{f(gx)}" cy="0.008" rx="{f(0.17 * k + 0.05)}" ry="{f(0.024 * k + 0.008)}" '
              f'fill="{th["shadow"]}" opacity="0.55"/>')
    T = PROPS.torso
    L = _local(P(j.neck), j.torso_deg)
    sw = 0.0011 * max(-50.0, min(50.0, pose.point)) - 0.0015 * pose.lean
    # the coat's tails, behind the legs: they swing late
    for s in (-1, 1):
        tail = [L(s * 0.070, T - 0.03), L(s * 0.114, T - 0.03),
                L(s * 0.150 + sw * 1.2, T + 0.21), L(s * 0.088 + sw * 1.6, T + 0.23)]
        _lit(d, curve_d(tail, closed=True), C)
    # legs: slim trousers, sneakers with gold soles
    for i, s in enumerate((-1, 1)):
        hip, knee, foot = P(j.hips[i]), P(j.knees[i]), P(j.feet[i])
        top = add(add(hip, mul(sub(P(j.pelvis), hip), 0.3)), (0, -0.03))
        _lit(d, tube_d([top, knee, add(foot, (0, -0.040))], [0.080, 0.066, 0.062]), C)
    for i, s in enumerate((-1, 1)):
        foot = P(j.feet[i])
        ang = -s * j.toes[i] * 0.6
        base = add(foot, (s * 0.022, -0.016))
        dd, tr = rrect_d(base[0], base[1] + 0.008, 0.132, 0.020, 0.010, ang)
        d.path(dd, C["gold"], cut=False, transform=tr)
        up = [(-0.056, 0.0), (-0.052, -0.046), (-0.020, -0.066), (0.030, -0.056), (0.064, -0.020), (0.064, 0.0)]
        _lit(d, curve_d([add(base, rot((x * s, y), ang)) for x, y in up], closed=True), C, r=RIM * 0.7)
    # the coat's body (open), the hood round the head
    coat = [L(-0.036, -0.004), L(-0.102, 0.008), L(-0.128, 0.052), L(-0.122, 0.150), L(-0.112, T),
            L(0.112, T), L(0.122, 0.150), L(0.128, 0.052), L(0.102, 0.008), L(0.036, -0.004)]
    _lit(d, curve_d(coat, closed=True), C)
    d.line([L(-0.020, 0.030), L(-0.010, T - 0.01)], 0.004, C["cool"], cut=False)
    d.line([L(0.020, 0.030), L(0.010, T - 0.01)], 0.004, C["cool"], cut=False)
    hc = P(j.head)
    H = _local(hc, j.head_deg)
    hood = [H(-0.092, 0.060), H(-0.104, -0.010), H(-0.090, -0.080), H(-0.040, -0.118), H(0.020, -0.124),
            H(0.074, -0.098), H(0.100, -0.040), H(0.098, 0.040), H(0.070, 0.090), H(0.0, 0.104), H(-0.070, 0.092)]
    _lit(d, curve_d(hood, closed=True), C, r=RIM * 1.2)
    # the face: a darker opening, two gold glints
    opening = [H(-0.060, -0.010), H(-0.040, -0.062), H(0.0, -0.072), H(0.044, -0.060), H(0.062, -0.006),
               H(0.046, 0.062), H(0.0, 0.078), H(-0.046, 0.062)]
    d.path(curve_d(opening, closed=True), "#050709", cut=False)
    if face:
        _eyes(d, H, pose, C)
    # gold headphones over the hood
    band = [H(0.100 * math.cos(math.radians(a)), -0.012 + 0.110 * math.sin(math.radians(a))) for a in range(195, 346, 15)]
    d.line(band, 0.012, C["gold"], cut=False, smooth=True)
    for s in (-1, 1):
        dd, tr = rrect_d(*H(s * 0.100, 0.006), 0.030, 0.052, 0.012, 0)
        d.path(dd, C["gold"], cut=False, transform=tr)
    # arms: coat sleeves, gloved hands, each with its rim so it reads in front of the coat
    for i, s in enumerate((-1, 1)):
        sh, el, ha = P(j.shoulders[i]), P(j.elbows[i]), P(j.hands[i])
        sh = add(sh, mul(sub(P(j.neck), sh), 0.12))
        _lit(d, tube_d([sh, el, ha], [0.074, 0.058, 0.048]), C)
        fd = sub(ha, el)
        fl = length(fd) or 1e-6
        hand = add(ha, mul(fd, 0.028 / fl))
        d.raw(f'<circle cx="{f(hand[0] - RIM)}" cy="{f(hand[1] - RIM * 0.35)}" r="0.031" fill="{C["gold"]}"/>')
        d.circle(hand, 0.031, C["ink"], cut=False)
    return d.svg()


def _eyes(d, H, pose, C):
    e = pose.eyes
    g = C["glint"]
    for s in (-1, 1):
        x = s * 0.026
        if e == "happy":
            d.line([H(x - 0.012, 0.006), H(x, -0.004), H(x + 0.012, 0.006)], 0.006, g, cut=False)
        elif e in ("closed",):
            continue
        elif e == "sleepy":
            d.line([H(x - 0.010, 0.004), H(x + 0.010, 0.004)], 0.004, C["gold"], cut=False)
        elif e == "wide":
            d.circle(H(x, 0.0), 0.010, g, cut=False)
        elif e == "wink" and s > 0:
            continue
        elif e == "cool":
            if s < 0:
                d.line([H(-0.046, 0.0), H(0.046, 0.0)], 0.008, g, cut=False)
        else:
            d.line([H(x - 0.012 * s, 0.002), H(x + 0.010 * s, -0.004)], 0.007, g, cut=False)


def key_poses():
    st = STYLE
    return [("Idle", Pose(eyes="open", hip=(0.05, 0.02), lean=-3, tilt=7, hands=((-0.52, 0.96), (0.40, 0.80)),
                          feet=((0, 0), (0.06, 0)), toe=(0, 10))),
            ("Liquid", liquid(0.5, st)), ("Sway", liquid(1.5, st)), ("Glide", glide(1.0, st)),
            ("Rise", wave(0.3, st)), ("Wave in", wave(0.45, st)), ("Wave across", wave(0.9, st)),
            ("Wave out", wave(1.4, st)), ("The look", wave(1.9, st))]


def mood_items():
    return [("Neutral", Pose(eyes="open")), ("Happy", Pose(eyes="happy")), ("Excited", Pose(eyes="wide", nod=-8)),
            ("Calm", Pose(eyes="closed", tilt=6)), ("Surprised", Pose(eyes="wide")),
            ("Sleepy", Pose(eyes="sleepy", nod=14, tilt=12)), ("Cool", Pose(eyes="cool", tilt=-6)),
            ("Wink", Pose(eyes="wink", tilt=8))]


MOOD_SIZE = 420
