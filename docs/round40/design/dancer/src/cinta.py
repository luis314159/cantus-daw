"""New — **Cinta**, the cassette: an 80s cassette that breakdances.

Its body and head are a cassette:
- a graphite shell with the knob's rim;
- a cream label with two retro stripes (vermilion and lapis), no brand and no logo;
- two **reels for eyes**, white hubs with teeth that turn as it dances (lids of the label
  make the moods);
- the tape opening below is its mouth.

A **backwards lapis cap** with a gold button, thin graphite limbs, white gloves, chunky
sneakers with gold laces. Its **tape slips out** of the bottom and swirls with every move: a
glossy ribbon whose curls follow the body late.

It grooves with a breakdancer's **toprock** (cross-steps, arms opening and closing).
Signature, **Rewind**: the dance runs backwards at eight times the speed — the reels spin
back, the tape whips, a gold ◀◀ flashes — then it hits a freeze on the beat and restarts.
"""
import math
from body import Props, solve, place
from moves import Style, MOVES as ALL_MOVES, steps, _hands, _lerp_pair
from rig import (Pose, add, sub, mul, rot, length, lerp, beat_down, cos1, sin1, smooth,
                 ease_in_out, ease_out_back, clamp01)
from svg import Doc, tube_d, rrect_d, curve_d, f

NAME = "Cinta"
PROPS = Props(leg=0.40, thigh=0.208, shin=0.198, torso=0.24, neck=0.02, head=0.10,
              shoulder=0.150, hipw=0.050, stance=0.070, upper=0.150, fore=0.145, foot=0.12)
STYLE = Style(bounce=0.10, sway=0.06, step=0.20, lag=0.12, hop=0.12, squash=0.06, snap=0.35,
              lift_step=0.10)
MOVES = {"groove": "cin_toprock", "steps": "cin_shuffle", "jump": "cin_rewind"}
CUT = 0.012
W, Hc = 0.42, 0.27         # the cassette


def lp(a, b, t):
    return (lerp(a[0], b[0], t), lerp(a[1], b[1], t))


def toprock(b, st):
    """Two beats of toprock: on each beat a foot crosses in front of the other with the
    arms flung open; on the off-beat it steps back, the fists in at the chest."""
    p = Pose()
    ph = b % 1.0
    k = int(math.floor(b)) % 2
    cross = math.sin(math.pi * ph) ** 0.8 if True else 0
    hit = 1 - cross                                  # open on the beat, closed between
    s = 1.0 if k == 0 else -1.0
    p.feet = ((0.18 * cross * (s < 0) * 1.0, -0.04 * cross * (s < 0)), (-0.18 * cross * (s > 0), -0.04 * cross * (s > 0)))
    p.hands = _lerp_pair(((-0.52, 0.78), (0.52, 0.78)), ((-0.95, 0.10), (0.95, 0.10)), beat_down(ph, 1.6))
    p.hip = (-0.05 * s * cross, st.bounce * beat_down(ph, 2.0))
    p.lean = -7.0 * s * cross
    p.nod = 10.0 * beat_down((b - st.lag) % 1.0) - 3.0
    p.tilt = 8.0 * s
    p.eyes = "open"
    p.point = 50 * math.sin(math.pi * (b - st.lag))
    p.accent = beat_down(ph, 8)
    p.level = 0.5 + 0.4 * beat_down(ph, 3.0)
    return p


def shuffle(b, st):
    p = steps(b, st)
    ph = b % 1.0
    keys = {0.0: ((-0.90, 0.40), (0.90, 0.40)), 1.0: ((-0.25, 0.45), (0.85, -0.70)),
            2.0: ((-0.90, 0.40), (0.90, 0.40)), 3.0: ((-0.85, -0.70), (0.25, 0.45))}
    h0, h1, t = _hands(keys, b, 4.0)
    p.hands = _lerp_pair(h0, h1, ease_in_out(t))
    p.point = 50 * math.sin(math.pi * (b - st.lag) / 2)
    p.eyes = "happy" if (b % 2.0) > 0.8 else "open"
    p.level = 0.5 + 0.4 * beat_down(ph, 3.0)
    return p


