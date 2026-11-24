import json, re, sys
from pathlib import Path
import numpy as np
tools = Path(r"F:\projects\daw-r42\study\tools"); sys.path.insert(0, str(tools))
from bowlib_core import load
from bowlib_note import analyse_pizz
from catalog import FAM
import aggregate
renders = Path(sys.argv[1])
for fam in ("cello", "bass"):
    recs = []
    for p in sorted(renders.glob(f"arcus-{fam}-pizz-*.wav")):
        key, dyn = re.match(rf"arcus-{fam}-pizz-(\d+)-(\w+)\.wav", p.name).groups()
        x, sr = load(str(p))
        r = analyse_pizz(x, sr, {"fmin": FAM[fam][0], "fmax": FAM[fam][1], "pre": None})
        if r is None: continue
        rec = {"path": str(p), "source": "arcus", "inst": "arcus", "family": fam, "tech": "pizz", "dyn": dyn,
               "dyn_id": {"pp": 1, "mf": 4, "ff": 6}[dyn], "section": False, "lossy": False, "multi": False,
               "string": None, "index": 0, "sr": sr, "midi": float(69 + 12 * np.log2(r["f0_hz"] / 440.0))}
        rec.update(r); recs.append(rec)
    ours = aggregate.family(recs, fam)
    ref = json.loads(Path(rf"F:\projects\daw-r42\study\refdb\{fam}.json").read_text())
    print("==", fam, len(recs), "plucks")
    for fq, d in ref["pizz_t60_by_freq"].items():
        o = ours["pizz_t60_by_freq"].get(fq)
        lo, hi = (d["p25"], d["p75"]) if d.get("n_inst", 0) >= 4 and "p25" in d else (d["min"], d["max"])
        print(f"  T60 @ {fq}: arcus {o['median'] if o else None!s:.6} | band {lo:.2f} … {hi:.2f}")
    t = ref["pizz_thump_excess_db"]; o = ours.get("pizz_thump_excess_db")
    print("  thump bands", t.get("fc", t.get("bands", "")))
    print("   arcus", [round(v, 1) if v is not None else None for v in (o["median"] if o else [])])
    print("   min  ", [round(v, 1) if v is not None else None for v in t["min"]])
    print("   max  ", [round(v, 1) if v is not None else None for v in t["max"]])
    print("  B by octave ref", {k: v.get("median") for k, v in ref.get("pizz_B_by_octave", {}).items()}, "arcus", {k: v.get("median") for k, v in (ours.get("pizz_B_by_octave") or {}).items()})
