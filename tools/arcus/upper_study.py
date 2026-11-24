"""upper_study.py - Arcus's violin and viola measured by the study's own chain (round 42,
arcus-upper): every render of `arcus_upper_listen -- <out> study` analysed by the study's
`extract.run` (the same estimator as the ~5 100 recorded notes, so its biases cancel), then each
arco scalar's median per dynamic group set beside the family's band of real instruments
(refdb/<family>.json: [p25, p75] with four or more instruments, else [min, max]).

Usage (the study's venv: numpy, scipy, soundfile):
  <study>/venv/Scripts/python.exe -I tools/arcus/upper_study.py <renders>/study <study dir> [out.json]

Nothing is written into the study's folder; the per-note records go to out.json if named.
"""
import json
import statistics
import sys
from pathlib import Path

renders = Path(sys.argv[1])
study = Path(sys.argv[2])
out = Path(sys.argv[3]) if len(sys.argv) > 3 else None
sys.path.insert(0, str(study / "tools"))
import extract  # noqa: E402  (the study's estimator, unchanged)
from aggregate import ARCO_FEATS  # noqa: E402  (its own reading of each feature)

DYN = {"pp": 1, "mf": 4, "ff": 6}
GROUP = {1: "soft", 2: "soft", 3: "medium", 4: "medium", 5: "loud", 6: "loud"}
SCALARS = ["centroid_hz", "centroid_harm", "noise_db_re_harm", "attack_t_rise_s",
           "attack_t_steady_s", "attack_noise_burst_db", "release_decay_db_s",
           "vibrato_rate_hz", "vibrato_extent_cents"]

records = []
for fam_dir in sorted(p for p in renders.iterdir() if p.is_dir()):
    family = fam_dir.name
    for wav in sorted(fam_dir.glob("arcus_*.wav")):
        _, tech, midi, dyn = wav.stem.split("_")
        job = {"path": str(wav), "source": "arcus", "inst": f"arcus-{family}", "family": family,
               "tech": tech, "dyn": dyn, "dyn_id": DYN[dyn], "nominal": wav.stem,
               "string": None, "lossy": False, "section": False, "multi": False}
        _, recs, err = extract.run(job)
        if err:
            print("error", wav.name, err.splitlines()[-1])
        records.extend(recs)

report = []
for family in sorted({r["family"] for r in records}):
    band = json.loads((study / "refdb" / f"{family}.json").read_text())
    report.append(f"{family} (Arcus {sum(1 for r in records if r['family'] == family)} notes, "
                  f"the band of {len(band['instruments'])} real instruments):")
    arco = [r for r in records if r["family"] == family and r["tech"] == "arco"]
    for key in SCALARS:
        cells = band["arco_scalars"].get(key, {})
        for grp in ["soft", "medium", "loud", "all"]:
            vals = [ARCO_FEATS[key](r) for r in arco if grp == "all" or GROUP[r["dyn_id"]] == grp]
            vals = [v for v in vals if v is not None]
            c = cells.get(grp)
            if not vals or not c:
                continue
            m = statistics.median(vals)
            lo, hi = (c["p25"], c["p75"]) if c.get("p25") is not None else (c["min"], c["max"])
            where = "inside" if lo <= m <= hi else ("under" if m < lo else "over")
            report.append(f"  {key:22} {grp:6} Arcus {m:9.3f}   band [{lo:.3f}, {hi:.3f}]"
                          f" (median {c['median']:.3f})  {where}")
print("\n".join(report))
if out:
    out.write_text(json.dumps({"report": report, "notes": records}, indent=0))
