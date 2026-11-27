#!/usr/bin/env python3
"""Brushes on a snare, measured off the recordings (round 42, tympanum-real; the doc's §5).

    python -I tools/tympanum-real/brushes.py [--json out.json]

Reads the packs' own sample files (no render: what a microphone heard) — Big Rusty's 14x8
snare (`Samples/snare_14/{brush,bstir,bdig,center}/top`, CC0) and Unruly's brushed snare
(`Samples/s14`, CC0) when present — and prints per articulation and velocity layer, median of
its round robins:

- `rise_ms`: the envelope (1-ms RMS) from 10 % to 90 % of its peak, `peak_ms` when it peaks —
  a stick's contact is one ~1-ms push, a brush's bristles arrive over many milliseconds;
- `len_ms`: how long it stays within 10 dB of its peak (a tap's body, a sweep's whole);
- `cent_early`, `cent_late`: the spectral centroid at 0–30 ms and 100–300 ms (Hz);
- `hf_db`: the 3–12 kHz share of the first 300 ms (the wires: a brushed snare is mostly wires);
- `low_db`: the 80–400 Hz share (the head's body);
- `level_db`: the first 300 ms's RMS (dBFS), for the layers' law.
"""
import json
import re
import sys
from collections import defaultdict
from pathlib import Path

import numpy as np
import soundfile as sf

ROOT = Path(r"F:\projects\cantus-refs\drums\content")


def share(x, fs, lo, hi):
    X = np.abs(np.fft.rfft(x * np.hanning(len(x)))) ** 2
    f = np.fft.rfftfreq(len(x), 1 / fs)
    s = X[f > 30].sum()
    return float(10 * np.log10(X[(f >= lo) & (f < hi)].sum() / s + 1e-20)) if s > 0 else float("nan")


def centroid(x, fs):
    if len(x) < 64:
        return float("nan")
    X = np.abs(np.fft.rfft(x * np.hanning(len(x)))) ** 2
    f = np.fft.rfftfreq(len(x), 1 / fs)
    return float((f * X).sum() / X.sum()) if X.sum() > 0 else float("nan")


def one(path):
    x, fs = sf.read(str(path), dtype="float64", always_2d=True)
    m = x.mean(axis=1)
    a = np.abs(m)
    if a.max() <= 0:
        return None
    i0 = int(np.argmax(a > a.max() * 0.01))
    m = m[i0:]
    h = max(1, int(0.001 * fs))
    env = np.sqrt(np.convolve(m ** 2, np.ones(h) / h, mode="same"))
    pk = int(np.argmax(env[:int(2.0 * fs)]))
    e = env[:pk + 1]
    r10 = int(np.argmax(e >= 0.1 * env[pk]))
    r90 = int(np.argmax(e >= 0.9 * env[pk]))
    above = np.nonzero(env >= env[pk] * 10 ** (-10 / 20))[0]
    first = m[:int(0.3 * fs)]
    return {
        "rise_ms": (r90 - r10) / fs * 1000, "peak_ms": pk / fs * 1000,
        "len_ms": (above[-1] - above[0]) / fs * 1000 if len(above) else 0.0,
        "cent_early": centroid(m[:int(0.03 * fs)], fs),
        "cent_late": centroid(m[int(0.1 * fs):int(0.3 * fs)], fs),
        "hf_db": share(first, fs, 3000, 12000), "low_db": share(first, fs, 80, 400),
        "level_db": float(20 * np.log10(np.sqrt(np.mean(first ** 2)) + 1e-20)),
    }


def sets():
    br = ROOT / "big-rusty-drums" / "Samples" / "snare_14"
    for art in ("center", "brush", "bdig", "bstir", "bflutter", "brelease"):
        d = br / art / "top"
        if d.exists():
            yield "Big Rusty", art, sorted(d.glob("*.*"))
    un = ROOT / "unruly-drums" / "Samples" / "s14"
    if un.exists():
        for f in sorted(un.rglob("*.*")):
            pass
        groups = defaultdict(list)
        for f in sorted(un.rglob("*.wav")) + sorted(un.rglob("*.flac")):
            groups[f.parent.name].append(f)
        for g, fs in groups.items():
            yield "Unruly", g, fs


def layer(name):
    m = re.search(r"(vl|dl|v)(\d+)", name)
    return int(m.group(2)) if m else 0


def main():
    out = {}
    keys = ["rise_ms", "peak_ms", "len_ms", "cent_early", "cent_late", "hf_db", "low_db", "level_db"]
    print(f"{'kit':10} {'art':10} {'lay':>3} {'n':>2} " + " ".join(f"{k:>10}" for k in keys))
    for kit, art, files in sets():
        by = defaultdict(list)
        for f in files:
            r = one(f)
            if r:
                by[layer(f.stem)].append(r)
        for lay in sorted(by):
            rs = by[lay]
            med = {k: float(np.nanmedian([r[k] for r in rs])) for k in keys}
            out.setdefault(kit, {}).setdefault(art, {})[lay] = med
            print(f"{kit:10} {art:10} {lay:3} {len(rs):2} " + " ".join(f"{med[k]:10.1f}" for k in keys))
    if "--json" in sys.argv:
        Path(sys.argv[sys.argv.index("--json") + 1]).write_text(json.dumps(out, indent=1), encoding="utf-8")


if __name__ == "__main__":
    main()
