"""r42:arcus-hands, pass 6 — the table's rows solved again with the hand kept out of the
instrument **between** them too. The app blends two rows for any position between the
table's (``neck_hand::row``: the wrist lerped, the turns nlerped, then the hand shifted onto
the stop); pass 5's rows were each clean, but a thumb under the neck at the 9th semitone and
beside the heel at the 12th cut through the neck on the way (8-10 mm at the gate's +14),
and some rows were never clean (a forearm 11-13 mm into the viola's body at the 7th).

Each row's own residuals (``left_hand.Pose.residuals``, the rules, the gaps, the skin kept
out densely) and, beside them, the skin of its blends with the rows either side at
``KS`` of the way, held out of the instrument as the row's own (the playing pad may press
by ``PAD_PRESS``, the forearm keeps ``SLEEVE``, the rest ``SKIN_GAP``), the neighbours held.
Sweeps every (string, finger) chain up and down (Gauss-Seidel), a row kept when the whole
objective falls.

    python tools/arcus-figures/blend_fix.py <dump.json> --only=woman-violin --strings=0,1
        [--keys=0-2-9,0-2-12] [--sweeps=2] [--worse=0.3]

Reads the caches beside the dump (as ``left_hand.py``) and writes
``left_hand_cache-<who>-<inst>-s<strings>blend.json`` (merged by ``pack`` with the rest).
Without ``--keys`` it takes the rows whose own skin or whose blends go deeper than
``--worse`` mm.
"""
import copy
import json
import os
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import blend_check as B  # noqa: E402
import left_hand as L  # noqa: E402
import rule_check as RC  # noqa: E402
import hand_rig as hr  # noqa: E402

# r42:arcus-hands, pass 7 — ``--rules``: the gate's rules (``rule_check.py``) held on the
# gate's cases this row's blends make and on the row's own pose up to RULES_TOP semitones of
# the hand's position, with the palm's way freer by PALM_FREE_REACH degrees where the
# outline's rule holds the hand edge-on (the reach and high rows: crop-02, crop-11).
RULES_TOP = 12.0
PALM_FREE_REACH = 20.0
RULES_W = 1.0
# The back of the hand at a slant to the eye on the reach and high rows' own poses: at most
# FACING_MOST (the cosine; the photographs' slanted backs, w/l 0.5-0.9 — crop-02's hand at
# the body's shoulder nearly edge-on; the table's read 0.85-0.92 there, broadside), and the
# hinge's weight. The rows are started too from the hand turned about its own length by
# TURN_STARTS degrees (LM alone does not leave a broadside pose for an edge-on one).
FACING_MOST = 0.6
FACING_W = 100.0
TURN_STARTS = (-45.0, -25.0, 25.0, 45.0)
# A row is kept only if its skin and its blends go no deeper than before (or the slack, cm;
# ``--keep=<mm>``: turned edge-on at the 9th semitone her hand's blends went 1.5 mm into the
# instrument where the broadside pose had none).
KEEP_SLACK = 0.03

KS = (0.5,)
# The gate's own blends (``hand::target``: +14 is the 11th position, two thirds from the 9th
# to the 12th; +17 the 14th; +7 and +9 the 2nd and 4th, halfway).
KS_EXTRA = (1.0 / 3.0, 2.0 / 3.0)


def near(b, h, string, finger, at, mids, reach=2.0):
    """``h`` with only the skin points within ``reach`` cm of the instrument at the row or
    any of its blends (the rest cannot touch it within a settle's moves; the result is
    checked on the whole skin)."""
    keep = np.zeros(len(h.sk_pos), bool)
    for a in [at] + mids:
        W, Rh, Q, P, fore = b.pose(string, finger, a)
        d = b.solid.local_sd(b.solid.loc(h.skin(W, Rh, Q, P, fore)))
        keep |= d < reach
    s = copy.copy(h)
    for k in ("sk_pos", "sk_kind", "sk_top", "sk_ids", "sk_w"):
        setattr(s, k, getattr(h, k)[keep])
    return s


def me_of(pose, x):
    W, Rh, Q = pose.unpack(x)
    P = pose.h.fk(W, Rh, Q)
    _, way, _ = pose.elbow(x, W, Rh, P)
    return (W, L.quat(Rh), [L.quat(q) for q in Q.reshape(15, 3, 3)], way)


