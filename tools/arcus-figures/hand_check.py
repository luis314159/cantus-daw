"""r42:arcus-hands — the solved table's rows that break the gates' rules, measured on the
solver's own pose (the wrist's whole bend, the playing tip's lean from the fingerboard's
normal, its joints' ranges, the skin into the instrument), as ``--keys=`` lists for
``left_hand.py fix`` per figure, instrument and string share.

    python tools/arcus-figures/hand_check.py <dump.json> [--pen]
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


def main():
    dump = sys.argv[1]
    D = json.load(open(dump))
    pen = "--pen" in sys.argv
    for F in D["figs"]:
        who, inst = F["who"], F["inst"]
        c = {}
        for p in sorted(glob.glob(os.path.join(os.path.dirname(dump), f"left_hand_cache-{who}-{inst}*.json"))):
            c.update(json.load(open(p)))
        h = L.Hand(who)
        nk = L.Neck(F)
        R0, _ = L.start(h, L.rows_of(F, 0, 1)[0])
        bad = {}
        for row in F["rows"]:
            k = f"{who}-{inst}-{row['string']}-{row['finger']}-{row['at']:g}"
            if k not in c:
                continue
            pose = L.Pose(h, nk, F, row, R0)
            x = np.array(c[k]["x"])
            W, Rh, Q = pose.unpack(x)
            P = h.fk(W, Rh, Q)
            _, way, _ = pose.elbow(x, W, Rh, P)
            hd = hr.norm((P[2, 0] + P[3, 0]) * 0.5 - W)
            bend = np.degrees(np.arccos(np.clip(way @ -hd, -1, 1)))
            why = []
            if bend > (16.0 if row["at"] <= 9 else 30.0):
                why.append(f"wrist {bend:.0f}")
            p = row["playing"]
            if p >= 0:
                d2 = hr.norm(P[p + 1, 3] - P[p + 1, 2])
                up = np.degrees(np.arccos(np.clip(d2 @ -pose.z, -1, 1)))
                ang = h.angles(W, Rh, P)[p + 1]
                if up > 30:
                    why.append(f"tip {up:.0f}")
                if ang[1] > 108:
                    why.append(f"PIP {ang[1]:.0f}")
            if pen:
                sk = h.skin(W, Rh, Q, P, way)
                d = pose.solid.local_sd(pose.solid.loc(sk))
                if d.min() < -0.05:
                    why.append(f"in {-d.min() * 10:.1f} mm")
            if why:
                bad.setdefault(row["string"], []).append((f"{row['string']}-{row['finger']}-{row['at']:g}", why))
        for s, rows in sorted(bad.items()):
            print(f"{who}-{inst} string {s}: {len(rows)} rows:", "; ".join(f"{k} {','.join(w)}" for k, w in rows[:8]))
            print(f"  --keys={','.join(k for k, _ in rows)}")


if __name__ == "__main__":
    main()
