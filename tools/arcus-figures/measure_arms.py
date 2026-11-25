"""r42:arcus-polish — the realistic figures' anthropometry, measured on what
``build_figures.py`` builds (cm): the stature, the shoulder joints' span, each arm
segment's length, and the arm's girth (the convex outline of the mesh's section across the
arm) at the deltoid, the upper arm's middle, the elbow, the forearm's top and middle and
the wrist — of the skin and of what covers it (her velvet, his jacket).

    python tools/arcus-figures/measure_arms.py <makehuman/data> <system assets> [--only=woman|man]

Female references (slim adult; ANSUR II, Gordon et al. 2014, women's medians): stature
~163 cm, biacromial ~36 cm, upper arm (relaxed) ~28 cm (slim 25-27), elbow ~22-23 cm,
forearm (max) ~23 cm, wrist ~15 cm. Men: biacromial ~41 cm, upper arm ~33, forearm ~29,
wrist ~17.5.
"""

import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import build_figures as bf  # noqa: E402
import mh  # noqa: E402


def hull(pts):
    pts = sorted(map(tuple, pts))

    def cross(o, a, b):
        return (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
    lo, up = [], []
    for p in pts:
        while len(lo) >= 2 and cross(lo[-2], lo[-1], p) <= 0:
            lo.pop()
        lo.append(p)
    for p in reversed(pts):
        while len(up) >= 2 and cross(up[-2], up[-1], p) <= 0:
            up.pop()
        up.append(p)
    return np.array(lo[:-1] + up[:-1])


def girth(pos, tris, a, b, t, reach=7.0):
    """The convex outline's length of the mesh's section by the plane across a-b at
    ``t`` (every triangle edge the plane cuts, within ``reach`` of the axis)."""
    d = b - a
    L = np.linalg.norm(d)
    d = d / L
    c = a + d * (L * t)
    s = (pos - c) @ d
    e = np.concatenate([tris[:, [0, 1]], tris[:, [1, 2]], tris[:, [2, 0]]])
    e = e[np.sign(s[e[:, 0]]) != np.sign(s[e[:, 1]])]
    if len(e) < 6:
        return float("nan")
    k = s[e[:, 0]] / (s[e[:, 0]] - s[e[:, 1]])
    pts = pos[e[:, 0]] + (pos[e[:, 1]] - pos[e[:, 0]]) * k[:, None]
    radial = pts - c
    pts = radial[np.linalg.norm(radial, axis=1) < reach]
    if len(pts) < 6:
        return float("nan")
    u = np.cross(d, [0.0, 0.0, 1.0])
    u /= np.linalg.norm(u)
    v = np.cross(d, u)
    P = hull(np.stack([pts @ u, pts @ v], 1))
    return float(np.sum(np.linalg.norm(P - np.roll(P, 1, 0), axis=1)))


LEVELS = (("deltoid", 0, 0.18), ("upper arm", 0, 0.5), ("elbow", 0, 0.98),
          ("forearm top", 1, 0.2), ("forearm mid", 1, 0.5), ("wrist", 1, 0.93))


def measure(fig, J, cloth):
    tri = fig.tris
    out = {"stature": float(np.ptp(fig.pos[:, 1]))}
    sh = J["upperarm01.L____head"]
    out["shoulder joints' span"] = float(np.linalg.norm(sh - J["upperarm01.R____head"]))
    el, wr = J["lowerarm01.L____head"], J["wrist.L____head"]
    out["upper arm (joint to joint)"] = float(np.linalg.norm(el - sh))
    out["forearm (joint to joint)"] = float(np.linalg.norm(wr - el))
    left = fig.pos[:, 0] > 2.0
    for what, mats in (("skin", (bf.SKIN,)), ("cloth", cloth)):
        sel = tri[np.isin(fig.mat, mats) & left[tri].all(1)]
        for name, seg, t in LEVELS:
            a, b = (sh, el) if seg == 0 else (el, wr)
            out[f"{what}: {name}"] = girth(fig.pos, sel, a, b, t)
    return out


def main():
    data, assets = sys.argv[1:3]
    only = next((a.split("=", 1)[1] for a in sys.argv if a.startswith("--only=")), None)
    bf.BASE = mh.read_obj(os.path.join(data, "3dobjs", "base.obj"))
    for sex, name, cloth in (("female", "woman", (bf.VELVET,)), ("male", "man", (bf.WOOL,))):
        if only and only != name:
            continue
        fig, J = bf.make(sex, data, assets)
        m = measure(fig, J, cloth)
        print(name)
        for k, v in m.items():
            print(f"  {k}: {v:.1f}")


if __name__ == "__main__":
    main()
