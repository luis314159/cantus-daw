"""r42:arcus-hands, pass 6 — the table's hand where the app blends two rows: the gate's cases
(``arcus_hand_table::the_left_hand_keeps_out_of_the_neck_and_the_body``: each string at +1,
+3, +5, +7, +9, +12, +14, +17 semitones, the hand at ``hand::target``'s position, the finger
``left_at`` picks) and, with ``--mid``, every pair of neighbouring rows at a third and two
thirds. Each pose is built as ``neck_hand::row`` and ``place`` build it (the wrist lerped,
the turns nlerped, the hand shifted so the playing tip lands on its stop), skinned densely
and measured against the instrument as drawn: the deepest point of each part.

    python tools/arcus-figures/blend_check.py <dump.json> [--mid] [--only=woman-violin]

Prints one line a case over the gate's slack (0.3 mm; the playing pad 1 mm) and the rows
it blends, as ``--keys=`` for ``left_hand.py fix``.
"""
import glob
import json
import os
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import hand_rig as hr  # noqa: E402
import left_hand as L  # noqa: E402

SPAN = 5.0
HIGH_REACH = 3.0
PAD_PRESS = 0.1
SKIN_SLACK = 0.03
GATE_SEMIS = (1.0, 3.0, 5.0, 7.0, 9.0, 12.0, 14.0, 17.0)


def target(s, at=1.0, span=SPAN):
    """``hand::target`` (pass 4's shift high up)."""
    reach = span - max(span - HIGH_REACH, 0.0) * np.clip((s - 9.0) / 3.0, 0.0, 1.0)
    lo, hi = max(s - reach, 1.0), max(s, 1.0)
    if at < lo - 0.25:
        return lo
    if at > hi + 0.25:
        return hi
    return at


def nlerp(a, b, k):
    if a @ b < 0:
        b = -b
    q = a * (1 - k) + b * k
    return q / np.linalg.norm(q)


def cache_of(here, key):
    c = {}
    for p in sorted(glob.glob(os.path.join(here, f"left_hand_cache-{key}-s*.json"))):
        c.update(json.load(open(p)))
    p = os.path.join(here, f"left_hand_cache-{key}.json")
    if os.path.exists(p):
        c.update(json.load(open(p)))
    return c


def stop(rows, f, s):
    """The playing tip's stop ``s`` semitones up from two rows' stops on one string (the
    string's points are bridge + (nut - bridge) 2^(-s/12), lifted alike)."""
    (sa, pa), (sb, pb) = rows
    ta, tb = 2.0 ** (-sa / 12.0), 2.0 ** (-sb / 12.0)
    if abs(ta - tb) < 1e-6:
        return pa
    d = (pb - pa) / (tb - ta)
    return pa + d * (2.0 ** (-s / 12.0) - ta)


