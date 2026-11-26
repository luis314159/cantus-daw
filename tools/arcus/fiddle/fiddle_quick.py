"""fiddle_quick.py - the fiddle study's headline measures for a few files, side by side
(r42 fiddle): the lab's renders (`cargo run -p cantus-dsp --release --example fiddle_lab`)
or a demo's stems against the references' medians in fiddle_measures.json.

    python -I tools/arcus/fiddle/fiddle_quick.py <wav> [<wav> ...]

Prints per file: centroid / f0, the share above 2 kHz (dB re total), the fast notes'
noise burst (dB re steady) and its length, the steady noise-to-harmonic 2-8 kHz, the
previous pitch 60 ms after the next onset, the share of fast onsets with a flux cue, the
share of fast notes starting an octave off, and the double stops' lower-minus-upper level.
"""
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import fiddle_measure as fm  # noqa: E402

REF = {"centroid/f0": "2.21", ">2k dB": "-6.6", "burst dB": "14.5 dry", "burst ms": "62-117 dry",
       "NHR dB": "-6.7 (-22 dry cl.)", "prev+60 dB": "-10..-16 dry", "flux": "0.15",
       "oct-off": "0.06", "lo-hi dB": "+1.7"}


def med(rows, key):
    v = [r[key] for r in rows if r.get(key) is not None and np.isfinite(r[key])]
    return float(np.median(v)) if v else float("nan")


def summary(path):
    r = fm.analyse(path, "ours")
    fast = r.get("fast", [])
    tim = r.get("timbre", {})
    ds = r.get("double", {}).get("rows", [])
    out = {
        "centroid/f0": tim.get("centroid_over_f0", float("nan")),
        ">2k dB": tim.get("share_above_2k_db", float("nan")),
        "burst dB": med(fast, "burst_db"),
        "burst ms": med(fast, "burst_ms"),
        "NHR dB": med(fast, "nhr_steady_db"),
        "prev+60 dB": med(fast, "ring60_db"),
        "flux": float(np.mean([f.get("cue", 1) == 0 for f in fast])) if fast else float("nan"),
        "oct-off": med(fast, "octave_start") if fast else float("nan"),
        "lo-hi dB": med(ds, "level_lo_minus_hi_db"),
    }
    return out


def main(paths):
    rows = [(Path(p).stem, summary(p)) for p in paths]
    keys = list(REF)
    w = max(len(n) for n, _ in rows) + 2
    print("file".ljust(w) + "".join(k.rjust(12) for k in keys))
    print("references".ljust(w) + "".join(REF[k].rjust(12) for k in keys))
    for n, r in rows:
        print(n.ljust(w) + "".join(f"{r[k]:12.2f}" for k in keys))


if __name__ == "__main__":
    main(sys.argv[1:])
