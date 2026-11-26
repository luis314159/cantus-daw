"""r42:arcus-hands, pass 6 — the figures' left hands against average hands for their own
stature and sex (the owner, 2026-11-26: "¿Puedes volver a validar las falanges de los dedos
y compararlas con tamaños promedio para el tamaño del músico?").

Ours, measured on the figure as the app loads it (``hand_rig.read_fig``, HAND_SCALE applied),
at rest:
- the stature: the skull's skin top to the floor, the floor under the ankle joint by ANSUR
  II's lateral malleolus height (its share of the stature);
- hand length: the wrist joint (the distal wrist crease's level) to the middle finger's tip,
  along the straightened chain; palm length: hand length less the middle finger's length;
- each finger's length from its proximal (palmar) crease — taken at the web beside it, the
  crotch's most distal skin — to the tip;
- the hand's breadth across the metacarpal heads (index to little), skin to skin;
- each finger's joint spacing (MCP-PIP, PIP-DIP, DIP-tip on the skin), the PIP's and the
  DIP's breadths.

Expected:
- ANSUR II (Gordon et al. 2014; the public CSVs, 1 986 women and 4 082 men): hand length,
  palm length, hand breadth and hand length less palm length (the middle finger) by linear
  regression on stature, the residual's SD for the percentile;
- finger lengths (proximal crease to tip, left hand): Chatzioglou, Pinar & Govsa 2024, Anat.
  Cell Biol. 57(2):172-182 (32 women 163 ± 6 cm, 19 men 177 ± 5 cm), scaled by stature;
- the bones' ratios: Buryanov & Kotiuk 2010, Int. J. Morphol. 28(3) (middle 0.6-0.65 of the
  proximal; distal 0.6-0.75 of the middle on the index, middle and ring, ~1 on the little);
- the joint breadths: Greiner 1991 (US Army hand anthropometry) means, as quoted in pass 3.

    python tools/arcus-figures/hand_anthro.py <figures dir> <ANSUR II dir> [--json=<out>]

``<ANSUR II dir>`` holds ``female.csv`` and ``male.csv`` (ANSUR_II_*_Public.csv from
tools.openlab.psu.edu/publicData; not in the repository).
"""
import csv
import json
import math
import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import hand_rig as hr  # noqa: E402

NAMES = ("index", "middle", "ring", "little")
# Chatzioglou et al. 2024, Table 1, the left hand (mm): mean, SD; and the sample's stature.
FINGERS = {"female": ((62.2, 3.3), (67.9, 4.8), (62.5, 3.2), (50.2, 3.0), 163.0),
           "male": ((68.2, 2.9), (75.5, 3.3), (69.8, 3.0), (56.8, 2.6), 177.0)}
# Greiner 1991's means (cm) at the PIP and the DIP, as pass 3 quotes them (index/middle).
JOINTS = {"female": (1.8, 1.55), "male": (2.1, 1.8)}
BONE_MP = (0.60, 0.65)
BONE_DM = {"index": (0.6, 0.75), "middle": (0.6, 0.75), "ring": (0.6, 0.75),
           "little": (0.85, 1.0)}
# The fingertip's soft tissue past the distal phalanx's tuft (cm), to set the skin's last
# segment against the bones' ratio.
PULP = 0.3


def pct(x, mu, sd):
    return 50.0 * (1.0 + math.erf((x - mu) / (sd * math.sqrt(2.0))))


def ansur(path, sex):
    rows = list(csv.DictReader(open(os.path.join(path, f"{sex}.csv"), encoding="latin-1")))

    def g(k):
        return np.array([float(r[k]) for r in rows]) / 10.0
    st = g("stature")
    out = {"n": len(rows), "malleolus": float((g("lateralmalleolusheight") / st).mean())}
    hl, pl = g("handlength"), g("palmlength")
    for k, y in (("hand length", hl), ("palm length", pl), ("hand breadth", g("handbreadth")),
                 ("middle finger (HL-PL)", hl - pl)):
        b, a = np.polyfit(st, y, 1)
        sd = float(np.std(y - (a + b * st)))
        out[k] = (float(a), float(b), sd)
    return out