def rewind(b, st):
    """Two beats: the toprock runs backwards at ×8 for a beat (reels back, tape whipping,
    a gold ◀◀); on beat 2 a freeze — a hand on the hip, a fist up, a leg out; it holds and
    the dance restarts."""
    bb = b % 2.0
    if bb < 1.0:
        p = toprock(8.0 - bb * 8.0, st)
        p.shape = -3.0 * bb                         # the reels spin back, fast
        p.sig = 1.0
        p.point = 80 * math.sin(bb * 30)
        p.eyes = "wide"
    else:
        t = bb - 1.0
        hit = ease_out_back(clamp01(t / 0.2), 2.4)
        p = Pose()
        p.hands = _lerp_pair(((-0.52, 0.78), (0.52, 0.78)), ((-0.42, 0.78), (0.70, -1.0)), hit)
        p.feet = ((0.0, 0.0), (0.40 * hit, -0.10 * hit))
        p.hip = (-0.06 * hit, 0.10 * hit)
        p.lean = -12.0 * hit
        p.tilt = -10.0 * hit
        p.eyes = "cool"
        p.shape = -3.0
        p.sig = 0.0
        p.point = -40 * hit
    p.accent = beat_down(b % 1.0, 8)
    return p


ALL_MOVES.update({"cin_toprock": (toprock, 2.0), "cin_shuffle": (shuffle, 4.0), "cin_rewind": (rewind, 2.0)})


def colours(th):
    night = th["name"] == "Night"
    return {"shell": "#2B3239", "shell_l": "#3A434C", "rim": "#76818D", "label": "#EDE6DA",
            "verm": "#E0693B", "lapis": "#4664C0", "window": "#0D1014", "hub": "#F3F5F7",
            "limb": "#59636E", "glove": "#F3F5F7", "glove_s": "#BAC3CC", "cap": "#4664C0",
            "cap_s": "#2E4794", "tape": "#8A5A32", "tape_l": "#C08A52", "shoe": "#F3F5F7",
            "sole": "#2B3239", "gold": th["gold"]}


def _local(origin, deg):
    return lambda x, y: add(origin, rot((x, y), deg))


