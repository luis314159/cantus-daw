"""aggregate.py - the reference database's distributions (Arcus study).

Usage:  venv\\Scripts\\python.exe -I tools\\aggregate.py
Reads refdb/notes/*.jsonl (extract.py) and refdb/phrases.jsonl (phrases.py); writes
refdb/<family>.json for cello, bass, viola and violin, and refdb/summary.json.

Every distribution is taken **across instruments**: first each instrument's median over its
notes in the cell, then the median, min, max (and quartiles when >= 4 instruments) of those.
Sections (VSCO's) are kept out of the family's numbers and reported apart. Pooled per-note
percentiles (p10/p50/p90) are given beside them. "Inside the band of real instruments" means
inside [min, max] across instruments (or [p25, p75] when the cell has >= 4).
"""
import json
from collections import defaultdict
from pathlib import Path

import numpy as np

import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
from body import body_estimate, body_marks  # noqa: E402
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "sound-refs"))
import cantus_paths as cp  # noqa: E402  ($CANTUS_REFS, $CANTUS_STUDY_REFDB: tools/sound-refs)

R = cp.env_or("CANTUS_STUDY_REFDB", Path(__file__).resolve().parents[3] / "research" / "instruments-next" / "bowed-refdb")
FAMS = ["cello", "bass", "viola", "violin"]
LOW = {"cello": 36, "bass": 28, "viola": 48, "violin": 55}
DCLS = {1: "soft", 2: "soft", 3: "medium", 4: "medium", 5: "loud", 6: "loud"}


def load():
    recs = []
    for f in sorted((R / "notes").glob("*.jsonl")):
        for line in f.read_text().splitlines():
            try:
                recs.append(json.loads(line))
            except json.JSONDecodeError:
                pass  # a line being written by a running extract.py
    return recs


def get(r, path):
    for k in path.split("."):
        if r is None:
            return None
        r = r.get(k) if isinstance(r, dict) else None
    return r


def dist(by_inst):
    """by_inst: {inst: [values]} -> the across-instrument distribution."""
    med = {k: float(np.median(v)) for k, v in by_inst.items() if len(v)}
    if not med:
        return None
    m = np.array(list(med.values()))
    pooled = np.concatenate([np.asarray(v, float) for v in by_inst.values() if len(v)])
    out = {"median": float(np.median(m)), "min": float(m.min()), "max": float(m.max()),
           "n_inst": len(m), "n_notes": int(len(pooled)),
           "pooled_p10": float(np.percentile(pooled, 10)), "pooled_p50": float(np.percentile(pooled, 50)),
           "pooled_p90": float(np.percentile(pooled, 90)), "per_inst": {k: float(f"{v:.4g}") for k, v in med.items()}}
    if len(m) >= 4:
        out["p25"], out["p75"] = float(np.percentile(m, 25)), float(np.percentile(m, 75))
    return out


def vdist(by_inst, n):
    """Vector features (length n): per-index across-instrument distribution."""
    per = {}
    for k, rows in by_inst.items():
        A = np.full((len(rows), n), np.nan)
        for i, r in enumerate(rows):
            r = np.asarray([np.nan if v is None else v for v in r[:n]], float)
            A[i, :len(r)] = r
        with np.errstate(all="ignore"):
            per[k] = np.nanmedian(A, axis=0) if len(rows) else None
    if not per:
        return None
    M = np.array(list(per.values()))
    with np.errstate(all="ignore"):
        return {"median": np.nanmedian(M, 0).round(2).tolist(), "min": np.nanmin(M, 0).round(2).tolist(),
                "max": np.nanmax(M, 0).round(2).tolist(), "n_inst": len(per),
                "per_inst": {k: v.round(2).tolist() for k, v in per.items()}}


def slope_within(rows, xkey, yfun):
    """Within-note slope of y on the dynamic ordinal (notes grouped by rounded midi + string)."""
    g = defaultdict(list)
    for r in rows:
        y = yfun(r)
        if y is not None and r.get(xkey) is not None:
            g[(round(r["midi"]), r.get("string"), r["source"])].append((r[xkey], y))
    xs, ys = [], []
    for v in g.values():
        if len({a for a, _ in v}) < 2:
            continue
        a = np.array(v, float)
        xs += list(a[:, 0] - a[:, 0].mean())
        ys += list(a[:, 1] - a[:, 1].mean())
    if len(xs) < 4:
        return None
    xs, ys = np.array(xs), np.array(ys)
    return float(np.sum(xs * ys) / np.sum(xs * xs))


def scalar_cells(rows, feats):
    out = {}
    for name, fn in feats.items():
        cell = defaultdict(lambda: defaultdict(list))
        for r in rows:
            v = fn(r)
            if v is None or (isinstance(v, float) and not np.isfinite(v)):
                continue
            cell["all"][r["inst"]].append(v)
            dc = DCLS.get(r.get("dyn_id"))
            if dc:
                cell[dc][r["inst"]].append(v)
        out[name] = {k: dist(v) for k, v in cell.items()}
    return out