def measure(path):
    fig = hr.read_fig(path)
    o = hr.side(0)
    B = fig["bones"]
    pos, ids, ws = fig["pos"], fig["ids"], fig["ws"]
    tris, mat = fig["tris"], fig["mat"]
    skin = np.unique(tris[mat == 0])
    top = pos[skin, 1].max()
    ankle = min(b[1][1] for b in B)
    wr = B[o + hr.HAND][1]
    heads = np.array([[B[o + hr.finger(f, k)][1] for k in range(3)] for f in range(5)])
    tails = np.array([B[o + hr.finger(f, 2)][2] for f in range(5)])

    def share(bone):
        return ((ids == bone) * ws).sum(1)
    kn = lambda f: heads[f, 0]  # noqa: E731
    back = np.cross(kn(1) - kn(4), (kn(2) + kn(3)) * 0.5 - wr)
    back /= np.linalg.norm(back)
    if back[0] < 0:
        back = -back
    out = {"top": float(top), "ankle": float(ankle), "fingers": {}}
    # Each finger's skin end down its last bone, and its webs.
    ends, axes = {}, {}
    for f in range(1, 5):
        a2 = tails[f] - heads[f, 2]
        a2 /= np.linalg.norm(a2)
        own = share(o + hr.finger(f, 2)) > 0.5
        ends[f] = float(((pos[own] - heads[f, 2]) @ a2).max())
        ax = heads[f, 1] - heads[f, 0]
        axes[f] = ax / np.linalg.norm(ax)
    webs = {}
    for f, g in ((1, 2), (2, 3), (3, 4)):
        both = (share(o + hr.finger(f, 0)) > 0.15) & (share(o + hr.finger(g, 0)) > 0.15)
        p = pos[both]
        # The crotch's most distal skin, as far along each finger from its knuckle.
        webs[(f, g)] = (float(((p - heads[f, 0]) @ axes[f]).max()),
                        float(((p - heads[g, 0]) @ axes[g]).max()))
    crease = {1: webs[(1, 2)][0], 2: 0.5 * (webs[(1, 2)][1] + webs[(2, 3)][0]),
              3: 0.5 * (webs[(2, 3)][1] + webs[(3, 4)][0]), 4: webs[(3, 4)][1]}
    for f in range(1, 5):
        P = float(np.linalg.norm(heads[f, 1] - heads[f, 0]))
        M = float(np.linalg.norm(heads[f, 2] - heads[f, 1]))
        D = ends[f]
        # Breadths just past the PIP and the DIP, side to side.
        br = []
        for k in (1, 2):
            a = (heads[f, k + 1] if k < 2 else tails[f]) - heads[f, k]
            L = np.linalg.norm(a)
            a /= L
            side = np.cross(a, back)
            side /= np.linalg.norm(side)
            own = share(o + hr.finger(f, k)) > 0.5
            q = pos[own] - heads[f, k]
            t = (q @ a) / L
            m = np.abs(t - 0.1) < 0.06
            br.append(float(np.ptp(q[m] @ side)))
        out["fingers"][NAMES[f - 1]] = {
            "P": P, "M": M, "D": D, "crease": crease[f], "length": P - crease[f] + M + D,
            "mcp_tip": P + M + D, "pip_w": br[0], "dip_w": br[1]}
    mid = out["fingers"]["middle"]
    out["hand length"] = float(np.linalg.norm(heads[2, 0] - wr)) + mid["mcp_tip"]
    out["middle finger (HL-PL)"] = mid["length"]
    out["palm length"] = out["hand length"] - mid["length"]
    # The breadth across the metacarpal heads: the skin within 0.5 cm of the knuckles' line.
    across = kn(1) - kn(4)
    across /= np.linalg.norm(across)
    c = (kn(1) + kn(4)) * 0.5
    along = (kn(2) + kn(3)) * 0.5 - wr
    along /= np.linalg.norm(along)
    palm = np.isin(ids[:, 0], [o + hr.HAND] + [o + hr.finger(f, 0) for f in range(1, 5)])
    q = pos[palm] - c
    m = (np.abs(q @ along) < 0.5) & (np.abs(q @ back) < 2.5) & (np.linalg.norm(q, axis=1) < 6)
    out["hand breadth"] = float(np.ptp(q[m] @ across))
    return out


