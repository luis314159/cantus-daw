#!/usr/bin/env python3
"""When each band of a drum stroke peaks (round 42, tympanum-real: model 6's onset).

    python -I tools/tympanum-real/onset.py <render.wav> [<render.wav> …] [--json out.json]

Each render is a `tympanum_real hits` output with its `.csv`. For every centre stroke of a
kick, snare or tom: the onset (`features.onset`, on the channels' mean), then per band
(`BANDS`) the time its envelope peaks after the onset — the band zero-phase band-passed
(fourth-order Butterworth, forward and back: no group delay), its Hilbert envelope smoothed
over 0.5 ms, the first 60 ms searched — and the waveform's own peak. Printed as medians per
kit, piece and velocity group (soft <= 48, mid 64-80, hard >= 100); `--json` keeps them.
"""
import csv
import json
import sys
from pathlib import Path

import numpy as np
import soundfile as sf
from scipy import signal

sys.path.insert(0, str(Path(__file__).resolve().parent))
from features import onset  # noqa: E402

BANDS = [("lo", 60, 400), ("mid", 400, 1500), ("hm", 1500, 4000), ("hi", 4000, 12000)]
GROUPS = [("soft", 0, 48), ("mid", 49, 90), ("hard", 91, 127)]
KINDS = ("kick", "snare", "rimshot", "racktom", "floortom")


def peaks(x, fs):
    """A stroke (mono, from a little before its onset): each band's peak (ms after the
    onset), the waveform's, and the first 10 ms's crest (dB)."""
    i0 = onset(x, fs)
    y = x[i0:i0 + int(0.08 * fs)]
    out = {}
    sm = max(1, int(0.0005 * fs))
    k = np.ones(sm) / sm
    for name, lo, hi in BANDS:
        sos = signal.butter(4, [lo, min(hi, 0.45 * fs)], btype="band", fs=fs, output="sos")
        z = signal.sosfiltfilt(sos, x[max(0, i0 - int(0.02 * fs)):i0 + int(0.1 * fs)])
        z = z[min(i0, int(0.02 * fs)):]
        e = np.convolve(np.abs(signal.hilbert(z)), k, mode="same")[:int(0.06 * fs)]
        out[name] = float(np.argmax(e) / fs * 1000.0)
    a = np.abs(y)
    out["wave"] = float(np.argmax(a[:int(0.06 * fs)]) / fs * 1000.0)
    r10 = np.sqrt(np.mean(y[:int(0.01 * fs)] ** 2) + 1e-30)
    out["crest10"] = float(20 * np.log10(a[:int(0.01 * fs)].max() / r10 + 1e-30))
    return out


def measure(wav):
    x, fs = sf.read(str(wav), dtype="float64", always_2d=True)
    m = x.mean(axis=1)
    rows = list(csv.DictReader(open(Path(wav).with_suffix(".csv"), encoding="utf-8")))
    res = []
    pre = int(0.01 * fs)
    for r in rows:
        kind = "rimshot" if r["kind"] == "snare" and r["art"] == "rimshot" else r["kind"]
        if kind not in KINDS or r["art"] not in ("center", "kick", "head", "rimshot"):
            continue
        a = int(r["start"]) - pre
        st = m[max(0, a):a + int(0.3 * fs)]
        if len(st) < int(0.2 * fs) or np.abs(st).max() <= 0:
            continue
        p = peaks(st, fs)
        p.update(kind=kind, vel=int(r["vel"]), piece=r["piece"].strip('"'))
        res.append(p)
    return res


def summary(name, res):
    lines = []
    for kind in KINDS:
        for g, lo, hi in GROUPS:
            s = [r for r in res if r["kind"] == kind and lo <= r["vel"] <= hi]
            if not s:
                continue
            med = {k: float(np.median([r[k] for r in s])) for k in
                   [b[0] for b in BANDS] + ["wave", "crest10"]}
            lines.append((name, kind, g, len(s), med))
    return lines


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    jout = sys.argv[sys.argv.index("--json") + 1] if "--json" in sys.argv else None
    args = [a for a in args if a != jout]
    allr = {}
    print(f"{'kit':12} {'piece':9} {'vel':5} {'n':>3}  " +
          "  ".join(f"{b[0]:>5}" for b in BANDS) + "   wave crest10")
    for w in args:
        name = Path(w).parent.name + "/" + Path(w).stem
        res = measure(w)
        allr[name] = res
        for (n, kind, g, cnt, med) in summary(name, res):
            print(f"{n:12} {kind:9} {g:5} {cnt:3}  " +
                  "  ".join(f"{med[b[0]]:5.1f}" for b in BANDS) +
                  f"  {med['wave']:5.1f}  {med['crest10']:5.1f}")
    if jout:
        Path(jout).write_text(json.dumps(allr), encoding="utf-8")


if __name__ == "__main__":
    main()
