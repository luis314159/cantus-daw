"""extract.py - run the per-note analysis over the catalog (Arcus reference study).

Usage:  venv\\Scripts\\python.exe -I tools\\extract.py [--workers 3] [--only SUBSTR] [--limit N]
Reads refdb/catalog.json (tools/catalog.py), writes refdb/notes/<inst>.jsonl, one JSON object
per analysed note: the catalog's job fields + `index` (the note's place in a multi-note file) +
`midi` (from the measured f0) + the features of bowlib_note.analyse_arco / analyse_pizz.
Resumable: a (path, index) already in the output is skipped. Errors go to refdb/notes/errors.log.
"""
import argparse
import json
import sys
import traceback
from multiprocessing import Pool
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
from bowlib_core import load, segment  # noqa: E402
from bowlib_note import analyse_arco, analyse_pizz  # noqa: E402
from catalog import FAM  # noqa: E402

ROOT = Path(__file__).resolve().parents[3] / "research" / "instruments-next" / "bowed-refdb"


def clean(o):
    if isinstance(o, dict):
        return {k: clean(v) for k, v in o.items()}
    if isinstance(o, (list, tuple)):
        return [clean(v) for v in o]
    if isinstance(o, (np.floating, float)):
        v = float(o)
        return None if not np.isfinite(v) else round(v, 5)
    if isinstance(o, np.integer):
        return int(o)
    if isinstance(o, np.bool_):
        return bool(o)
    return o


def run(j):
    try:
        x, sr = load(j["path"])
        fmin, fmax = FAM[j["family"]]
        if j.get("multi"):
            segs = segment(x, sr)
        else:
            segs = [(0, len(x))]
        out = []
        prev_end = 0
        for i, (s0, s1) in enumerate(segs):
            pre_a = max(prev_end + int(0.05 * sr), s0 - int(1.0 * sr))
            pre = x[pre_a:max(s0 - int(0.02 * sr), pre_a)] if j.get("multi") else None
            a = max(s0 - int(0.02 * sr), 0) if j.get("multi") else 0
            note = x[a:s1]
            ctx = {"fmin": fmin, "fmax": fmax, "pre": pre if pre is not None and len(pre) > 0.1 * sr else None}
            f = analyse_pizz if j["tech"].startswith("pizz") else analyse_arco
            r = f(note, sr, ctx)
            prev_end = s1
            if r is None:
                continue
            rec = dict(j)
            rec["index"] = i
            rec["sr"] = sr
            rec["midi"] = float(69 + 12 * np.log2(r["f0_hz"] / 440.0))
            rec.update(r)
            out.append(clean(rec))
        return j, out, None
    except Exception:
        return j, [], traceback.format_exc()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--workers", type=int, default=3)
    ap.add_argument("--only", default="")
    ap.add_argument("--limit", type=int, default=0)
    a = ap.parse_args()
    jobs = json.loads((ROOT / "catalog.json").read_text())
    jobs = [j for j in jobs if a.only in j["inst"] or a.only in j["path"]]
    nd = ROOT / "notes"
    nd.mkdir(exist_ok=True)
    done = set()
    for f in nd.glob("*.jsonl"):
        for line in f.read_text().splitlines():
            r = json.loads(line)
            done.add(r["path"])
    jobs = [j for j in jobs if j["path"] not in done]
    if a.limit:
        jobs = jobs[: a.limit]
    print("jobs", len(jobs), flush=True)
    err = open(nd / "errors.log", "a")
    with Pool(a.workers) as pool:
        for k, (j, recs, e) in enumerate(pool.imap_unordered(run, jobs, chunksize=1)):
            if e:
                err.write(j["path"] + "\n" + e + "\n")
                err.flush()
            with open(nd / f"{j['inst']}.jsonl", "a") as fh:
                for r in recs:
                    fh.write(json.dumps(r) + "\n")
            if k % 25 == 0:
                print(k, j["inst"], len(recs), flush=True)


if __name__ == "__main__":
    main()
