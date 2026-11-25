"""study_compare.py - Arcus measured by the reference study's own estimator, against the band
of real instruments (round 42, docs/round42/arcus-match.md, pass 2).

Usage (the study's venv: numpy, scipy, soundfile):
    <venv>\\Scripts\\python.exe -I tools\\arcus\\study_compare.py <study tools> <refdb> <renders> <out.json>

<renders> holds Arcus's held-out notes, `arcus-<cello|bass>-<arco|pizz>-<key>-<pp|mf|ff>.wav`
(`cargo run -p cantus-dsp --release --example arcus_match -- render <renders>`). Each is
analysed by the study's `bowlib_note` (the same estimator as the recordings, so its biases
cancel), the notes gathered by the study's `aggregate.family` as one more instrument, and each
feature compared with the family's band: [p25, p75] across instruments when the cell has four
or more, else [min, max]. The distance is how far Arcus lies outside the band (0 inside);
vectors (harmonics, spectrum, noise, AM, thump) by the RMS of their partials' or bands'
distances. Writes <out.json> and prints one line per feature.
"""
import json
import re
import sys
from pathlib import Path

import numpy as np

tools, refdb, renders, out_path = (Path(a) for a in sys.argv[1:5])
sys.path.insert(0, str(tools))
from bowlib_core import load  # noqa: E402
from bowlib_note import analyse_arco, analyse_pizz  # noqa: E402
from catalog import FAM  # noqa: E402
import aggregate  # noqa: E402

DYN = {"pp": 1, "mf": 4, "ff": 6}


def band(d):
    if not d:
        return None
    if d.get("n_inst", 0) >= 4 and "p25" in d:
        return d["p25"], d["p75"]
    return d["min"], d["max"]


def outside(x, b):
    if x is None or b is None or b[0] is None:
        return None
    lo, hi = b
    return max(lo - x, 0.0, x - hi)


def vec_outside(x, ref):
    if not x or not ref:
        return None
    lo, hi = ref["min"], ref["max"]
    d = []
    for v, a, b in zip(x, lo, hi):
        if v is None or a is None or b is None or not np.isfinite([v, a, b]).all():
            continue
        d.append(max(a - v, 0.0, v - b))
    return float(np.sqrt(np.mean(np.square(d)))) if d else None


recs = []
for p in sorted(renders.glob("arcus-*.wav")):
    m = re.match(r"arcus-(cello|bass)-(arco|pizz)-(\d+)-(pp|mf|ff)\.wav", p.name)
    if not m:
        continue
    fam, tech, key, dyn = m.groups()
    x, sr = load(str(p))
    fmin, fmax = FAM[fam]
    f = analyse_pizz if tech == "pizz" else analyse_arco
    r = f(x, sr, {"fmin": fmin, "fmax": fmax, "pre": None})
    if r is None:
        continue
    rec = {"path": str(p), "source": "arcus", "inst": "arcus", "family": fam, "tech": tech,
           "dyn": dyn, "dyn_id": DYN[dyn], "section": False, "lossy": False, "multi": False,
           "string": None, "index": 0, "sr": sr, "midi": float(69 + 12 * np.log2(r["f0_hz"] / 440.0))}
    rec.update(r)
    recs.append(rec)