def vib_ok(r):
    """Vibrato present, its rate inside 3.5-8.5 Hz (a peak at the search band's edge is drift)."""
    return bool(get(r, "vibrato.present")) and 3.5 <= (get(r, "vibrato.rate_hz") or 0) <= 8.5


def rel(r):
    x = r.get("release") or {}
    return x.get("decay_db_s") if x.get("released") and not x.get("truncated") else None


ARCO_FEATS = {
    "centroid_hz": lambda r: r.get("centroid_hz"),
    "centroid_harm": lambda r: r.get("centroid_harm"),
    "f1_re_max_db": lambda r: r.get("f1_re_max_db"),
    "noise_db_re_harm": lambda r: get(r, "noise.res_valid_db_re_harm"),
    "attack_t_steady_s": lambda r: get(r, "attack.t_steady_s"),
    "attack_t_rise_s": lambda r: get(r, "attack.t_rise_3db_s"),
    "attack_noise_burst_db": lambda r: get(r, "attack.noise_burst_db_over_steady"),
    "attack_build_order_rho": lambda r: get(r, "attack.build_order_rho"),
    "release_decay_db_s": rel,
    "mode_lock_dev_cents": lambda r: (float(np.nanmedian(np.abs([v for v in r["partial_dev_cents"] if v is not None])))
                                      if any(v is not None for v in r.get("partial_dev_cents", [])) else None),
    "vibrato_rate_hz": lambda r: get(r, "vibrato.rate_hz") if vib_ok(r) else None,
    "vibrato_extent_cents": lambda r: get(r, "vibrato.extent_cents") if vib_ok(r) else None,
    "vibrato_present": lambda r: 1.0 if vib_ok(r) else 0.0,
}


