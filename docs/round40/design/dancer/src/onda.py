"""New — **Onda**, the wave: a living ribbon, the most abstract of the family.

A ribbon stands up from a curl on the floor and travels as a wave to a round head; it
**twists** as it moves, so its front (lapis, a step lighter on Night) and its back (gold)
take turns — a waveform you could pick up. Faceless like Compás: its eyes are negative space
cut into the head (they blink, they squint happy). Its dance is the wave itself: the
amplitude pulses on the beat, it leans and travels; and its signature, **Square drop**: on the
drop it springs up and the sine turns into a square wave (then a saw on the way down) before
melting back into a sine.
"""
import math
from body import Props
from moves import Style, MOVES as ALL_MOVES, steps
from rig import (Pose, add, sub, mul, rot, length, lerp, beat_down, cos1, sin1, smooth, ease_out,
                 ease_out_back, clamp01, arc)
from svg import Doc, f

NAME = "Onda"
PROPS = Props()          # unused by its drawing (a ribbon, not a skeleton); kept for the sheet
STYLE = Style(bounce=0.08, sway=0.08, step=0.22, lag=0.12, hop=0.16)
MOVES = {"groove": "onda_pulse", "steps": "onda_travel", "jump": "onda_square"}
HEIGHT = 0.90            # where the head sits
CUT = 0.014


def pulse(b, st):
    p = Pose()
    ph = b % 1.0
    p.accent = beat_down(ph, 3.0)
    p.level = 0.55 + 0.45 * beat_down(ph, 2.0)
    p.lean = 10.0 * cos1(b / 2.0)
    p.hip = (0.04 * cos1(b / 2.0), 0.0)
    p.squash = 0.06 * beat_down(ph, 4.0)
    p.eyes = "happy" if ph < 0.25 else "open"
    return p


def travel(b, st):
    q = steps(b, st)
    p = Pose()
    ph = b % 1.0
    p.hip = (q.hip[0], 0.0)
    p.lean = -q.lean * 0.8
    p.accent = beat_down(ph, 3.0)
    p.level = 0.5 + 0.4 * beat_down(ph, 2.0)
    p.squash = 0.05 * beat_down(ph, 4.0)
    p.eyes = "open"
    return p


def square(b, st):
    p = Pose()
    bb = b % 2.0
    crouch = smooth(bb / 0.45) * (1 - smooth((bb - 0.45) / 0.15))
    p.squash = 0.18 * crouch
    air = clamp01((bb - 0.55) / 1.0)
    p.lift = st.hop * arc(air) ** 0.8 if 0.55 < bb < 1.55 else 0.0
    if 0.55 <= bb < 1.55:
        p.squash = -0.10 * math.sin(math.pi * air)
    land = clamp01((bb - 1.55) / 0.45)
    p.squash += 0.12 * (1 - ease_out(land)) * (1 if bb >= 1.55 else 0)
    p.shape = ease_out_back(clamp01((bb - 0.6) / 0.25), 1.4) * (1 - smooth((bb - 1.5) / 0.5))
    p.sig = 1.0 if 1.15 < bb < 1.6 else 0.0      # the saw on the way down
    p.accent = 1.0 if 0.55 < bb < 1.5 else beat_down(b % 1.0, 3.0)
    p.level = 1.0 if 0.55 < bb < 1.6 else 0.5
    p.eyes = "closed" if bb < 0.5 else ("wide" if bb < 1.5 else "happy")
    return p


ALL_MOVES.update({"onda_pulse": (pulse, 2.0), "onda_travel": (travel, 4.0), "onda_square": (square, 2.0)})


def colours(th):
    night = th["name"] == "Night"
    return {"front": "#7D97E6" if night else "#2E4794", "front_s": "#5C78CC" if night else "#24397A",
            "back": "#E2B85A" if night else "#B8882C", "back_s": "#B38A35" if night else "#8F6A1E"}


def _wave(x, shape, saw):
    """A sine that becomes a square (shape → 1) or a saw."""
    s = math.sin(2 * math.pi * x)
    if saw:
        return 2.0 * ((x + 0.25) % 1.0) - 1.0
    sq = math.copysign(abs(s) ** max(0.08, 1.0 - shape * 0.95), s)
    return sq


def centreline(pose, n=60):
    t = pose.t
    pts = []
    for i in range(n + 1):
        s = i / n
        env = math.sin(math.pi * min(1.0, s * 1.15)) if s < 0.87 else math.sin(math.pi * 0.87 * 1.15) * (1 - (s - 0.87) * 2)
        amp = (0.060 + 0.050 * pose.accent + 0.020 * pose.shape) * (0.2 + 0.8 * env)
        x = amp * _wave(s * 1.25 - t * 0.5, pose.shape, pose.sig > 0.5)
        y = -HEIGHT * s
        x += math.sin(math.radians(pose.lean)) * (-y) + pose.hip[0] * 0.5
        pts.append((x, y))
    return pts


def width(s):
    """Narrow at the floor, widest under the head, the head round."""
    return 0.040 + 0.105 * smooth(s / 0.85)


