#!/usr/bin/env python3
"""At what levels real drummers strike, per genre (round 42, tympanum-real).

    python -I tools/tympanum-real/gmd_levels.py <gmd groove dir> <out.json>

The Groove MIDI Dataset's 1 150 performances (ten drummers, a Roland TD-11, CC BY 4.0): per
style and class, the velocity's quartiles and 5th/95th percentiles, the share of snare ghost
notes (velocity < 40), the hat's accent contrast (the 90th over the 10th percentile), the
rimshots' share of the snare's strokes, and how open the hat is played (the CC4 at its strokes).
The velocities are the module's own scale — what a player's force reads as on a TD-11 pad.
"""
import csv
import json
import sys
from collections import defaultdict
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
from midi import GMD, read_smf  # noqa: E402


def main():
    root, out = Path(sys.argv[1]), Path(sys.argv[2])
    vel = defaultdict(lambda: defaultdict(list))
    hat_open = defaultdict(list)
    for r in csv.DictReader(open(root / "info.csv", encoding="utf-8")):
        style = r["style"].split("/")[0]
        cc = 127
        for t, kind, a, b in read_smf(root / r["midi_filename"]):
            if kind == 0xB0 and a == 4:
                cc = b
            if kind == 0x90 and b > 0 and a in GMD:
                c = GMD[a]
                vel[style][c].append(b)
                vel["(todos)"][c].append(b)
                if c.startswith("hat_") and c != "hat_pedal":
                    hat_open[style].append(1 - cc / 127)
    res = {}
    for style, d in vel.items():
        s = {}
        for c, v in d.items():
            if len(v) < 30:
                continue
            v = np.array(v)
            s[c] = {"n": int(len(v)), "p05": float(np.percentile(v, 5)), "q1": float(np.percentile(v, 25)),
                    "med": float(np.median(v)), "q3": float(np.percentile(v, 75)), "p95": float(np.percentile(v, 95))}
        sn = np.array(d.get("snare", []) or [0])
        s["ghost_share"] = float(np.mean(sn < 40))
        hc = np.array(d.get("hat_closed", []) or [1])
        s["hat_accent_ratio"] = float(np.percentile(hc, 90) / max(1, np.percentile(hc, 10)))
        rim = len(d.get("rimshot", []))
        s["rimshot_share"] = rim / max(1, rim + len(d.get("snare", [])))
        if style in hat_open:
            s["hat_open_med"] = float(np.median(hat_open[style]))
        res[style] = s
    out.write_text(json.dumps(res, indent=1), encoding="utf-8")
    chart(res, out.with_suffix(".png"))
    for st in ("(todos)", "rock", "pop", "funk", "jazz", "latin", "soul", "gospel", "punk", "hiphop"):
        if st in res:
            r = res[st]
            print(f"{st:10} snare med {r.get('snare', {}).get('med', 0):5.0f} ghosts {r['ghost_share']:.2f}"
                  f" kick med {r.get('kick', {}).get('med', 0):5.0f} hat {r.get('hat_closed', {}).get('q1', 0):.0f}-"
                  f"{r.get('hat_closed', {}).get('q3', 0):.0f} ride med {r.get('ride', {}).get('med', 0):.0f}"
                  f" rim {r['rimshot_share']:.2f}")



def chart(res, out):
    """Each genre's snare and kick velocities (quartiles, 5th-95th) and the ghost notes' share."""
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    styles = [s for s in ("rock", "pop", "funk", "soul", "gospel", "jazz", "latin", "afrocuban", "hiphop",
                          "punk", "reggae", "neworleans") if s in res]
    fig, ax = plt.subplots(figsize=(9, 0.42 * len(styles) + 1.4), dpi=130)
    fig.patch.set_facecolor("#fcfcfb")
    ax.set_facecolor("#fcfcfb")
    for i, st in enumerate(styles):
        y = len(styles) - 1 - i
        for cls, col, dy in (("snare", "#2a78d6", 0.13), ("kick", "#eb6834", -0.13)):
            q = res[st].get(cls)
            if not q:
                continue
            ax.plot([q["p05"], q["p95"]], [y + dy] * 2, color=col, lw=1, alpha=0.6)
            ax.plot([q["q1"], q["q3"]], [y + dy] * 2, color=col, lw=5, solid_capstyle="round")
            ax.plot([q["med"]], [y + dy], "o", color="#fcfcfb", ms=4)
        ax.text(131, y, f"{100 * res[st]['ghost_share']:.0f} %", va="center", fontsize=8.5, color="#52514e")
    ax.set_yticks(range(len(styles)))
    ax.set_yticklabels(list(reversed(styles)), fontsize=9)
    ax.set_xlim(0, 142)
    ax.set_xlabel("velocidad MIDI (Roland TD-11)", fontsize=9, color="#52514e")
    ax.text(131, len(styles) - 0.4, "fantasmas\n(caja < 40)", fontsize=8, color="#52514e", va="bottom")
    for s in ("top", "right", "left"):
        ax.spines[s].set_visible(False)
    ax.grid(axis="x", color="#e4e3df", lw=0.6)
    ax.plot([], [], color="#2a78d6", lw=5, label="caja")
    ax.plot([], [], color="#eb6834", lw=5, label="bombo")
    ax.legend(frameon=False, fontsize=8.5, loc="lower left", bbox_to_anchor=(0, 1.0), ncol=2)
    ax.set_title("A qué fuerza tocan los bateristas reales, por género (Groove MIDI Dataset)",
                 loc="left", fontsize=11, pad=26)
    fig.tight_layout()
    fig.savefig(out, facecolor="#fcfcfb")


if __name__ == "__main__":
    main()