def main():
    figs, ans = sys.argv[1], sys.argv[2]
    js = next((a.split("=", 1)[1] for a in sys.argv if a.startswith("--json=")), None)
    report = {}
    for who, sex in (("woman", "female"), ("man", "male")):
        A = ansur(ans, sex)
        m = measure(os.path.join(figs, f"{who}.fig"))
        st = (m["top"] - m["ankle"]) / (1.0 - A["malleolus"])
        rows = []

        def row(name, ours, mu, sd, note=""):
            rows.append({"what": name, "ours": ours, "mean": mu, "sd": sd,
                         "pct": pct(ours, mu, sd) if sd else None, "note": note})
        for k in ("hand length", "palm length", "hand breadth", "middle finger (HL-PL)"):
            a, b, sd = A[k]
            row(k, m[k], a + b * st, sd, "ANSUR II")
        F = FINGERS[sex]
        for i, n in enumerate(NAMES):
            mu, sd = F[i]
            k = st / F[4]
            row(f"{n} finger (crease-tip)", m["fingers"][n]["length"], mu / 10 * k, sd / 10 * k,
                "Chatzioglou 2024")
        for n in NAMES:
            f = m["fingers"][n]
            rows.append({"what": f"{n}: MCP-PIP / PIP-DIP / DIP-tip (skin)",
                         "ours": (f["P"], f["M"], f["D"])})
            rows.append({"what": f"{n}: middle/proximal", "ours": f["M"] / f["P"],
                         "range": BONE_MP, "note": "Buryanov & Kotiuk 2010 (bones)"})
            rows.append({"what": f"{n}: distal/middle (bone, skin less {PULP} cm)",
                         "ours": (f["D"] - PULP) / f["M"], "range": BONE_DM[n],
                         "note": "Buryanov & Kotiuk 2010 (bones)"})
        for n in ("index", "middle"):
            f = m["fingers"][n]
            pip, dip = JOINTS[sex]
            rows.append({"what": f"{n}: PIP / DIP breadth", "ours": (f["pip_w"], f["dip_w"]),
                         "mean": (pip, dip), "note": "Greiner 1991 (means)"})
        report[who] = {"stature": st, "rows": rows, "ansur_n": A["n"]}
        print(f"== {who}: stature {st:.1f} cm (ANSUR II n {A['n']})")
        for r in rows:
            o = r["ours"]
            os_ = " / ".join(f"{v:.2f}" for v in o) if isinstance(o, tuple) else f"{o:.2f}"
            if r.get("sd"):
                print(f"  {r['what']:42s} ours {os_:>18s}  expected {r['mean']:.2f} ± {r['sd']:.2f}"
                      f"  pct {r['pct']:5.1f}  [{r['note']}]")
            elif "range" in r:
                print(f"  {r['what']:42s} ours {os_:>18s}  range {r['range'][0]}-{r['range'][1]}"
                      f"  [{r['note']}]")
            elif "mean" in r:
                print(f"  {r['what']:42s} ours {os_:>18s}  mean {r['mean']}  [{r['note']}]")
            else:
                print(f"  {r['what']:42s} ours {os_:>18s}")
    if js:
        json.dump(report, open(js, "w"), indent=1)


if __name__ == "__main__":
    main()