def draw(pose, th, cut_color=None, trails=None, floor=True, face=True):
    room = cut_color or th["base"]
    C = colours(th)
    d = Doc(cut_color=room, cut=CUT)
    sq = pose.squash
    tr = lambda p: (p[0] * (1 + 0.6 * sq), p[1] * (1 - sq) - pose.lift)
    bx = pose.hip[0]
    if floor:
        k = max(0.0, 1.0 - pose.lift * 3.5)
        d.raw(f'<ellipse cx="{f(bx)}" cy="0.01" rx="{f(0.15 * k + 0.04)}" ry="{f(0.022 * k + 0.006)}" '
              f'fill="{th["shadow"]}" opacity="0.5"/>')
    pts = [tr((x + bx, y)) for x, y in centreline(pose)]
    n = len(pts) - 1
    # the curl on the floor
    curl = [pts[0]] + [tr((bx + 0.040 * math.sin(math.radians(v)), -0.040 + 0.040 * math.cos(math.radians(v))))
                       for v in range(0, 300, 30)]
    d.line(curl, 0.026, C["front_s"], cut=True, smooth=True)
    # edges; the twist strongest at the tail, none at the head (the face always shows)
    L, R, side = [], [], []
    for i in range(n + 1):
        s = i / n
        a, b = pts[max(i - 1, 0)], pts[min(i + 1, n)]
        dx, dy = b[0] - a[0], b[1] - a[1]
        ln = math.hypot(dx, dy) or 1e-6
        nx, ny = -dy / ln, dx / ln
        tw = math.cos(math.pi * (1 - s) * (1.5 + 0.5 * math.sin(pose.t * 0.8)) - math.pi * pose.t * 0.5 * (1 - s) ** 1.5)
        h = width(s) / 2 * (0.15 + 0.85 * abs(tw))
        p = pts[i]
        L.append((p[0] + nx * h, p[1] + ny * h))
        R.append((p[0] - nx * h, p[1] - ny * h))
        side.append(tw >= 0)
    runs, start = [], 0
    for i in range(1, n + 1):
        if side[i] != side[start] or i == n:
            runs.append((start, i, side[start]))
            start = i
    poly = lambda i0, i1, A, B: "M" + " L".join(f"{f(x)},{f(y)}" for x, y in A[i0:i1 + 1] + B[i0:i1 + 1][::-1]) + " Z"
    full = poly(0, n, L, R)
    d.raw(f'<path d="{full}" fill="{room}" stroke="{room}" stroke-width="{f(CUT * 2)}" stroke-linejoin="round"/>')
    M = [((l[0] * 0.62 + r[0] * 0.38), (l[1] * 0.62 + r[1] * 0.38)) for l, r in zip(L, R)]
    for i0, i1, front in runs:
        key = "front" if front else "back"
        d.raw(f'<path d="{poly(i0, i1, L, R)}" fill="{C[key]}" stroke="{C[key]}" stroke-width="0.003" stroke-linejoin="round"/>')
        d.raw(f'<path d="{poly(i0, i1, M, R)}" fill="{C[key + "_s"]}" stroke="{C[key + "_s"]}" stroke-width="0.003" stroke-linejoin="round"/>')
    # the head: the ribbon's round end, eyes cut into it
    top = pts[-1]
    hr = width(1.0) / 2 + 0.012
    hc = (top[0], top[1] + 0.004)
    d.circle(add(hc, (0.004, 0.006)), hr, C["front_s"], cut=False)
    d.circle(add(hc, (-0.004, -0.004)), hr * 0.94, C["front"], cut=False)
    if face:
        _eyes(d, add(hc, (0.0, 0.012)), pose, room)
    return d.svg()


def _eyes(d, hc, pose, room):
    e = pose.eyes
    for s in (-1, 1):
        c = add(hc, (s * 0.026, 0.004))
        if e == "happy":
            d.line([add(c, (-0.013, 0.008)), add(c, (0, -0.006)), add(c, (0.013, 0.008))], 0.010, room, cut=False)
        elif e in ("closed", "sleepy"):
            d.line([add(c, (-0.013, 0.0)), add(c, (0, 0.007)), add(c, (0.013, 0.0))], 0.009, room, cut=False)
        elif e == "cool":
            d.line([add(hc, (-0.050, 0.0)), add(hc, (0.050, 0.0))], 0.022, room, cut=False, cap="butt")
            return
        else:
            big = 1.3 if e == "wide" else 1.0
            d.ellipse(c, 0.010 * big, 0.014 * big, room)


def key_poses():
    def at(fn, b):
        p = fn(b, STYLE)
        p.t = b
        return p
    return [("Idle", Pose(eyes="open", t=0.3)), ("Pulse", at(pulse, 0.0)), ("Sway", at(pulse, 1.0)),
            ("Travel", at(travel, 0.4)), ("Arrive", at(travel, 1.0)), ("Crouch", at(square, 0.4)),
            ("Square", at(square, 0.95)), ("Saw", at(square, 1.3)), ("Melt", at(square, 1.75))]


def mood_items():
    return [("Neutral", Pose(eyes="open", t=0.2)), ("Happy", Pose(eyes="happy", accent=0.6, t=0.4)),
            ("Excited", Pose(eyes="wide", accent=1.0, shape=0.3, t=0.6)), ("Calm", Pose(eyes="closed", t=0.1)),
            ("Surprised", Pose(eyes="wide", shape=0.8, t=0.0)), ("Sleepy", Pose(eyes="sleepy", lean=-14, t=0.0, squash=0.08)),
            ("Cool", Pose(eyes="cool", lean=8, t=0.5)), ("Saw", Pose(eyes="open", sig=1.0, t=0.3))]


MOOD_FULL = True
