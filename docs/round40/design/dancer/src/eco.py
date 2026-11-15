"""New — **Eco**, the delay's ghost: a small floating ghost and its echoes.

A small ghost: a dome, a scalloped hem that ripples, two arm nubs, little gold headphones,
the family's cut eyes (negative space, as Compás and Onda). It wears the theme's ink — pale
at night, graphite on paper.

Behind it, **two or three echoes** repeat each move a quarter beat late (Delay's taps). Each
is the same pose function at an earlier time, one value further into the room's colour:
opaque, so the echoes cost fills, not transparency.

It floats and grooves lightly. Signature, **Feedback**: the echoes multiply in a widening
spiral until they fill the frame, then all go out at once on the beat; the ghost pops.
"""
import math
from body import Props
from moves import Style, MOVES as ALL_MOVES, steps
from rig import (Pose, add, sub, mul, rot, length, lerp, beat_down, cos1, sin1, smooth,
                 ease_in, ease_out, ease_out_back, clamp01)
from svg import Doc, curve_d, rrect_d, f
from theme import mix

NAME = "Eco"
PROPS = Props()
STYLE = Style(bounce=0.04, sway=0.10, step=0.22, lag=0.10, hop=0.10)
MOVES = {"groove": "eco_float", "steps": "eco_drift", "jump": "eco_feedback"}
ECHO, ECHO_STEP, ECHO_N = True, 0.25, 3
CUT = 0.012


def float_(b, st):
    p = Pose()
    ph = b % 1.0
    p.lift = 0.03 * math.sin(math.pi * 2 * (b / 2.0)) + 0.02 * beat_down(ph, 2.0)
    p.lean = 9.0 * cos1(b / 2.0)
    p.hip = (0.04 * cos1(b / 2.0), 0.0)
    k = int(math.floor(b)) % 2
    up = beat_down(ph, 1.5)
    p.hands = ((-1.0, 0.2 - 1.0 * up if k == 0 else 0.3), (1.0, 0.2 - 1.0 * up if k == 1 else 0.3))
    p.squash = 0.06 * beat_down(ph, 4.0)
    p.eyes = "happy" if ph < 0.2 else "open"
    p.mouth = 0.6 * beat_down(ph, 3.0)
    p.accent = beat_down(ph, 8)
    return p


def drift(b, st):
    q = steps(b, st)
    p = float_(b, st)
    p.hip = (q.hip[0] * 1.6, 0.0)
    p.lean = -q.lean * 1.6
    p.hands = ((-1.0, 0.6), (1.0, 0.6)) if q.lean > 0 else ((-1.0, 0.6), (1.0, 0.6))
    return p


def feedback(b, st):
    """Two beats: the echoes multiply (sig 0 → 1) in a widening spiral while the ghost
    spins its arms; on beat 2 every echo goes out at once and the ghost pops."""
    p = Pose()
    bb = b % 2.0
    if bb < 1.0:
        p.sig = ease_in(bb)
        p.lift = 0.04 + 0.04 * p.sig
        p.lean = 12 * math.sin(bb * math.pi * 4)
        a = bb * math.pi * 6
        p.hands = ((-math.cos(a), math.sin(a)), (math.cos(a), -math.sin(a)))
        p.eyes = "happy" if bb < 0.6 else "wide"
        p.mouth = 0.3 + 0.5 * p.sig
    else:
        t = bb - 1.0
        p.sig = 0.0
        p.squash = -0.18 * (1 - ease_out(clamp01(t / 0.2))) + 0.08 * math.sin(clamp01(t / 0.6) * math.pi) * (1 - t)
        p.lift = 0.10 * (1 - ease_out(clamp01(t / 0.5)))
        p.hands = ((-1.0, -1.0), (1.0, -1.0)) if t < 0.5 else ((-1.0, 0.3), (1.0, 0.3))
        p.eyes = "wide" if t < 0.4 else "happy"
        p.mouth = 0.9 if t < 0.4 else 0.2
        p.landed = max(0.0, 1 - t * 2.5)       # the flash ring
    p.accent = beat_down(b % 1.0, 8)
    return p


ALL_MOVES.update({"eco_float": (float_, 2.0), "eco_drift": (drift, 4.0), "eco_feedback": (feedback, 2.0)})


def colours(th):
    night = th["name"] == "Night"
    return {"body": "#DCE3EA" if night else "#38444F", "shade": "#AEB8C2" if night else "#1F2730",
            "gold": th["gold"]}


