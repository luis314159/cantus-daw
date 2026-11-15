"""New — **Vinilo**, the DJ: a turntable that plays itself.

Its head is a record: black vinyl with grooves (value rings) and a vermilion label that
carries its face. The gold spindle is its nose; a light sheen and a gold marker turn with
the spin, the face stays upright. It wears DJ headphones (one cup on, one pushed back).
Its body is a graphite plinth with a lapis light strip. Its right arm is the **tonearm** — a
gold pivot at the shoulder, a pale rod, a headshell with a gold stylus for a hand; its left
is a sleeve with a white glove that scratches the record's edge. Slim trousers, white
sneakers. No brand's look: no plinth layout, logo or label design of any maker.

It grooves by **scratching**: the record rocks back and forth on the sixteenths under its
glove while the tonearm pumps on the beat. Signature, **Backspin**: it brakes the record,
spins it back in a blur, and lands on the drop with the record running forward, arms up.
"""
import math
from body import Props, solve, place
from moves import Style, MOVES as ALL_MOVES, steps, _hands, _lerp_pair
from rig import (Pose, add, sub, mul, rot, length, lerp, beat_down, cos1, sin1, smooth,
                 ease_in, ease_out, ease_out_back, clamp01)
from svg import Doc, tube_d, rrect_d, curve_d, f

NAME = "Vinilo"
PROPS = Props(leg=0.36, thigh=0.186, shin=0.180, torso=0.24, neck=0.02, head=0.19,
              shoulder=0.120, hipw=0.050, stance=0.070, upper=0.150, fore=0.150, foot=0.11)
STYLE = Style(bounce=0.08, sway=0.06, step=0.20, lag=0.10, hop=0.12, squash=0.05, snap=0.4)
MOVES = {"groove": "vin_scratch", "steps": "vin_step", "jump": "vin_backspin"}
CUT = 0.012
RPM = 0.55        # turns a beat (33⅓ at 125 BPM is about 0.27; ours is livelier)


def lp(a, b, t):
    return (lerp(a[0], b[0], t), lerp(a[1], b[1], t))


GRIP = (-0.62, -0.80)      # the glove on the record's left edge (neck-relative, A units)


def scratch(b, st):
    """Two beats: the glove on the record's edge rocks it back and forth on the 16ths
    (the record follows), the tonearm pumps up on the beat, the head nods."""
    p = Pose()
    ph = b % 1.0
    rock = math.sin(2 * math.pi * 2 * b)            # two scratches a beat
    p.shape = 0.10 * rock - RPM * b                 # cancel the turn, rock it: a scratch
    p.hands = ((GRIP[0], GRIP[1] + 0.16 * rock), lp((0.95, 0.10), (0.85, -0.55), beat_down(ph, 1.6)))
    p.hip = (0.03 * cos1(b / 2), st.bounce * beat_down(ph, 2.0))
    p.lean = 4.0 * cos1(b / 2)
    p.nod = 10.0 * beat_down((b - st.lag) % 1.0) - 3.0
    p.tilt = 6.0 * cos1(b / 2)
    p.eyes = "cool" if ph < 0.15 else "open"
    p.accent = beat_down(ph, 8)
    p.level = 0.5 + 0.45 * beat_down(ph, 3.0)
    return p


def step(b, st):
    p = steps(b, st)
    ph = b % 1.0
    p.hands = (lp((-0.55, 0.92), GRIP, beat_down(ph, 1.2)), lp((0.95, 0.20), (0.90, -0.45), beat_down(ph, 2.0)))
    p.eyes = "open"
    p.level = 0.5 + 0.4 * beat_down(ph, 3.0)
    return p


def backspin(b, st):
    """Two beats: the glove brakes the record (it stops), then spins it back three turns
    in a blur, leaning back; on beat 2 it lets go — the record runs forward, arms up."""
    p = Pose()
    bb = b % 2.0
    if bb < 1.0:
        brake = smooth(bb / 0.25)
        back = ease_in(clamp01((bb - 0.3) / 0.7))
        p.shape = -RPM * b * brake - 3.0 * back
        p.hands = (lp((-0.55, 0.92), GRIP, brake), (0.95, 0.15))
        p.lean = -10.0 * back
        p.hip = (0.03 * back, 0.05 * back)
        p.eyes = "wide" if bb > 0.3 else "open"
        p.sig = back                                # the blur
    else:
        t = bb - 1.0
        up = ease_out_back(clamp01(t / 0.25), 2.0)
        p.shape = -RPM * 1.0 - 3.0 + 1.2 * t        # forward again, fast
        p.hands = (lp(GRIP, (-0.90, -1.05), up), lp((0.95, 0.15), (0.80, -1.10), up))
        p.hip = (0.0, 0.12 * up * (1 - smooth((t - 0.5) / 0.5)))
        p.lean = 6.0 * up * (1 - t)
        p.eyes = "cool"
        p.sig = max(0.0, 0.6 - t * 2)
    p.accent = beat_down(b % 1.0, 8)
    p.level = 1.0 if bb >= 1.0 else 0.4
    return p


