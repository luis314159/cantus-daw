"""New — **Bajo**, the speaker: a studio monitor with a heavy, calm groove.

A tall cabinet is its body and head: two tweeters near the top are its eyes (a gold dome
for each pupil, lids of the cabinet's colour for moods), the woofer below is its chest and
pumps with the music, a bass port is its belt. Short thick legs on isolation pads, short
arms with big mitts. Graphite in both themes (a monitor is black), held at 3:1 on Night by
the knob's rim. It moves like weight: a slow heavy nod, a stomp, and its signature, **Drop
the bass**: it winds up, then the woofer blasts — gold shockwave rings, the cabinet jolts
and shakes.
"""
import math
from body import Props, solve, place
from moves import Style, MOVES as ALL_MOVES, steps, _hands, _lerp_pair
from rig import (Pose, add, sub, mul, rot, length, lerp, snap_time, beat_down, cos1, sin1,
                 smooth, ease_out, ease_out_back, clamp01, arc)
from svg import Doc, tube_d, rrect_d, curve_d, f

NAME = "Bajo"
PROPS = Props(leg=0.20, thigh=0.108, shin=0.100, torso=0.36, neck=0.02, head=0.10,
              shoulder=0.235, hipw=0.10, stance=0.11, upper=0.13, fore=0.13, foot=0.14)
STYLE = Style(bounce=0.16, sway=0.10, step=0.30, arms=1.0, lag=0.16, hop=0.08, squash=0.10,
              snap=0.25, lean=1.0, lift_step=0.16)
MOVES = {"groove": "bajo_heavy", "steps": "bajo_stomp", "jump": "bajo_drop"}
CUT = 0.014
CAB_W, CAB_TOP, CAB_BOT = 0.46, 0.40, 0.04   # width; above the neck; below the pelvis


def lp(a, b, t):
    return (lerp(a[0], b[0], t), lerp(a[1], b[1], t))


def heavy(b, st):
    """Two beats: the cabinet rocks side to side and squashes on each beat, the knees take
    the weight, the woofer pumps; the mitts swing low and slow."""
    b = snap_time(b, 0.5, move=0.5, amount=st.snap)
    p = Pose()
    ph = b % 1.0
    down = beat_down(ph, 1.6)
    side = cos1(b / 2.0)
    p.hip = (0.03 * side, st.bounce * down)
    p.lean = 5.0 * side
    p.squash = 0.06 * beat_down(ph, 3.0)
    p.hands = ((-0.62 - 0.12 * side, 0.92), (0.62 - 0.12 * side, 0.92))
    p.eyes = "open"
    p.accent = beat_down(ph, 8)
    p.level = 0.45 + 0.55 * beat_down(ph, 2.5)
    p.nod = 6.0 * beat_down((b - st.lag) % 1.0)
    p.point = 20 * side
    return p


def stomp(b, st):
    """Four beats: the step-touch as stomps — each foot lifts high and lands hard, the
    cabinet squashing on landing, the arms out for balance."""
    p = steps(b, st)
    ph = b % 1.0
    p.squash = 0.09 * beat_down(ph, 5.0)
    p.landed = max(0.0, 1 - ph * 4)
    p.hands = ((-0.95, 0.55), (0.95, 0.55))
    p.lean *= 1.4
    p.eyes = "open"
    p.level = 0.5 + 0.45 * beat_down(ph, 3.0)
    return p


def drop(b, st):
    """Two beats, its signature: it crouches and winds up (mitts raised, squashed), holds a
    breath — then on beat 2 the woofer blasts: rings, a jolt up, a shake, and it settles."""
    p = Pose()
    bb = b % 2.0
    wind = smooth(bb / 0.8)
    blast = clamp01((bb - 1.0) / 1.0)
    hit = 1.0 if bb >= 1.0 else 0.0
    settle = smooth((bb - 1.5) / 0.5)
    p.squash = 0.14 * wind * (1 - hit) - 0.10 * hit * (1 - smooth(blast * 3)) + 0.02 * hit * smooth(blast * 3)
    p.hip = (0.0, 0.14 * wind * (1 - hit) + 0.02 * hit)
    p.lift = 0.06 * hit * arc(clamp01(blast * 2.5))
    shake = 0.018 * hit * (1 - settle) * math.sin(bb * 60)
    p.hip = (p.hip[0] + shake, p.hip[1])
    raised = ((-0.95, -0.30), (0.95, -0.30))
    flung = ((-1.05, 0.40), (1.05, 0.40))
    rest = ((-0.62, 0.92), (0.62, 0.92))
    h = _lerp_pair(rest, raised, wind)
    h = _lerp_pair(h, flung, ease_out_back(clamp01(blast * 4), 2.0) * hit)
    p.hands = _lerp_pair(h, rest, settle)
    p.feet = ((-0.10, 0.0), (0.10, 0.0))
    p.sig = blast * hit
    p.eyes = "closed" if 0.6 < bb < 1.0 else ("wide" if hit and blast < 0.35 else "open")
    p.level = max(0.3, hit * (1 - settle * 0.5))
    p.accent = beat_down(b % 1.0, 8)
    return p


