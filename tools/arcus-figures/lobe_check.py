"""r42:arcus-hands, pass 6 — ``arcus_hand_shape``'s lobe in Python: the left hand in first
position (B4 on the A string: the middle finger at the 1st position, as ``left_at`` picks
it) posed from the table as the app blends and shifts it, skinned densely, seen from the
app's eye; the share of the hand's outline past the line from the little finger's base
knuckle to the wrist (away from the hand's middle) against the back of the hand's (both
as convex hulls on screen), and which bones those points follow.

    python tools/arcus-figures/lobe_check.py <dump.json> [--only=woman-violin]
"""
import json
import os
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import blend_check as B  # noqa: E402
import hand_rig as hr  # noqa: E402
import left_hand as L  # noqa: E402


def hull_area(p):
    if len(p) < 3:
        return 0.0
    p = sorted(map(tuple, p))

    def cross(o, a, b):
        return (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
    lo, up = [], []
    for q in p:
        while len(lo) >= 2 and cross(lo[-2], lo[-1], q) <= 0:
            lo.pop()
        lo.append(q)
    for q in reversed(p):
        while len(up) >= 2 and cross(up[-2], up[-1], q) <= 0:
            up.pop()
        up.append(q)
    h = lo[:-1] + up[:-1]
    return 0.5 * abs(sum(h[i][0] * h[(i + 1) % len(h)][1] - h[(i + 1) % len(h)][0] * h[i][1]
                         for i in range(len(h))))


def lobe(b, F, string=2, finger=1, at=1.0, semis=2.0):
    W, Rh, Q, P, fore = b.pose(string, finger, at, semis)
    sk = b.h.skin(W, Rh, Q, P, fore)
    eye = np.array(F["eye"])
    f, r, u = (np.array(F[k]) for k in ("cam_f", "cam_r", "cam_u"))

    def scr(p):
        d = np.atleast_2d(p) - eye
        z = d @ f
        return np.stack([(d @ r) / z, (d @ u) / z], -1)
    top = b.h.sk_top
    hand = (top >= L.HAND) & (top <= L.O + hr.finger(4, 2))
    s = scr(sk)
    k, w, mid = scr(P[4, 0])[0], scr(W)[0], scr(P[2, 0])[0]
    d = w - k
    n = np.array([-d[1], d[0]])
    n /= np.linalg.norm(n)
    if n @ (mid - k) > 0:
        n = -n
    t = ((s - k) @ d) / (d @ d)
    out = hand & (t >= 0) & (t <= 1) & (((s - k) @ n) > 0)
    back = top == L.HAND
    share = hull_area(s[out]) / max(hull_area(s[back]), 1e-9)
    names = {}
    for tb in top[out]:
        nm = B.name(tb)
        names[nm] = names.get(nm, 0) + 1
    return share, names


def main():
    dump = sys.argv[1]
    D = json.load(open(dump))
    L.dense()
    here = os.path.dirname(os.path.abspath(dump))
    only = next((a.split("=", 1)[1] for a in sys.argv if a.startswith("--only=")), None)
    for F in D["figs"]:
        key = f'{F["who"]}-{F["inst"]}'
        if only and key != only:
            continue
        b = B.Blend(F, L.Hand(F["who"]), B.cache_of(here, key))
        share, names = lobe(b, F)
        print(f"{key}: lobe {share:.3f}", dict(sorted(names.items(), key=lambda x: -x[1])))


if __name__ == "__main__":
    main()