def _ghost(d, pose, C, th, room, at, tint=0.0, face=True, scale=1.0):
    """One ghost at `at` (its centre), `tint` toward the room (0 itself … 1 gone)."""
    body = mix(C["body"], room, tint)
    shade = mix(C["shade"], room, tint)
    sq = pose.squash
    sx, sy = (1 + 0.6 * sq) * scale, (1 - sq) * scale
    T = lambda x, y: add(at, rot((x * sx, y * sy), pose.lean))
    R = 0.17
    pts = [T(R * math.cos(math.radians(a)), -0.05 + R * math.sin(math.radians(a))) for a in range(180, 361, 15)]
    pts += [T(R, 0.10), T(R * 0.98, 0.15)]
    for n in range(7):                         # the scalloped hem, rippling
        x = R * (1 - 2 * (n + 0.5) / 7)
        y = 0.16 + 0.03 * math.sin(n * 2.2 + pose.t * 6.0) + (0.025 if n % 2 else -0.005)
        pts.append(T(x, y))
    pts += [T(-R * 0.98, 0.15), T(-R, 0.10)]
    dd = curve_d(pts, closed=True)
    if tint == 0.0:
        d.raw(f'<path d="{dd}" fill="{room}" stroke="{room}" stroke-width="{f(2 * CUT)}" stroke-linejoin="round"/>')
    d.cel(dd, body, shade, off=(-0.016, -0.012), cut=False)
    # arm nubs toward the hands
    for i, s in enumerate((-1, 1)):
        hx, hy = pose.hands[i]
        ang = math.atan2(hy, hx * s)
        base = T(s * 0.150, 0.03)
        tip = add(base, rot((s * 0.10 * math.cos(ang) * scale, 0.10 * math.sin(ang) * scale), pose.lean))
        d.line([base, tip], 0.050 * scale, body, cut=False)
    # little gold headphones
    gold = mix(th["gold"], room, tint)
    band = [T(0.15 * math.cos(math.radians(a)), -0.05 + 0.15 * math.sin(math.radians(a)) - 0.02) for a in range(205, 336, 15)]
    d.line(band, 0.016 * scale, gold, cut=False, smooth=True)
    for s in (-1, 1):
        cx, cy = T(s * 0.150, -0.075)
        dd2, tr = rrect_d(cx, cy, 0.034 * scale, 0.056 * scale, 0.014 * scale, pose.lean)
        d.path(dd2, gold, cut=False, transform=tr)
    if face:
        _face(d, T, pose, room if tint == 0.0 else mix(room, body, 0.4))


def _face(d, T, pose, cutc):
    e = pose.eyes
    for s in (-1, 1):
        c = T(s * 0.056, -0.050)
        if e == "happy":
            d.line([T(s * 0.056 - 0.024, -0.040), T(s * 0.056, -0.064), T(s * 0.056 + 0.024, -0.040)], 0.016, cutc, cut=False)
        elif e in ("closed", "sleepy"):
            d.line([T(s * 0.056 - 0.024, -0.050), T(s * 0.056, -0.040), T(s * 0.056 + 0.024, -0.050)], 0.014, cutc, cut=False)
        elif e == "cool":
            if s < 0:
                d.line([T(-0.090, -0.052), T(0.090, -0.052)], 0.034, cutc, cut=False, cap="butt")
        else:
            big = 1.25 if e == "wide" else 1.0
            d.ellipse(c, 0.024 * big, 0.034 * big, cutc)
    if pose.mouth > 0.2:
        d.ellipse(T(0.0, 0.020), 0.020, 0.010 + 0.022 * pose.mouth, cutc)
    else:
        d.line([T(-0.022, 0.012), T(0.0, 0.024), T(0.022, 0.012)], 0.012, cutc, cut=False, smooth=True)


def draw(pose, th, cut_color=None, trails=None, floor=True, face=True):
    room = cut_color or th["base"]
    C = colours(th)
    d = Doc()
    x = pose.hip[0] * 0.47
    y = -0.40 - 0.10 - pose.lift
    if floor:
        k = max(0.25, 1.0 - (pose.lift + 0.1) * 2.0)
        d.raw(f'<ellipse cx="{f(x)}" cy="0.006" rx="{f(0.13 * k)}" ry="{f(0.020 * k)}" fill="{th["shadow"]}" opacity="0.45"/>')
        if pose.landed > 0.05:
            r = 0.20 + 0.45 * (1 - pose.landed)
            d.raw(f'<ellipse cx="{f(x)}" cy="{f(y)}" rx="{f(r)}" ry="{f(r)}" fill="none" stroke="{th["gold"]}" '
                  f'stroke-width="{f(0.012 * pose.landed)}" opacity="{f(pose.landed)}"/>')
    # Feedback: the echoes multiplying in a widening spiral (behind everything)
    if pose.sig > 0:
        n = int(3 + 10 * pose.sig)
        for k in range(n, 0, -1):
            a = math.radians(k * 47 + pose.t * 90)
            r = 0.10 + 0.045 * k * (0.5 + pose.sig)
            at = (x + r * math.cos(a), y + r * math.sin(a) * 0.8)
            _ghost(d, pose, C, th, room, at, tint=min(0.85, 0.30 + 0.05 * k), face=False, scale=0.85)
    # the echoes: the same dance a quarter beat late each, trailing behind
    echoes = trails if trails else [pose, pose, pose]
    for k in range(len(echoes), 0, -1):
        e = echoes[k - 1]
        ex = e.hip[0] * 0.47 - 0.085 * k
        ey = -0.40 - 0.10 - e.lift + 0.012 * k
        _ghost(d, e, C, th, room, (ex, ey), tint=0.28 + 0.20 * (k - 1), face=True, scale=1.0 - 0.06 * k)
    _ghost(d, pose, C, th, room, (x, y), tint=0.0, face=face)
    return d.svg()


def key_poses():
    def at(fn, b):
        p = fn(b, STYLE)
        p.t = b
        return p
    return [("Idle", Pose(eyes="open", lift=0.02)), ("Float", at(float_, 0.0)), ("Wave", at(float_, 1.0)),
            ("Drift", at(drift, 0.5)), ("Drift back", at(drift, 2.5)), ("Feedback", at(feedback, 0.5)),
            ("Full", at(feedback, 0.95)), ("Out!", at(feedback, 1.05)), ("Again", at(feedback, 1.6))]


def mood_items():
    return [("Neutral", Pose(eyes="open")), ("Happy", Pose(eyes="happy")), ("Excited", Pose(eyes="wide", mouth=0.8)),
            ("Calm", Pose(eyes="closed")), ("Surprised", Pose(eyes="wide", mouth=0.5)),
            ("Sleepy", Pose(eyes="sleepy", lean=12)), ("Cool", Pose(eyes="cool", lean=-6)), ("Boo", Pose(eyes="wide", mouth=1.0))]


MOOD_FOCUS = -0.52
MOOD_SIZE = 280
