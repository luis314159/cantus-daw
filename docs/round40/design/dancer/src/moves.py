"""The moves the previews show, as pure functions of the beat (design rig).

`b` is beats since the move started; beat 0 is the beat. Each takes a `Style` so the same
choreography is danced in each character's way (ADR 0090 §1's rule, kept).
"""
import math
from dataclasses import dataclass, replace
from rig import (Pose, blend, snap_time, smooth, ease_out, ease_in, ease_in_out, arc,
                 beat_down, lerp, lerp_p, cos1, sin1, clamp01)


@dataclass
class Style:
    bounce: float = 0.07      # pelvis dip on a beat (L units)
    sway: float = 0.06        # hips side to side (L units)
    step: float = 0.16        # a step's travel (L units)
    arms: float = 1.0         # how far arms reach out (× the choreography's)
    lag: float = 0.10         # head and what hangs, behind the beat (beats)
    hop: float = 0.20         # jump height (units of height)
    squash: float = 0.10      # landing squash
    snap: float = 0.0         # 0 fluid … 1 pose-to-pose holds on the half beat
    lean: float = 1.0
    open: tuple = (0.50, 0.84)     # the hands between claps (neck-relative, A units)
    chest: float = 0.02           # the chest's sway against the hips (T units)
    lift_step: float = 0.07       # how high a step lifts the foot (L units)


def _hands(keys, b, period):
    """Hands from keyed positions at beats (key beat → (left, right)), eased between."""
    ks = sorted(keys)
    bb = b % period
    for i, k in enumerate(ks):
        nk = ks[(i + 1) % len(ks)]
        span = (nk - k) % period or period
        if 0 <= (bb - k) % period < span:
            t = ((bb - k) % period) / span
            return keys[k], keys[nk], t
    return keys[ks[0]], keys[ks[0]], 0.0


def _lerp_pair(a, b, t):
    return (lerp_p(a[0], b[0], t), lerp_p(a[1], b[1], t))


# ------------------------------------------------------------------ groove: bounce + clap
def groove(b, st: Style):
    """Two beats: a bounce down on each beat; a clap in front of the chest on beat 1 and
    high over the right shoulder on beat 2; the hips sway toward the high clap."""
    if st.snap:
        b = snap_time(b, 0.5, amount=st.snap)
    p = Pose()
    ph = b % 1.0
    k = int(math.floor(b)) % 2
    down = beat_down(ph, 2.2)
    side = cos1((b - 1) / 2)              # +1 at beat 2 (right), −1 at beat 1
    p.hip = (st.sway * side, st.bounce * down)
    p.lean = 4.0 * side * st.lean
    p.chest = (st.chest * side, 0.03 * down)
    p.roll = 5.0 * cos1((b - 1 - st.lag) / 2)
    p.nod = 10.0 * beat_down((b - st.lag) % 1.0, 2.0) - 3.0
    p.tilt = 7.0 * cos1((b - 1 - st.lag) / 2)
    # hands: meet on the beat, open between
    a = st.arms
    meet = [((-0.035, 0.50), (0.035, 0.50)), ((0.30, 0.22), (0.38, 0.22))]
    open_ = ((-st.open[0] * a, st.open[1]), (st.open[0] * a, st.open[1]))
    m0, m1 = meet[k], meet[(k + 1) % 2]
    if ph < 0.5:
        p.hands = _lerp_pair(m0, open_, ease_out(ph * 2))
    else:
        p.hands = _lerp_pair(open_, m1, ease_in(ph * 2 - 1))
    p.elbow = (1.0, 1.0)
    # the foot away from the sway taps on the off-beat
    tap = arc(clamp01((ph - 0.25) * 2)) * 0.05
    p.feet = ((0.0, -tap if side > 0 else 0.0), (0.0, -tap if side < 0 else 0.0))
    p.claps = max(0.0, 1 - ph * 4)
    p.accent = beat_down(ph, 8) * (1.0 if k == 0 else 0.7)
    p.mouth = 0.7 * beat_down(ph, 3) if k == 1 else 0.0
    p.eyes = "happy"
    p.point = 28 * side
    p.level = 0.55 + 0.4 * beat_down(ph, 3)
    return p


# ------------------------------------------------------------------ step-touch
def _foot_track(b, moves, period, lift=0.07):
    """A foot's (x, y) from its landings: [(land_beat, from_x, to_x)], each travelled in
    the half beat before it lands, lifted on an arc."""
    bb = b % period
    for land, x0, x1 in moves:
        t = (bb - (land - 0.5)) % period
        if t < 0.5:
            u = ease_in_out(t / 0.5)
            return (lerp(x0, x1, u), -lift * arc(t / 0.5))
    # holding: the last landing before now
    best = None
    for land, x0, x1 in moves:
        since = (bb - land) % period
        if best is None or since < best[0]:
            best = (since, x1)
    return (best[1], 0.0)


