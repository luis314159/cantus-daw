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

ROOT = os.path.join(HERE, "..", "..")
FIGS = os.path.join(ROOT, "crates", "cantus-ui", "assets", "figures")
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

    def __init__(self, who):
        self.fig = hr.read_fig(os.path.join(FIGS, f"{who}.fig"))
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
PLAY = ((45, 70), (70, 100), (25, 50))
HOVER = ((35, 70), (60, 100), (15, 50))
# Each finger's spread at rest (rad): the index a little toward the scroll, the little
# finger a little away.
SPREAD = np.array([-0.12, -0.03, 0.07, 0.17])
# Each finger's angles near the middle of its range (deg; the owner's review: curled to
# the ranges' ends the fingers read short, a fist on the fingerboard).
MID = (55.0, 82.0, 32.0)
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
ROLL = (18.0, 34.0)
YAW = 14.0
PALM_MIN = 0.978
PALM_W = 45.0
# Each finger's middle joint over the fingerboard's top (cm): index, middle, ring, little.
ARCHES = (3.0, 3.3, 3.1, 2.4)
# The playing finger's last phalanx within this of the fingerboard's normal (cos).
UPRIGHT_COS = 0.94
# Each first phalanx rising from its knuckle: within this of the normal (cos 40 deg).
RISE_COS = 0.766
# The wrist under the neck (cm across from the E edge, below the top).
WRIST_X = (-1.7, 2.0)
# The wrist's deviation toward the little finger (deg): its range and where it rests.
WRIST_DEV = (3.0, 13.0)
WRIST_DEV_MID = 8.0
# How far a finger that does not play may hover from over its note (cm).
HOVER_SLACK = 1.4
WRIST_Z = (-10.5, -5.0)
# How high the knuckles stand over the fingerboard's top, at least (cm).
KNUCKLES = -0.2
# How high each finger's middle joint stands over the fingerboard's top (cm).
ARCH = 2.4
# The thumb's flex (MCP, IP): relaxed.
THUMB = ((5, 35), (5, 35))


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
        self.tips = np.array(row["tips"])
        self.playing = row["playing"]
        self.want_names = False

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
                add(f"upright{f}", 60.0 * max(0.0, UPRIGHT_COS - d2 @ (-self.z)))
            else:
                # Hovering over its own string a little across from the playing one (each
                # finger falls toward its own string, at its own angle — photograph 4).
                want = self.tips[f] + self.kx * (0.35 * (f - max(p, 0)))
                miss = np.linalg.norm(tip - want)
                add(f"hover{f}", 3.0 * max(0.0, miss - HOVER_SLACK))
            rng = PLAY if f == p else HOVER
            for k in range(3):
                wk = 3.0 if f == p else 1.0
                add(f"ang{f}{k}", wk * over(ang[f + 1, k], *rng[k]))
                add(f"mid{f}{k}", 0.08 * (ang[f + 1, k] - MID[k]))
            add(f"spread{f}", 1.0 * (x[6 + 4 * f + 1] - SPREAD[f]))
            # The arch: the middle joint high over the fingerboard.
            ql = loc(P[f + 1, 1])
            add(f"arch{f}", 6.0 * max(0.0, nk.top(ql[1]) + ARCHES[f] - ql[2]))
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
        add("crook_x", 5.0 * (qb[0] - (nk.hw(qb[1]) + 0.85)))
        add("crook_z", 3.0 * over(qb[2] - nk.top(qb[1]), -0.7, 0.3))
        # The knuckles up beside the fingerboard (the owner's photograph 2: the hand rises
        # from the wrist just under the neck to the knuckles over it).
        for f in range(1, 5):
            qk = loc(P[f, 0])
            add("knuckles", 3.0 * max(0.0, nk.top(qk[1]) + KNUCKLES - qk[2]))
        # The thumb up the G side, its tip about level with the fingerboard, opposite the
        # first and second fingers.
        qt = loc(P[0, 3])
        add("thumb_x", 4.0 * (qt[0] + nk.hw(qt[1]) + 0.65))
        add("thumb_z", 6.0 * over(qt[2] - nk.top(qt[1]), -0.6, 0.3))
        y12 = loc(self.tips[0])[1] * 0.6 + loc(self.tips[1])[1] * 0.4
        add("thumb_y", 2.0 * over(qt[1] - y12, -0.8, 1.2))
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
        el, _, dev = self.elbow(x, W, Rh, P)
        add("wrist_dev", 1.5 * over(np.degrees(dev), *WRIST_DEV) + 0.05 * (np.degrees(dev) - WRIST_DEV_MID))
        add("upper_len", 5.0 * over(np.linalg.norm(el - self.sh) - self.upper, -1.0, 1.0))
        qe = loc(el)
        add("elbow_y", 1.0 * over(qe[1], 2.0, nk.body - 2.0))
        add("elbow_z", 1.0 * max(0.0, qe[2] + 13.0))
        # The palm toward the neck and up toward the player.
        add("palm_way", PALM_W * max(0.0, PALM_MIN - npalm @ PALM_WAY(self)))
        qw = loc(W)
        add("wrist_x", 3.0 * over(qw[0] - nk.hw(qw[1]), *WRIST_X))
        add("wrist_z", 3.0 * over(qw[2] - nk.top(qw[1]), *WRIST_Z))
        if prev is not None:
            add("prev", *((x - prev) * 0.15))
        if self.want_names:
            return names
        return np.array(r, float)


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

