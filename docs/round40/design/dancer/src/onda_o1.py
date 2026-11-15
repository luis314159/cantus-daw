"""Onda O1 — **Traza**, the oscilloscope: a figure drawn by a phosphor trace.

No body at all: the figure is a scope's trace. Every limb is a line rippling with the
waveform (its amplitude the level). The head is a **Lissajous figure** — a loop that turns
in depth as the beat goes by — with two bright dwell points for eyes. Amber phosphor (the
cast's gold), a halo at night, a sharp dark-amber line on paper. A bright **beam dot** runs
along the trace, drawing it, and a faint **afterglow** of where it was a moment ago stays
behind (persistence).

It dances loose and light. Signature, **Square**: on the drop the ripples snap into a square
wave and its poses go angular (pop and lock); then a saw, and it melts back into a sine.
"""
import math
from body import Props, solve, place
from moves import Style, MOVES as ALL_MOVES, steps, groove, pump
from rig import (Pose, add, sub, mul, rot, length, lerp, beat_down, smooth, ease_out_back, clamp01)
from svg import Doc, f

NAME = "Onda"
PROPS = Props(leg=0.47, thigh=0.245, shin=0.235, torso=0.28, neck=0.04, head=0.085,
              shoulder=0.085, hipw=0.045, stance=0.065, upper=0.155, fore=0.150, foot=0.10)
STYLE = Style(bounce=0.07, sway=0.07, step=0.17, lag=0.10, hop=0.14, squash=0.05, snap=0.0,
              open=(0.62, 0.88))
SQ_STYLE = Style(bounce=0.06, sway=0.05, snap=1.0)
MOVES = {"groove": "o1_groove", "steps": "o1_step", "jump": "o1_square"}
TRAILS, TRAIL_STEP, TRAIL_N = True, 0.06, 1


def o1_groove(b, st):
    p = groove(b, st)
    p.level = 0.4 + 0.5 * beat_down(b % 1.0, 2.5)
    p.eyes = "open"
    return p


def o1_step(b, st):
    p = steps(b, st)
    p.level = 0.4 + 0.4 * beat_down(b % 1.0, 2.5)
    p.eyes = "open"
    return p


def o1_square(b, st):
    """Two beats: the ripples snap square and the poses go angular (pop and lock); half way
    a saw; it melts back to a sine at the end."""
    bb = b % 2.0
    p = pump(b, SQ_STYLE)
    p.shape = ease_out_back(clamp01(bb / 0.15), 1.6) * (1 - smooth((bb - 1.6) / 0.4))
    p.sig = 1.0 if 1.0 <= bb < 1.6 else 0.0           # the saw
    p.level = 0.95
    p.eyes = "wide" if bb < 0.3 else "open"
    return p


ALL_MOVES.update({"o1_groove": (o1_groove, 2.0), "o1_step": (o1_step, 4.0), "o1_square": (o1_square, 2.0)})


def colours(th):
    night = th["name"] == "Night"
    return {"line": "#F2C766" if night else "#9C711C", "core": "#FFF4D6", "halo": "#E2B85A",
            "glow": "#E2B85A", "dot": "#FFF4D6" if night else "#5E4210", "night": night}


def _wave(x, shape, saw):
    if saw:
        return 2.0 * ((x + 0.25) % 1.0) - 1.0
    s = math.sin(2 * math.pi * x)
    return math.copysign(abs(s) ** max(0.06, 1.0 - shape * 0.96), s)


def trace(points, pose, wl=0.075, phase=0.0):
    """A polyline rippled along its length by the waveform (amplitude from the level)."""
    out = []
    amp = (0.006 + 0.016 * pose.level) * (1.0 + 0.6 * pose.shape)
    run = 0.0
    for n in range(len(points) - 1):
        a, b = points[n], points[n + 1]
        L = length(sub(b, a)) or 1e-6
        u = mul(sub(b, a), 1 / L)
        nrm = (-u[1], u[0])
        steps_ = max(4, int(L / 0.008))
        for k in range(steps_ + (1 if n == len(points) - 2 else 0)):
            s = k / steps_
            pos = add(a, mul(u, L * s))
            w = _wave((run + L * s) / wl - pose.t * 2.0 + phase, pose.shape, pose.sig > 0.5)
            fade = min(1.0, (run + L * s) / 0.03, (sum(length(sub(points[i + 1], points[i])) for i in range(len(points) - 1)) - run - L * s) / 0.03)
            out.append(add(pos, mul(nrm, amp * w * max(0.0, fade))))
        run += L
    return out


def _d(pts):
    return "M" + " L".join(f"{f(x)},{f(y)}" for x, y in pts)


def glow(d, pts, C, w=0.010, opacity=1.0):
    dd = _d(pts)
    if C["night"]:
        d.raw(f'<path d="{dd}" fill="none" stroke="{C["halo"]}" stroke-width="{f(w * 3.4)}" stroke-linecap="round" '
              f'stroke-linejoin="round" opacity="{f(0.22 * opacity)}"/>')
    d.raw(f'<path d="{dd}" fill="none" stroke="{C["line"]}" stroke-width="{f(w if C["night"] else w * 1.2)}" '
          f'stroke-linecap="round" stroke-linejoin="round" opacity="{f(opacity)}"/>')
    if C["night"]:
        d.raw(f'<path d="{dd}" fill="none" stroke="{C["core"]}" stroke-width="{f(w * 0.35)}" stroke-linecap="round" '
              f'stroke-linejoin="round" opacity="{f(opacity)}"/>')


