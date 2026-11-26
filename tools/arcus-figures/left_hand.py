"""r42:arcus-upper — the realistic figures' left hand on a violin's or a viola's neck,
authored offline (the owner, 2026-11-24: "the violin's left hand is a disaster").

Each pose is solved on the figure's own MakeHuman hand (``woman.fig`` / ``man.fig``: three
phalanges a finger, the thumb's CMC, MCP and IP) against what a violinist's hand keeps —
the owner's photographs 1-5 (``docs/round42/arcus-upper/ref-owner/``):

- the neck in the crook between the thumb and the index finger's base, a gap under the palm;
- the thumb along the neck's G side, opposite the first and second fingers, its tip about
  level with the fingerboard;
- each finger rising from the neck's E side and arching high over the fingerboard: MCP
  45-70 deg, PIP 70-100 deg, DIP 20-50 deg, so the playing tip comes down nearly upright;
- the other fingers hovering, curved, over their notes;
- the wrist straight (the forearm on the hand's line, its elbow under the violin);
- the palm toward the neck and the player.

The inputs (the strings, the fingerboard, the shoulder, the camera, every row's targets)
come from the app: ``CANTUS_HAND_DUMP=<dump.json> cargo test -p cantus-ui --test
arcus_hand_table -- --ignored dump``. The table the app reads is written to
``crates/cantus-ui/assets/figures/left_hand.bin`` (format: ``scene/figure/neck_hand.rs``).

    python tools/arcus-figures/left_hand.py solve <dump.json> --only=woman-violin  (x4)
    python tools/arcus-figures/left_hand.py fix <dump.json> --only=woman-violin  (x4)
    python tools/arcus-figures/left_hand.py pack <dump.json>
    python tools/arcus-figures/left_hand.py look <dump.json> <out dir> [who inst string finger at]

``look`` renders a row from the picture's camera, from photo 4's angle (the player's eye
over the G side), from photo 3's (the G side, level) and beside the photographs.
"""
import json
import os
import struct
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import hand_rig as hr  # noqa: E402
import hand_solid  # noqa: E402

ROOT = os.path.join(HERE, "..", "..")
# (r42:arcus-hands: CANTUS_FIGURE_DIR names a folder build_figures.py wrote, as the app reads it.)
FIGS = os.environ.get("CANTUS_FIGURE_DIR") or os.path.join(ROOT, "crates", "cantus-ui", "assets", "figures")
OUT = os.path.join(FIGS, "left_hand.bin")
O = hr.side(0)
HAND = O + hr.HAND
RAD = np.pi / 180.0


def rotvec(r):
    a = np.linalg.norm(r)
    if a < 1e-9:
        return np.eye(3)
    return hr.about(r / a, a)


def quat(R):
    """Unit quaternion (w, x, y, z) of a rotation matrix."""
    t = np.trace(R)
    if t > 0:
        s = np.sqrt(t + 1.0) * 2
        q = [0.25 * s, (R[2, 1] - R[1, 2]) / s, (R[0, 2] - R[2, 0]) / s, (R[1, 0] - R[0, 1]) / s]
    elif R[0, 0] > R[1, 1] and R[0, 0] > R[2, 2]:
        s = np.sqrt(1.0 + R[0, 0] - R[1, 1] - R[2, 2]) * 2
        q = [(R[2, 1] - R[1, 2]) / s, 0.25 * s, (R[0, 1] + R[1, 0]) / s, (R[0, 2] + R[2, 0]) / s]
    elif R[1, 1] > R[2, 2]:
        s = np.sqrt(1.0 + R[1, 1] - R[0, 0] - R[2, 2]) * 2
        q = [(R[0, 2] - R[2, 0]) / s, (R[0, 1] + R[1, 0]) / s, 0.25 * s, (R[1, 2] + R[2, 1]) / s]
    else:
        s = np.sqrt(1.0 + R[2, 2] - R[0, 0] - R[1, 1]) * 2
        q = [(R[1, 0] - R[0, 1]) / s, (R[0, 2] + R[2, 0]) / s, (R[1, 2] + R[2, 1]) / s, 0.25 * s]
    q = np.array(q)
    q /= np.linalg.norm(q)
    return q if q[0] >= 0 else -q


def qrot(q):
    w, x, y, z = q
    return np.array([
        [1 - 2 * (y * y + z * z), 2 * (x * y - w * z), 2 * (x * z + w * y)],
        [2 * (x * y + w * z), 1 - 2 * (x * x + z * z), 2 * (y * z - w * x)],
        [2 * (x * z - w * y), 2 * (y * z + w * x), 1 - 2 * (x * x + y * y)],
    ])


class Hand:
    """A figure's left hand at rest: its bones, the palm's normal, each joint's flex axis
    and its angle at rest (so the anatomical angles can be bounded)."""

    def __init__(self, who, knuckles=True):
        self.fig = hr.read_fig(os.path.join(FIGS, f"{who}.fig"), knuckles)
        B = self.fig["bones"]
        self.B = B
        self.wr = B[HAND][1]
        self.n = hr.palm_normal(self.fig, 0)
        self.heads = np.array([[B[O + hr.finger(f, k)][1] for k in range(3)] for f in range(5)])
        self.tails = np.array([B[O + hr.finger(f, 2)][2] for f in range(5)])
        # The chain's points at rest (base, middle, last joint, tip) and its bones' ways.
        self.pts = np.concatenate([self.heads, self.tails[:, None]], 1)
        self.dirs = np.array([[hr.norm(self.pts[f, k + 1] - self.pts[f, k]) for k in range(3)]
                              for f in range(5)])
        self.axes = np.zeros((5, 3, 3))
        for f in range(1, 5):
            for k in range(3):
                self.axes[f, k] = hr.norm(np.cross(self.dirs[f, k], self.n))
        # The thumb's pad faces the index finger's base.
        for k in range(3):
            d = self.dirs[0, k]
            to = self.heads[1, 0] - self.pts[0, k]
            nt = hr.norm(to - d * (to @ d))
            self.axes[0, k] = hr.norm(np.cross(d, nt))
        # The metacarpals' ways (wrist to each knuckle) and the joints' angles at rest.
        self.meta = np.array([hr.norm(self.heads[f, 0] - self.wr) for f in range(5)])
        self.rest = np.zeros((5, 3))
        for f in range(1, 5):
            prev = self.meta[f]
            for k in range(3):
                d = self.dirs[f, k]
                a = self.axes[f, k]
                self.rest[f, k] = np.arctan2(np.cross(prev, d) @ a, prev @ d)
                prev = d
        self.skin_setup()

    # r42:arcus-hands — the skin the solver keeps out of the instrument: the hand's and
    # the forearm's own vertices (each at least 0.6 theirs, as the gate takes them), thinned
    # (the fingers kept densest), each skinned by its weights on the hand's, the fingers'
    # and the forearm's bones (linear blend, as the app's ``repose_arm``).
    SKIN_MOST = {"finger": 420, "thumb": 100, "palm": 420, "fore": 120}

    def skin_setup(self):
        fig = self.fig
        self.bone_ids = [O + hr.FORE1, O + hr.FORE2, HAND] + [O + hr.finger(f, k)
                                                             for f in range(5) for k in range(3)]
        ids, ws, pos = fig["ids"], fig["ws"], fig["pos"]
        mine = np.isin(ids, self.bone_ids)
        share = (ws * mine).sum(1)
        cand = np.nonzero(share > 0.6)[0]
        w = np.where(mine[cand], ws[cand], 0.0)
        w /= w.sum(1, keepdims=True)
        top = ids[cand, np.argmax(w, 1)]
        kind = np.where(np.isin(top, self.bone_ids[:2]), "fore",
                        np.where(top == HAND, "palm",
                                 np.where(np.isin(top, [O + hr.finger(0, k) for k in range(3)]),
                                          "thumb", "finger")))
        keep = []
        for k, most in self.SKIN_MOST.items():
            at = np.nonzero(kind == k)[0]
            step = max(1, int(np.ceil(len(at) / most)))
            keep.extend(at[::step].tolist())
        keep = np.array(sorted(keep))
        idx = cand[keep]
        self.sk_pos = pos[idx]
        self.sk_kind = kind[keep]
        self.sk_top = top[keep]
        lut = {b: i for i, b in enumerate(self.bone_ids)}
        self.sk_ids = np.vectorize(lambda b: lut.get(b, 0))(ids[idx])
        self.sk_w = w[keep]

    def skin(self, W, Rh, Q, P, way=None):
        """The kept skin points posed (the forearm on ``way`` from the wrist)."""
        R = np.zeros((len(self.bone_ids), 3, 3))
        c = np.zeros((len(self.bone_ids), 3))
        Rf = Rh
        if way is not None:
            base = hr.norm(W - (P[2, 0] + P[3, 0]) * 0.5)
            Rf = hr.between(base, np.asarray(way)) @ Rh
        for i, (Rb, at, head) in enumerate(
                [(Rf, W, self.wr), (Rf, W, self.wr), (Rh, W, self.wr)]
                + [(Rh @ Q[f, k], P[f, k], self.heads[f, k]) for f in range(5) for k in range(3)]):
            R[i] = Rb
            c[i] = at - Rb @ head
        out = np.zeros_like(self.sk_pos)
        for j in range(4):
            Rj = R[self.sk_ids[:, j]]
            out += self.sk_w[:, j, None] * (np.einsum("nij,nj->ni", Rj, self.sk_pos)
                                             + c[self.sk_ids[:, j]])
        return out

    def local(self, th):
        """The bones' turns against the hand's (15 matrices) from the angles ``th``: per
        finger (MCP flex, MCP spread, PIP, DIP), the thumb's CMC (a turn vector), MCP, IP."""
        Q = np.zeros((5, 3, 3, 3))
        for f in range(1, 5):
            mcp, ab, pip, dip = th[4 * (f - 1):4 * f]
            q0 = hr.about(self.axes[f, 0], mcp) @ hr.about(self.n, ab)
            q1 = q0 @ hr.about(self.axes[f, 1], pip)
            q2 = q1 @ hr.about(self.axes[f, 2], dip)
            Q[f] = [q0, q1, q2]
        c = th[16:19]
        q0 = rotvec(c)
        q1 = q0 @ hr.about(self.axes[0, 1], th[19])
        q2 = q1 @ hr.about(self.axes[0, 2], th[20])
        Q[0] = [q0, q1, q2]
        return Q

    def fk(self, W, Rh, Q):
        """Each chain's points (5 x 4 x 3) for the wrist ``W``, the hand's turn ``Rh`` and
        the bones' turns ``Q`` (as neck_hand::place carries them)."""
        P = np.zeros((5, 4, 3))
        for f in range(5):
            at = W + Rh @ (self.heads[f, 0] - self.wr)
            P[f, 0] = at
            for k in range(3):
                at = at + (Rh @ Q[f, k]) @ (self.pts[f, k + 1] - self.pts[f, k])
                P[f, k + 1] = at
        return P

    def angles(self, W, Rh, P):
        """The anatomical angles (deg) of each finger's MCP, PIP, DIP from the posed chain."""
        out = np.zeros((5, 3))
        for f in range(1, 5):
            prev = Rh @ self.meta[f]
            for k in range(3):
                d = hr.norm(P[f, k + 1] - P[f, k])
                out[f, k] = np.degrees(np.arccos(np.clip(prev @ d, -1, 1)))
                prev = d
        return out