def steps(b, st: Style):
    """Four beats of step-touch: step right (1), close left (2), step left (3), close
    right (4); the arms swing low on a step and snap to the side on a close."""
    if st.snap:
        b = snap_time(b, 0.5, amount=st.snap)
    D = st.step
    p = Pose()
    ph = b % 1.0
    left = _foot_track(b, [(1.0, -D, D), (2.0, D, -D)], 4.0, st.lift_step)
    right = _foot_track(b, [(0.0, -D, D), (3.0, D, -D)], 4.0, st.lift_step)
    p.feet = (left, right)
    hx = (left[0] + right[0]) / 2
    ahead = (_foot_track(b + 0.15, [(1.0, -D, D), (2.0, D, -D)], 4.0)[0]
             + _foot_track(b + 0.15, [(0.0, -D, D), (3.0, D, -D)], 4.0)[0]) / 2
    p.hip = (hx * 1.05, st.bounce * beat_down(ph, 2.0) + 0.02)
    p.lean = (ahead - hx) * 40.0 * st.lean
    a = st.arms
    keys = {
        0.0: ((-0.50 * a, 0.70), (0.50 * a, 0.70)),     # step right: arms low and open
        1.0: ((0.12, 0.30), (0.78 * a, -0.02)),         # close: a snap to the right
        2.0: ((-0.50 * a, 0.70), (0.50 * a, 0.70)),
        3.0: ((-0.78 * a, -0.02), (-0.12, 0.30)),       # a snap to the left
    }
    h0, h1, t = _hands(keys, b, 4.0)
    p.hands = _lerp_pair(h0, h1, ease_in_out(t))
    sidek = math.floor(b % 4.0)
    snapside = 1.0 if 0.5 <= (b % 4.0) < 2.5 else -1.0
    p.tilt = 9.0 * math.sin(math.pi * ((b - st.lag) % 4.0) / 2.0 - math.pi / 4)
    p.roll = 6.0 * snapside * smooth(1 - abs(((b % 2.0) - 1.0)))
    p.nod = 8.0 * beat_down((b - st.lag) % 1.0) - 2.0
    p.chest = (0.03 * snapside, 0.02 * beat_down(ph))
    p.accent = beat_down(ph, 8) * (1.0 if sidek in (0, 2) else 0.7)
    p.eyes = "happy" if sidek in (1, 3) else "open"
    p.mouth = 0.0
    p.point = 25 * snapside
    p.level = 0.5 + 0.4 * beat_down(ph, 3)
    return p


# ------------------------------------------------------------------ the jump (the drop)
def jump(b, st: Style):
    """Two beats, landing on the beat: recover, crouch with the arms crossed low, spring
    into a star — arms up in a V, legs apart — and land on the next beat."""
    p = Pose()
    bb = b % 2.0
    H = st.hop
    if bb < 0.5:                                   # landed: squash, recover
        u = bb / 0.5
        p.hip = (0.0, lerp(0.22, 0.02, ease_out(u)))
        p.squash = st.squash * (1 - ease_out(u * 1.4))
        a = st.arms
        p.hands = _lerp_pair(((-0.95 * a, 0.32), (0.95 * a, 0.32)), ((-0.50 * a, 0.92), (0.50 * a, 0.92)),
                             ease_out(u))
        p.landed = max(0.0, 1 - bb * 3)
        p.eyes = "happy"
        p.nod = lerp(12, 0, ease_out(u))
    elif bb < 1.0:                                 # crouch: the anticipation
        u = (bb - 0.5) / 0.5
        p.hip = (0.0, lerp(0.02, 0.30, ease_in_out(u)))
        a = st.arms
        p.hands = _lerp_pair(((-0.50 * a, 0.92), (0.50 * a, 0.92)), ((-0.62 * a, 0.86), (0.62 * a, 0.86)),
                             ease_in_out(u))
        p.lean = 0.0
        p.nod = lerp(0, 14, u)
        p.eyes = "closed" if u > 0.6 else "open"
        p.squash = 0.04 * ease_in(u)
    else:                                          # in the air: a star, then land
        u = (bb - 1.0) / 1.0
        p.lift = H * arc(u) ** 0.85
        up = ease_out(clamp01(u / 0.22))
        p.hip = (0.0, lerp(0.30, -0.02, up) if u < 0.22 else lerp(-0.02, 0.06, smooth((u - 0.6) / 0.4)))
        star = math.sin(math.pi * clamp01((u - 0.08) / 0.8))
        p.feet = ((-0.16 * star, -0.10 * star), (0.16 * star, -0.10 * star))
        a = st.arms
        vee = ((-0.62 * a, -0.90), (0.62 * a, -0.90))
        down = ((-0.95 * a, 0.32), (0.95 * a, 0.32))
        if u < 0.25:
            p.hands = _lerp_pair(((-0.62 * a, 0.86), (0.62 * a, 0.86)), vee, ease_out(u / 0.25))
        elif u < 0.65:
            p.hands = vee
        else:
            p.hands = _lerp_pair(vee, down, ease_in_out((u - 0.65) / 0.35))
        p.squash = -0.08 * math.sin(math.pi * clamp01(u / 0.3)) - 0.04 * smooth((u - 0.7) / 0.3)
        p.nod = lerp(14, -10, up) if u < 0.5 else lerp(-10, 6, (u - 0.5) / 0.5)
        p.eyes = "wide"
        p.mouth = 0.8 * math.sin(math.pi * clamp01((u - 0.1) / 0.7))
    p.accent = beat_down((b % 1.0), 8) * (1.0 if bb < 1.0 else 0.6)
    p.level = 0.9 if bb >= 1.0 else 0.5
    p.point = 40 * math.sin(math.pi * clamp01(bb - 1.0)) if bb >= 1.0 else -10 * (bb % 1.0)
    return p


