"""yin_pitch.py - Arcus pass 5: the study's own f0 tracker (bowlib_core.yin_track) on renders
or recordings, the note's cents from its key: the median over the frames, their p10 and p90.

    python -I tools/arcus/pass5/yin_pitch.py <file.f32|file.wav>:<midi key>:<from s>:<to s> ...

A `.f32` is raw little-endian float32 at 48 kHz (the probes' `ARCUS_DUMP`).
"""
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "study"))
from bowlib_core import load, yin_track  # noqa: E402


def read(path):
    if path.endswith(".f32"):
        return np.fromfile(path, dtype="<f4").astype(np.float64), 48000
    return load(path)


def cents_of(x, sr, key, a, b):
    f = 440.0 * 2 ** ((key - 69) / 12)
    seg = x[int(a * sr):int(b * sr)]
    t, f0, ap = yin_track(seg, sr, f / 1.5, f * 1.5, win_periods=3, f_hint=f)
    ok = ap < 0.35
    if ok.sum() < 3:
        return float("nan"), float("nan"), float("nan"), 0.0
    c = 1200 * np.log2(f0[ok] / f)
    return float(np.median(c)), float(np.percentile(c, 10)), float(np.percentile(c, 90)), float(ok.mean())


def main():
    for arg in sys.argv[1:]:
        path, key, a, b = arg.rsplit(":", 3)
        x, sr = read(path)
        med, p10, p90, share = cents_of(x, sr, float(key), float(a), float(b))
        print(f"{Path(path).name:28s} key {key}: {med:+7.1f} ct  p10 {p10:+7.1f}  p90 {p90:+7.1f}  periodic {share:.2f}")


if __name__ == "__main__":
    main()