def family(recs, fam):
    allf = [r for r in recs if r["family"] == fam]
    solo = [r for r in allf if not r.get("section")]
    arco = [r for r in solo if r["tech"].startswith("arco") and "h_db_re_max" in r]
    pizz = [r for r in solo if r["tech"] == "pizz" and "t60_s" in r]
    out = {"family": fam, "instruments": sorted({r["inst"] for r in solo}),
           "notes": {"arco": len(arco), "pizz": len(pizz)},
           "per_instrument_notes": {k: sum(1 for r in arco if r["inst"] == k) for k in sorted({r["inst"] for r in arco})}}
    out["arco_scalars"] = scalar_cells(arco, ARCO_FEATS)
    # registers: octaves from the family's lowest note
    regs = defaultdict(lambda: defaultdict(list))
    for r in arco:
        o = int((r["midi"] - LOW[fam] + 0.5) // 12)
        dc = DCLS.get(r.get("dyn_id"), "na")
        regs[(o, dc)][r["inst"]].append(r["h_db_re_max"])
        regs[(o, "all")][r["inst"]].append(r["h_db_re_max"])
    out["harmonics_db_re_max"] = {f"oct{o}_{dc}": dict(vdist(v, 40), midi_lo=LOW[fam] + 12 * o)
                                  for (o, dc), v in sorted(regs.items())}
    lt = defaultdict(lambda: defaultdict(list))
    fc = None
    for r in arco:
        fc = fc or r["ltas_fc"]
        lt[DCLS.get(r.get("dyn_id"), "na")][r["inst"]].append(r["ltas_db"])
        lt["all"][r["inst"]].append(r["ltas_db"])
    out["ltas_sixth_octave"] = {"fc_hz": fc, **{k: vdist(v, len(fc)) for k, v in lt.items()}} if fc else None
    nb = defaultdict(lambda: defaultdict(list))
    bands = None
    for r in arco:
        nz = r.get("noise") or {}
        if "hnr_band_db" not in nz:
            continue
        bands = bands or nz["bands_hz"]
        vals = [h if ok else None for h, ok in zip(nz["hnr_band_db"], nz["band_valid"])]
        shape = [h if ok else None for h, ok in zip(nz["res_band_db_re_harm_total"], nz["band_valid"])]
        for dc in ("all", DCLS.get(r.get("dyn_id"), "na")):
            nb["hnr_" + dc][r["inst"]].append(vals)
            nb["shape_" + dc][r["inst"]].append(shape)
    out["noise_bands"] = {"fc_hz": bands, **{k: vdist(v, len(bands)) for k, v in nb.items()}} if bands else None
    out["dynamics_slopes_per_step"] = {}
    for k, fn in {"log2_centroid_oct": lambda r: np.log2(r["centroid_hz"]),
                  "level_db": lambda r: r.get("level_db_steady"),
                  "noise_db_re_harm": lambda r: get(r, "noise.res_valid_db_re_harm"),
                  "f1_re_max_db": lambda r: r.get("f1_re_max_db")}.items():
        out["dynamics_slopes_per_step"][k] = {i: slope_within([r for r in arco if r["inst"] == i], "dyn_id", fn)
                                              for i in out["per_instrument_notes"]}
    vib = defaultdict(list)
    for r in arco:
        if vib_ok(r) and get(r, "vibrato.am_db"):
            vib[r["inst"]].append(r["vibrato"]["am_db"])
    out["vibrato_am_db_per_partial"] = vdist(vib, 20) if vib else None
    out["body"] = {}
    for inst in out["per_instrument_notes"]:
        rows = [r for r in arco if r["inst"] == inst]
        if len(rows) >= 8:
            fcb, H, Hup, npts = body_estimate(rows)
            out["body"][inst] = {"fc_hz": fcb, "H_db": H, "H_upper_db": Hup, "n_points": npts,
                                 "marks": body_marks(fcb, H, fam)}
    # pizzicato
    if pizz:
        pz = defaultdict(lambda: defaultdict(list))
        for r in pizz:
            for n, t in zip(r["partials"], r["t60_s"]):
                if t is None or t <= 0 or t > 60:
                    continue
                fq = n * r["f0_hz"]
                b = int(np.round(3 * np.log2(fq / 1000.0)))
                pz[b][r["inst"]].append(t)
        out["pizz_t60_by_freq"] = {f"{1000 * 2 ** (b / 3):.0f}": dist(v) for b, v in sorted(pz.items())}
        out["pizz_B"] = dist({i: [r["B"] for r in pizz if r["inst"] == i and r.get("n_partials_fit", 0) >= 6]
                              for i in {r["inst"] for r in pizz}})
        out["pizz_B_by_octave"] = {}
        for o in range(5):
            cell = {i: [r["B"] for r in pizz if r["inst"] == i and r.get("n_partials_fit", 0) >= 6
                        and int((r["midi"] - LOW[fam] + 0.5) // 12) == o] for i in {r["inst"] for r in pizz}}
            cell = {k: v for k, v in cell.items() if v}
            if cell:
                out["pizz_B_by_octave"][f"oct{o}"] = dist(cell)
        th = defaultdict(list)
        tfc = None
        for r in pizz:
            if "thump_excess_db" in r:
                tfc = tfc or r["thump_fc"]
                th[r["inst"]].append(r["thump_excess_db"])
        out["pizz_thump_excess_db"] = {"fc_hz": tfc, **(vdist(th, len(tfc)) or {})} if tfc else None
        out["pizz_t60_fundamental"] = dist({i: [r["t60_s"][0] for r in pizz if r["inst"] == i and r["t60_s"] and r["t60_s"][0]]
                                            for i in {r["inst"] for r in pizz}})
    sec = [r for r in allf if r.get("section") and "h_db_re_max" in r]
    if sec:
        out["section_apart"] = {"inst": sorted({r["inst"] for r in sec}),
                                "arco_scalars": scalar_cells(sec, ARCO_FEATS)}
    return out


def phrases(fam):
    p = R / "phrases.jsonl"
    if not p.exists():
        return None
    rows = [json.loads(line) for line in p.read_text().splitlines()]
    rows = [r for r in rows if r["family"] == fam]
    out = {}
    for key in sorted({(r["source"], r["tech"]) for r in rows}):
        tech = f"{key[0]}:{key[1]}"
        sel = [r for r in rows if (r["source"], r["tech"]) == key]
        tr = [t for r in sel for t in r["transitions"]]
        rb = [t for r in sel for t in r["rebows"]]
        vb = [t for r in sel for t in r["vibrato"] if t.get("present") and 3.5 <= t["rate_hz"] <= 8.5]
        q = lambda v: [round(float(np.percentile(v, p)), 2) for p in (10, 50, 90)] if len(v) else None  # noqa: E731
        out[tech] = {"files": len(sel), "n_transitions": len(tr),
                     "trans_ms_p10_50_90": q([t["trans_ms"] for t in tr]),
                     "dip_db_p10_50_90": q([t["dip_db"] for t in tr]),
                     "ap_max_p10_50_90": q([t["ap_max"] for t in tr]),
                     "n_rebows": len(rb), "rebow_depth_db_p10_50_90": q([t["depth_db"] for t in rb]),
                     "rebow_dur_ms_p10_50_90": q([t["dur_ms"] for t in rb]),
                     "n_vibrato_notes": len(vb), "vib_rate_hz_p10_50_90": q([t["rate_hz"] for t in vb]),
                     "vib_extent_cents_p10_50_90": q([t["extent_cents"] for t in vb])}
    return out


def main():
    recs = load()
    summ = {}
    for fam in FAMS:
        d = family(recs, fam)
        d["phrases"] = phrases(fam)
        (R / f"{fam}.json").write_text(json.dumps(d, indent=1))
        s = d["arco_scalars"]
        summ[fam] = {"instruments": d["instruments"], "notes": d["notes"],
                     "centroid_harm_all": (s["centroid_harm"].get("all") or {}).get("median"),
                     "noise_db_all": (s["noise_db_re_harm"].get("all") or {}).get("median"),
                     "body_marks": {k: v["marks"] for k, v in d["body"].items()}}
    (R / "summary.json").write_text(json.dumps(summ, indent=1))
    print(json.dumps(summ, indent=1)[:4000])


if __name__ == "__main__":
    main()