MOVES = {"groove": (groove, 2.0), "steps": (steps, 4.0), "jump": (jump, 2.0)}


def routine(b, st: Style, plan):
    """A routine: [(move, beats)], blended over a quarter beat at each change, the move
    left behind running on (its phase continues), as `pose_at` blends."""
    total = sum(n for _, n in plan)
    bb = b % total
    start = 0.0
    for i, (name, n) in enumerate(plan):
        if bb < start + n:
            fn, _ = MOVES[name]
            pose = fn(bb - start, st)
            prev_name, prev_n = plan[i - 1]
            prev_start = (start - prev_n) % total
            into = bb - start
            if into < 0.25:
                pfn, _ = MOVES[prev_name]
                before = pfn((bb - prev_start) % total, st)
                pose = blend(before, pose, smooth(into / 0.25))
            return pose
        start += n
    return Pose()


# ------------------------------------------------------------------ key poses (sheets)
def key_poses(st: Style):
    rest = Pose(eyes="open")
    point = Pose(hip=(-0.07, 0.03), lean=6, hands=((-0.44, 0.42), (0.62, -0.95)),
                 feet=((0, 0), (0.14, 0)), toe=(0, 14), tilt=-8, nod=-6, eyes="cool", point=40)
    kick = Pose(hip=(-0.10, 0.04), lean=-10, hands=((-0.86, 0.10), (0.70, -0.30)),
                feet=((0, 0), (0.62, -0.42)), tilt=-6, eyes="happy", point=-30)
    hero = Pose(feet=((-0.20, 0), (0.20, 0)), hands=((-0.43, 0.40), (0.43, 0.40)),
                elbow=(1, 1), chest=(0, -0.03), nod=-8, eyes="cool")
    bow = Pose(hip=(0.0, 0.06), lean=0, chest=(0.0, 0.10), nod=26, tilt=4,
               hands=((0.08, 0.55), (0.80, 0.30)), feet=((0, 0), (0.10, 0)), eyes="closed")
    return [
        ("Idle", rest),
        ("Clap", groove(0.0, replace(st, snap=0))),
        ("Snap", steps(1.0, replace(st, snap=0))),
        ("Point", point),
        ("Crouch", jump(0.92, st)),
        ("Star jump", jump(1.42, st)),
        ("Kick", kick),
        ("Hero", hero),
        ("Bow", bow),
    ]


# ------------------------------------------------------------------ open-silhouette moves
# (a character may replace any move: these keep the limbs clear of the body in straight
# bars at clean angles — Compás's; arms by angle and bend, as rig_dance's Body)
def _fk(keys, b, period, ease=None):
    k0, k1, t = _hands(keys, b, period)
    t = (ease or ease_in_out)(t)
    return tuple((lerp(k0[i][0], k1[i][0], t), lerp(k0[i][1], k1[i][1], t)) for i in range(2))


UP, DOWN, OUT, W, HIP = (148, 0), (38, 0), (92, 0), (100, 82), (34, -76)


