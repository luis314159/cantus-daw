"""The held-out band method: the lab's core report.

1. **The split.** Every note has a cell (articulation, MIDI note, dynamic). A seeded rule holds
   a third of the cells out: `{"rule": "hash", "seed": S}` (SHA-256 of "S|art|note|dyn", one in
   three) or `{"rule": "mod", "expr": "(note + dyn) % 3 == 0", "dyn_index": {...}}` (Arcus's
   own rule, so the cells its match was never fitted on are the ones judged). A model is
   fitted, by hand or by a tool, on the references of the other cells only.
2. **The band.** The held-out cells' reference notes are grouped (articulation, register
   octave from the subject's lowest note, dynamic class); per group and per feature the band
   is the **real-instrument spread**: the weighted 10th-90th percentile, each physical
   instrument weighing as one (a cello with 300 notes counts as much as one with 30).
3. **The placement.** The candidate's notes in the same held-out cells, their median per
   group, placed in the band or outside it: the distance outside (feature units) and that
   distance in band widths. A vector (harmonics, HNR per band, T60 per partial) is placed
   element by element and summarised by the RMS of its distances outside.
4. **Before -> after.** Each run of the candidate is kept; the report puts a run's placements
   beside an earlier run's, feature by feature.
"""
import ast
import hashlib

import numpy as np

AXES = {"noise.bands_hz", "band_decay.bands_hz", "centroid.at_s"}
# Identities, not qualities: never banded.
SKIP = {"pitch.f0_hz", "attack.onset_s", "vibrato.cycles", "inharmonicity.n_partials_fit"}
DYN_CLASS = {"pp": "soft", "p": "soft", "mp": "medium", "mf": "medium", "f": "loud", "ff": "loud"}


def flatten(features):
    """{"feature.key": scalar or list} from analyse()'s nested dict (errors dropped)."""
    out = {}
    for f, d in features.items():
        if f.startswith("_"):
            continue
        for k, v in d.items():
            out[f"{f}.{k}"] = v
    return out


def _cell_hash(seed, art, note, dyn):
    return int(hashlib.sha256(f"{seed}|{art}|{note}|{dyn}".encode()).hexdigest(), 16)


_OK_NODES = (ast.Expression, ast.BinOp, ast.Compare, ast.Name, ast.Constant, ast.Load,
             ast.Add, ast.Sub, ast.Mult, ast.Mod, ast.FloorDiv, ast.Eq, ast.NotEq, ast.Lt,
             ast.LtE, ast.Gt, ast.GtE, ast.BoolOp, ast.And, ast.Or, ast.UnaryOp, ast.Not, ast.USub)


def _safe_eval(expr, names):
    tree = ast.parse(expr, mode="eval")
    for node in ast.walk(tree):
        if not isinstance(node, _OK_NODES):
            raise ValueError(f"the split's expression may use only arithmetic and comparisons: {expr}")
        if isinstance(node, ast.Name) and node.id not in names:
            raise ValueError(f"the split's expression names {node.id}: only {sorted(names)}")
    return eval(compile(tree, "<split>", "eval"), {"__builtins__": {}}, dict(names))  # noqa: S307


def held_out(cell, rule):
    """Whether a cell (art, note, dyn) is in the held-out third under the subject's rule."""
    art, note, dyn = cell
    if note is None:
        return False
    if rule.get("rule", "hash") == "hash":
        return _cell_hash(rule.get("seed", 42), art, int(note), dyn) % int(rule.get("of", 3)) == 0
    idx = rule.get("dyn_index", {})
    return bool(_safe_eval(rule["expr"], {"note": int(note), "dyn": int(idx.get(dyn, 0)),
                                          "art": 0}))


