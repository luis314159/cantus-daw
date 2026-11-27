"""partials.py (r42 arcus-classical pass 2) <inst> <run> [<run> ...]: per note range and dynamic, the fundamental and the first
partials re the strongest (dB), refs by source vs Arcus runs (held-out cells)."""
import sys, os, json, collections
from pathlib import Path
import numpy as np
REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO / "tools" / "soundlab"))
os.environ.setdefault("SOUNDLAB_WORK", r"F:\projects\cantus-work\arcus-classical\soundlab")
from soundlab.subject import Subject
from soundlab.measure import measure_refs
inst = sys.argv[1]
s = Subject(REPO / "tools" / "soundlab" / "subjects" / f"arcus-{inst}" / "subject.json")
refs = measure_refs(s, log=lambda *a: None)
W = os.environ["SOUNDLAB_WORK"]
def runrows(run):
    return json.load(open(rf"{W}\arcus-{inst}\runs\{run}\measures.json"))["rows"]
ranges = {"violin": [(55, 61), (62, 68), (69, 75), (76, 86), (87, 100)], "viola": [(48, 54), (55, 61), (62, 70), (71, 90)], "cello": [(36, 47), (48, 59), (60, 71), (72, 84)], "bass": [(28, 39), (40, 51), (52, 67)]}[inst]
def stats(rows, lo, hi, dyn):
    v = [r["f"].get("harmonics.h_db_re_max") for r in rows if r["cell"][0] == "arco" and r["cell"][1] is not None
         and lo <= r["cell"][1] <= hi and r["cell"][2] in dyn and r["f"].get("harmonics.h_db_re_max")]
    if not v:
        return None
    n = min(len(x) for x in v)
    a = np.array([x[:min(n, 8)] for x in v])
    return len(v), np.median(a, axis=0)
groups = collections.defaultdict(list)
for r in refs:
    groups[r["source"]].append(r)
for lo, hi in ranges:
    for dyn in (("ff", "f"), ("mf", "mp"), ("pp", "p")):
        print(f"notes {lo}-{hi} {dyn[0]}")
        for src, rows in sorted(groups.items()):
            st = stats(rows, lo, hi, dyn)
            if st:
                print(f"   {src:14s} n={st[0]:3d} " + " ".join(f"{x:6.1f}" for x in st[1]))
        for run in sys.argv[2:]:
            st = stats(runrows(run), lo, hi, dyn)
            if st:
                print(f"   *{run:13s} n={st[0]:3d} " + " ".join(f"{x:6.1f}" for x in st[1]))