ALL_MOVES.update({"bajo_heavy": (heavy, 2.0), "bajo_stomp": (stomp, 4.0), "bajo_drop": (drop, 2.0)})

PAL = {
    "cab": "#262D34", "cab_lip": "#3A434C", "baffle": "#2F373F", "rim": "#76818D",
    "edge": "#0D1014", "surround": "#14191E", "cone": "#4C5661", "cone_s": "#3A434C",
    "port": "#0D1014", "limb": "#6A7682", "mitt": "#B4BDC6", "pad": "#14191E",
}


def draw(pose, th, cut_color=None, trails=None, floor=True, face=True):
    j = solve(pose, PROPS)
    P = lambda p: place(p, j)
    c = PAL
    room = cut_color or th["base"]
    d = Doc(cut_color=room, cut=CUT)
    s = j.squash
    sx, sy = 1 + 0.6 * s, 1 - s
    gx = (P(j.feet[0])[0] + P(j.feet[1])[0]) / 2
    if floor:
        k = max(0.0, 1.0 - pose.lift * 4.0)
        d.raw(f'<ellipse cx="{f(gx)}" cy="0.01" rx="{f(0.26 * k + 0.05)}" ry="{f(0.032 * k + 0.008)}" '
              f'fill="{th["shadow"]}" opacity="0.6"/>')
        if pose.landed > 0.05:   # dust where a stomp lands
            for n, sd in enumerate((-1, 1)):
                fx = P(j.feet[n])
                r = 0.03 + 0.05 * (1 - pose.landed)
                d.raw(f'<ellipse cx="{f(fx[0] + sd * 0.09)}" cy="-0.012" rx="{f(r)}" ry="{f(r * 0.45)}" '
                      f'fill="{th["ink4"]}" opacity="{f(pose.landed)}"/>')
    # legs on isolation pads
    for i, sd in enumerate((-1, 1)):
        hip, knee, foot = P(j.hips[i]), P(j.knees[i]), P(j.feet[i])
        d.line([hip, knee, add(foot, (0, -0.03))], 0.072, c["limb"], cut=False)
        dd, tr = rrect_d(foot[0] + sd * 0.012, foot[1] - 0.024, 0.16, 0.052, 0.018, -sd * j.toes[i] * 0.4)
        d.path(dd, c["rim"], transform=tr)
        dd, tr = rrect_d(foot[0] + sd * 0.012, foot[1] - 0.026, 0.148, 0.040, 0.014, -sd * j.toes[i] * 0.4)
        d.path(dd, c["pad"], cut=False, transform=tr)
    # the cabinet, rigid, turned with the torso about the pelvis
    deg = j.torso_deg
    T = PROPS.torso
    C = lambda x, y: add(P(j.pelvis), rot((x * sx, y * sy), deg))
    cy = (-(T + CAB_TOP) + CAB_BOT) / 2
    H = T + CAB_TOP + CAB_BOT
    cx_, cy_ = C(0, cy)
    for w_, col, cut in ((0.0, c["rim"], True), (-0.010, c["cab_lip"], False)):
        dd, tr = rrect_d(cx_, cy_, (CAB_W + w_) * sx, (H + w_) * sy, 0.06, deg)
        d.path(dd, col, cut=cut, transform=tr)
    bx, by = C(0, cy + 0.006)
    dd, tr = rrect_d(bx, by, (CAB_W - 0.016) * sx, (H - 0.024) * sy, 0.052, deg)
    d.path(dd, c["cab"], cut=False, transform=tr)
    dd, tr = rrect_d(bx, by, (CAB_W - 0.060) * sx, (H - 0.070) * sy, 0.036, deg)
    d.path(dd, c["baffle"], cut=False, transform=tr)
    # the woofer: surround, cone (two values), gold dust cap; it pumps with the level
    wc = C(0, -0.30)
    pump = 1.0 + 0.07 * pose.level + 0.05 * pose.accent
    d.circle(wc, 0.168, c["surround"], cut=False)
    d.circle(wc, 0.142 * pump, c["cone_s"], cut=False)
    d.circle(add(wc, (-0.008, -0.010)), 0.126 * pump, c["cone"], cut=False)
    d.circle(wc, 0.050 * pump, th["gold"] if th["name"] == "Night" else "#E2B85A", cut=False)
    if pose.sig > 0:     # the blast: three gold rings out from the woofer
        for n in range(3):
            t = clamp01(pose.sig * 1.6 - n * 0.22)
            if 0 < t < 1:
                r = 0.18 + 0.55 * t
                d.raw(f'<ellipse cx="{f(wc[0])}" cy="{f(wc[1])}" rx="{f(r)}" ry="{f(r * 0.92)}" fill="none" '
                      f'stroke="{th["gold"]}" stroke-width="{f(0.016 * (1 - t) + 0.003)}" opacity="{f(1 - t)}"/>')
    # the port: its belt
    pc = C(0, -0.03)
    dd, tr = rrect_d(pc[0], pc[1], 0.20 * sx, 0.034 * sy, 0.017, deg)
    d.path(dd, c["port"], cut=False, transform=tr)
    if face:
        _eyes(d, C, pose, c, th, deg)
    # arms from the cabinet's sides, big mitts
    for i in range(2):
        sh, el, ha = P(j.shoulders[i]), P(j.elbows[i]), P(j.hands[i])
        d.line([sh, el, ha], 0.060, c["limb"], cut=True)
        d.circle(ha, 0.054, c["edge"], cut=False)
        d.circle(ha, 0.047, c["mitt"], cut=False)
    return d.svg()