class Row:
    def __init__(self, b, pose, string, finger, at):
        self.b, self.pose = b, pose
        self.string, self.finger, self.at = string, finger, at
        self.mids = self.mids_of(b, at)
        h = pose.h
        pad = b.pads(finger, h)
        fore = h.sk_kind == "fore"
        self.need = np.where(pad, -L.PAD_PRESS, np.where(fore, L.SLEEVE, L.SKIN_GAP))

    @staticmethod
    def mids_of(b, at):
        i = b.ats.index(at)
        mids = []
        for j in (i - 1, i + 1):
            if 0 <= j < len(b.ats):
                mids += [at + (b.ats[j] - at) * k for k in KS + KS_EXTRA]
        return mids

    def blends(self, x):
        me = me_of(self.pose, x)
        out = []
        for a in self.mids:
            W, Rh, Q, P, fore = self.b.pose(self.string, self.finger, a, None, x_over={self.at: me})
            sk = self.pose.h.skin(W, Rh, Q, P, fore)
            d = self.b.solid.local_sd(self.b.solid.loc(sk))
            out.append(np.maximum(0.0, self.need - d))
        return np.concatenate(out)

    def gate(self, rules):
        """r42:arcus-hands, pass 7 — the gate's cases whose blend uses this row (the same
        finger playing) and the row's own stop, as (semis, hand position)."""
        self.rules = rules
        self.cases = []
        for s in RC.GATE_SEMIS:
            at, f = RC.target_finger(s)
            if f != self.finger:
                continue
            lo = max(a for a in self.b.ats if a <= at)
            hi = min((a for a in self.b.ats if a >= at), default=lo)
            if self.at == lo or (self.at == hi and at > lo):
                self.cases.append((s, at, lo))
        if self.at <= RULES_TOP and self.finger < 4:
            row = self.b.rows[(self.string, self.finger, self.at)]
            self.cases.append((row["semis"], self.at, self.at))
        row = self.b.rows[(self.string, self.finger, self.at)]
        self.edge_on = self.finger < 4 and row["semis"] > 3.0
        return self

    def held(self, x):
        if not getattr(self, "cases", None):
            return np.zeros(0)
        me = me_of(self.pose, x)
        out = []
        for s, at, lo in self.cases:
            W, Rh, Q, P, fore = self.b.pose(self.string, self.finger, at, s, x_over={self.at: me})
            row = self.rules.rows[(self.string, self.finger, lo)]
            m = self.rules.measure(self.string, s, at, self.finger, W, Rh, Q, P, fore, row)
            out += RC.hold(m)
            if self.edge_on and lo == self.at and at == self.at:
                out.append(FACING_W * max(0.0, m["facing"][0] - FACING_MOST))
        return RULES_W * np.array(out)

    def fun(self, prev):
        def f(x):
            r = self.pose.residuals(x, prev)
            return np.concatenate([r, self.pose.pen_w * self.blends(x), self.held(x)])
        return f


def worst(b, string, finger, at, me=None):
    """The deepest skin (cm) at the row and at its blends either side (``me``: the row's
    pose to use instead of the cache's)."""
    i = b.ats.index(at)
    out = 0.0
    pts = [at]
    for j in (i - 1, i + 1):
        if 0 <= j < len(b.ats):
            pts += [at + (b.ats[j] - at) * k for k in KS + KS_EXTRA]
    for a in pts:
        W, Rh, Q, P, fore = b.pose(string, finger, a, None,
                                   x_over=None if me is None else {at: me})
        dp, dr, _, _ = b.depth(W, Rh, Q, P, fore, finger)
        out = max(out, dr, dp - B.PAD_PRESS)
    return out


def rules(pose, x, at):
    """Whether the row breaks one of ``hand_check.py``'s rules (as the gates measure them),
    or the index's base stands out past ``left_hand.INDEX_OUT``."""
    h = pose.h
    W, Rh, Q = pose.unpack(x)
    P = h.fk(W, Rh, Q)
    _, way, _ = pose.elbow(x, W, Rh, P)
    hd = (P[2, 0] + P[3, 0]) * 0.5 - W
    hd = hd / np.linalg.norm(hd)
    if np.degrees(np.arccos(np.clip(way @ -hd, -1, 1))) > (16.0 if at < 9 else 30.0):
        return True
    p = pose.playing
    if p >= 0:
        d2 = P[p + 1, 3] - P[p + 1, 2]
        d2 = d2 / np.linalg.norm(d2)
        if np.degrees(np.arccos(np.clip(d2 @ -pose.z, -1, 1))) > 30 or \
                h.angles(W, Rh, P)[p + 1][1] > 108:
            return True
    qi = pose.neck.loc(P[1, 0])
    if qi[0] - pose.neck.hw(qi[1]) > 2.1 * pose.k:
        return True
    # (and the thumb's tip past the neck's far side from the app's eye: left_hand's
    # thumb_peek.)
    qt = pose.neck.loc(P[0, 3])
    if qt[2] < pose.neck.top(qt[1]) - 1.5 or qt[0] > -pose.neck.hw(qt[1]) * 0.5:
        return True
    most = pose.peek_most()
    return any(pose.peek(P[0, k]) + r * pose.k - most > 0.1 for k, r in ((2, 0.75), (3, 0.6)))


