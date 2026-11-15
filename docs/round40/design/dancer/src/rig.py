"""The design rig for round 40's dancer sheets (design only; Python, no cargo).

Mirrors cantus-dance's conventions (ADR 0090) so a builder can follow it:
- a character stands about 1 unit tall, origin on the ground between its feet, y down;
- **beat 0 is the beat**: a step lands, a clap meets, a jump lands on it;
- index 0 is the limb on the viewer's left.

What it adds over today's `rig_dance.rs` `Body` (each direction's note says which it
needs): a pelvis that moves (hip offset), **two-bone IK** for legs (a foot planted, a knee
that bends) and for arms (a hand that reaches a target), a chest that isolates from the
hips, a head that lags, and **snap time** (pose-to-pose holds on the grid).

A pose is a pure function of the beat — the same rule as `pose::pose_at`.
"""
import math
from dataclasses import dataclass, field, fields, replace


# ---------------------------------------------------------------- maths
def clamp01(t):
    return max(0.0, min(1.0, t))


def lerp(a, b, t):
    return a + (b - a) * t


def lerp_p(a, b, t):
    return (lerp(a[0], b[0], t), lerp(a[1], b[1], t))


def smooth(t):
    t = clamp01(t)
    return t * t * (3 - 2 * t)


def ease_out(t):
    t = clamp01(t)
    return 1 - (1 - t) ** 3


def ease_in(t):
    t = clamp01(t)
    return t ** 3


def ease_in_out(t):
    t = clamp01(t)
    return 4 * t ** 3 if t < 0.5 else 1 - (-2 * t + 2) ** 3 / 2


def ease_out_back(t, s=1.9):
    t = clamp01(t) - 1
    return 1 + (s + 1) * t ** 3 + s * t ** 2


def sin1(t):
    return math.sin(2 * math.pi * t)


def cos1(t):
    return math.cos(2 * math.pi * t)


def arc(t):
    """0 at both ends, 1 in the middle (a hop)."""
    t = clamp01(t)
    return 4 * t * (1 - t)


def beat_down(ph, sharp=2.0):
    """1 on the beat, 0 half way: how far down a bounce is (ph = phase in the beat)."""
    return (0.5 + 0.5 * math.cos(2 * math.pi * ph)) ** sharp


def add(a, b):
    return (a[0] + b[0], a[1] + b[1])


def sub(a, b):
    return (a[0] - b[0], a[1] - b[1])


def mul(a, k):
    return (a[0] * k, a[1] * k)


def length(a):
    return math.hypot(a[0], a[1])


def rot(p, deg, about=(0.0, 0.0)):
    r = math.radians(deg)
    c, s = math.cos(r), math.sin(r)
    x, y = p[0] - about[0], p[1] - about[1]
    return (about[0] + x * c - y * s, about[1] + x * s + y * c)


def ik2(root, target, l1, l2, bend):
    """Two-bone IK: the joint between `root` and the reached end. `bend` +1/-1 picks the
    side the joint bulges to (+1: to the right of the root→target line, y down)."""
    d = sub(target, root)
    dist = length(d)
    reach = (l1 + l2) * 0.999
    if dist > reach:
        d = mul(d, reach / dist)
        dist = reach
        target = add(root, d)
    dist = max(dist, 1e-6)
    a = (l1 * l1 - l2 * l2 + dist * dist) / (2 * dist)
    h = math.sqrt(max(l1 * l1 - a * a, 0.0))
    ux, uy = d[0] / dist, d[1] / dist
    mid = (root[0] + ux * a, root[1] + uy * a)
    # perpendicular to the right of the direction (y down): (-uy, ux)
    joint = (mid[0] - uy * h * bend, mid[1] + ux * h * bend)
    return joint, target


# ---------------------------------------------------------------- the pose
@dataclass
class Pose:
    """A pose, independent of the character's proportions. Lengths are shares of the
    character's leg (L), arm (A) or torso (T); angles in degrees."""

    hip: tuple = (0.0, 0.0)        # pelvis offset from standing (L units; y + is down)
    lift: float = 0.0              # whole body off the ground (units of height; + is up)
    lean: float = 0.0              # torso about the pelvis (+ leans to the viewer's right)
    chest: tuple = (0.0, 0.0)      # chest isolation from the torso line (T units)
    roll: float = 0.0              # shoulder roll (+: the right shoulder up), degrees
    nod: float = 0.0               # head down (+) / up (−)
    tilt: float = 0.0              # head roll (+ clockwise)
    feet: tuple = ((0.0, 0.0), (0.0, 0.0))   # from where they stand (L units; y − is lifted)
    toe: tuple = (0.0, 0.0)        # foot angles (tip up +)
    hands: tuple = ((-0.40, 0.92), (0.40, 0.92))  # from the neck's base (A units)
    elbow: tuple = (1.0, 1.0)      # +1 the elbow bulges outward, −1 inward
    # forward kinematics instead of hand targets, as rig_dance's `arm`/`bend`: degrees from
    # hanging (0) through out to the side (90) to straight up (180); the forearm turns
    # `bend` further the same way (negative: back toward the body). None: use `hands`.
    fk: tuple = None
    knee: tuple = (1.0, 1.0)       # +1 the knee bulges outward, −1 inward
    squash: float = 0.0            # > 0 squashed, < 0 stretched (keeps the area)
    turn: float = 0.0              # −1 … 1, a hint of three-quarter (shoulders narrow)
    # the face and what the beat adds
    mood: str = "happy"
    eyes: str = "open"             # open, happy, closed, wide, wink, cool, sleepy
    mouth: float = 0.0             # 0 closed … 1 open (singing)
    accent: float = 0.0            # 1 on the beat, decaying (the floor ring, the lights)
    level: float = 0.6             # what the music is doing (0…1), for lights/meters
    claps: float = 0.0             # 1 the frame two hands meet, decaying
    landed: float = 0.0            # 1 on a landing, decaying (dust)
    point: float = 0.0             # pointer/antenna swing (Tono), degrees
    t: float = 0.0                 # beats since the move began (for what a character animates itself)
    sig: float = 0.0               # a signature move's own progress (0…1), per character
    shape: float = 0.0             # a character's own morph (Onda: sine 0 → square 1)


NUMERIC = [f.name for f in fields(Pose) if f.name not in ("mood", "eyes")]


def _blend(a, b, t):
    if a is None or b is None:
        return b if t >= 0.5 else a
    if isinstance(a, tuple):
        return tuple(_blend(x, y, t) for x, y in zip(a, b))
    return lerp(a, b, t)


def blend(p, q, t):
    """Pose p → q (`Body::lerp`'s rule); the face switches half way."""
    out = replace(p)
    for n in NUMERIC:
        setattr(out, n, _blend(getattr(p, n), getattr(q, n), t))
    if t >= 0.5:
        out.mood, out.eyes = q.mood, q.eyes
    return out


def snap_time(b, step=0.5, move=0.32, amount=1.0, overshoot=1.9):
    """Pose-to-pose time: the pose holds a key, then travels to the next in `move` of a
    `step` (eased out with an overshoot), crossing the key **on** its grid line and settling
    just after it — pop and lock: the hit is on the beat, the settle after it."""
    lead = (1 - overshoot / (overshoot + 1)) * move * step  # where ease_out_back first reaches 1
    b2 = b + lead
    k = math.floor(b2 / step)
    f = (b2 - k * step) / step
    g = ease_out_back(f / move, overshoot) if f < move else 1.0
    warped = (k - 1 + g) * step
    return lerp(b, warped, amount)