# ---- the neck ---------------------------------------------------------------------------

class Neck:
    def __init__(self, F):
        self.o = np.array(F["o"])
        self.E = np.array([F["ex"], F["ey"], F["ez"]])  # rows: the instrument's axes
        b = np.array(F["board"])
        self.by, self.bhw, self.btop = b[:, 0], b[:, 1], b[:, 2]
        self.depth = F["depth"]
        self.body = F["body"]
        self.nut_y = F["nut_y"]
        self.strings = [(np.array(s["bridge"]), np.array(s["nut"])) for s in F["strings"]]

    def loc(self, p):
        return (np.asarray(p) - self.o) @ self.E.T

    def world(self, q):
        return self.o + np.asarray(q) @ self.E

    def hw(self, y):
        return np.interp(y, self.by, self.bhw)

    def top(self, y):
        return np.interp(y, self.by, self.btop)

    def outside(self, p):
        """How far a world point is outside the neck's box (negative inside)."""
        q = self.loc(p)
        y = np.clip(q[..., 1], self.body - 0.5, self.nut_y + 0.4)
        dx = np.abs(q[..., 0]) - (self.hw(y) + 0.1)
        top = self.top(y)
        dz = np.maximum(q[..., 2] - top, (top - self.depth) - q[..., 2])
        dy = np.maximum(self.body - 0.5 - q[..., 1], q[..., 1] - self.nut_y - 0.4)
        d = np.stack([dx, dz, dy], -1)
        outside = np.linalg.norm(np.maximum(d, 0), axis=-1)
        inside = np.minimum(d.max(-1), 0)
        return outside + inside


# ---- the rules -------------------------------------------------------------------------

# The anatomical ranges (deg): the playing finger's and a hovering one's MCP, PIP, DIP.
# (r42:arcus-hands: the base joints straighter and the middle joints less bent — the owner,
# 2026-11-25: the fingers "short and stubby, curled tight near the board"; in the
# photographs the first phalanges rise high from the knuckles, nearly on the back's line.)
# (r42:arcus-hands, the coordinator on sheet hands-1: still hooks — in crop-08 and crop-11
# each finger is a smooth open arch, the last phalanx coming down nearly straight onto
# the string: the DIP 5-20 deg on a playing finger, the bend spread over MCP and PIP.)
# (r42:arcus-hands, pass 2: the PIP to 100 again with the tip within 28 deg of the normal —
# tips held to 25 deg folded every playing finger to the PIP's end, 110-112: the
# coordinator's "fingers hooking sharply"; the hand now stands higher, the knuckles over
# the fingerboard as in crop-11.)
PLAY = ((18, 70), (55, 100), (4, 25))
HOVER = ((12, 55), (35, 85), (3, 24))
# Each finger's spread at rest (rad): the index a little toward the scroll, the little
# finger a little away.
# (r42:arcus-hands: the little finger reaching out toward the bridge, the hand's span
# open as in crop-08 and crop-11; was 0.02 / 0.05.)
# (r42:arcus-hands, pass 2 — the coordinator on the 400 % tiles: "splayed wide apart, long
# and spidery"; in crop-08 and crop-11 the fingers sit close, nearly touching at their bases:
# the ring and the little finger brought back in, and every finger's spread held within
# SPREAD_MAX of its own — a hovering index had swung 50° off the middle finger to stay
# over its note.)
# (Measured, MakeHuman's rest hand fans its fingers 15.6, -10.2 and -18.6 degrees off the
# middle finger — index, ring, little; a turn of +0.1 rad fans a finger 5.8 degrees out the
# index's way: three quarters of the fan taken back, the fingers nearly parallel.)
# (r42:arcus-hands, pass 5 — the coordinator: "real fingers are never glued parallel"; in
# crop-11 light shows between the free fingers: each finger's fan 4, 3 and 5 deg wider than
# pass 2's — the index 7.9 deg off the middle, the ring 5.5, the little 9.7, neighbours 4-8
# apart; was -0.204, 0, 0.134, 0.243.)
SPREAD = np.array([-0.135, 0.0, 0.082, 0.157])
SPREAD_MAX = 8.0
# Each finger's angles near the middle of its range (deg; the owner's review: curled to
# the ranges' ends the fingers read short, a fist on the fingerboard).
MID = (36.0, 72.0, 10.0)
# Where the palm faces (the owner's photographs 2-4: toward the player as much as toward the
# neck, a little up), and how near to it it must keep.
def back_way(pose):
    """Where the back of the hand faces (the instrument's frame turned to the row's ways):
    the E side, turned down by the roll (the palm up toward the neck's underside — seen from
    the audience the back shows at a slant, the hand narrow, as photographs 2 and 5 and the
    front views in left_hand_refs.md show it) and toward the scroll by the yaw. The roll eases
    toward the G string, where the elbow swings in and the fingers reach further over."""
    s = pose.row["string"]
    roll = np.radians(ROLL[0] + (ROLL[1] - ROLL[0]) * s / 3.0)
    yaw = np.radians(YAW)
    return hr.norm(pose.kx * np.cos(roll) * np.cos(yaw) - pose.bw * np.cos(roll) * np.sin(yaw)
                   - pose.z * np.sin(roll))


def PALM_WAY(pose):
    return -back_way(pose)