def pump(b, st: Style):
    """Two beats: one arm punches up on the diagonal and the other down, swapping on each
    beat; between, both arms flex in a W; the foot under the low arm flicks out."""
    if st.snap:
        b = snap_time(b, 0.5, amount=st.snap)
    p = Pose()
    ph = b % 1.0
    k = int(math.floor(b)) % 2
    p.fk = _fk({0.0: (DOWN, UP), 0.5: (W, W), 1.0: (UP, DOWN), 1.5: (W, W)}, b, 2.0)
    sw = cos1(b / 2.0)                            # +1 on beat 1 (right arm up), −1 on beat 2
    p.hip = (-st.sway * sw, st.bounce * beat_down(ph, 2.0))
    p.lean = 5.0 * sw * st.lean
    p.tilt = 12.0 * sw
    p.nod = 6.0 * beat_down(ph, 2.0) - 4.0
    flick = beat_down(ph, 1.5)
    p.feet = (((-0.12 * flick, -0.12 * flick), (0.0, 0.0)) if sw > 0
              else ((0.0, 0.0), (0.12 * flick, -0.12 * flick)))
    p.accent = beat_down(ph, 8) * (1.0 if k == 0 else 0.7)
    p.eyes = "open"
    p.point = 30 * sw
    p.level = 0.55 + 0.4 * beat_down(ph, 3)
    return p


def steps_open(b, st: Style):
    """Step-touch with the arms in clear lines: out in a T on a step, a disco point to the
    side it closes toward, the other hand on the hip."""
    p = steps(b, st)
    if st.snap:
        b = snap_time(b, 0.5, amount=st.snap)
    p.fk = _fk({0.0: (OUT, OUT), 1.0: (HIP, UP), 2.0: (OUT, OUT), 3.0: (UP, HIP)}, b, 4.0)
    return p


def star(b, st: Style):
    """The jump with straight arms: down and out on landing, low behind in the crouch, a V
    in the air (the star jump)."""
    p = jump(b, st)
    bb = b % 2.0
    LAND, LOW, V = (70, 0), (22, 0), (150, 0)
    if bb < 0.5:
        u = ease_out(bb / 0.5)
        p.fk = ((lerp(LAND[0], 12, u), 0), (lerp(LAND[0], 12, u), 0))
    elif bb < 1.0:
        u = ease_in_out((bb - 0.5) / 0.5)
        p.fk = ((lerp(12, LOW[0], u), 0), (lerp(12, LOW[0], u), 0))
    else:
        u = (bb - 1.0)
        if u < 0.22:
            a = lerp(LOW[0], V[0], ease_out(u / 0.22))
        elif u < 0.65:
            a = V[0]
        else:
            a = lerp(V[0], LAND[0], ease_in_out((u - 0.65) / 0.35))
        p.fk = ((a, 0), (a, 0))
    return p


def steps_human(b, st: Style):
    """Step-touch for a human figure: the arms swing low and loose on a step; on a close the
    near arm reaches out and up on the diagonal (a snap) and the far hand comes to the chest."""
    p = steps(b, st)
    keys = {
        0.0: ((-0.55, 0.95), (0.55, 0.95)),
        1.0: ((0.10, 0.42), (1.00, -0.62)),
        2.0: ((-0.55, 0.95), (0.55, 0.95)),
        3.0: ((-1.00, -0.62), (-0.10, 0.42)),
    }
    h0, h1, t = _hands(keys, b, 4.0)
    p.hands = _lerp_pair(h0, h1, ease_in_out(t))
    snapside = 1.0 if 0.5 <= (b % 4.0) < 2.5 else -1.0
    p.chest = (-0.05 * snapside, p.chest[1])
    return p


def steps_wave(b, st: Style):
    """Step-touch for a big-headed character: nothing crosses the body — on a close the
    near arm waves up and out, the far one swings out low."""
    p = steps(b, st)
    a = st.arms
    keys = {
        0.0: ((-0.62 * a, 0.80), (0.62 * a, 0.80)),
        1.0: ((-0.70 * a, 0.70), (0.80 * a, -0.62)),
        2.0: ((-0.62 * a, 0.80), (0.62 * a, 0.80)),
        3.0: ((-0.80 * a, -0.62), (0.70 * a, 0.70)),
    }
    h0, h1, t = _hands(keys, b, 4.0)
    p.hands = _lerp_pair(h0, h1, ease_in_out(t))
    return p


MOVES.update({"pump": (pump, 2.0), "steps_open": (steps_open, 4.0), "star": (star, 2.0),
              "steps_human": (steps_human, 4.0), "steps_wave": (steps_wave, 4.0)})
