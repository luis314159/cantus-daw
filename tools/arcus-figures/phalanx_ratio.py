"""r42:arcus-hands, pass 6 — the fingers' segments as the app loads the figures (the owner,
2026-11-26: "las últimas falanges sí se ven un poco largas"): per finger, on the rest pose,

- the joints' spacing: MCP -> PIP -> DIP (the bones' heads) and DIP -> the last bone's tail;
- the skin: DIP -> the skin's end (the farthest skin point down the last bone: the pulp
  and the nail past the bone), the nail's overhang past the tail;
- the back's crests: the most dorsal skin point at each joint's plane, crest to crest, and
  the DIP's crest to the skin's end — the lengths a front camera sees on the dorsum;
- the ratios middle/proximal and distal/middle (joints, and the dorsum's), and the middle
  finger's MCP-to-tip over wrist-to-MCP.

    python tools/arcus-figures/phalanx_ratio.py <figures dir> [woman|man]
"""
import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import hand_rig as hr  # noqa: E402


def measure(fig, f, back, s=0):
    o = hr.side(s)
    B = fig["bones"]
    heads = [B[o + hr.finger(f, k)][1] for k in range(3)]
    tail = B[o + hr.finger(f, 2)][2]
    ids = [o + hr.finger(f, k) for k in range(3)]
    share = np.zeros(len(fig["pos"]))
    for b in ids:
        share += ((fig["ids"] == b) * fig["ws"]).sum(1)
    mine = share > 0.5
    q = fig["pos"][mine]
    own2 = ((fig["ids"][mine] == ids[2]) * fig["ws"][mine]).sum(1) > 0.5
    a2 = tail - heads[2]
    a2 = a2 / np.linalg.norm(a2)
    end = (q[own2] - heads[2]) @ a2
    skin_end = heads[2] + a2 * end.max()
    joints = heads + [tail]
    lens = [np.linalg.norm(joints[k + 1] - joints[k]) for k in range(3)]
    # The dorsal crests: at each joint's plane (perpendicular to the mean of the bones
    # either side) the most dorsal skin point within 1.5 mm of it.
    crests = []
    for k in range(3):
        if k == 0:
            ax = joints[1] - joints[0]
        else:
            ax = (joints[k + 1] - joints[k]) / np.linalg.norm(joints[k + 1] - joints[k]) + \
                (joints[k] - joints[k - 1]) / np.linalg.norm(joints[k] - joints[k - 1])
        ax = ax / np.linalg.norm(ax)
        allp = fig["pos"]
        rel = allp - joints[k]
        along = rel @ ax
        rad = np.linalg.norm(rel - np.outer(along, ax), axis=1)
        m = (np.abs(along) < 0.15) & (rad < 1.2)
        dd = back - ax * (back @ ax)
        dd = dd / np.linalg.norm(dd)
        i = np.argmax(rel[m] @ dd)
        crests.append(allp[m][i])
    dors = [np.linalg.norm(crests[1] - crests[0]), np.linalg.norm(crests[2] - crests[1]),
            np.linalg.norm(skin_end - crests[2])]
    return {
        "joints": lens,
        "skin_tip": float(np.linalg.norm(skin_end - heads[2])),
        "past_tail": float(end.max() - np.linalg.norm(tail - heads[2])),
        "dorsum": dors,
        "mcp_tip": float(np.linalg.norm(skin_end - heads[0])),
    }


def main():
    figdir = sys.argv[1]
    whos = [a for a in sys.argv[2:] if not a.startswith("--")] or ["woman", "man"]
    for who in whos:
        fig = hr.read_fig(os.path.join(figdir, f"{who}.fig"))
        o = hr.side(0)
        kn = lambda f: fig["bones"][o + hr.finger(f, 0)][1]  # noqa: E731
        wr = fig["bones"][o + hr.HAND][1]
        back = np.cross(kn(1) - kn(4), (kn(2) + kn(3)) * 0.5 - wr)
        back = back / np.linalg.norm(back)
        if back[0] < 0:
            back = -back
        print(f"== {who}")
        print("  finger  joints P/M/D(tail)   D skin  past  | dorsum P/M/D(tip)   | M/P  D/M  Dskin/M  dD/dM | MCP-tip/wrist-MCP")
        for f, name in zip(range(1, 5), ("index", "middle", "ring", "little")):
            r = measure(fig, f, back)
            P, M, D = r["joints"]
            dP, dM, dD = r["dorsum"]
            hand = np.linalg.norm(kn(f) - wr)
            print(f"  {name:7s} {P:.2f} / {M:.2f} / {D:.2f}   {r['skin_tip']:.2f}  {r['past_tail']:+.2f} | "
                  f"{dP:.2f} / {dM:.2f} / {dD:.2f}  | {M / P:.2f} {D / M:.2f} {r['skin_tip'] / M:.2f}    "
                  f"{dD / dM:.2f}  | {r['mcp_tip'] / hand:.2f} ({r['mcp_tip']:.2f} / {hand:.2f})")


if __name__ == "__main__":
    main()