# The back's roll down from the E side on the G string and the E string, its yaw toward the
# scroll (deg).
ROLL = (20.0, 26.0)
# (r42:arcus-hands: 10 — turned 30 deg toward the scroll the hand leant along the neck.)
YAW = 10.0
PALM_MIN = 0.985
PALM_W = 150.0
# Each finger's middle joint over the fingerboard's top (cm): index, middle, ring, little.
# (r42:arcus-hands: the photographs' median arch, 1.3 neck thicknesses, on the thinner neck —
# 2.03 cm deep; 3.5-3.8 held the knuckles 1.9 cm over the fingerboard's top.)
ARCHES = (2.7, 2.9, 2.8, 2.3)
# The playing finger's last phalanx within this of the fingerboard's normal (cos 25 deg; was 24 — with the last joint nearly straight, the middle joint takes the
# bend).
UPRIGHT_COS = 0.883
# (r42:arcus-hands, pass 2: 300 — at 150 the high rows let their tips lean 32-40 deg.)
# (r42:arcus-hands, pass 3: 600 — at 300 a third of the rows leant 30-34, the gate 30.)
UPRIGHT_W = 600.0
# Each first phalanx rising from its knuckle: within this of the normal (cos 50 deg; was 40 — the first phalanx leans over the board toward the string, as in
# crop-08 and crop-11, so the bend is spread over three joints, not folded at two).
RISE_COS = 0.64
# The wrist under the neck (cm across from the E edge, below the top).
# (r42:arcus-hands: the photographs' -1.7 … +2.0, as left_hand_refs.md has it; was 0 … 3.5.)
WRIST_X = (-1.7, 2.0)
# The wrist's deviation toward the little finger (deg): its range and where it rests.
# (r42:arcus-hands: 0-14, resting at 9 — the photographs' hand stands steeper than the
# forearm by 3-23 deg, median ~10; at most 6 the hand could not stand up from the neck;
# the gates hold the wrist within 16.)
WRIST_DEV = (0.0, 12.0)
WRIST_DEV_MID = 8.0
# The wrist's whole bend at the most (deg), low and high up (the gates: 16 low).
WRIST_TOTAL = (14.0, 28.0)
# (r42:arcus-hands, pass 3: its weight — at 2 the high rows bent 31-34, the gate 30.)
WRIST_TOTAL_W = 6.0
# How far a finger that does not play may hover from over its note (cm).
# (r42:arcus-hands, pass 2: looser — a finger close to its neighbours rather than over its
# own note; was 1.2, 1.4, 1.8, 2.6.)
HOVER_SLACK = (2.2, 2.2, 2.4, 3.0)
# r42:arcus-hands, pass 5 — the gap between neighbouring fingers' skins past the webs (cm,
# her hand): the playing finger stands apart from its neighbours, the free ones are never
# glued (crop-11's light between them). Measured on pass 4's table (finger_gap.py): the
# middle and ring fingers 0.9-2.1 mm apart, the playing finger nearer its neighbours (1.2-2.8)
# than the free ones each other (2.2-3.7) — the wrong way round. Each finger as capsules
# along its outer first phalanx, its middle and its last (radii GAP_R of its PIP's breadth,
# FINGER_W's, her hand's), and the rule's weight.
GAP_PLAY = 0.28
GAP_FREE = 0.15
GAP_W = 8.0
GAP_PIP = (1.62, 1.66, 1.55, 1.36)
GAP_R = (0.47, 0.45, 0.4)
# r42:arcus-hands, pass 3 — how high over the fingerboard's top a free fingertip may hover
# (cm, her hand; the strings stand 0.3-0.7 over it), and the rule's weight.
HOVER_UP = 1.7
HOVER_UP_W = 4.0
WRIST_Z = (-10.5, -5.0)
# How high the knuckles stand over the fingerboard's top, at least (cm).
KNUCKLES = (-0.8, 2.0)
# Her hand's length, wrist to the middle knuckles (cm, after knuckle_fix): the rules' unit.
HAND_LEN = 7.61
# How far out from the E edge the base knuckles stand (cm): beside the neck.
KNUCKLES_OUT = (0.5, 4.0)
# The index finger's base against the E side: out from the edge, and its height (cm).
CROOK_X = 0.85
# (r42:arcus-hands: 15 — at 5 the upright hand stood its index base 2.3 cm out.)
CROOK_W = 15.0
CROOK_Z = (-1.2, 0.3)
# r42:arcus-hands, pass 6 — the index's base joint at most this far out from the E edge
# (cm, her hand; the gate's INDEX_OUT 2.2 with a margin), and the rule's weight.
INDEX_OUT = 1.8
INDEX_OUT_W = 12.0
# r42:arcus-hands, pass 6 — how far (cm) the thumb's skin may show past the neck's far
# silhouette from the app's eye (a sliver of the pad; in first position the tip's end, as
# crop-08 shows it), a little more past the heel, and the rule's weight.
THUMB_PEEK = 0.15
THUMB_PEEK_FIRST = 0.6
THUMB_PEEK_HIGH = 0.2
THUMB_PEEK_W = 10.0
# How high each finger's middle joint stands over the fingerboard's top (cm).
ARCH = 2.4
# The thumb's flex (MCP, IP): relaxed.
THUMB = ((5, 35), (5, 35))
# The thumb's base (CMC) turn at most (deg).
THUMB_CMC = 30.0
# r42:arcus-hands — the hand's line (wrist to the middle knuckles) and the forearm's at
# least this steep to the neck, seen from the E side (deg; left_hand_refs.md).
# r42:arcus-hands — the upper arm this far below level, shoulder to elbow (deg; the
# photographs ~52; polish2's tests/arcus_arm_pose.rs holds it within 30-80).
# (r42:arcus-hands, pass 2: from 38 — polish3's arcus_arm_pose measured her viola's upper arm at
# 26° on screen where the table had it 32-36° in the room.)
UPPER_BELOW = (38.0, 60.0)
# r42:arcus-hands — the violin's upper bout as a slab (half-width, length back from the
# neck's root, cm) and the clearance the hand and forearm keep over its top plate.
BOUT = (8.5, 12.0)
CLEAR = 0.4
# (r42:arcus-hands, pass 3: 84 and 1.5 — at 78 and 0.6 the hand stood 62-68 deg to the neck in
# the picture on the A and E strings low down, the gate 68-92: the rule is the room's angle,
# the gate the picture's.)
HAND_UP = 84.0
HAND_UP_W = 1.5
FORE_UP = 64.0
# r42:arcus-hands — the skin kept out of the instrument as the app draws it
# (``hand_solid.Solid``: the neck's rounded back, the fingerboard, the strings, the body):
# the playing fingertip's pad may press its string by PAD_PRESS (the gate: 1 mm), the rest
# of the hand clears everything by SKIN_GAP, the forearm the body by SLEEVE (its sleeve).
PAD_PRESS = 0.07
SKIN_GAP = 0.12
SLEEVE = 0.35
PEN_W = 30.0
# r42:arcus-hands — high up the neck (the owner: "cuando se toca muy cerca del cuerpo del
# violín el músico suele subir la muñeca para no topar con el cuerpo"; crop-11, ref-02,
# ref-12, ref-40): past the neck's heel the thumb stays in the heel's curve, then creeps
# along the fingerboard's G side over the body; the wrist rises toward the top plate. HIGH
# is how far (cm) past the heel the first and second fingers' middle has gone when that is
# complete; HEEL how far up the neck from the body's edge the thumb's tip nests.
HEEL = 1.6
HIGH = 5.0
THUMB_HIGH_X = 0.45
# r42:arcus-hands, pass 7 — past the heel the thumb's tip may sit in the heel's curve under the
# neck (left_hand_refs.md §5: "under the neck's shoulder in 3rd"; from the heel on it stays
# there, under the neck and hidden from the audience, while the fingers go on over the body —
# beside the fingerboard over the body there is no room for it over the top plate): its tip
# may drop THUMB_DROP_HIGH cm further under the fingerboard's top and come THUMB_UNDER_HIGH of
# the half width in from the G side (at the hand's full ``high``; the gate's bounds from the
# 12th semitone, arcus_hand_table).
THUMB_DROP_HIGH = 2.2
THUMB_UNDER_HIGH = 0.75
# Past the heel the thumb moves this share of the fingers' way (it lags them, then leaves
# the heel's curve for the fingerboard's G side over the body).
THUMB_LAG = 0.5
# High up: how much the knuckles and the crook rise (cm), how much freer the palm's way
# (deg), how much further out from the E edge the wrist may go (cm: beside the upper bout's
# rib, ref-40).
# (r42:arcus-hands, pass 3: 0.6 — at 1.4 the knuckles stood 2.6-3.4 cm over the board high up;
# the photographs' high positions put them lower against the
# neck, never higher — ref-02 -1.5 neck thicknesses, crop-02 below the fingerboard's edge —:
# the rise the owner asks for near the body is the wrist's, WRIST_Z_HIGH; the gate's
# KNUCKLES_UP allows the photographs' whole span, to 3.4.)
HIGH_RISE = 0.6
# The wrist's bend toward the palm (negative) or the back (positive), deg: low and high up.
# High up the hand and the forearm may lean further from upright (HAND_UP_HIGH,
# FORE_UP_HIGH deg less at the most).
WRIST_FLEX = (-10.0, 10.0)
WRIST_FLEX_HIGH = (-20.0, 30.0)
HAND_UP_HIGH = 25.0
FORE_UP_HIGH = 35.0
PALM_FREE_HIGH = 70.0
WRIST_X_HIGH = 5.0
WRIST_Z_HIGH = (-7.0, -1.5)
SOLIDS = {}


def solid_of(F):
    key = (F["who"], F["inst"])
    if key not in SOLIDS:
        SOLIDS[key] = hand_solid.Solid(F)
    return SOLIDS[key]


def over(x, lo, hi):
    return np.maximum(lo - x, 0) + np.maximum(x - hi, 0)


