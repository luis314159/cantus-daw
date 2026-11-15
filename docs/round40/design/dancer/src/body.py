"""From a Pose and a character's proportions to the joints it draws (design rig)."""
import math
from dataclasses import dataclass
from rig import add, sub, mul, rot, length, ik2, lerp


@dataclass
class Props:
    leg: float = 0.47          # hip height standing (L)
    thigh: float = 0.245
    shin: float = 0.235
    torso: float = 0.30        # pelvis to the neck's base (T)
    neck: float = 0.035        # neck base to the head's bottom
    head: float = 0.075        # head radius
    shoulder: float = 0.075    # half the shoulder width
    hipw: float = 0.045        # half the hip width
    stance: float = 0.06       # half the distance between the feet standing
    upper: float = 0.165
    fore: float = 0.16
    foot: float = 0.06         # foot length

    @property
    def arm(self):
        return self.upper + self.fore


@dataclass
class Joints:
    pelvis: tuple
    neck: tuple
    head: tuple            # the head's centre
    head_deg: float        # the head's roll
    torso_deg: float
    hips: tuple            # (left, right)
    knees: tuple
    feet: tuple
    toes: tuple            # foot angles
    shoulders: tuple
    elbows: tuple
    hands: tuple
    squash: float
    lift: float


def _outward(root, target, l1, l2, side, sign):
    """IK picking the bend so the joint bulges away from the body (`sign` +1) or toward
    it (−1); `side` is −1 for the viewer's left limb."""
    j1, end = ik2(root, target, l1, l2, 1.0)
    j2, _ = ik2(root, target, l1, l2, -1.0)
    out1 = (j1[0] - (root[0] + end[0]) / 2) * side
    out2 = (j2[0] - (root[0] + end[0]) / 2) * side
    if abs(out1 - out2) < 1e-4:  # the limb is horizontal: bulge down when out, up when in
        pick = j1 if (j1[1] > j2[1]) == (sign > 0) else j2
    else:
        pick = j1 if (out1 > out2) == (sign > 0) else j2
    return pick, end


def solve(p, pr: Props):
    L = pr.leg
    pelvis = (p.hip[0] * L, -L + p.hip[1] * L)
    torso_deg = p.lean
    up = rot((0.0, -pr.torso), torso_deg)
    neck = add(add(pelvis, up), (p.chest[0] * pr.torso, p.chest[1] * pr.torso))
    # shoulders: across the torso's top, rolled, narrowed by a turn
    sw = pr.shoulder * (1 - 0.35 * abs(p.turn))
    across = rot((sw, 0.0), torso_deg + p.roll * -1)
    shoulders = (sub(neck, across), add(neck, across))
    hw = pr.hipw * (1 - 0.3 * abs(p.turn))
    hacross = rot((hw, 0.0), torso_deg * 0.4)
    hips = (sub(pelvis, hacross), add(pelvis, hacross))
    head_deg = torso_deg * 0.6 + p.tilt
    hup = rot((0.0, -(pr.neck + pr.head)), torso_deg * 0.5 + p.tilt * 0.3)
    head = add(neck, hup)
    head = add(head, (0.0, p.nod * 0.0012))  # a nod drops the head a little
    # the ground holds: a leg never stretches past its length — the pelvis drops instead
    want = [add((side * pr.stance, 0.0), (p.feet[i][0] * L, p.feet[i][1] * L))
            for i, side in enumerate((-1.0, 1.0))]
    reach = (pr.thigh + pr.shin) * 0.995
    drop = 0.0
    for i in range(2):
        dx = want[i][0] - hips[i][0]
        gap = want[i][1] - hips[i][1]
        room = (max(reach * reach - dx * dx, 0.0)) ** 0.5
        drop = max(drop, gap - room)
    if drop > 0:
        down = (0.0, -drop)
        pelvis, neck = sub(pelvis, down), sub(neck, down)
        shoulders = (sub(shoulders[0], down), sub(shoulders[1], down))
        hips = (sub(hips[0], down), sub(hips[1], down))
        head = sub(head, down)
    feet, knees = [], []
    for i, side in enumerate((-1.0, 1.0)):
        f = want[i]
        k, f = _outward(hips[i], f, pr.thigh, pr.shin, side, p.knee[i])
        feet.append(f)
        knees.append(k)
    elbows, hands = [], []
    for i, side in enumerate((-1.0, 1.0)):
        if p.fk is not None:
            a, bend = p.fk[i]
            a -= torso_deg * side  # the arms turn with the torso
            d1 = (side * math.sin(math.radians(a)), math.cos(math.radians(a)))
            d2 = (side * math.sin(math.radians(a + bend)), math.cos(math.radians(a + bend)))
            e = add(shoulders[i], mul(d1, pr.upper))
            h = add(e, mul(d2, pr.fore))
            elbows.append(e)
            hands.append(h)
            continue
        t = add(neck, mul(p.hands[i], pr.arm))
        e, h = _outward(shoulders[i], t, pr.upper, pr.fore, side, p.elbow[i])
        elbows.append(e)
        hands.append(h)
    return Joints(pelvis, neck, head, head_deg, torso_deg, hips, tuple(knees), tuple(feet),
                  p.toe, shoulders, tuple(elbows), tuple(hands), p.squash, p.lift)


def place(pt, j: Joints):
    """Squash about the ground (area kept), then lift: what every drawn point goes through."""
    s = j.squash
    return (pt[0] * (1.0 + s * 0.6), pt[1] * (1.0 - s) - j.lift)