class Blend:
    def __init__(self, F, h, cache):
        self.F, self.h, self.cache = F, h, cache
        self.key = f'{F["who"]}-{F["inst"]}'
        self.ats = sorted({r["at"] for r in F["rows"]})
        self.rows = {(r["string"], r["finger"], r["at"]): r for r in F["rows"]}
        self.solid = L.solid_of(F)
        self.pad_bone = {f: L.O + hr.finger(f + 1, 2) for f in range(4)}

    def row_pose(self, string, finger, at):
        c = self.cache[f"{self.key}-{string}-{finger}-{at:g}"]
        Q = np.array(c["Q"]).reshape(15, 3, 3)
        return (np.array(c["W"]), L.quat(np.array(c["Rh"])), [L.quat(q) for q in Q],
                np.array(c["fore"]))

    def pose(self, string, finger, at, s=None, x_over=None):
        """The blended pose at hand position ``at`` (``finger`` playing ``s`` semitones up,
        or its row's own stop), shifted onto the stop. ``x_over`` maps a row's ``at`` to a
        solver vector to use instead of the cache's (for ``fix``)."""
        n = len(self.ats)
        j = next((i for i, a in enumerate(self.ats) if a > at), None)
        if j is None:
            i, k = n - 1, 0.0
        elif j == 0:
            i, k = 0, 0.0
        else:
            i, k = j - 1, (at - self.ats[j - 1]) / (self.ats[j] - self.ats[j - 1])
        parts = [(self.ats[i], 1.0 - k)] + ([(self.ats[i + 1], k)] if k > 0 else [])
        got = []
        for a, _ in parts:
            if x_over is not None and a in x_over:
                got.append(x_over[a])
            else:
                got.append(self.row_pose(string, finger, a))
        if len(got) == 1:
            W, qh, qs, fore = got[0]
        else:
            (Wa, qa, qsa, fa), (Wb, qb, qsb, fb) = got
            W = Wa * (1 - k) + Wb * k
            qh = nlerp(qa, qb, k)
            qs = [nlerp(x, y, k) for x, y in zip(qsa, qsb)]
            fore = hr.norm(fa * (1 - k) + fb * k)
        Rh = L.qrot(qh)
        Q = np.array([L.qrot(q) for q in qs]).reshape(5, 3, 3, 3)
        P = self.h.fk(W, Rh, Q)
        if finger == 4:
            return W, Rh, Q, P, fore  # (an open string: no stop, no shift at rest)
        rows = [self.rows[(string, finger, a)] for a, _ in parts]
        if s is None:
            s = rows[0]["semis"] * (1 - k) + (rows[-1]["semis"] * k if k > 0 else 0.0)
        pts = [(r["semis"], np.array(r["tips"][finger])) for r in rows]
        if len(pts) == 1:
            near = [a for a in self.ats if a != parts[0][0]][-1 if i == n - 1 else 0]
            r2 = self.rows[(string, finger, near)]
            pts.append((r2["semis"], np.array(r2["tips"][finger])))
        t = stop(pts, finger, s)
        sh = t - P[finger + 1, 3]
        W, P = W + sh, P + sh
        return W, Rh, Q, P, fore

    def depth(self, W, Rh, Q, P, fore, finger):
        sk = self.h.skin(W, Rh, Q, P, fore)
        d = self.solid.local_sd(self.solid.loc(sk))
        pad = self.h.sk_top == self.pad_bone.get(finger, -1)
        dp = -min(0.0, d[pad].min()) if pad.any() else 0.0
        rest = np.where(pad, 9.0, d)
        i = int(np.argmin(rest))
        return dp, -min(0.0, rest[i]), self.h.sk_top[i], self.h.sk_kind[i]


def name(top):
    for f in range(5):
        for k in range(3):
            if top == L.O + hr.finger(f, k):
                return f"finger {f} bone {k}"
    return "the palm" if top == L.HAND else "the forearm"


def main():
    dump = sys.argv[1]
    D = json.load(open(dump))
    L.dense()
    here = os.path.dirname(os.path.abspath(dump))
    only = next((a.split("=", 1)[1] for a in sys.argv if a.startswith("--only=")), None)
    mid = "--mid" in sys.argv
    total = 0
    for F in D["figs"]:
        key = f'{F["who"]}-{F["inst"]}'
        if only and key != only:
            continue
        b = Blend(F, L.Hand(F["who"]), cache_of(here, key))
        cases = []
        for string in range(4):
            for s in GATE_SEMIS:
                at = target(s)
                f = int(np.clip(round((s - at) / (SPAN / 3.0)), 0, 3))
                cases.append((string, f, at, s, f"+{s:g}"))
            if mid:
                for f in range(5):
                    for a in b.ats:
                        cases.append((string, f, a, None, f"f{f} row {a:g}"))
                    for a0, a1 in zip(b.ats, b.ats[1:]):
                        for k in (1 / 3, 2 / 3):
                            cases.append((string, f, a0 + (a1 - a0) * k, None, f"f{f} at {a0 + (a1 - a0) * k:.1f}"))
        keys = []
        for string, f, at, s, tag in cases:
            W, Rh, Q, P, fore = b.pose(string, f, at, s)
            dp, dr, top, _ = b.depth(W, Rh, Q, P, fore, f)
            why = []
            if dp > PAD_PRESS:
                why.append(f"the playing pad {dp * 10:.1f} mm in")
            if dr > SKIN_SLACK:
                why.append(f"{name(top)} {dr * 10:.1f} mm in")
            if why:
                total += 1
                lo = max(a for a in b.ats if a <= at)
                hi = min((a for a in b.ats if a >= at), default=lo)
                print(f"{key} string {string} {tag} (f{f} rows {lo:g}/{hi:g}): {'; '.join(why)}")
                keys += [f"{string}-{f}-{lo:g}", f"{string}-{f}-{hi:g}"]
        if keys:
            print(f"  {key} --keys={','.join(dict.fromkeys(keys))}")
    print("cases", total)


if __name__ == "__main__":
    main()
