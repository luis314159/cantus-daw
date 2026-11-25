import sys, json, re, numpy as np
from pathlib import Path
tools = Path(__file__).resolve().parents[1] / "study"; sys.path.insert(0, str(tools))  # the study's estimator
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "sound-refs")); import cantus_paths as cp  # noqa: E402
from bowlib_core import load
from bowlib_note import analyse_arco
fam = sys.argv[2] if len(sys.argv) > 2 else "cello"
ref = json.loads(cp.refdb(f"{fam}.json").read_text())  # $CANTUS_STUDY_REFDB
lo = {"cello": 36, "bass": 28}[fam]
cls = {"pp": "soft", "mf": "medium", "ff": "loud"}
acc = {}
for p in sorted(Path(sys.argv[1]).glob(f"arcus-{fam}-arco-*.wav")):
    key, dyn = re.match(rf"arcus-{fam}-arco-(\d+)-(\w+)\.wav", p.name).groups()
    x, sr = load(str(p))
    r = analyse_arco(x, sr, {"fmin": 30, "fmax": 1100, "pre": None})
    if not r: continue
    o = (int(key) - lo) // 12
    acc.setdefault((o, cls[dyn]), []).append((r["h_db_re_max"][:12], r["centroid_hz"]))
for (o, c), v in sorted(acc.items()):
    k = f"oct{o}_{c}"
    if k not in ref["harmonics_db_re_max"]: continue
    b = ref["harmonics_db_re_max"][k]
    m = np.median([a for a, _ in v], axis=0)
    print(f"{k} n={len(v)} centroid {np.median([c for _, c in v]):.0f}")
    print("   arcus", " ".join("%5.0f" % x for x in m))
    print("   min  ", " ".join("%5.0f" % x for x in b["min"][:12]))
    print("   max  ", " ".join("%5.0f" % x for x in b["max"][:12]))
print("harmonic-number centroid (from the harmonics re max, 1..40) per register x dynamic: arcus | refs per instrument [min..max]")
def hc(v):
    v = np.array([x for x in v if x is not None and np.isfinite(x)]); p = 10 ** (v / 10); n = np.arange(1, len(v) + 1)
    return float((n * p).sum() / p.sum())
for (o, c), v in sorted(acc.items()):
    k = f"oct{o}_{c}"
    if k not in ref["harmonics_db_re_max"]: continue
    per = ref["harmonics_db_re_max"][k].get("per_inst", {})
    rv = sorted(hc(x[:40]) for x in per.values())
    print(f"  {k}: arcus {hc(np.median([a for a, _ in v], axis=0)):.2f} (12 only) | refs (12 only) {[round(hc(x[:12]),2) for x in per.values()]}")
