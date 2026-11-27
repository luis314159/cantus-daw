"""r42:arcus-hands, pass 3 — each finger's breadth (side to side) and depth (back to pad)
along its length, on the figure as the app loads it (``hand_rig.read_fig``: the load-time
edits applied), in rest pose: the numbers the fingers' form is set against (the
photographs crop-08/02/11 and the hand tables; ``build_figures.py::FINGER_FORM``).

    python tools/arcus-figures/finger_profile.py <figures dir> [woman|man] [--side=0]

Per finger (index … little) and phalanx: the bone's length and, at a few places along it,
the skin's breadth and depth (cm), the nail's length and breadth.
"""
import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import hand_rig as hr  # noqa: E402

NAIL = 2  # the .fig's nail material


def profile(fig, s, f, back):
    """[(k, L, [(t, breadth, depth)])] for finger ``f`` (1 index … 4 little) of side ``s``."""
    out = []
    o = hr.side(s)
    for k in range(3):
        b = o + hr.finger(f, k)
        _, h, tl, _ = fig["bones"][b]
        ax = tl - h
        L = np.linalg.norm(ax)
        a = ax / L
        d = back - a * (back @ a)
        d = d / np.linalg.norm(d)
        side = np.cross(a, d)
        own = (fig["ids"] == b) * fig["ws"]
        w = own.sum(1)
        sel = w > 0.5
        q = fig["pos"][sel] - h
        t = (q @ a) / L
        rows = []
        for tt in (0.1, 0.3, 0.5, 0.7, 0.9) + ((1.05, 1.15) if k == 2 else ()):
            m = np.abs(t - tt) < 0.06
            if m.sum() < 4:
                continue
            x, y = q[m] @ side, q[m] @ d
            rows.append((tt, x.max() - x.min(), y.max() - y.min()))
        out.append((k, L, rows))
    return out


def nails(fig, s, f):
    o = hr.side(s)
    b = o + hr.finger(f, 2)
    _, h, tl, _ = fig["bones"][b]
    a = (tl - h) / np.linalg.norm(tl - h)
    tri = fig["tris"][fig["mat"] == NAIL]
    v = np.unique(tri)
    own = ((fig["ids"][v] == b) * fig["ws"][v]).sum(1) > 0.5
    v = v[own]
    if len(v) == 0:
        return None
    q = fig["pos"][v] - h
    al = q @ a
    r = q - np.outer(al, a)
    return al.max() - al.min(), max(np.ptp(r[:, i]) for i in range(3))


def main():
    figdir = sys.argv[1]
    whos = [a for a in sys.argv[2:] if not a.startswith("--")] or ["woman", "man"]
    s = int(next((a.split("=")[1] for a in sys.argv if a.startswith("--side=")), "0"))
    for who in whos:
        fig = hr.read_fig(os.path.join(figdir, f"{who}.fig"))
        o = hr.side(s)
        kn = lambda f: fig["bones"][o + hr.finger(f, 0)][1]  # noqa: E731
        wr = fig["bones"][o + hr.HAND][1]
        back = np.cross(kn(1) - kn(4), (kn(2) + kn(3)) * 0.5 - wr)
        back = back / np.linalg.norm(back)
        if (back[0] < 0) == (s == 0):
            back = -back
        print(f"== {who} (side {s}): hand length wrist->middle MCP "
              f"{np.linalg.norm(kn(2) - wr):.2f} cm, MCP span {np.linalg.norm(kn(1) - kn(4)):.2f}")
        for f, name in zip(range(1, 5), ("index", "middle", "ring", "little")):
            pr = profile(fig, s, f, back)
            tot = sum(L for _, L, _ in pr)
            print(f"  {name}: bones {' / '.join('%.2f' % L for _, L, _ in pr)} (sum {tot:.2f})")
            for k, L, rows in pr:
                print("    P%d " % (k + 1) + "  ".join(
                    f"t{t:.2f} {b_:.2f}x{d_:.2f}" for t, b_, d_ in rows))
            n = nails(fig, s, f)
            if n:
                print(f"    nail {n[0]:.2f} long, {n[1]:.2f} across")


if __name__ == "__main__":
    main()
