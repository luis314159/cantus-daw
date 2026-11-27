"""r42:arcus-hands, pass 5 — the free fingers' gaps and fan in a solved table, measured on the
solver's own skin (every skin point, as ``--dense``): for each neighbouring pair (index-middle,
middle-ring, ring-little) the nearest distance between their skins past the webs (the first
phalanx's outer half, the middle and the last phalanges) and the angle between their first
phalanges seen on the palm's plane (the fan). Real fingers are never glued: crop-11 shows
light between the free fingers, the playing finger standing apart.

    python tools/arcus-figures/finger_gap.py <dump.json> [--only=woman-violin] [--rows]

Prints, per figure and instrument, the share of rows whose pair touches (under 0.5 mm) and
the gaps' and the fan's quartiles; ``--rows`` lists each row.
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

PAIRS = ((1, 2), (2, 3), (3, 4))


def finger_sets(h):
    """Each finger's skin points past its web: the first phalanx's outer half (by where the
    point lies along it at rest), the middle and the last phalanges."""
    out = {}
    for f in range(1, 5):
        b0, b1, b2 = (L.O + hr.finger(f, k) for k in range(3))
        a, b = h.pts[f, 0], h.pts[f, 1]
        d = b - a
        t = (h.sk_pos - a) @ d / (d @ d)
        m = (h.sk_top == b1) | (h.sk_top == b2) | ((h.sk_top == b0) & (t > 0.5))
        out[f] = np.nonzero(m)[0]
    return out


def gaps(h, sets, sk):
    g = {}
    for f, k in PAIRS:
        a, b = sk[sets[f]], sk[sets[k]]
        d = np.linalg.norm(a[:, None, :] - b[None, :, :], axis=-1)
        g[(f, k)] = float(d.min())
    return g


def fan(h, Rh, P):
    n = Rh @ h.n
    out = {}
    for f, k in PAIRS:
        a = P[f, 1] - P[f, 0]
        b = P[k, 1] - P[k, 0]
        a = hr.norm(a - n * (a @ n))
        b = hr.norm(b - n * (b @ n))
        # Positive: the pair diverging (the second finger turned away from the first, toward
        # its own side of the hand).
        side = P[k, 0] - P[f, 0]
        side = hr.norm(side - n * (side @ n))
        out[(f, k)] = float(np.degrees(np.arcsin(np.clip((b - a) @ side, -1, 1))))
    return out


def main():
    dump = sys.argv[1]
    only = next((a.split("=", 1)[1] for a in sys.argv if a.startswith("--only=")), None)
    rows_out = "--rows" in sys.argv
    L.dense()
    D = json.load(open(dump))
    for F in D["figs"]:
        who, inst = F["who"], F["inst"]
        if only and f"{who}-{inst}" != only:
            continue
        c = {}
        for p in sorted(glob.glob(os.path.join(os.path.dirname(dump), f"left_hand_cache-{who}-{inst}*.json"))):
            c.update(json.load(open(p)))
        h = L.Hand(who)
        sets = finger_sets(h)
        nk = L.Neck(F)
        R0, _ = L.start(h, L.rows_of(F, 0, 1)[0])
        G, A, touch = [], [], 0
        # Each pair's gap by role: beside the playing finger, or both free.
        by_role = {"play": [], "free": []}
        for row in F["rows"]:
            k = f"{who}-{inst}-{row['string']}-{row['finger']}-{row['at']:g}"
            if k not in c:
                continue
            pose = L.Pose(h, nk, F, row, R0)
            x = np.array(c[k]["x"])
            W, Rh, Q = pose.unpack(x)
            P = h.fk(W, Rh, Q)
            _, way, _ = pose.elbow(x, W, Rh, P)
            sk = h.skin(W, Rh, Q, P, way)
            g = gaps(h, sets, sk)
            a = fan(h, Rh, P)
            G.append([g[p] for p in PAIRS])
            A.append([a[p] for p in PAIRS])
            if min(g.values()) < 0.05:
                touch += 1
            pl = row["playing"] + 1
            for p in PAIRS:
                by_role["play" if pl in p else "free"].append(g[p] * 10)
            if rows_out:
                print(k, "gaps mm", " ".join("%.1f" % (g[p] * 10) for p in PAIRS),
                      "fan", " ".join("%.1f" % a[p] for p in PAIRS))
        G, A = np.array(G) * 10, np.array(A)
        q = lambda v: "/".join("%.1f" % x for x in np.percentile(v, (10, 50, 90)))
        print(f"{who}-{inst}: {len(G)} rows, {touch} with a pair under 0.5 mm")
        for i, p in enumerate(PAIRS):
            print(f"  {p[0]}-{p[1]}: gap mm p10/50/90 {q(G[:, i])}  fan deg {q(A[:, i])}")
        for r, v in by_role.items():
            v = np.array(v)
            print(f"  {r}: gap mm p10/50/90 {q(v)}, under 1 mm {np.mean(v < 1.0) * 100:.0f} %")


if __name__ == "__main__":
    main()