def draw(pose, th, cut_color=None, trails=None, floor=True, face=True):
    j = solve(pose, PROPS)
    P = lambda p: place(p, j)
    C = colours(th)
    room = cut_color or th["base"]
    d = Doc(cut_color=room, cut=CUT)
    gx = (P(j.feet[0])[0] + P(j.feet[1])[0]) / 2
    if floor:
        d.raw(f'<ellipse cx="{f(gx)}" cy="0.010" rx="0.20" ry="0.028" fill="{th["shadow"]}" opacity="0.5"/>')
    deg = j.torso_deg
    L = _local(P(j.neck), deg)
    # the tape slipping out of the bottom, swirling late (behind the legs)
    tp = []
    whip = 1.0 + 1.5 * pose.sig
    for i in range(26):
        s = i / 25
        a = 2 * math.pi * (1.1 * s) + pose.t * 2.2 + math.radians(pose.point) * 0.8
        x = 0.02 + 0.16 * s * math.sin(a) * whip + 0.05 * s * math.sin(6 * math.pi * s + pose.t * 3)
        y = Hc / 2 + 0.01 + 0.40 * s + 0.04 * s * math.cos(6 * math.pi * s + pose.t * 3) * whip
        tp.append(L(x, y))
    d.line(tp, 0.018, C["tape"], cut=True, smooth=True)
    d.line(tp, 0.005, C["tape_l"], cut=False, smooth=True)
    # track pants (lapis, a vermilion stripe), sneakers
    for i, s in enumerate((-1, 1)):
        hip, knee, foot = P(j.hips[i]), P(j.knees[i]), P(j.feet[i])
        top = add(add(hip, mul(sub(P(j.pelvis), hip), 0.3)), (0, -0.03))
        pts = [top, knee, add(foot, (0, -0.040))]
        d.path(tube_d(pts, [0.084, 0.070, 0.074]), C["lapis"], cut=False)
        d.line([add(p, (s * 0.030, 0)) for p in pts], 0.010, C["verm"], cut=False)
        ang = -s * j.toes[i] * 0.6
        base = add(foot, (s * 0.024, -0.016))
        dd, tr = rrect_d(base[0], base[1] + 0.008, 0.150, 0.024, 0.012, ang)
        d.path(dd, C["sole"], cut=False, transform=tr)
        dd, tr = rrect_d(base[0], base[1] - 0.020, 0.136, 0.056, 0.024, ang)
        d.path(dd, C["shoe"], cut=False, transform=tr)
        d.line([add(base, rot((-0.020 * s, -0.036), ang)), add(base, rot((0.020 * s, -0.026), ang))], 0.010, C["gold"], cut=False)
    pel = P(j.pelvis)
    dd, tr = rrect_d(pel[0], pel[1] - 0.04, 0.17, 0.10, 0.03, deg)
    d.path(dd, C["lapis"], cut=False, transform=tr)
    # the cassette: rim, shell, the label with its stripes, the window with the reels
    c = L(0.0, -0.03)
    for w_, col, cut in ((0.012, C["rim"], True), (0.0, C["shell"], False)):
        dd, tr = rrect_d(c[0], c[1], W + w_, Hc + w_, 0.030, deg)
        d.path(dd, col, cut=cut, transform=tr)
    for sx in (-1, 1):
        for sy in (-1, 1):
            d.circle(L(sx * (W / 2 - 0.022), -0.03 + sy * (Hc / 2 - 0.020)), 0.008, C["rim"], cut=False)
    lab = L(0.0, -0.06)
    dd, tr = rrect_d(lab[0], lab[1], W - 0.060, Hc * 0.62, 0.016, deg)
    d.path(dd, C["label"], cut=False, transform=tr)
    for (y, h, col) in ((-0.135, 0.022, C["verm"]), (-0.108, 0.014, C["lapis"])):
        sc = L(0.0, y)
        dd, tr = rrect_d(sc[0], sc[1], W - 0.060, h, 0.002, deg)
        d.path(dd, col, cut=False, transform=tr)
    win = L(0.0, -0.040)
    dd, tr = rrect_d(win[0], win[1], 0.27, 0.090, 0.040, deg)
    d.path(dd, C["window"], cut=False, transform=tr)
    spin = 360.0 * (0.8 * pose.t + pose.shape)
    if face:
        _reels(d, L, pose, C, spin, deg)
    # the tape opening below: its mouth
    m = 0.4 + 0.6 * pose.mouth if pose.mouth else 0.4
    mouth = [L(-0.070, 0.090), L(0.070, 0.090), L(0.085, 0.090 + 0.035 * m), L(-0.085, 0.090 + 0.035 * m)]
    d.path("M" + " L".join(f"{f(x)},{f(y)}" for x, y in mouth) + " Z", C["window"], cut=False)
    # the backwards cap, its brim behind, a gold button
    cap = [L(-0.120 + 0.13 * math.cos(math.radians(a)) + 0.12, -0.03 - Hc / 2 + 0.075 * math.sin(math.radians(a)))
           for a in range(180, 361, 15)]
    d.cel(curve_d(cap + [L(0.12, -0.03 - Hc / 2 + 0.006), L(-0.12, -0.03 - Hc / 2 + 0.006)], closed=True),
          C["cap"], C["cap_s"], off=(-0.010, -0.010), cut=True)
    brim = L(0.120, -0.03 - Hc / 2 - 0.004)
    dd, tr = rrect_d(brim[0] + 0.035, brim[1] - 0.010, 0.10, 0.022, 0.010, deg - 18)
    d.path(dd, C["cap_s"], cut=False, transform=tr)
    d.circle(L(0.0, -0.03 - Hc / 2 - 0.074), 0.012, C["gold"], cut=False)
    if pose.sig > 0:     # ◀◀ over its head while it rewinds
        for k in (0, 1):
            x0 = -0.02 + k * 0.07
            tri = [L(x0 - 0.045, -0.36), L(x0 + 0.020, -0.40), L(x0 + 0.020, -0.32)]
            d.path("M" + " L".join(f"{f(x)},{f(y)}" for x, y in tri) + " Z", C["gold"], cut=True)
    # arms: thin limbs from the cassette's sides, white gloves
    for i, s in enumerate((-1, 1)):
        sh, el, ha = P(j.shoulders[i]), P(j.elbows[i]), P(j.hands[i])
        d.line([sh, el, ha], 0.040, C["limb"], cut=True)
        d.ellipse(ha, 0.040, 0.036, C["glove_s"], cut=True)
        d.ellipse(add(ha, (-0.004, -0.004)), 0.035, 0.031, C["glove"])
    return d.svg()


