#!/usr/bin/env python3
"""Measure every hit of a `tympanum_real hits` render (round 42, tympanum-real).

    python -I tools/tympanum-real/measure.py <render.wav> [<out.jsonl>] [--slot 5]

Reads the render and its `<render>.csv` (one row per hit), cuts each hit's window, and
writes one JSON line per hit: the row's fields (piece, kind, art, key, open, vel, take) and
`features.features`. The default output is `<render>.jsonl`.
"""
import csv
import json
import sys
from pathlib import Path

import numpy as np
import soundfile as sf

sys.path.insert(0, str(Path(__file__).resolve().parent))
from features import features  # noqa: E402


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    slot = 30.0
    if "--slot" in sys.argv:
        slot = float(sys.argv[sys.argv.index("--slot") + 1])
        args = [a for a in args if a != str(sys.argv[sys.argv.index("--slot") + 1])]
    wav = Path(args[0])
    out = Path(args[1]) if len(args) > 1 else wav.with_suffix(".jsonl")
    x, fs = sf.read(str(wav), dtype="float64", always_2d=True)
    rows = list(csv.DictReader(open(wav.with_suffix(".csv"), encoding="utf-8")))
    pre = int(0.01 * fs)
    n = 0
    starts = [int(r["start"]) for r in rows] + [len(x) + pre]
    with open(out, "w", encoding="utf-8") as fh:
        for i, r in enumerate(rows):
            a = int(r["start"]) - pre
            # each hit's own window: to the next hit (a cymbal's is longer), `--slot` at most
            b = min(starts[i + 1] - pre - int(0.05 * fs), a + int((slot - 0.05) * fs))
            st = x[max(0, a):b]
            if len(st) < fs // 2:
                continue
            f = features(st, fs, r["kind"])
            if f is None:
                continue
            rec = {k: r[k] for k in ("piece", "kind", "art", "key", "open", "vel", "take")}
            rec["piece"] = rec["piece"].strip('"')
            for k in ("key", "vel", "take"):
                rec[k] = int(rec[k])
            rec["open"] = float(rec["open"])
            rec.update(f)
            fh.write(json.dumps(rec, allow_nan=True) + "\n")
            n += 1
    print(f"{wav.name}: {n} hits -> {out}")


if __name__ == "__main__":
    main()
