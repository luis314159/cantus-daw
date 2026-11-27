"""r42 arcus-classical — the classical pass's numbers, before and after, from the sound lab's
held-out reports (`tools/soundlab`, ADR 0160) of the four instruments: per feature the groups
inside the band, the mean band widths outside and the candidate's median, side by side for
every run named; and Arcus's absolute pitch (the player on) on its held-out notes.

    python -I tools/arcus/classical/summary.py <out.json> <label>=<work>/<id>/runs/<run> ...

A run folder holds `report.json` and `measures.json` (`lab.py measure|report`). Only the rows the
classical pass works on are printed (`ROWS`); the JSON keeps every scalar feature.
"""
import json
import sys
from pathlib import Path

import numpy as np

ROWS = [
    ("loudness.crest_db", "arco", "crest dB"),
    ("t_steady.t_steady_s", "arco", "t_steady s"),
    ("attack.rise_10_90_ms", "arco", "rise 10-90 ms"),
    ("attack.noise_burst_db_over_steady", "arco", "burst dB"),
    ("attack.burst_db", "arco", "0-25 ms re 60-160 dB"),
    ("decay.release_db_per_s", "arco", "release dB/s"),
    ("room.t20_s", "arco", "T20 s"),
    ("centroid.mean_hz", "arco", "centroid Hz"),
    ("centroid.harmonic_centroid_n", "arco", "harm. centroid n"),
    ("noise.res_valid_db_re_harm", "arco", "noise dB re harm"),
    ("vibrato.onset_s", "arco", "vib onset s"),
    ("vibrato.rate_hz", "arco", "vib rate Hz"),
    ("vibrato.extent_pp_cents", "arco", "vib extent ct"),
    ("pitch.cents_from_nominal", "arco", "pitch ct"),
    ("pitch.spread_cents_p10_p90", "arco", "pitch spread ct"),
    ("harmonics.h_db_re_max", "arco", "harmonics (vec)"),
    ("noise.hnr_band_db", "arco", "HNR bands (vec)"),
    ("harmonics.h_db_re_max", "pizz", "pizz harmonics (vec)"),
    ("noise.hnr_valid_median_db", "pizz", "pizz HNR dB"),
    ("attack.rise_10_90_ms", "pizz", "pizz rise ms"),
    ("decay.t60_f1_s", "pizz", "pizz T60 f1 s"),
]


def feat(rep, key, art):
    rows = []
    for g, d in sorted(rep["groups"].items()):
        if not g.startswith(art + "|") or key not in d:
            continue
        v = d[key]
        if "vector" in v:
            rows.append({"g": g, "vec": True, "rms": v["outside_rms"], "in": v["in_share"]})
        else:
            p, b = v["place"], v["band"]
            rows.append({"g": g, "value": p["value"], "status": p["status"], "outside": p["outside"],
                         "widths": p["widths"], "p10": b["p10"], "p90": b["p90"]})
    if not rows:
        return None
    if rows[0].get("vec"):
        return {"kind": "vec", "groups": len(rows), "rms_mean": float(np.mean([r["rms"] for r in rows])),
                "in_mean": float(np.mean([r["in"] for r in rows]))}
    n_in = sum(r["status"] == "in" for r in rows)
    return {"kind": "scalar", "groups": len(rows), "in": n_in,
            "widths_mean": float(np.mean([r["widths"] for r in rows])),
            "value_median": float(np.median([r["value"] for r in rows])),
            "band": [float(np.median([r["p10"] for r in rows])), float(np.median([r["p90"] for r in rows]))],
            "rows": [{k: r[k] for k in ("g", "value", "status", "p10", "p90")} for r in rows]}


def pitch_abs(meas):
    c = np.array([r["f"].get("pitch.cents_from_nominal") for r in meas
                  if r["cell"][0] == "arco" and r["held_out"] and r["f"].get("pitch.cents_from_nominal") is not None])
    pz = np.array([r["f"].get("pitch.cents_from_nominal") for r in meas
                   if r["cell"][0] == "pizz" and r["held_out"] and r["f"].get("pitch.cents_from_nominal") is not None])
    out = {}
    if len(c):
        out.update({"arco_n": int(len(c)), "arco_median": float(np.median(c)),
                    "arco_within_1ct": float(np.mean(np.abs(c) <= 1.0)), "arco_abs_max": float(np.max(np.abs(c)))})
    if len(pz):
        out.update({"pizz_n": int(len(pz)), "pizz_median": float(np.median(pz))})
    return out


def fmt(s):
    if s is None:
        return "—"
    if s["kind"] == "vec":
        return f"{100 * s['in_mean']:.0f}% in, {s['rms_mean']:.2f} RMS out"
    return f"{s['in']}/{s['groups']} in, {s['widths_mean']:.2f} w, med {s['value_median']:.3g}"


def main(out, specs):
    runs = []
    for sp in specs:
        label, path = sp.split("=", 1)
        p = Path(path)
        rep = json.loads((p / "report.json").read_text(encoding="utf-8"))
        meas = json.loads((p / "measures.json").read_text(encoding="utf-8"))["rows"]
        runs.append((label, rep, meas))
    res = {}
    for label, rep, meas in runs:
        res[label] = {"subject": rep["subject"],
                      "rows": {f"{a}:{k}": feat(rep, k, a) for k, a, _ in ROWS},
                      "pitch_abs": pitch_abs(meas)}
    Path(out).write_text(json.dumps(res, indent=1), encoding="utf-8")
    w = 26
    print(f"{'':22s}" + "".join(f"{lab[:w]:>{w + 2}s}" for lab, _, _ in runs))
    for k, a, name in ROWS:
        print(f"{name:22s}" + "".join(f"{fmt(res[lab]['rows'][f'{a}:{k}'])[:w]:>{w + 2}s}" for lab, _, _ in runs))
    for lab, _, _ in runs:
        print(lab, "pitch", {k: round(v, 3) for k, v in res[lab]["pitch_abs"].items()})


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(sys.argv[1], sys.argv[2:])