class Pose:
    """One row's problem: the hand ``h``, the neck, the figure's arm and the targets."""

    def __init__(self, h, neck, F, row, R0):
        self.h, self.neck, self.row, self.R0 = h, neck, row, R0
        self.sh = np.array(F["shoulder"])
        self.upper, self.fore = F["upper"], F["fore"]
        self.a = np.array(row["a"])
        self.bw, self.kx, self.z = (np.array(row[k]) for k in ("bw", "kx", "z"))
        self.hw, self.top = row["hw"], row["top"]
        # The hand's size against hers (the rules' lengths are hers): the man's hand is a
        # quarter longer, its arches higher, its knuckles further out, its wrist lower.
        self.k = np.linalg.norm((h.heads[2, 0] + h.heads[3, 0]) * 0.5 - h.wr) / HAND_LEN
        self.tips = np.array(row["tips"])
        self.playing = row["playing"]
        self.want_names = False
        self.solid = solid_of(F)
        self.eye = np.array(F["eye"])
        self.pen_w = PEN_W
        # r42:arcus-hands, pass 7 — how much freer the palm's way is (deg; blend_fix.py
        # --rules frees it where the gate's outline rule holds the hand edge-on instead).
        self.palm_free = 0.0
        # How far the hand is past the neck's heel, 0 (at or below it) to 1 (HIGH beyond).
        y12 = neck.loc(self.tips[0])[1] * 0.6 + neck.loc(self.tips[1])[1] * 0.4
        self.y12 = y12
        self.yh = neck.body + HEEL
        self.high = float(np.clip((self.yh - y12) / HIGH, 0.0, 1.0))

    def peek(self, p):
        """r42:arcus-hands, pass 6 — how far (cm) the point ``p`` stands past the neck's
        far (G) silhouette as the app's eye sees it: in the neck's cross-section at ``p``,
        across the eye's way, beyond the section's farthest point (the fingerboard's and the
        neck's polygons as drawn)."""
        q = self.neck.loc(p)
        e = self.neck.loc(self.eye)
        d = np.array([e[0] - q[0], e[2] - q[2]])
        d /= max(np.linalg.norm(d), 1e-6)
        n = np.array([-d[1], d[0]])
        if n[0] > 0:
            n = -n
        far = -1e9
        for ys, polys in (self.solid.board, self.solid.shaft):
            y = np.clip(q[1], ys[0], ys[-1])
            i = int(np.clip(np.searchsorted(ys, y), 1, len(ys) - 1))
            t = np.clip((y - ys[i - 1]) / max(ys[i] - ys[i - 1], 1e-6), 0, 1)
            poly = polys[i - 1] * (1 - t) + polys[i] * t
            far = max(far, float((poly @ n).max()))
        return float(np.array([q[0], q[2]]) @ n) - far

    def peek_most(self):
        """How far the thumb's skin may show past the neck's far side here (cm): crop-08's
        first position shows its tip's end over the neck (~0.6 cm), crop-11's none, crop-02's
        third and high positions none."""
        at = self.row["at"]
        first = float(np.clip((3.0 - at) / 2.0, 0.0, 1.0))
        return THUMB_PEEK + (THUMB_PEEK_FIRST - THUMB_PEEK) * first + THUMB_PEEK_HIGH * self.high

    def unpack(self, x):
        W = x[:3]
        Rh = rotvec(x[3:6]) @ self.R0
        Q = self.h.local(x[6:])
        return W, Rh, Q

    def elbow(self, x, W, Rh, P):
        """The elbow, the forearm's way from the wrist and the wrist's deviation: the forearm
        on the hand's line turned toward the little finger's side by ``x[27]`` (the
        photographs' hand stands steeper to the neck than the forearm, 5-25 degrees), its own
        length back."""
        dev = x[27] if len(x) > 27 else 0.0
        mid = (P[2, 0] + P[3, 0]) * 0.5
        base = hr.norm(W - mid)
        across = P[1, 0] - P[4, 0]
        xp = hr.norm(across - base * (across @ base))
        way = hr.norm(base * np.cos(dev) - xp * np.sin(dev))
        # r42:arcus-hands — and bent at the wrist toward the palm or the back by ``x[28]``
        # (positive: the forearm toward the palm's side, the hand bent back): high up the
        # neck the forearm leans along the instrument from an elbow the upper arm can reach
        # while the hand stands up over the body's shoulder.
        if len(x) > 28:
            n = Rh @ self.h.n
            n = hr.norm(n - way * (n @ way))
            way = hr.norm(way * np.cos(x[28]) + n * np.sin(x[28]))
        return W + way * self.fore, way, dev

    def residuals(self, x, prev=None):
        h, nk = self.h, self.neck
        W, Rh, Q = self.unpack(x)
        P = h.fk(W, Rh, Q)
        ang = h.angles(W, Rh, P)
        r = []
        names = []
        # Points kept clear of the neck: (name, weight, the gap, the point), measured at once.
        probe = []

        def add(name, *v):
            r.extend(float(x) for x in v)
            names.extend([name] * len(v))
        loc = nk.loc
        p = self.playing
        # Fingers: the playing tip on its stop, the others over their notes.
        for f in range(4):
            tip = P[f + 1, 3]
            if f == p:
                add(f"tip{f}", *((tip - self.tips[f]) * 40.0))
                d2 = hr.norm(P[f + 1, 3] - P[f + 1, 2])
                add(f"upright{f}", UPRIGHT_W * max(0.0, UPRIGHT_COS - d2 @ (-self.z)))
            else:
                # Hovering over its own string a little across from the playing one (each
                # finger falls toward its own string, at its own angle — photograph 4).
                want = self.tips[f] + self.kx * (0.35 * (f - max(p, 0)))
                miss = np.linalg.norm(tip - want)
                add(f"hover{f}", 3.0 * max(0.0, miss - HOVER_SLACK[f] * self.k))
                # r42:arcus-hands, pass 3 — and close over the strings (the coordinator on the
                # first position: the raised index floated far above them; in crop-08 and
                # crop-02 the free fingers hover curved a centimetre or so over the strings).
                qh = loc(tip)
                add(f"hover_up{f}", HOVER_UP_W * max(0.0, qh[2] - nk.top(qh[1]) - HOVER_UP * self.k))
            rng = PLAY if f == p else HOVER
            for k in range(3):
                wk = 3.0 if f == p else 1.0
                add(f"ang{f}{k}", wk * over(ang[f + 1, k], *rng[k]))
                add(f"mid{f}{k}", 0.08 * (ang[f + 1, k] - MID[k]))
            # r42:arcus-hands, pass 5 — apart from the next finger (GAP_PLAY, GAP_FREE).
            if f < 3:
                segs = []
                for g in (f, f + 1):
                    Pg = P[g + 1]
                    segs.append([(Pg[0] * 0.5 + Pg[1] * 0.5, Pg[1], GAP_R[0] * GAP_PIP[g]),
                                 (Pg[1], Pg[2], GAP_R[1] * GAP_PIP[g]),
                                 (Pg[2], Pg[3], GAP_R[2] * GAP_PIP[g])])
                gap = min(seg_dist(a0, a1, b0, b1) - ra * self.k - rb * self.k
                          for a0, a1, ra in segs[0] for b0, b1, rb in segs[1])
                need = (GAP_PLAY if p in (f, f + 1) else GAP_FREE) * self.k
                add(f"gap{f}", GAP_W * max(0.0, need - gap))
            sp = x[6 + 4 * f + 1] - SPREAD[f]
            add(f"spread{f}", 12.0 * sp + 2.0 * over(np.degrees(sp), -SPREAD_MAX, SPREAD_MAX))
            # The arch: the middle joint high over the fingerboard.
            ql = loc(P[f + 1, 1])
            add(f"arch{f}", 6.0 * max(0.0, nk.top(ql[1]) + ARCHES[f] * self.k - ql[2]))
            d1 = hr.norm(P[f + 1, 1] - P[f + 1, 0])
            add(f"rise{f}", 3.0 * max(0.0, RISE_COS - d1 @ self.z))
            # Nothing but the playing tip inside the fingerboard and the strings.
            for k in (1, 2, 3):
                if f == p and k == 3:
                    continue
                for t in (0.5, 1.0):
                    probe.append((f"inboard{f}{k}", 4.0, 0.75,
                                  P[f + 1, k - 1] * (1 - t) + P[f + 1, k] * t))
        # The crook: the index finger's base against the neck's E side, just under the
        # fingerboard's top.
        base = P[1, 0] * 0.7 + P[1, 1] * 0.3
        qb = loc(base)
        add("crook_x", CROOK_W * (qb[0] - (nk.hw(qb[1]) + CROOK_X)))
        # (r42:arcus-hands, pass 6: and the index's base joint itself within INDEX_OUT of the
        # E edge, as arcus_hand_table measures it — the crook's point a third up the first
        # phalanx stood at 0.85 while the joint, the hand upright, stood 2.2-3.4 cm out on
        # the viola's A and E strings low down.)
        qi = loc(P[1, 0])
        add("index_out", INDEX_OUT_W * over(qi[0] - nk.hw(qi[1]), -1.0, INDEX_OUT * self.k))
        # (r42:arcus-hands: high up, over the body, the knuckles and the crook rise with the
        # hand over the upper bout's shoulder — HIGH_RISE at the most.)
        rise = HIGH_RISE * self.high
        add("crook_z", 3.0 * over(qb[2] - nk.top(qb[1]), CROOK_Z[0] + rise, CROOK_Z[1] + rise))
        # The knuckles up beside the fingerboard (the owner's photograph 2: the hand rises
        # from the wrist just under the neck to the knuckles over it).
        for f in range(1, 5):
            qk = loc(P[f, 0])
            add("knuckles", 4.0 * over(qk[2] - nk.top(qk[1]), KNUCKLES[0] + rise, KNUCKLES[1] + rise))
            add("knuckles_out", 3.0 * over(qk[0] - nk.hw(qk[1]), KNUCKLES_OUT[0], KNUCKLES_OUT[1] * self.k))
        # The thumb up the G side, its tip about level with the fingerboard, opposite the
        # first and second fingers.
        # (r42:arcus-hands: past the heel the thumb stays in its curve, then creeps along
        # the fingerboard's G side over the body, closer to the edge and a little higher.)
        qt = loc(P[0, 3])
        hi = self.high
        add("thumb_x", 4.0 * (1.0 - THUMB_UNDER_HIGH * hi)
            * (qt[0] + nk.hw(qt[1]) + 0.65 + (THUMB_HIGH_X - 0.65) * hi))
        add("thumb_z", 6.0 * over(qt[2] - nk.top(qt[1]), -0.6 + (0.2 - THUMB_DROP_HIGH) * hi,
                                  0.3 + 0.3 * hi))
        y12 = self.y12
        yt = y12 if y12 >= self.yh else self.yh - THUMB_LAG * (self.yh - y12)
        add("thumb_y", 4.0 * over(qt[1] - yt, -0.8, 1.2))
        # (r42:arcus-hands, pass 6 — the coordinator at 400 %: in third position the
        # thumb's tip peeked out past the neck's far side, a stray fingertip; crop-02 and
        # crop-11 show the thumb under the neck, at most a sliver of its pad beside the
        # edge. Seen from the app's eye, the thumb's last joint and tip, with their skin,
        # stand no more than THUMB_PEEK past the neck's far silhouette.)
        for k_, r_ in ((2, 0.75), (3, 0.6)):
            add(f"thumb_peek{k_}", THUMB_PEEK_W * max(0.0, self.peek(P[0, k_]) + r_ * self.k
                                                         - self.peek_most()))
        # (and, hidden, still the gate's thumb: its tip no lower than 1.3 cm under the
        # fingerboard's top and on the G side's half — arcus_hand_table's −1.6 and −hw/2.)
        add("thumb_floor", 20.0 * max(0.0, nk.top(qt[1]) - 1.3 - THUMB_DROP_HIGH * hi - qt[2]))
        add("thumb_side", 10.0 * max(0.0, qt[0] + nk.hw(qt[1]) * (0.5 - THUMB_UNDER_HIGH * hi) + 0.2))
        # The thumb's base turned no more than a relaxed hand's (r42:arcus-hand: turned 65
        # degrees it pushed the thumb's ball out into a mitten's bulge — the owner's "parte
        # abultada gorda").
        add("thumb_cmc", 3.0 * over(np.degrees(np.linalg.norm(x[22:25])), 0.0, THUMB_CMC))
        add("thumb_mcp", 0.3 * over(np.degrees(x[25]), *THUMB[0]))
        add("thumb_ip", 0.3 * over(np.degrees(x[26]), *THUMB[1]))
        # The thumb and the palm clear of the neck (the gap under the palm).
        for k in (1, 2):
            for t in (0.5, 1.0):
                probe.append(("thumb_clear", 4.0, 0.8, P[0, k - 1] * (1 - t) + P[0, k] * t))
        npalm = Rh @ h.n
        for f in (1, 2, 3, 4):
            for t in (0.45, 0.8):
                probe.append(("palm_gap", 3.0, 0.4, W + (P[f, 0] - W) * t + npalm * 1.1))
        out = nk.outside(np.array([q for *_, q in probe]))
        for (name, w, gap, _), o in zip(probe, out):
            add(name, w * max(0.0, gap - o))
        # The wrist straight: the elbow on the hand's line at the forearm's length, the upper
        # arm its own length from the shoulder; under the violin, the forearm and the neck a
        # narrow V.
        el, way, dev = self.elbow(x, W, Rh, P)
        # r42:arcus-hands — the hand and the forearm standing up from the neck as the
        # photographs' front views show them (left_hand_refs.md: the hand's line 70-89 deg to
        # the neck, the forearm's 50-85; the table before this pass: 53-55 and 48 — the
        # hand leant toward the scroll, its fingers crossing the board slantwise, a ring).
        hd = hr.norm((P[2, 0] + P[3, 0]) * 0.5 - W)
        up_h = np.degrees(np.arctan2(hd @ self.z, hd @ (-self.bw)))
        add("hand_up", HAND_UP_W * over(up_h, HAND_UP - HAND_UP_HIGH * self.high, 100.0))
        up_f = np.degrees(np.arctan2(-way @ self.z, -way @ (-self.bw)))
        add("fore_up", 0.4 * over(up_f, FORE_UP - FORE_UP_HIGH * self.high, 100.0))
        if len(x) > 28:
            fl = np.degrees(x[28])
            lo, hi_ = (WRIST_FLEX[i] + (WRIST_FLEX_HIGH[i] - WRIST_FLEX[i]) * self.high for i in (0, 1))
            add("wrist_flex", 1.5 * over(fl, lo, hi_) + 0.04 * fl)
        add("wrist_dev", 1.5 * over(np.degrees(dev), *WRIST_DEV) + 0.05 * (np.degrees(dev) - WRIST_DEV_MID))
        # (r42:arcus-hands, pass 2: the wrist's whole bend, the forearm against the hand's
        # line, as arcus_hold and arcus_hand_table measure it — the sideways turn and the
        # bend toward the back together had reached 21 deg.)
        bend = np.degrees(np.arccos(np.clip(way @ -hd, -1, 1)))
        add("wrist_total", WRIST_TOTAL_W * over(bend, 0.0, WRIST_TOTAL[0] + (WRIST_TOTAL[1] - WRIST_TOTAL[0]) * self.high))
        add("upper_len", 5.0 * over(np.linalg.norm(el - self.sh) - self.upper, -1.0, 1.0))
        # r42:arcus-hands — the upper arm hanging from the shoulder to an elbow under the
        # violin, not out to the side like a wing (the owner: "ese brazo aún se ve raro";
        # ref-10: ~52 deg below level; the table had hers at 2 and his at 26).
        # r42:arcus-hands — the hand and the forearm clear of the violin's body (the owner:
        # high up, "se solapa la mano y antebrazo con el cuerpo"): their points stay over
        # the top plate's height by CLEAR where they lie over the upper bout (the body as a
        # slab: BOUT cm across, from the neck's root back), so in high positions the wrist
        # rises over the bout as players' does.
        # (r42:arcus-hands: the slab is replaced by the body as drawn — "pen" below.)
        sk = h.skin(W, Rh, Q, P, way)
        q = self.solid.loc(sk)
        dsol = self.solid.local_sd(q)
        dstr = self.solid.strings_d(q)
        pad = (h.sk_top == O + hr.finger(p + 1, 2)) if p >= 0 else np.zeros(len(q), bool)
        fore = h.sk_kind == "fore"
        need = np.where(pad, -PAD_PRESS, np.where(fore, SLEEVE, SKIN_GAP))
        dd = np.where(pad | fore, dsol, np.minimum(dsol, dstr))
        add("pen", *(self.pen_w * np.maximum(0.0, need - dd)))
        ua = el - self.sh
        below = np.degrees(np.arcsin(np.clip(-ua[1] / max(np.linalg.norm(ua), 1e-6), -1, 1)))
        add("elbow_down", 6.0 * over(below, *UPPER_BELOW))
        qe = loc(el)
        add("elbow_y", 1.0 * over(qe[1], 2.0, nk.body - 2.0))
        add("elbow_z", 1.0 * max(0.0, qe[2] + 13.0))
        # The palm toward the neck and up toward the player.
        # (r42:arcus-hands: high up the hand turns round the body's shoulder, its palm freer.)
        pmin = np.cos(np.arccos(PALM_MIN) + np.radians(PALM_FREE_HIGH) * self.high
                      + np.radians(self.palm_free))
        add("palm_way", PALM_W * max(0.0, pmin - npalm @ PALM_WAY(self)))
        qw = loc(W)
        add("wrist_x", 3.0 * over(qw[0] - nk.hw(qw[1]), WRIST_X[0], WRIST_X[1] + WRIST_X_HIGH * self.high))
        wz = [(WRIST_Z[i] + (WRIST_Z_HIGH[i] - WRIST_Z[i]) * hi) * self.k for i in (0, 1)]
        add("wrist_z", 3.0 * over(qw[2] - nk.top(qw[1]), *wz))
        if prev is not None:
            add("prev", *((x - prev) * 0.15))
        if self.want_names:
            return names
        return np.array(r, float)


