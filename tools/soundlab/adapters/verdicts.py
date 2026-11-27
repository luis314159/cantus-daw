"""Two lab reports' classical verdicts side by side (round 42, strings-refs): the feature list
and the verdict rules of the r42 Arcus analysis (Part 2: "dentro" every group in the band,
"dentro casi siempre" three quarters, else "fuera"; a vector "dentro" at 85 % of its elements
or an RMS outside under 0.5) on an old and a new report — e.g. the analysis's references
against strings-hq's, or one candidate against another.

    python -I tools/soundlab/adapters/verdicts.py <out.json> <inst>=<old report.json>,<new report.json> ...
"""
import json
import sys
from pathlib import Path

import numpy as np

FEATS = [
    ("harmonics.h_db_re_max", "arco", "Armónicos 1–20 (dB re el más fuerte)", "dB", 1),
    ("centroid.harmonic_centroid_n", "arco", "Centroide armónico (nº de armónico)", "", 2),
    ("centroid.mean_hz", "arco", "Centroide espectral medio", "Hz", 0),
    ("noise.res_valid_db_re_harm", "arco", "Ruido entre armónicos (dB re armónicos)", "dB", 1),
    ("noise.hnr_band_db", "arco", "HNR por tercio de octava", "dB", 1),
    ("attack.rise_10_90_ms", "arco", "Ataque: subida 10–90 %", "ms", 0),
    ("attack.noise_burst_db_over_steady", "arco", "Ataque: ráfaga de ruido sobre el estable", "dB", 1),
    ("attack.burst_db", "arco", "Ataque: energía 0–25 ms re 60–160 ms", "dB", 1),
    ("attack.build_order_rho", "arco", "Ataque: orden en que entran los armónicos (ρ)", "", 2),
    ("t_steady.t_steady_s", "arco", "t_steady (el espectro se asienta)", "s", 3),
    ("vibrato.rate_hz", "arco", "Vibrato: frecuencia", "Hz", 2),
    ("vibrato.extent_pp_cents", "arco", "Vibrato: profundidad pico a pico", "cents", 1),
    ("vibrato.onset_s", "arco", "Vibrato: inicio", "s", 2),
    ("vibrato.extent_jitter", "arco", "Vibrato: irregularidad de la profundidad", "", 2),
    ("vibrato.am_db", "arco", "Vibrato: AM por armónico", "dB", 1),
    ("pitch.cents_from_nominal", "arco", "Afinación (cents re la nota)", "cents", 1),
    ("pitch.spread_cents_p10_p90", "arco", "Dispersión de la afinación p10–p90", "cents", 1),
    ("loudness.crest_db", "arco", "Factor de cresta", "dB", 1),
    ("decay.release_db_per_s", "arco", "Final de nota: caída", "dB/s", 0),
    ("room.t20_s", "arco", "Sala: T20", "s", 2),
    ("harmonics.h_db_re_max", "pizz", "Pizz: armónicos 1–20", "dB", 1),
    ("decay.t60_f1_s", "pizz", "Pizz: T60 de la fundamental", "s", 2),
    ("decay.t60_s", "pizz", "Pizz: T60 por armónico", "s", 2),
    ("attack.rise_10_90_ms", "pizz", "Pizz: subida 10–90 %", "ms", 1),
    ("noise.hnr_valid_median_db", "pizz", "Pizz: HNR mediano", "dB", 1),
]


def summary(rep, key, art):
    rows = []
    for g, d in sorted(rep["groups"].items()):
        if not g.startswith(art + "|") or key not in d:
            continue
        v = d[key]
        if "vector" in v:
            rows.append({"vector": True, "rms": v["outside_rms"], "in_share": v["in_share"], "group": g})
        else:
            p, b = v["place"], v["band"]
            rows.append({"value": p["value"], "p10": b["p10"], "p90": b["p90"], "outside": p["outside"],
                         "widths": p["widths"], "status": p["status"], "n_inst": b["n_inst"], "n": b["n"],
                         "n_cand": v.get("n_cand"), "group": g})
    if not rows:
        return None
    if rows[0].get("vector"):
        return {"kind": "vector", "groups": len(rows), "rms_out_mean": float(np.mean([r["rms"] for r in rows])),
                "in_share_mean": float(np.mean([r["in_share"] for r in rows]))}
    outs = [r for r in rows if r["status"] != "in"]
    above = sum(r["status"] == "above" for r in outs)
    return {"kind": "scalar", "groups": len(rows), "in": sum(r["status"] == "in" for r in rows),
            "widths_mean": float(np.mean([r["widths"] for r in rows])),
            "out_median": float(np.median([r["outside"] for r in outs])) if outs else 0.0,
            "dir": None if not outs else ("por encima" if above * 2 >= len(outs) else "por debajo"),
            "value_median": float(np.median([r["value"] for r in rows])),
            "band": [float(np.median([r["p10"] for r in rows])), float(np.median([r["p90"] for r in rows]))],
            "n_inst_median": float(np.median([r["n_inst"] for r in rows])),
            "n_median": float(np.median([r["n"] for r in rows])),
            "n_cand_median": float(np.median([r["n_cand"] or 0 for r in rows]))}


def cls(s):
    if s is None:
        return "na"
    if s["kind"] == "vector":
        return "in" if (s["rms_out_mean"] < 0.5 or s["in_share_mean"] >= 0.85) else "out"
    if s["in"] == s["groups"]:
        return "in"
    return "mostly" if s["in"] >= 0.75 * s["groups"] else "out"


def short(s, unit, fmt):
    if s is None:
        return "—"
    if s["kind"] == "vector":
        return f"{100 * s['in_share_mean']:.0f}% dentro, RMS fuera {s['rms_out_mean']:.{fmt}f}"
    tail = f", fuera {s['out_median']:.{fmt}f} {unit} {s['dir']}" if s["dir"] else ""
    return (f"{s['in']}/{s['groups']} dentro{tail}; Arcus {s['value_median']:.{fmt}g} vs "
            f"[{s['band'][0]:.{fmt}g}, {s['band'][1]:.{fmt}g}] (inst {s['n_inst_median']:.0f}, n {s['n_median']:.0f}, cand {s['n_cand_median']:.0f})")


def main(out, specs):
    res = {}
    for sp in specs:
        inst, paths = sp.split("=", 1)
        old_p, new_p = paths.split(",")
        old, new = json.loads(Path(old_p).read_text()), json.loads(Path(new_p).read_text())
        res[inst] = []
        print(f"\n===== {inst}")
        for key, art, label, unit, fmt in FEATS:
            so, sn = summary(old, key, art), summary(new, key, art)
            co, cn = cls(so), cls(sn)
            flag = "  " if co == cn else "**"
            res[inst].append({"key": f"{art}:{key}", "label": label, "old": so, "new": sn, "old_class": co,
                              "new_class": cn, "changed": co != cn})
            print(f"{flag} {label[:42]:42s} {co:6s} -> {cn:6s} | {short(sn, unit, fmt)}")
    Path(out).write_text(json.dumps(res, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(sys.argv[1], sys.argv[2:])