def start(h, row):
    """A first guess: the knuckles along the neck's E side, the hand rising from under it,
    the fingers half curled."""
    a, bw, kx, z = (np.array(row[k]) for k in ("a", "bw", "kx", "z"))
    tips = np.array(row["tips"])
    along = (tips[0] - a) @ bw
    k_idx = a + bw * (along - 1.6) + kx * (row["hw"] + 1.3) - z * 0.3
    rx = h.heads[1, 0] - h.heads[4, 0]
    rest_mid = (h.heads[2, 0] + h.heads[3, 0]) * 0.5
    ry = rest_mid - h.wr
    py = hr.norm(z * 0.9 + kx * 0.35)
    R0 = hr.frame(-bw, py) @ hr.frame(rx, ry).T
    mid = k_idx + bw * 2.4
    W = mid - R0 @ (rest_mid - h.wr)
    th = np.zeros(21)
    for f in range(1, 5):
        th[4 * (f - 1):4 * f] = [55 * RAD - h.rest[f, 0], SPREAD[f - 1],
                                 85 * RAD - h.rest[f, 1], 35 * RAD - h.rest[f, 2]]
    th[19:21] = [15 * RAD, 15 * RAD]
    return R0, np.concatenate([W, np.zeros(3), th, [WRIST_DEV_MID * RAD]])


def entry(pose, x, cost):
    """A solved row as the cache keeps it (the forearm's way from the wrist too)."""
    W, Rh, Q = pose.unpack(x)
    P = pose.h.fk(W, Rh, Q)
    _, way, _ = pose.elbow(x, W, Rh, P)
    return {"W": W.tolist(), "Rh": Rh.tolist(), "Q": Q.reshape(15, 3, 3).tolist(),
            "fore": way.tolist(), "cost": float(cost), "x": x.tolist()}


def rows_of(F, string, finger):
    return [r for r in F["rows"] if r["string"] == string and r["finger"] == finger]


def solve_fig(F, h, cache, verbose=False):
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
    for string in range(4):
        for finger in (1, 0, 2, 3, 4):
            prev = None
            for row in rows_of(F, string, finger):
                at = row["at"]
                pose = Pose(h, neck, F, row, R0)
                starts = []
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
                fun = (lambda x: pose.residuals(x, prev))
                best = None
                for x_ in starts:
                    x, cost = lm(fun, x_, iters=50)
                    if best is None or cost < best[1]:
                        best = (x, cost)
                tries = 0
                while best[1] > 250.0 and tries < 6:
                    xs = best[0].copy()
                    xs[:3] += rng.normal(0, 0.8, 3)
                    xs[3:6] += rng.normal(0, 0.15, 3)
                    xs[6:] += rng.normal(0, 0.12, len(xs) - 6)
                    x, cost = lm(fun, xs, iters=80)
                    if cost < best[1]:
                        best = (x, cost)
                    tries += 1
                x, cost = best
                got[(string, finger, at)] = x
                prev = x
                cache[f'{key}-{string}-{finger}-{at:g}'] = entry(pose, x, cost)
                if verbose:
                    print(key, string, finger, at, "cost %.2f" % cost, flush=True)


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
        k = f'{key}-{row["string"]}-{row["finger"]}-{row["at"]:g}'
        c = cache[k]
        if c["cost"] <= worse and not (FORCE and any(k.endswith(x) for x in FORCE)):
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
        for x_ in starts:
            x, cost = lm(fun, x_, iters=80)
            if cost < best[1]:
                best = (x, cost)
        for _ in range(tries):
            xs = best[0].copy()
            xs[:3] += rng.normal(0, 0.8, 3)
            xs[3:6] += rng.normal(0, 0.2, 3)
            xs[6:] += rng.normal(0, 0.15, len(xs) - 6)
            x, cost = lm(fun, xs, iters=80)
            if cost < best[1]:
                best = (x, cost)
        x, cost = best
        if verbose:
            print(k, "cost %.1f -> %.1f" % (c["cost"], cost), flush=True)
        if cost < c["cost"]:
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


def main():
    cmd, dump_path = sys.argv[1], sys.argv[2]
    dump = json.load(open(dump_path))
    here = os.path.dirname(os.path.abspath(dump_path))
    only = next((a.split("=", 1)[1] for a in sys.argv if a.startswith("--only=")), None)

    def cache_of(key):
        p = os.path.join(here, f"left_hand_cache-{key}.json")
        return json.load(open(p)) if os.path.exists(p) else {}

    keys = [f"{w}-{i}" for w in ("woman", "man") for i in ("violin", "viola")]
    if cmd == "solve":
        # One figure and instrument a process (run the four side by side), then ``pack``.
        hands = {}
        for F in dump["figs"]:
            key = f'{F["who"]}-{F["inst"]}'
            if only and key != only:
                continue
            h = hands.setdefault(F["who"], Hand(F["who"]))
            cache = {}
            solve_fig(F, h, cache, verbose=True)
            json.dump(cache, open(os.path.join(here, f"left_hand_cache-{key}.json"), "w"))
    elif cmd == "fix":
        keys_arg = next((a.split("=", 1)[1] for a in sys.argv if a.startswith("--keys=")), None)
        if keys_arg:
            FORCE.extend("-" + k for k in keys_arg.split(","))
        # The poor rows of one figure and instrument again (run the four side by side).
        for F in dump["figs"]:
            key = f'{F["who"]}-{F["inst"]}'
            if only and key != only:
                continue
            cache = cache_of(key)
            path = os.path.join(here, f"left_hand_cache-{key}.json")
            fix_fig(F, Hand(F["who"]), cache, verbose=True, tries=3,
                    save=lambda c, p=path: json.dump(c, open(p, "w")))
            json.dump(cache, open(path, "w"))
    elif cmd == "pack":
        cache = {}
        for k in keys:
            cache.update(cache_of(k))
        write_bin(dump, cache)
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