result = {}
for fam in ("cello", "bass"):
    ref = json.loads((refdb / f"{fam}.json").read_text())
    ours = aggregate.family([r for r in recs if r["family"] == fam], fam)
    res = {}
    for feat, cells in ref["arco_scalars"].items():
        for cls, d in cells.items():
            o = (ours["arco_scalars"].get(feat) or {}).get(cls)
            if o and d:
                res[f"{feat}.{cls}"] = {"arcus": o["median"], "band": band(d), "outside": outside(o["median"], band(d))}
    for key, d in ref["harmonics_db_re_max"].items():
        o = ours["harmonics_db_re_max"].get(key)
        if o and d and key.endswith(("_soft", "_medium", "_loud")):
            res[f"harmonics.{key}"] = {"outside_rms_db": vec_outside(o["median"][:20], {"min": d["min"][:20], "max": d["max"][:20]})}
    for cls in ("soft", "medium", "loud"):
        for k, name in (("ltas_sixth_octave", "ltas"), ("noise_bands", "hnr")):
            rk = cls if k == "ltas_sixth_octave" else f"hnr_{cls}"
            d, o = (ref.get(k) or {}).get(rk), (ours.get(k) or {}).get(rk)
            if d and o:
                res[f"{name}.{cls}"] = {"outside_rms_db": vec_outside(o["median"], d)}
    for k, per in ref["dynamics_slopes_per_step"].items():
        vals = [v for v in per.values() if v is not None]
        o = (ours["dynamics_slopes_per_step"].get(k) or {}).get("arcus")
        if vals and o is not None:
            res[f"dyn_slope.{k}"] = {"arcus": o, "band": (min(vals), max(vals)), "outside": outside(o, (min(vals), max(vals)))}
    if ref.get("vibrato_am_db_per_partial") and ours.get("vibrato_am_db_per_partial"):
        res["vibrato_am_db"] = {"outside_rms_db": vec_outside(ours["vibrato_am_db_per_partial"]["median"], ref["vibrato_am_db_per_partial"])}
    if ours["body"].get("arcus"):
        mk = ours["body"]["arcus"]["marks"]
        for m_key in mk:
            vals = [b["marks"].get(m_key) for b in ref["body"].values() if isinstance(b.get("marks", {}).get(m_key), (int, float))]
            if vals and isinstance(mk[m_key], (int, float)):
                res[f"body.{m_key}"] = {"arcus": mk[m_key], "band": (min(vals), max(vals)), "outside": outside(mk[m_key], (min(vals), max(vals)))}
    # r42:arcus-p6: the body's harmonic-peak curve (the study's `body.body_estimate`) against
    # the measured instruments' [min, max] per 1/12-octave bin, RMS dB outside, by octave band
    # from 250 Hz to 10 kHz (the curves are levelled on 200 Hz - 4 kHz by the estimator).
    if ours["body"].get("arcus") and ref.get("body"):
        a = ours["body"]["arcus"]
        fc = np.array(a["fc_hz"], float)
        ha = np.array([np.nan if v is None else v for v in a["H_db"]], float)
        hr = np.array([[np.nan if v is None else v for v in b["H_db"]] for b in ref["body"].values()], float)
        lo, hi = np.nanmin(hr, axis=0), np.nanmax(hr, axis=0)
        med = np.nanmedian(hr, axis=0)
        for f0, f1 in ((250, 500), (500, 1000), (1000, 2000), (2000, 4000), (4000, 7000), (7000, 10000)):
            m = (fc >= f0) & (fc < f1) & np.isfinite(ha) & np.isfinite(lo)
            if m.sum():
                d = np.maximum(np.maximum(lo[m] - ha[m], 0.0), ha[m] - hi[m])
                res[f"body_curve.{f0}_{f1}"] = {"arcus": float(np.mean(ha[m])), "band": (float(np.mean(lo[m])), float(np.mean(hi[m]))),
                                                "outside": float(np.sqrt(np.mean(d * d))), "ref_median": float(np.mean(med[m]))}
    if ref.get("pizz_t60_by_freq") and ours.get("pizz_t60_by_freq"):
        ds = []
        for fq, d in ref["pizz_t60_by_freq"].items():
            o = ours["pizz_t60_by_freq"].get(fq)
            if o and d:
                b = band(d)
                ds.append(max(np.log2(b[0] / o["median"]), 0.0, np.log2(o["median"] / b[1])))
        res["pizz_t60_octaves"] = {"outside_rms_oct": float(np.sqrt(np.mean(np.square(ds)))) if ds else None}
    if ref.get("pizz_thump_excess_db") and ours.get("pizz_thump_excess_db"):
        res["pizz_thump"] = {"outside_rms_db": vec_outside(ours["pizz_thump_excess_db"]["median"], ref["pizz_thump_excess_db"])}
    o, d = ours.get("pizz_t60_fundamental"), ref.get("pizz_t60_fundamental")
    if o and d:
        res["pizz_t60_f1_s"] = {"arcus": o["median"], "band": band(d), "outside": outside(o["median"], band(d))}
    result[fam] = res
    print(f"== {fam} ({sum(1 for r in recs if r['family'] == fam)} notes)")
    for k, v in res.items():
        print(f"  {k}: " + ", ".join(f"{a}={b if not isinstance(b, float) else round(b, 3)}" for a, b in v.items()))

out_path.write_text(json.dumps(result, indent=1, default=lambda o: None if isinstance(o, float) and not np.isfinite(o) else str(o)))