ALL_MOVES.update({"vin_scratch": (scratch, 2.0), "vin_step": (step, 4.0), "vin_backspin": (backspin, 2.0)})


def colours(th):
    night = th["name"] == "Night"
    return {"vinyl": "#0D1014", "groove": "#262D34", "sheen": "#59636E" if night else "#47515B",
            "rim": "#76818D", "label": "#E0693B", "label_s": "#B24E29", "face": "#14191E",
            "plinth": "#2B3239", "plinth_l": "#3A434C", "led": "#8FAAF5" if night else "#5B79D6",
            "arm": "#B4BDC6", "glove": "#F3F5F7", "glove_s": "#BAC3CC", "pants": "#2B3239",
            "shoe": "#F3F5F7", "gold": th["gold"]}


def _local(origin, deg):
    return lambda x, y: add(origin, rot((x, y), deg))


def _arc(c, r, a0, a1, n=8):
    return [add(c, (r * math.cos(math.radians(a0 + (a1 - a0) * i / n)), r * math.sin(math.radians(a0 + (a1 - a0) * i / n))))
            for i in range(n + 1)]


def draw(pose, th, cut_color=None, trails=None, floor=True, face=True):
    j = solve(pose, PROPS)
    P = lambda p: place(p, j)
    C = colours(th)
    room = cut_color or th["base"]
    d = Doc(cut_color=room, cut=CUT)
    gx = (P(j.feet[0])[0] + P(j.feet[1])[0]) / 2
    if floor:
        d.raw(f'<ellipse cx="{f(gx)}" cy="0.010" rx="0.20" ry="0.028" fill="{th["shadow"]}" opacity="0.5"/>')
    # legs, sneakers
    for i, s in enumerate((-1, 1)):
        hip, knee, foot = P(j.hips[i]), P(j.knees[i]), P(j.feet[i])
        d.path(tube_d([hip, knee, add(foot, (0, -0.04))], [0.072, 0.062, 0.058]), "#59636E", cut=False)
        ang = -s * j.toes[i] * 0.6
        base = add(foot, (s * 0.020, -0.016))
        dd, tr = rrect_d(base[0], base[1] + 0.006, 0.130, 0.022, 0.010, ang)
        d.path(dd, C["gold"], cut=False, transform=tr)
        dd, tr = rrect_d(base[0], base[1] - 0.018, 0.120, 0.046, 0.020, ang)
        d.path(dd, C["shoe"], cut=False, transform=tr)
    # the plinth: rim, body, a lapis light strip, two little knobs
    top, bot = P(j.neck), add(P(j.pelvis), (0, 0.04))
    mid = mul(add(top, bot), 0.5)
    h = length(sub(top, bot)) + 0.02
    deg = j.torso_deg
    for w_, col, cut in ((0.010, C["rim"], True), (0.0, C["plinth"], False)):
        dd, tr = rrect_d(mid[0], mid[1], 0.26 + w_, h + w_, 0.045, deg)
        d.path(dd, col, cut=cut, transform=tr)
    L = _local(mid, deg)
    led = 0.4 + 0.6 * pose.level
    d.line([L(-0.095, h / 2 - 0.040), L(-0.095 + 0.19 * led, h / 2 - 0.040)], 0.012, C["led"], cut=False)
    # DJ bling: a gold chain with a tiny record for a pendant, swinging a little late
    sw = 0.02 * math.sin(math.radians(pose.lean * 3 + pose.tilt * 2))
    chain = [L(-0.085, -h / 2 + 0.010), L(-0.040 + sw, -h / 2 + 0.080), L(0.0 + sw * 1.4, -h / 2 + 0.098),
             L(0.040 + sw, -h / 2 + 0.080), L(0.085, -h / 2 + 0.010)]
    d.line(chain, 0.010, C["gold"], cut=False, smooth=True)
    pend = L(0.0 + sw * 1.6, -h / 2 + 0.122)
    d.circle(pend, 0.024, C["gold"], cut=False)
    d.circle(pend, 0.016, C["vinyl"], cut=False)
    d.circle(pend, 0.006, C["label"], cut=False)
    # the record: its rim, the grooves, a sheen and a marker that turn, the label's face
    hc = P(j.head)
    R = PROPS.head
    turn = 360.0 * (RPM * pose.t + pose.shape)
    d.circle(hc, R + 0.007, C["rim"])
    d.circle(hc, R, C["vinyl"], cut=False)
    for r in (0.172, 0.152, 0.132, 0.114):
        d.raw(f'<circle cx="{f(hc[0])}" cy="{f(hc[1])}" r="{r}" fill="none" stroke="{C["groove"]}" stroke-width="0.004"/>')
    blur = 1 + int(3 * pose.sig)
    for k in range(blur):
        for base in (turn, turn + 180):
            a = base - k * 22
            d.line(_arc(hc, 0.150, a - 18, a + 18), 0.022 - 0.004 * k, C["sheen"], cut=False, smooth=True)
    m = add(hc, (0.176 * math.cos(math.radians(turn)), 0.176 * math.sin(math.radians(turn))))
    d.circle(m, 0.010, C["gold"], cut=False)
    H = _local(hc, j.head_deg)
    lr = 0.098
    d.circle(hc, lr, C["label_s"], cut=False)
    d.circle(add(hc, (-0.005, -0.005)), lr - 0.006, C["label"], cut=False)
    if face:
        _face(d, H, pose, C)
    d.circle(H(0.0, 0.010), 0.011, C["gold"], cut=False)
    # DJ headphones: the band over the record, one cup on, one pushed back
    band = _arc(hc, R + 0.022, 200, 330, 10)
    d.line(band, 0.016, "#38444F", cut=True, smooth=True)
    dd, tr = rrect_d(*H(-R - 0.010, 0.010), 0.042, 0.080, 0.016, j.head_deg)
    d.path(dd, C["gold"], transform=tr)
    dd, tr = rrect_d(*H(R - 0.010, -0.150), 0.040, 0.070, 0.016, j.head_deg + 50)
    d.path(dd, C["gold"], transform=tr)
    # the left arm: a sleeve and a white glove
    sh, el, ha = P(j.shoulders[0]), P(j.elbows[0]), P(j.hands[0])
    d.path(tube_d([sh, el, ha], [0.064, 0.056, 0.050]), C["plinth"], cut=True)
    d.ellipse(ha, 0.040, 0.036, C["glove_s"], cut=True)
    d.ellipse(add(ha, (-0.004, -0.004)), 0.035, 0.031, C["glove"])
    # the right arm: the tonearm — pivot, counterweight, rod, headshell, stylus
    sh, el, ha = P(j.shoulders[1]), P(j.elbows[1]), P(j.hands[1])
    up = sub(el, sh)
    ln = length(up) or 1e-6
    cw = sub(sh, mul(up, 0.06 / ln))
    d.line([sh, cw], 0.050, "#38444F", cut=True, cap="butt")
    d.line([sh, el, ha], 0.020, C["arm"], cut=True)
    d.circle(sh, 0.034, C["gold"], cut=False)
    d.circle(sh, 0.014, C["plinth"], cut=False)
    fd = sub(ha, el)
    fl = length(fd) or 1e-6
    ang = math.degrees(math.atan2(fd[1], fd[0]))
    head = add(ha, mul(fd, 0.020 / fl))
    dd, tr = rrect_d(head[0], head[1], 0.060, 0.034, 0.008, ang)
    d.path(dd, "#38444F", transform=tr)
    d.circle(add(head, rot((0.022, 0.016), ang)), 0.009, C["gold"], cut=False)
    return d.svg()