def _reels(d, L, pose, C, spin, deg):
    """Two reels — the eyes: white hubs with six teeth, turning; lids of the label."""
    e = pose.eyes
    for s in (-1, 1):
        c = L(s * 0.082, -0.040)
        if e in ("closed", "sleepy", "happy") or (e == "wink" and s > 0):
            y = 0.006 if e != "happy" else -0.004
            pts = ([L(s * 0.082 - 0.030, -0.034), L(s * 0.082, -0.058), L(s * 0.082 + 0.030, -0.034)] if e == "happy" else
                   [L(s * 0.082 - 0.030, -0.040), L(s * 0.082, -0.028), L(s * 0.082 + 0.030, -0.040)])
            d.line(pts, 0.012, C["hub"], cut=False, smooth=True)
            continue
        r = 0.036 * (1.15 if e == "wide" else 1.0)
        d.circle(c, r, C["hub"], cut=False)
        for k in range(6):
            a = math.radians(spin + k * 60)
            tip = add(c, (r * 0.62 * math.cos(a), r * 0.62 * math.sin(a)))
            d.line([add(c, (r * 0.25 * math.cos(a), r * 0.25 * math.sin(a))), tip], 0.008, C["window"], cut=False)
        d.circle(c, r * 0.22, C["window"], cut=False)
        if e == "cool":   # a lid of the label across the top: unimpressed
            lid = L(s * 0.082, -0.040 - r * 0.55)
            dd, tr = rrect_d(lid[0], lid[1], r * 2.3, r * 1.0, 0.004, deg)
            d.path(dd, C["label"], cut=False, transform=tr)


def key_poses():
    def at(fn, b):
        p = fn(b, STYLE)
        p.t = b
        return p
    return [("Idle", Pose(eyes="open", hands=((-0.30, 0.50), (0.30, 0.50)), t=0.2)), ("Toprock", at(toprock, 0.0)),
            ("Cross", at(toprock, 0.5)), ("Cross back", at(toprock, 1.5)), ("Shuffle", at(shuffle, 1.0)),
            ("Rewind", at(rewind, 0.3)), ("◀◀", at(rewind, 0.7)), ("Freeze", at(rewind, 1.3)),
            ("Cool", Pose(eyes="cool", lean=-6, hands=((-0.42, 0.78), (0.80, 0.0)), feet=((0, 0), (0.10, 0))))]


def mood_items():
    return [("Neutral", Pose(eyes="open")), ("Happy", Pose(eyes="happy")), ("Excited", Pose(eyes="wide", mouth=1.0)),
            ("Calm", Pose(eyes="closed")), ("Surprised", Pose(eyes="wide", mouth=0.6)), ("Sleepy", Pose(eyes="sleepy", tilt=12)),
            ("Cool", Pose(eyes="cool")), ("Rewinding", Pose(eyes="wide", sig=1.0, shape=-0.5))]


MOOD_FOCUS = -0.62
MOOD_SIZE = 260
