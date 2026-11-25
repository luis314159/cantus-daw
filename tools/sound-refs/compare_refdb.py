"""A rebuilt study refdb beside the distributions committed in the repository (round 42,
sound-data): every JSON in research/instruments-next/bowed-refdb/ that the rebuilt folder also
holds, compared value by value — equal, equal to rounding (a relative 1e-12: extract.py records
the notes in the order its workers finish them, so a sum over them may end in another last
bit), or the largest numeric difference and the first paths that differ. Exit 1 when one
differs beyond rounding. Stdlib only.

    python -I tools/sound-refs/compare_refdb.py <rebuilt refdb> [<committed refdb>]
"""
import json
import math
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]


def walk(a, b, path, out):
    if isinstance(a, dict) and isinstance(b, dict):
        for k in sorted(set(a) | set(b)):
            if k not in a or k not in b:
                out["shape"].append(f"{path}/{k}: only in {'rebuilt' if k in a else 'committed'}")
            else:
                walk(a[k], b[k], f"{path}/{k}", out)
    elif isinstance(a, list) and isinstance(b, list):
        if len(a) != len(b):
            out["shape"].append(f"{path}: {len(a)} items, committed {len(b)}")
        for i, (x, y) in enumerate(zip(a, b)):
            walk(x, y, f"{path}[{i}]", out)
    elif isinstance(a, (int, float)) and isinstance(b, (int, float)) and not isinstance(a, bool):
        if a != b and not (isinstance(a, float) and isinstance(b, float) and math.isnan(a) and math.isnan(b)):
            d = abs(a - b)
            if d <= 1e-12 * max(abs(a), abs(b)):
                out["ulp"] += 1
                return
            out["n"] += 1
            if d > out["max"]:
                out["max"], out["where"] = d, f"{path}: {a} vs {b}"
    elif a != b:
        out["shape"].append(f"{path}: {a!r} vs {b!r}"[:160])


def main():
    new = Path(sys.argv[1])
    old = Path(sys.argv[2]) if len(sys.argv) > 2 else REPO / "research" / "instruments-next" / "bowed-refdb"
    bad = 0
    for f in sorted(old.glob("*.json")):
        g = new / f.name
        if not g.exists():
            print(f"--   {f.name}: not rebuilt")
            continue
        out = {"n": 0, "ulp": 0, "max": 0.0, "where": "", "shape": []}
        walk(json.loads(g.read_text(encoding="utf-8")), json.loads(f.read_text(encoding="utf-8")), "", out)
        if not out["n"] and not out["shape"]:
            print(f"SAME {f.name}" + (f" (to rounding: {out['ulp']} numbers differ in the last bits)" if out["ulp"] else ""))
            continue
        bad += 1
        print(f"DIFF {f.name}: {out['n']} numbers differ (largest {out['max']:.3g} at {out['where']}), "
              f"{len(out['shape'])} other differences")
        for s in out["shape"][:5]:
            print(f"       {s}")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