def _eyes(d, C, pose, c, th, deg):
    """Two tweeters: a dark ring, a gold dome for a pupil; moods are lids of the baffle."""
    e = pose.eyes
    if e == "cool":
        a, b = C(-0.16, -0.62), C(0.16, -0.62)
        d.line([a, b], 0.110, c["edge"], cut=False)
        d.line([C(-0.13, -0.645), C(-0.07, -0.645)], 0.010, th["gold"], cut=False)
        return
    for sd in (-1, 1):
        tc = C(sd * 0.095, -0.62)
        big = 1.15 if e == "wide" else 1.0
        d.circle(tc, 0.060 * big, c["edge"], cut=False)
        d.circle(tc, 0.050 * big, c["rim"], cut=False)
        d.circle(tc, 0.040 * big, c["surround"], cut=False)
        d.circle(add(tc, (0.004, 0.004)), 0.020 * big, "#E2B85A", cut=False)
        d.circle(add(tc, (-0.006, -0.008)), 0.006, "#FFF1C9", cut=False)
        if e in ("open", "sleepy"):     # a heavy lid: calm and sure
            drop_ = 0.030 if e == "open" else 0.052
            dd, tr = rrect_d(*C(sd * 0.095, -0.62 - 0.064 + drop_), 0.15, 0.064, 0.01, deg)
            d.path(dd, c["baffle"], cut=False, transform=tr)
            d.line([C(sd * 0.095 - 0.062, -0.62 - 0.032 + drop_), C(sd * 0.095 + 0.062, -0.62 - 0.032 + drop_ - sd * 0.004)],
                   0.010, c["edge"], cut=False)
        elif e in ("happy", "closed"):
            dd, tr = rrect_d(*C(sd * 0.095, -0.62 + 0.040 if e == "happy" else -0.62), 0.15, 0.070 if e == "happy" else 0.13, 0.03, deg)
            d.path(dd, c["baffle"], cut=False, transform=tr)
            y = -0.62 + 0.006
            pts = ([C(sd * 0.095 - 0.05, y + 0.012), C(sd * 0.095, y - 0.012), C(sd * 0.095 + 0.05, y + 0.012)]
                   if e == "happy" else [C(sd * 0.095 - 0.05, y), C(sd * 0.095, y + 0.012), C(sd * 0.095 + 0.05, y)])
            d.line(pts, 0.014, "#E2B85A", cut=False, smooth=True)


def key_poses():
    return [
        ("Idle", Pose(eyes="open", hands=((-0.62, 0.92), (0.62, 0.92)), level=0.3)),
        ("Heavy nod", heavy(0.0, STYLE)),
        ("Rock", heavy(1.0, STYLE)),
        ("Stomp", stomp(0.0, STYLE)),
        ("Lift", stomp(0.75, STYLE)),
        ("Wind up", drop(0.8, STYLE)),
        ("Drop the bass", drop(1.25, STYLE)),
        ("Cool", Pose(eyes="cool", lean=-4, hands=((-0.50, 0.55), (0.50, 0.55)), level=0.6)),
        ("Flex", Pose(eyes="happy", hands=((-0.95, -0.35), (0.95, -0.35)), level=0.9, hip=(0, 0.06))),
    ]


def mood_items():
    return [("Neutral", Pose(eyes="open")), ("Happy", Pose(eyes="happy", level=0.8)),
            ("Excited", Pose(eyes="happy", level=1.0, accent=1.0)), ("Calm", Pose(eyes="closed")),
            ("Surprised", Pose(eyes="wide", level=0.2)), ("Sleepy", Pose(eyes="sleepy", level=0.1, lean=6)),
            ("Cool", Pose(eyes="cool", level=0.6)), ("Blast", Pose(eyes="wide", level=1.0, sig=0.35))]


MOOD_FOCUS = -0.76   # where the face is (units), for the sheet's close-ups


MOOD_SIZE = 300   # the sheet's face close-ups