def seg_dist(a0, a1, b0, b1):
    """The nearest distance between segments a0-a1 and b0-b1."""
    d1, d2, r = a1 - a0, b1 - b0, a0 - b0
    a, e, f = d1 @ d1, d2 @ d2, d2 @ r
    c, b = d1 @ r, d1 @ d2
    den = a * e - b * b
    s_ = np.clip((b * f - c * e) / den, 0.0, 1.0) if den > 1e-9 else 0.0
    t = np.clip((b * s_ + f) / e, 0.0, 1.0) if e > 1e-9 else 0.0
    s_ = np.clip((b * t - c) / a, 0.0, 1.0) if a > 1e-9 else 0.0
    return float(np.linalg.norm(a0 + d1 * s_ - (b0 + d2 * t)))


def lm(fun, x0, iters=80, lam=1e-2):
    """Levenberg-Marquardt with a finite-difference Jacobian."""
    x = x0.copy()
    r = fun(x)
    cost = r @ r
    for _ in range(iters):
        J = np.zeros((len(r), len(x)))
        for i in range(len(x)):
            e = 1e-4 if i >= 3 else 1e-3
            dx = x.copy()
            dx[i] += e
            J[:, i] = (fun(dx) - r) / e
        A = J.T @ J
        g = J.T @ r
        while True:
            step = np.linalg.solve(A + lam * np.diag(np.diag(A) + 1e-6), -g)
            xn = x + step
            rn = fun(xn)
            cn = rn @ rn
            if cn < cost:
                x, r, cost = xn, rn, cn
                lam = max(lam * 0.3, 1e-7)
                break
            lam *= 4.0
            if lam > 1e8:
                return x, cost
        if np.linalg.norm(step) < 1e-6:
            break
    return x, cost


# ---- solving every row -----------------------------------------------------------------