def _chains(j, P):
    hips = [P(h) for h in j.hips]
    return [[hips[0], P(j.knees[0]), P(j.feet[0])], [hips[1], P(j.knees[1]), P(j.feet[1])],
            [P(j.pelvis), P(j.neck)], [P(j.shoulders[0]), P(j.neck), P(j.shoulders[1])],
            [P(j.shoulders[0]), P(j.elbows[0]), P(j.hands[0])], [P(j.shoulders[1]), P(j.elbows[1]), P(j.hands[1])],
            [hips[0], P(j.pelvis), hips[1]]]


def draw(pose, th, cut_color=None, trails=None, floor=True, face=True):
    j = solve(pose, PROPS)
    P = lambda p: place(p, j)
    C = colours(th)
    d = Doc()
    gx = (P(j.feet[0])[0] + P(j.feet[1])[0]) / 2
    if floor:
        d.raw(f'<ellipse cx="{f(gx)}" cy="0.006" rx="0.15" ry="0.020" fill="{th["shadow"]}" opacity="0.45"/>')
        if C["night"]:
            d.raw(f'<ellipse cx="{f(gx)}" cy="0.004" rx="0.12" ry="0.012" fill="{C["glow"]}" opacity="0.10"/>')
    # persistence: a faint afterglow of where the trace was a moment ago (night only)
    if trails and C["night"]:
        t0 = trails[0]
        Pt = lambda p: place(p, t0)
        for ch in _chains(t0, Pt):
            d.raw(f'<path d="{_d(ch)}" fill="none" stroke="{C["line"]}" stroke-width="0.010" '
                  f'stroke-linecap="round" stroke-linejoin="round" opacity="0.16"/>')
    allpts = []
    for k, ch in enumerate(_chains(j, P)):
        tr = trace(ch, pose, phase=k * 0.37)
        glow(d, tr, C, 0.010)
        allpts += tr
    # the head: a Lissajous loop turning in depth
    hc = P(j.head)
    r = 0.070
    dl = pose.t * math.pi * 0.5
    loop = [add(hc, (r * math.sin(3 * a + dl), r * 1.08 * math.sin(2 * a))) for a in [i * 2 * math.pi / 140 for i in range(141)]]
    glow(d, loop, C, 0.008)
    if face:
        _face(d, hc, pose, C)
    # the beam dot, drawing the trace
    if allpts:
        bp = allpts[int((pose.t * 0.8 % 1.0) * (len(allpts) - 1))]
        if C["night"]:
            d.raw(f'<circle cx="{f(bp[0])}" cy="{f(bp[1])}" r="0.022" fill="{C["halo"]}" opacity="0.35"/>')
        d.circle(bp, 0.010, C["dot"], cut=False)
    return d.svg()


def _face(d, hc, pose, C):
    e = pose.eyes
    for s in (-1, 1):
        c = add(hc, (s * 0.024, -0.006))
        if e == "happy":
            glow(d, [add(c, (-0.010, 0.006)), add(c, (0, -0.004)), add(c, (0.010, 0.006))], C, 0.006)
        elif e in ("closed", "sleepy"):
            glow(d, [add(c, (-0.010, 0.002)), add(c, (0.010, 0.002))], C, 0.005, opacity=0.6 if e == "sleepy" else 1.0)
        elif e == "cool":
            if s < 0:
                glow(d, [add(hc, (-0.040, -0.006)), add(hc, (0.040, -0.006))], C, 0.008)
        else:
            if C["night"]:
                d.raw(f'<circle cx="{f(c[0])}" cy="{f(c[1])}" r="{0.016 if e == "wide" else 0.012}" fill="{C["halo"]}" opacity="0.35"/>')
            d.circle(c, 0.008 if e == "wide" else 0.006, C["dot"], cut=False)


def key_poses():
    def at(fn, b):
        p = fn(b, STYLE)
        p.t = b
        return p
    return [("Idle", Pose(eyes="open", level=0.4, t=0.3, hip=(0.03, 0.02), tilt=6)), ("Groove", at(o1_groove, 0.0)),
            ("Clap", at(o1_groove, 1.0)), ("Step", at(o1_step, 1.0)), ("Square!", at(o1_square, 0.1)),
            ("Pop", at(o1_square, 0.5)), ("Lock", at(o1_square, 0.9)), ("Saw", at(o1_square, 1.2)),
            ("Sine again", at(o1_square, 1.9))]


def mood_items():
    return [("Neutral", Pose(eyes="open", level=0.4, t=0.0)), ("Happy", Pose(eyes="happy", level=0.6, t=0.2)),
            ("Excited", Pose(eyes="wide", level=1.0, t=0.4)), ("Calm", Pose(eyes="closed", level=0.2, t=0.6)),
            ("Surprised", Pose(eyes="wide", level=0.5, shape=1.0, t=0.8)), ("Sleepy", Pose(eyes="sleepy", level=0.05, t=1.0)),
            ("Cool", Pose(eyes="cool", level=0.5, t=1.2)), ("Square", Pose(eyes="open", level=0.8, shape=1.0, t=1.4))]


MOOD_SIZE = 420