def _face(d, H, pose, C):
    e = pose.eyes
    k = C["face"]
    for s in (-1, 1):
        c = H(s * 0.036, -0.026)
        if e == "happy":
            d.line([H(s * 0.036 - 0.016, -0.020), H(s * 0.036, -0.034), H(s * 0.036 + 0.016, -0.020)], 0.008, k, cut=False)
        elif e in ("closed", "sleepy"):
            d.line([H(s * 0.036 - 0.016, -0.026), H(s * 0.036 + 0.016, -0.024)], 0.008, k, cut=False)
        elif e == "cool":
            dd, tr = rrect_d(*c, 0.044, 0.022, 0.008, 0)
            d.path(dd, k, cut=False, transform=tr)
        else:
            big = 1.3 if e == "wide" else 1.0
            d.ellipse(c, 0.012 * big, 0.016 * big, k)
        d.line([H(s * 0.022, -0.052), H(s * 0.050, -0.056 + (0.006 if e == "cool" else 0))], 0.007, k, cut=False)
    if e == "wide":
        d.ellipse(H(0.0, 0.050), 0.014, 0.014, k)
    elif e == "happy":
        d.line([H(-0.030, 0.040), H(0.0, 0.058), H(0.030, 0.040)], 0.008, k, cut=False, smooth=True)
    else:          # a DJ's half smile
        d.line([H(-0.026, 0.050), H(0.004, 0.054), H(0.030, 0.042)], 0.008, k, cut=False, smooth=True)


def key_poses():
    def at(fn, b):
        p = fn(b, STYLE)
        p.t = b
        return p
    return [("Idle", Pose(eyes="open", hands=((-0.55, 0.92), (0.95, 0.15)))), ("Scratch", at(scratch, 0.0)),
            ("Scratch back", at(scratch, 0.25)), ("Tonearm up", at(scratch, 1.0)), ("Step", at(step, 1.0)),
            ("Brake", at(backspin, 0.25)), ("Backspin", at(backspin, 0.8)), ("The drop", at(backspin, 1.2)),
            ("Cool", Pose(eyes="cool", lean=-5, hands=(GRIP, (0.95, 0.10))))]


def mood_items():
    return [("Neutral", Pose(eyes="open")), ("Happy", Pose(eyes="happy")), ("Excited", Pose(eyes="wide", sig=0.6)),
            ("Calm", Pose(eyes="closed")), ("Surprised", Pose(eyes="wide")), ("Sleepy", Pose(eyes="sleepy", tilt=12)),
            ("Cool", Pose(eyes="cool")), ("Spinning", Pose(eyes="happy", sig=1.0, t=0.4))]


MOOD_SIZE = 300
