"""r42:arcus-upper — the left hand on the neck away from the app (sheet v7): a pose dumped by
``CANTUS_DUMP_HAND=1 cargo test -p cantus-ui --test arcus_hold
the_left_arm_keeps_the_photographs_angles -- --nocapture`` (its ``HAND {...}`` lines),
posed as ``arm_aims`` poses it (``hand_rig.py``) on the figure's left hand, drawn with the
neck and its strings from the hall's side, the scroll's and the player's.

    python tools/arcus-figures/hand_left.py <dump.txt> <Bowed> <Hold> <semis> <out.png>
"""
import json
import os
import sys

import numpy as np
from PIL import Image

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import hand_rig as hr  # noqa: E402
import preview  # noqa: E402


def load(path, b, hold, semis):
    for line in open(path, encoding="utf-8", errors="replace"):
        if line.startswith("HAND "):
            d = json.loads(line[5:])
            if d["b"] == b and d["hold"] == hold and abs(d["semis"] - semis) < 0.01:
                return d
    raise SystemExit("no such pose in the dump")


def main():
    dump, b, hold, semis, out = sys.argv[1:6]
    d = load(dump, b, hold, float(semis))
    who = "woman" if hold == "Woman" else "man"
    fig = hr.read_fig(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..",
                                   "crates", "cantus-ui", "assets", "figures", f"{who}.fig"))
    A = lambda v: np.array(v, float)  # noqa: E731
    fingers = [[A(p) for p in f] for f in d["fingers"]]
    thumb = [A(p) for p in d["thumb"]]
    hr.EXACT_F = d["playing"] + 1 if d["playing"] >= 0 else -1
    if os.environ.get("NORELAX"):
        hr.RELAX = (0.0, 99.0)
    xf = hr.pose_joints(fig, 0, A(d["wrist"]), fingers, thumb)
    pos, _ = hr.skin_hand(fig, 0, xf)
    tris = hr.hand_tris(fig, 0, 0.5)
    det = fig["det"]
    skin = np.array([218, 166, 136]) / 255.0
    col = np.clip(skin[None] * det[:, :1] * (1 + det[:, 1:2] * np.array([0.22, -0.12, -0.18])), 0, 1)
    # The neck: a box along the strings under the fingerboard, the strings over it.
    ex, ey, ez = A(d["ex"]), A(d["ey"]), A(d["ez"])
    nut, br = A(d["nut"]), A(d["bridge"])
    w = 1.2 if d["b"] in ("Violin", "Viola") else 1.6
    depth = 2.2 if d["b"] in ("Violin", "Viola") else 3.6
    pts, tt = [], []
    for y0 in (0.0, 1.0):
        c = nut + (br - nut) * (y0 * 0.7)
        for xx in (-w, w):
            for zz in (-0.2, -depth):
                pts.append(c + ex * xx + ez * zz)
    box = [[0, 1, 3], [0, 3, 2], [4, 6, 7], [4, 7, 5], [0, 4, 5], [0, 5, 1], [2, 3, 7],
           [2, 7, 6], [0, 2, 6], [0, 6, 4], [1, 5, 7], [1, 7, 3]]
    pts = np.array(pts)
    allp = np.vstack([pos, pts])
    allt = np.vstack([tris, np.array(box) + len(pos)])
    allc = np.vstack([col, np.tile([[0.12, 0.08, 0.06]], (len(pts), 1))])
    flat = allt.ravel()
    nrm = preview.normals(allp, allt)[flat]
    pp, cc = allp[flat], allc[flat]
    ti = np.arange(len(flat)).reshape(-1, 3)
    c = fingers[1][0]
    ims = []
    for k, eye in enumerate((c + ex * 22 + ez * 10 + ey * 2, c - ey * 30 + ez * 8,
                             c + ez * 25 - ex * 4)):
        o = out.replace(".png", f"-{k}.png")
        preview.render(pp, ti, cc, cam=(eye, c), size=(420, 420), fov=0.6, nrm=nrm, out=o,
                       key=(-0.4, 0.8, 0.6))
        ims.append(Image.open(o))
    im = Image.new("RGB", (1260, 420))
    for i, x in enumerate(ims):
        im.paste(x, (420 * i, 0))
    im.save(out)


if __name__ == "__main__":
    main()