def group_of(cell, lowest, dyn_class=None):
    art, note, dyn = cell
    dc = (dyn_class or DYN_CLASS).get(dyn, dyn)
    reg = None if note is None else int((int(note) - lowest) // 12)
    return f"{art}|r{reg}|{dc}"


def wpercentile(values, weights, qs):
    """Weighted percentiles (the weighted empirical CDF's midpoints, linearly interpolated)."""
    v, w = np.asarray(values, float), np.asarray(weights, float)
    o = np.argsort(v)
    v, w = v[o], w[o]
    c = (np.cumsum(w) - 0.5 * w) / w.sum()
    return [float(np.interp(q / 100.0, c, v)) for q in qs]


def _values(rows, key):
    """[(inst, value)] of rows that hold key (scalars; vectors element-wise elsewhere)."""
    return [(r["inst"], r["f"][key]) for r in rows if r["f"].get(key) is not None]


def _el(row, key, j):
    v = row["f"].get(key)
    return v[j] if isinstance(v, list) and j < len(v) else None


def band_scalar(pairs, min_notes=3):
    """The band of [(inst, value)]: p10, p50, p90 with each instrument weighing one."""
    pairs = [(i, float(v)) for i, v in pairs if v is not None and np.isfinite(v)]
    if len(pairs) < min_notes:
        return None
    count = {}
    for i, _ in pairs:
        count[i] = count.get(i, 0) + 1
    w = [1.0 / count[i] for i, _ in pairs]
    p10, p50, p90 = wpercentile([v for _, v in pairs], w, (10, 50, 90))
    return {"p10": p10, "p50": p50, "p90": p90, "n": len(pairs), "n_inst": len(count)}


def place(value, band):
    if band is None or value is None:
        return None
    lo, hi = band["p10"], band["p90"]
    out = max(lo - value, 0.0, value - hi)
    width = max(hi - lo, 1e-9)
    return {"value": float(value), "outside": float(out), "widths": float(out / width),
            "status": "in" if out == 0 else ("below" if value < lo else "above")}


def bands(ref_rows, cand_rows, min_notes=3):
    """Per group and per feature key: the band of the held-out references and the
    candidate's placement. ref_rows / cand_rows: [{"group", "inst", "f": flat features}]."""
    groups = sorted({r["group"] for r in cand_rows} & {r["group"] for r in ref_rows})
    out = {}
    for g in groups:
        R = [r for r in ref_rows if r["group"] == g]
        C = [r for r in cand_rows if r["group"] == g]
        keys = sorted({k for r in C for k in r["f"]} - AXES - SKIP)
        res = {}
        for k in keys:
            sample = next((r["f"][k] for r in C if r["f"].get(k) is not None), None)
            if isinstance(sample, list):
                n = len(sample)
                els, outs = [], []
                for j in range(n):
                    b = band_scalar([(r["inst"], _el(r, k, j)) for r in R], min_notes)
                    cv = [_el(r, k, j) for r in C if _el(r, k, j) is not None]
                    p = place(float(np.median(cv)), b) if cv else None
                    els.append({"band": b, "place": p})
                    if p:
                        outs.append(p["outside"])
                if outs:
                    res[k] = {"vector": els, "outside_rms": float(np.sqrt(np.mean(np.square(outs)))),
                              "in_share": float(np.mean([o == 0 for o in outs])), "n_el": len(outs)}
            else:
                b = band_scalar(_values(R, k), min_notes)
                cv = [r["f"][k] for r in C if r["f"].get(k) is not None]
                p = place(float(np.median(cv)), b) if cv else None
                if p:
                    res[k] = {"band": b, "place": p, "n_cand": len(cv),
                              "refs": [float(v) for _, v in _values(R, k)][:400]}
        out[g] = res
    return out


def summarise(b):
    """Per feature key over groups: how many in the band and how far out (scalars: in band
    widths; vectors: the RMS of the elements' distances, in the feature's units), median and
    mean over the groups."""
    keys = sorted({k for g in b.values() for k in g})
    out = {}
    for k in keys:
        cells = [g[k] for g in b.values() if k in g]
        if "vector" in cells[0]:
            v = [c["outside_rms"] for c in cells]
            out[k] = {"groups": len(cells), "in_share": float(np.mean([c["in_share"] for c in cells])),
                      "outside_rms_median": float(np.median(v)), "outside_rms_mean": float(np.mean(v))}
        else:
            v = [c["place"]["widths"] for c in cells]
            out[k] = {"groups": len(cells), "in": sum(c["place"]["status"] == "in" for c in cells),
                      "widths_median": float(np.median(v)), "widths_mean": float(np.mean(v))}
    return out


def compare(now, before, rel=0.05, floor=0.02):
    """Feature by feature: the mean distance over groups before -> now; "closer" or "further"
    when it moved by more than `rel` of the larger and more than `floor` (band widths, or the
    vector's units), else "same"."""
    out = {}
    for k, s in now.items():
        b = before.get(k)
        if not b:
            continue
        key = "outside_rms_mean" if "outside_rms_mean" in s else "widths_mean"
        d = s[key] - b[key]
        moved = abs(d) > max(floor, rel * max(abs(s[key]), abs(b[key])))
        out[k] = {"before": b[key], "now": s[key], "delta": d,
                  "verdict": ("closer" if d < 0 else "further") if moved else "same"}
    return out