def start(h, row, high=False):
    """A first guess: the knuckles along the neck's E side, the hand rising from under it,
    the fingers half curled. ``high`` (r42:arcus-hands): the hand rising over the body's
    shoulder from a wrist beside the upper bout's E rib (ref-40), the knuckles over the
    fingerboard's E edge."""
    a, bw, kx, z = (np.array(row[k]) for k in ("a", "bw", "kx", "z"))
    tips = np.array(row["tips"])
    along = (tips[0] - a) @ bw
    k_idx = a + bw * (along - 1.6) + kx * (row["hw"] + 1.3) - z * 0.3
    rx = h.heads[1, 0] - h.heads[4, 0]
    rest_mid = (h.heads[2, 0] + h.heads[3, 0]) * 0.5
    ry = rest_mid - h.wr
    py = hr.norm(z * 0.9 + kx * 0.35)
    if high:
        k_idx = a + bw * (along - 1.2) + kx * (row["hw"] + 0.9) + z * 0.8
        py = hr.norm(z - kx * 0.6)
    R0 = hr.frame(-bw, py) @ hr.frame(rx, ry).T
    mid = k_idx + bw * 2.4
    W = mid - R0 @ (rest_mid - h.wr)
    th = np.zeros(21)
    for f in range(1, 5):
        th[4 * (f - 1):4 * f] = [40 * RAD - h.rest[f, 0], SPREAD[f - 1],
                                 75 * RAD - h.rest[f, 1], 12 * RAD - h.rest[f, 2]]
    th[19:21] = [15 * RAD, 15 * RAD]
    return R0, np.concatenate([W, np.zeros(3), th, [WRIST_DEV_MID * RAD, 0.0]])


def entry(pose, x, cost):
    """A solved row as the cache keeps it (the forearm's way from the wrist too)."""
    W, Rh, Q = pose.unpack(x)
    P = pose.h.fk(W, Rh, Q)
    _, way, _ = pose.elbow(x, W, Rh, P)
    return {"W": W.tolist(), "Rh": Rh.tolist(), "Q": Q.reshape(15, 3, 3).tolist(),
            "fore": way.tolist(), "cost": float(cost), "x": x.tolist()}


def rows_of(F, string, finger):
    return [r for r in F["rows"] if r["string"] == string and r["finger"] == finger]