def main():
    dump = sys.argv[1]
    D = json.load(open(dump))
    # (The rows are solved on the thinned skin, as left_hand.py solves them, near the
    # instrument only; what they reach is checked on every skin point.)
    thin = {}
    here = os.path.dirname(os.path.abspath(dump))
    arg = {a.split("=", 1)[0]: a.split("=", 1)[1] for a in sys.argv[2:] if "=" in a}
    only = arg["--only"]
    strings = [int(s) for s in arg.get("--strings", "0,1,2,3").split(",")]
    sweeps = int(arg.get("--sweeps", "2"))
    worse = float(arg.get("--worse", "0.3")) / 10.0
    keys = arg.get("--keys")
    use_rules = "--rules" in sys.argv[2:]
    keep_slack = float(arg.get("--keep", str(KEEP_SLACK * 10.0))) / 10.0
    F = next(F for F in D["figs"] if f'{F["who"]}-{F["inst"]}' == only)
    thin = L.Hand(F["who"])
    L.dense()
    h = L.Hand(F["who"])
    cache = B.cache_of(here, only)
    b = B.Blend(F, h, cache)
    neck = L.Neck(F)
    rules = RC.Rules(F, h, neck) if use_rules else None
    R0, _ = L.start(h, L.rows_of(F, 0, 1)[0])
    tag = "-s" + "".join(str(s) for s in strings) + "blend"
    path = os.path.join(here, f"left_hand_cache-{only}{tag}.json")

    def save():
        json.dump({k: v for k, v in cache.items() if int(k.split("-")[2]) in strings},
                  open(path, "w"))

    rng = np.random.default_rng(11)
    for sweep in range(sweeps):
        todo = []
        for string in strings:
            for finger in range(5):
                for at in b.ats:
                    k = f"{string}-{finger}-{at:g}"
                    if keys is not None:
                        if k in keys.split(","):
                            todo.append((string, finger, at))
                    elif worst(b, string, finger, at) > worse or rules(
                            L.Pose(h, neck, F, b.rows[(string, finger, at)], R0),
                            np.array(cache[f"{only}-{k}"]["x"]), at):
                        todo.append((string, finger, at))
        print(f"sweep {sweep}: {len(todo)} rows", flush=True)
        if not todo:
            break
        # Up the chain on even sweeps, down on odd ones.
        if sweep % 2:
            todo = todo[::-1]
        for string, finger, at in todo:
            key = f"{only}-{string}-{finger}-{at:g}"
            row = b.rows[(string, finger, at)]
            mids = Row.mids_of(b, at)
            hs = near(b, thin, string, finger, at, mids)
            pose = L.Pose(hs, neck, F, row, R0)
            i = b.ats.index(at)
            lo = cache.get(f"{only}-{string}-{finger}-{b.ats[i - 1]:g}") if i > 0 else None
            prev = np.array(lo["x"]) if lo is not None else None
            rr = Row(b, pose, string, finger, at)
            if rules is not None:
                rr.gate(rules)
                if row["semis"] > 3.0 and finger < 4:
                    pose.palm_free = PALM_FREE_REACH
            fun = rr.fun(prev)
            x0 = np.array(cache[key]["x"])
            if len(x0) < 29:
                x0 = np.concatenate([x0, np.zeros(29 - len(x0))])

            def total(x):
                r = fun(x)
                return float(r @ r)
            pose.pen_w = L.PEN_W
            best = (x0, total(x0))
            first = best[1]
            starts = [x0]
            for j in (i - 1, i + 1):
                if 0 <= j < len(b.ats):
                    n = cache.get(f"{only}-{string}-{finger}-{b.ats[j]:g}")
                    if n is not None:
                        xn = np.array(n["x"])
                        if len(xn) == len(x0):
                            # (the neighbour's hand and fingers, this row's wrist)
                            xs = xn.copy()
                            xs[:3] = x0[:3]
                            starts.append(xs)
            for _ in range(0):
                xs = x0.copy()
                xs[:3] += rng.normal(0, 0.5, 3)
                xs[3:6] += rng.normal(0, 0.12, 3)
                xs[6:] += rng.normal(0, 0.1, len(xs) - 6)
                starts.append(xs)
            if rules is not None and rr.edge_on:
                Wx, Rhx, Qx = pose.unpack(x0)
                Px = pose.h.fk(Wx, Rhx, Qx)
                ax = hr.norm((Px[2, 0] + Px[3, 0]) * 0.5 - Wx)
                for ang in TURN_STARTS:
                    xs = x0.copy()
                    xs[3:6] = L.rotlog(hr.about(ax, np.radians(ang)) @ Rhx @ pose.R0.T)
                    starts.append(xs)
            was = worst(b, string, finger, at)
            for xs in starts:
                x, _ = L.settle(pose, fun, xs, 30)
                pose.pen_w = L.PEN_W
                c = total(x)
                if c < best[1] and (rules is None or worst(b, string, finger, at, me_of(pose, x))
                                    <= max(was, keep_slack) + 1e-4):
                    best = (x, c)
            x, c = best
            if c < first:
                own = pose.residuals(x, prev)
                cache[key] = L.entry(pose, x, float(own @ own))
                save()
            print(f"{key}: {first:.1f} -> {c:.1f}, worst {worst(b, string, finger, at) * 10:.1f} mm",
                  flush=True)
    save()


if __name__ == "__main__":
    main()
