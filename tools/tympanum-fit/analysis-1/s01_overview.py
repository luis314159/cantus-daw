"""Per hit: peak, RMS(0-300 ms), crest, centroid, stereo correlation; sanity-check segmentation."""
import sys
sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
import numpy as np
from common import *

M = load("golpes-modelado-studio.flac")
R = load("golpes-grabado-drskit.flac")


def stats(seg, on):
    m = seg.mean(1)
    w = m[on:on + int(0.3 * SR)]
    pk = 20 * np.log10(np.abs(seg).max() + 1e-12)
    rms = 20 * np.log10(np.sqrt(np.mean(w ** 2)) + 1e-12)
    f = np.fft.rfftfreq(len(w), 1 / SR)
    P = np.abs(np.fft.rfft(w * np.hanning(len(w)))) ** 2
    cen = (f * P).sum() / P.sum()
    L, Rr = seg[on:on + int(0.5 * SR), 0], seg[on:on + int(0.5 * SR), 1]
    corr = np.sum(L * Rr) / np.sqrt(np.sum(L * L) * np.sum(Rr * Rr) + 1e-30)
    side = 10 * np.log10(np.sum((L - Rr) ** 2) / (np.sum((L + Rr) ** 2) + 1e-30) + 1e-30)
    # length until -60 dB below peak (10 ms rms)
    e = np.sqrt(np.convolve(m * m, np.ones(480) / 480, "same"))
    ed = 20 * np.log10(e / e.max() + 1e-12)
    last = np.where(ed > -60)[0]
    dur = (last[-1] - on) / SR if len(last) else 0
    return pk, rms, cen, corr, side, dur


print(f"{'piece':11s} vel | {'pk M':>6s} {'pk R':>6s} | {'rms M':>6s} {'rms R':>6s} | {'cen M':>6s} {'cen R':>6s} | {'corr M':>6s} {'corr R':>6s} | {'S/M M':>6s} {'S/M R':>6s} | {'dur M':>5s} {'dur R':>5s}")
for p in PIECES:
    for v in VELS:
        sm, om = hit(M, p, v)
        sr_, orr = hit(R, p, v)
        a = stats(sm, om)
        b = stats(sr_, orr)
        print(f"{p:11s} {v:3d} | {a[0]:6.1f} {b[0]:6.1f} | {a[1]:6.1f} {b[1]:6.1f} | {a[2]:6.0f} {b[2]:6.0f} | {a[3]:6.2f} {b[3]:6.2f} | {a[4]:6.1f} {b[4]:6.1f} | {a[5]:5.2f} {b[5]:5.2f}")