def settle(pose, fun, x0, iters):
    """r42:arcus-hands — LM in two steps: the instrument's walls soft first (a thumb or a
    finger started inside the neck slides out where its rules want it, not to the nearest
    wall), then at their full weight."""
    pose.pen_w = PEN_W * 0.05
    x, _ = lm(fun, x0, iters=max(10, iters // 3))
    pose.pen_w = PEN_W
    return lm(fun, x, iters=iters)


def solve_fig(F, h, cache, verbose=False, save=None, seed=None):
    """Every row of one figure on one instrument. Each row is solved from several starts —
    the same finger a position lower, the middle finger at this position, the same finger on
    the string below, a fresh guess and two moved off it — and the best kept; a row still
    poor is tried again from the best moved at random (seeded: the table is the same each
    run). Each row is held near the one a position lower, so the table blends smoothly."""
    neck = Neck(F)
    key = f'{F["who"]}-{F["inst"]}'
    R0, _ = start(h, rows_of(F, 0, 1)[0])
    rng = np.random.default_rng(42)
    got = {}
    for string in STRINGS:
        for finger in (1, 0, 2, 3, 4):
            prev = None
            for row in rows_of(F, string, finger):
                at = row["at"]
                pose = Pose(h, neck, F, row, R0)
                starts = []
                # (r42:arcus-hands: ``--seed=<dir>``: the row's own earlier solution first, so a
                # re-solve after a small change of rules or geometry is one settle a row.)
                sk = f'{key}-{string}-{finger}-{at:g}'
                if seed and sk in seed:
                    sx = np.array(seed[sk]["x"])
                    if len(sx) < 29:
                        sx = np.concatenate([sx, np.zeros(29 - len(sx))])
                    starts.append(sx)
                if prev is not None:
                    starts.append(prev)
                for k in ((string, 1, at), (string - 1, finger, at)):
                    if k in got:
                        starts.append(got[k])
                R1, x0 = start(h, row)
                x0[3:6] = rotlog(R1 @ R0.T)
                for dz in (0.0,) if starts else (0.0, -0.8, 0.8):
                    xs = x0.copy()
                    xs[:3] += np.array(row["z"]) * dz
                    starts.append(xs)
                if pose.high > 0.2:
                    R1, x0 = start(h, row, high=True)
                    x0[3:6] = rotlog(R1 @ R0.T)
                    starts.append(x0)
                fun = (lambda x: pose.residuals(x, prev))
                best = None
                for x_ in starts:
                    x, cost = settle(pose, fun, x_, 45)
                    if best is None or cost < best[1]:
                        best = (x, cost)
                    # (r42:arcus-hands: a row solved well from the one a position lower is
                    # kept — the table blends smoothly and a solve is ~10 s, not ~45.)
                    if best[1] < GOOD:
                        break
                tries = 0
                while best[1] > 250.0 and tries < 6:
                    xs = best[0].copy()
                    xs[:3] += rng.normal(0, 0.8, 3)
                    xs[3:6] += rng.normal(0, 0.15, 3)
                    xs[6:] += rng.normal(0, 0.12, len(xs) - 6)
                    x, cost = settle(pose, fun, xs, 60)
                    if cost < best[1]:
                        best = (x, cost)
                    tries += 1
                x, cost = best
                got[(string, finger, at)] = x
                prev = x
                cache[f'{key}-{string}-{finger}-{at:g}'] = entry(pose, x, cost)
                if save:
                    save(cache)
                if verbose:
                    print(key, string, finger, at, "cost %.2f" % cost, flush=True)


# The strings a process solves (``--strings=0,1``: a figure's table in two processes).
STRINGS = [0, 1, 2, 3]
# A row's cost under which no further start is tried.
GOOD = 30.0
# Positions `fix` takes again whatever their cost (`--ats=5,7,9`), and its threshold
# (`--worse=250`).
FORCE_AT = []
# Rows `fix` takes again whatever their cost (`--keys=0-3-1,0-3-3`: string-finger-position).
FORCE = []


def fix_fig(F, h, cache, worse=60.0, verbose=False, tries=8, save=None):
    """Rows solved poorly (cost over ``worse``) tried again: from the row a position lower
    and the one higher, a fresh guess and eight moves off the best at random (seeded); the
    best kept if better."""
    neck = Neck(F)
    key = f'{F["who"]}-{F["inst"]}'
    R0, _ = start(h, rows_of(F, 0, 1)[0])
    rng = np.random.default_rng(7)
    ats = sorted({r["at"] for r in F["rows"]})
    for row in F["rows"]:
        if row["string"] not in STRINGS:
            continue
        k = f'{key}-{row["string"]}-{row["finger"]}-{row["at"]:g}'
        c = cache[k]
        if c["cost"] <= worse and not (FORCE and any(k.endswith(x) for x in FORCE))                 and row["at"] not in FORCE_AT:
            continue
        i = ats.index(row["at"])
        near = [cache.get(f'{key}-{row["string"]}-{row["finger"]}-{a:g}') for a in
                (ats[i - 1] if i > 0 else None, ats[i + 1] if i + 1 < len(ats) else None)
                if a is not None]
        prev = np.array(near[0]["x"]) if i > 0 and near else None
        pose = Pose(h, neck, F, row, R0)
        fun = (lambda x: pose.residuals(x, prev))
        best = (np.array(c["x"]), float(fun(np.array(c["x"])) @ fun(np.array(c["x"]))))
        starts = [np.array(n["x"]) for n in near if n is not None]
        R1, x0 = start(h, row)
        x0[3:6] = rotlog(R1 @ R0.T)
        starts.append(x0)
        if pose.high > 0.2:
            R1, x0 = start(h, row, high=True)
            x0[3:6] = rotlog(R1 @ R0.T)
            starts.append(x0)
        for x_ in starts:
            x, cost = settle(pose, fun, x_, 70)
            if cost < best[1]:
                best = (x, cost)
        for _ in range(tries):
            xs = best[0].copy()
            xs[:3] += rng.normal(0, 0.8, 3)
            xs[3:6] += rng.normal(0, 0.2, 3)
            xs[6:] += rng.normal(0, 0.15, len(xs) - 6)
            x, cost = settle(pose, fun, xs, 70)
            if cost < best[1]:
                best = (x, cost)
        x, cost = best
        if verbose:
            print(k, "cost %.1f -> %.1f" % (c["cost"], cost), flush=True)
        # (r42:arcus-hands: a forced row is kept whatever its old cost — the instrument may
        # have changed under it, its old cost no longer its own.)
        if cost < c["cost"] or row["at"] in FORCE_AT or any(k.endswith(x) for x in FORCE):
            cache[k] = entry(pose, x, cost)
            if save:
                save(cache)


def rotlog(R):
    """The turn vector of a rotation."""
    c = np.clip((np.trace(R) - 1) * 0.5, -1, 1)
    a = np.arccos(c)
    if a < 1e-6:
        return np.zeros(3)
    v = np.array([R[2, 1] - R[1, 2], R[0, 2] - R[2, 0], R[1, 0] - R[0, 1]])
    return v / (2 * np.sin(a)) * a


def write_bin(dump, cache, out=OUT):
    at = dump["at"]
    b = bytearray(b"CLHT")
    b += struct.pack("<HH", 2, len(at))
    b += struct.pack(f"<{len(at)}f", *at)
    figs = {f'{F["who"]}-{F["inst"]}': F for F in dump["figs"]}
    hands = {}
    for who in ("woman", "man"):
        for inst in ("violin", "viola"):
            F = figs[f"{who}-{inst}"]
            E = np.array([F["ex"], F["ey"], F["ez"]])
            o = np.array(F["o"])
            for string in range(4):
                for finger in range(5):
                    for a in at:
                        c = cache[f"{who}-{inst}-{string}-{finger}-{a:g}"]
                        w = E @ (np.array(c["W"]) - o)
                        H = E @ np.array(c["Rh"])
                        vals = [int(round(v * 100)) for v in w]
                        fore = c.get("fore")
                        if fore is None:
                            # (A cache from before the forearm's way: on the hand's line.)
                            h = hands.setdefault(who, Hand(who))
                            P = h.fk(np.array(c["W"]), np.array(c["Rh"]),
                                     np.array(c["Q"]).reshape(5, 3, 3, 3))
                            fore = hr.norm(np.array(c["W"]) - (P[2, 0] + P[3, 0]) * 0.5)
                        fw = E @ np.array(fore)
                        vals += [int(round(v * 32767)) for v in fw / np.linalg.norm(fw)]
                        for R in [H] + [np.array(q) for q in c["Q"]]:
                            vals += [int(round(v * 32767)) for v in quat(R)]
                        b += struct.pack("<70h", *vals)
    open(out, "wb").write(bytes(b))
    print("wrote", out, len(b), "bytes")


# ---- looking at a row ------------------------------------------------------------------

STUFF = {0: (0.86, 0.66, 0.55), 1: (0.75, 0.42, 0.40), 3: (0.05, 0.04, 0.05),
         4: (0.10, 0.10, 0.13), 5: (0.92, 0.92, 0.90), 11: (0.93, 0.76, 0.70)}


def hand_mesh(h, W, Rh, Q, P, way=None):
    """The left hand and forearm skinned (the forearm on ``way`` from the wrist when given,
    else carried straight on with the hand)."""
    fig = h.fig
    xf = {HAND: (Rh, W, h.wr)}
    Rf = Rh
    if way is not None:
        base = hr.norm(W - (P[2, 0] + P[3, 0]) * 0.5)
        Rf = hr.between(base, np.asarray(way)) @ Rh
    for b in (O + hr.FORE1, O + hr.FORE2):
        xf[b] = (Rf, W, h.wr)
    for f in range(5):
        for k in range(3):
            xf[O + hr.finger(f, k)] = (Rh @ Q[f, k], P[f, k], h.heads[f, k])
    pos, tot = hr.skin_hand(fig, 0, xf)
    ok = tot > 0.6
    pos = np.where(ok[:, None], pos / np.maximum(tot, 1e-6)[:, None], 0)
    t = fig["tris"]
    keep = ok[t].all(1)
    det = fig["det"]
    col = np.zeros((len(pos), 3))
    for i, m in zip(t[keep].ravel(), np.repeat(fig["mat"][keep], 3)):
        col[i] = STUFF.get(int(m), (0.5, 0.5, 0.5))
    tone = det[:, :1] * (1 + det[:, 1:2] * np.array([0.22, -0.12, -0.18]))
    return pos, t[keep], np.clip(col * tone, 0, 1)


def neck_mesh(neck, F):
    """The neck and the fingerboard as boxes along their length, the strings as thin boxes,
    a slab of the body: enough to judge a grip."""
    pts, tris, cols = [], [], []

    def box(corners, c):
        n0 = len(pts)
        pts.extend(corners)
        for f in ([0, 1, 3], [0, 3, 2], [4, 6, 7], [4, 7, 5], [0, 4, 5], [0, 5, 1], [2, 3, 7],
                  [2, 7, 6], [0, 2, 6], [0, 6, 4], [1, 5, 7], [1, 7, 3]):
            tris.append([n0 + i for i in f])
        cols.extend([c] * 8)

    ys = np.linspace(neck.body - 0.5, neck.nut_y + 0.4, 14)
    for y0, y1 in zip(ys[:-1], ys[1:]):
        c = []
        for y in (y0, y1):
            hw, top = neck.hw(y), neck.top(y)
            for x in (-hw - 0.1, hw + 0.1):
                for z in (top - neck.depth, top - 0.55):
                    c.append(neck.world([x, y, z]))
        box([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]], (0.48, 0.25, 0.12))
        c = []
        for y in (y0, y1):
            hw, top = neck.hw(y), neck.top(y)
            for x in (-hw, hw):
                for z in (top - 0.55, top):
                    c.append(neck.world([x, y, z]))
        box(c, (0.06, 0.05, 0.05))
    for br, nu in neck.strings:
        lb, ln = neck.loc(br), neck.loc(nu)
        c = []
        for q in (lb, ln):
            for dx in (-0.04, 0.04):
                for dz in (-0.04, 0.04):
                    c.append(neck.world(q + np.array([dx, 0, dz])))
        box(c, (0.82, 0.80, 0.74))
    # The body's upper bout as a slab (the violin's top), to see where the hand is.
    c = []
    for y in (neck.body - 12.0, neck.body):
        for x in (-8.0, 8.0):
            for z in (-3.5, neck.top(neck.body) - 1.6):
                c.append(neck.world([x, y, z]))
    box(c, (0.55, 0.28, 0.10))
    return np.array(pts), np.array(tris), np.array(cols)


def views(neck, row, P, F):
    """(name, eye, target, up): the picture's camera; photo 4's angle (the player's eye over
    the G side, the scroll to the left); photo 3's (the G side, level)."""
    c = (P[1, 1] + P[2, 1] + P[0, 2]) / 3.0
    kx, z, bw = (np.array(row[k]) for k in ("kx", "z", "bw"))
    eye = np.array(F["eye"])
    cam = ("scene", c + hr.norm(eye - c) * 40.0, c, np.array(F["cam_u"]))
    p4 = ("photo4", c - kx * 16.0 + z * 20.0 - bw * 10.0, c, z)
    p3 = ("photo3", c - kx * 38.0 + z * 4.0, c, z)
    p2 = ("photo2", c + kx * 38.0 - z * 6.0 + bw * 6.0, c, z)
    return [cam, p4, p3, p2]


def render(pos, tris, cols, eye, at, up, size, out, fov=0.42):
    import preview
    flat = tris.ravel()
    nrm = preview.normals(pos, tris)[flat]
    ti = np.arange(len(flat)).reshape(-1, 3)
    pv = preview.look
    preview.look = lambda e, a, u=up: pv(e, a, u)
    try:
        preview.render(pos[flat], ti, cols[flat], cam=(eye, at), size=size, fov=fov, nrm=nrm,
                       out=out, key=tuple(hr.norm(eye - at + up * 20.0)), bg=(236, 236, 236))
    finally:
        preview.look = pv


def look(dump, outdir, who="woman", inst="violin", string=2, finger=1, at=1.0, cache=None):
    from PIL import Image
    F = next(F for F in dump["figs"] if F["who"] == who and F["inst"] == inst)
    h = Hand(who)
    neck = Neck(F)
    row = next(r for r in F["rows"] if r["string"] == string and r["finger"] == finger
               and abs(r["at"] - at) < 1e-3)
    c = cache[f"{who}-{inst}-{string}-{finger}-{at:g}"]
    W, Rh = np.array(c["W"]), np.array(c["Rh"])
    Q = np.array(c["Q"]).reshape(5, 3, 3, 3)
    P = h.fk(W, Rh, Q)
    hp, ht, hc = hand_mesh(h, W, Rh, Q, P)
    npos, ntri, ncol = neck_mesh(neck, F)
    pos = np.vstack([hp, npos])
    tris = np.vstack([ht, ntri + len(hp)])
    cols = np.vstack([hc, ncol])
    ims = []
    tag = f"{who}-{inst}-s{string}-f{finger}-at{at:g}"
    for name, eye, tgt, up in views(neck, row, P, F):
        o = os.path.join(outdir, f"hand-{tag}-{name}.png")
        render(pos, tris, cols, eye, tgt, up, (480, 400), o)
        ims.append(Image.open(o))
    im = Image.new("RGB", (480 * len(ims), 400))
    for i, x in enumerate(ims):
        im.paste(x, (480 * i, 0))
    out = os.path.join(outdir, f"hand-{tag}.png")
    im.save(out)
    ang = h.angles(W, Rh, P)
    print(tag, "MCP/PIP/DIP:", np.round(ang[1:]).astype(int).tolist(), "cost %.3f" % c["cost"])
    return out


def solid_mesh(solid):
    """r42:arcus-hands — the neck and the fingerboard as the app draws them (the solid's
    own sections, strip by strip), the strings as thin boxes and the body's upper half as a
    prism to its top's arch: what the penetration is measured against."""
    pts, tris, cols = [], [], []
    for (ys, polys), c in ((solid.board, (0.07, 0.06, 0.06)), (solid.shaft, (0.50, 0.27, 0.13))):
        m = polys.shape[1]
        for i in range(len(ys) - 1):
            n0 = len(pts)
            for r in (i, i + 1):
                for k in range(m):
                    x, z = polys[r, k]
                    pts.append(solid.o + np.array([x, ys[r], z]) @ solid.E)
                    cols.append(c)
            for k in range(m):
                a, b = k, (k + 1) % m
                tris.append([n0 + a, n0 + b, n0 + m + b])
                tris.append([n0 + a, n0 + m + b, n0 + m + a])
    o = solid.outline
    o = o[o[:, 1] > solid.body_len * 0.45]
    n0 = len(pts)
    zt = solid.arch_h
    for x, y in o:
        for z in (-solid.rib(y), 0.42):
            pts.append(solid.o + np.array([x, y, z]) @ solid.E)
            cols.append((0.55, 0.28, 0.10))
    cx = np.array([0.0, o[:, 1].mean(), zt])
    ci = len(pts)
    pts.append(solid.o + cx @ solid.E)
    cols.append((0.60, 0.33, 0.12))
    for k in range(len(o) - 1):
        a, b = n0 + 2 * k, n0 + 2 * k + 2
        tris += [[a, b, b + 1], [a, b + 1, a + 1], [a + 1, b + 1, ci]]
    for a, b, r in solid.strings:
        n1 = len(pts)
        for q in (a, b):
            for dx in (-0.04, 0.04):
                for dz in (-0.04, 0.04):
                    pts.append(solid.o + (q + np.array([dx, 0, dz])) @ solid.E)
                    cols.append((0.82, 0.80, 0.74))
        for f in ([0, 1, 5], [0, 5, 4], [2, 3, 7], [2, 7, 6], [0, 2, 6], [0, 6, 4], [1, 3, 7],
                  [1, 7, 5]):
            tris.append([n1 + i for i in f])
    return np.array(pts), np.array(tris), np.array(cols)


def pen_look(dump, outdir, who="woman", inst="violin", string=2, finger=1, at=1.0, cache=None):
    """r42:arcus-hands — a row's skinned hand and forearm with what goes into the neck, the
    fingerboard or the body in red (the playing pad's press, allowed to 1 mm, in orange; a
    string touched in yellow), from the app's camera, from under the neck and from the G
    side; prints the deepest of each."""
    from PIL import Image
    F = next(F for F in dump["figs"] if F["who"] == who and F["inst"] == inst)
    h = Hand(who)
    neck = Neck(F)
    solid = solid_of(F)
    row = next(r for r in F["rows"] if r["string"] == string and r["finger"] == finger
               and abs(r["at"] - at) < 1e-3)
    c = cache[f"{who}-{inst}-{string}-{finger}-{at:g}"]
    W, Rh = np.array(c["W"]), np.array(c["Rh"])
    Q = np.array(c["Q"]).reshape(5, 3, 3, 3)
    P = h.fk(W, Rh, Q)
    way = np.array(c["fore"]) if c.get("fore") is not None else None
    hp, ht, hc = hand_mesh(h, W, Rh, Q, P, way)
    used = np.unique(ht)
    dsol = np.full(len(hp), 9.0)
    dstr = np.full(len(hp), 9.0)
    dsol[used] = solid.sd(hp[used], strings=False)
    dstr[used] = solid.strings_d(solid.loc(hp[used]))
    # The playing pad: the playing finger's last bone's own vertices.
    fig = h.fig
    top = fig["ids"][np.arange(len(hp)), np.argmax(fig["ws"], 1)]
    pad = (top == O + hr.finger(row["playing"] + 1, 2)) if row["playing"] >= 0 else np.zeros(len(hp), bool)
    col = hc.copy()
    col[(dstr < 0) & ~pad] = (1.0, 0.85, 0.1)
    col[(dsol < 0) & pad] = (1.0, 0.55, 0.1)
    col[(dsol < 0) & ~pad] = (0.95, 0.05, 0.05)
    print(f"{who}-{inst}-s{string}-f{finger}-at{at:g}: pad {-min(0, dsol[pad].min() if pad.any() else 0) * 10:.1f} mm,"
          f" the rest {-min(0, dsol[~pad].min()) * 10:.1f} mm into the instrument, strings {-min(0, dstr[~pad].min()) * 10:.1f} mm")
    npos, ntri, ncol = solid_mesh(solid)
    pos = np.vstack([hp, npos])
    tris = np.vstack([ht, ntri + len(hp)])
    cols = np.vstack([col, ncol])
    cc = (P[1, 1] + P[2, 1] + P[0, 2]) / 3.0
    kx, z, bw = neck.E[0], neck.E[2], -neck.E[1]
    eye = np.array(F["eye"])
    cams = [("app", cc + hr.norm(eye - cc) * 40.0, np.array(F["cam_u"])),
            ("under", cc - z * 34.0 + kx * 6.0, -bw),
            ("gside", cc - kx * 36.0 + z * 6.0, z)]
    ims = []
    tag = f"{who}-{inst}-s{string}-f{finger}-at{at:g}"
    for name, e, up in cams:
        o = os.path.join(outdir, f"pen-{tag}-{name}.png")
        render(pos, tris, cols, e, cc, up, (440, 400), o)
        ims.append(Image.open(o))
    im = Image.new("RGB", (440 * len(ims), 400))
    for i, x in enumerate(ims):
        im.paste(x, (440 * i, 0))
    out = os.path.join(outdir, f"pen-{tag}.png")
    im.save(out)
    return out


def dense():
    """r42:arcus-hands, pass 3 — ``--dense``: every skin point of the hand and the forearm
    kept out of the instrument (as ``arcus_hand_table``'s gate measures it), not the thinned
    ~1 000 — for ``fix`` and ``hand_check.py``."""
    Hand.SKIN_MOST = {k: 1 << 30 for k in Hand.SKIN_MOST}


def main():
    cmd, dump_path = sys.argv[1], sys.argv[2]
    if "--dense" in sys.argv:
        dense()
    dump = json.load(open(dump_path))
    here = os.path.dirname(os.path.abspath(dump_path))
    only = next((a.split("=", 1)[1] for a in sys.argv if a.startswith("--only=")), None)

    def cache_of(key):
        # (r42:arcus-hands: and the parts solved string by string, ``--strings=``.)
        c = {}
        import glob
        for p in sorted(glob.glob(os.path.join(here, f"left_hand_cache-{key}-s*.json"))):
            c.update(json.load(open(p)))
        p = os.path.join(here, f"left_hand_cache-{key}.json")
        if os.path.exists(p):
            c.update(json.load(open(p)))
        return c

    keys = [f"{w}-{i}" for w in ("woman", "man") for i in ("violin", "viola")]
    if cmd == "solve":
        # One figure and instrument a process (run the four side by side), then ``pack``.
        hands = {}
        strs = next((a.split("=", 1)[1] for a in sys.argv if a.startswith("--strings=")), None)
        tag = ""
        if strs:
            STRINGS[:] = [int(x) for x in strs.split(",")]
            tag = "-s" + "".join(str(x) for x in STRINGS)
        for F in dump["figs"]:
            key = f'{F["who"]}-{F["inst"]}'
            if only and key != only:
                continue
            h = hands.setdefault(F["who"], Hand(F["who"]))
            cache = {}
            path = os.path.join(here, f"left_hand_cache-{key}{tag}.json")
            sd = next((a.split("=", 1)[1] for a in sys.argv if a.startswith("--seed=")), None)
            seed = None
            if sd:
                import glob
                seed = {}
                for p in sorted(glob.glob(os.path.join(sd, f"left_hand_cache-{key}*.json"))):
                    seed.update(json.load(open(p)))
            solve_fig(F, h, cache, verbose=True, seed=seed,
                      save=lambda c, p=path: json.dump(c, open(p, "w")))
            json.dump(cache, open(path, "w"))
    elif cmd == "fix":
        keys_arg = next((a.split("=", 1)[1] for a in sys.argv if a.startswith("--keys=")), None)
        if keys_arg:
            FORCE.extend("-" + k for k in keys_arg.split(","))
        # The poor rows of one figure and instrument again (run the four side by side; with
        # ``--strings=`` a share of them, each into its own file).
        strs = next((a.split("=", 1)[1] for a in sys.argv if a.startswith("--strings=")), None)
        tag = ""
        if strs:
            STRINGS[:] = [int(x) for x in strs.split(",")]
            tag = "-s" + "".join(str(x) for x in STRINGS) + "fix"
        for F in dump["figs"]:
            key = f'{F["who"]}-{F["inst"]}'
            if only and key != only:
                continue
            cache = cache_of(key)
            path = os.path.join(here, f"left_hand_cache-{key}{tag}.json")
            ats = next((a.split("=", 1)[1] for a in sys.argv if a.startswith("--ats=")), None)
            if ats:
                FORCE_AT.extend(float(a) for a in ats.split(","))
            worse = float(next((a.split("=", 1)[1] for a in sys.argv
                                if a.startswith("--worse=")), "60"))
            # (Each share saves only its own strings' rows, so the shares' files merge.)
            def mine(c):
                return {k: v for k, v in c.items() if int(k.split("-")[2]) in STRINGS}
            fix_fig(F, Hand(F["who"]), cache, worse=worse, verbose=True, tries=3,
                    save=lambda c, p=path: json.dump(mine(c), open(p, "w")))
            json.dump(mine(cache), open(path, "w"))
    elif cmd == "pack":
        cache = {}
        for k in keys:
            cache.update(cache_of(k))
        write_bin(dump, cache)
    elif cmd == "pen":
        # pen <dump> <dir> who inst string finger at (several "string,finger,at" after who inst)
        a = [x for x in sys.argv[4:] if not x.startswith("--")]
        who, inst = a[0], a[1]
        os.makedirs(sys.argv[3], exist_ok=True)
        cache = cache_of(f"{who}-{inst}")
        for spec in a[2:]:
            st, fi, at = spec.split(",")
            pen_look(dump, sys.argv[3], who, inst, int(st), int(fi), float(at), cache)
    elif cmd == "look":
        a = [x for x in sys.argv[4:] if not x.startswith("--")]
        who, inst = (a + ["woman", "violin"])[:2] if a else ("woman", "violin")
        string = int(a[2]) if len(a) > 2 else 2
        finger = int(a[3]) if len(a) > 3 else 1
        at = float(a[4]) if len(a) > 4 else 1.0
        os.makedirs(sys.argv[3], exist_ok=True)
        look(dump, sys.argv[3], who, inst, string, finger, at, cache_of(f"{who}-{inst}"))


if __name__ == "__main__":
    main()
