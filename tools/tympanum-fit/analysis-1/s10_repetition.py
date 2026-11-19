"""Hit-to-hit variation: for every onset, the best normalised cross-correlation (+-2 ms) of its first
40 ms (2-16 kHz and 80-1000 Hz bands) with any other onset. ~1.0 = an identical repeat (machine gun)."""
import sys
sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
import numpy as np
from scipy import signal
from common import *

def onsets(m):
    f, t, S = signal.stft(m, SR, nperseg=1024, noverlap=1024 - 256)
    L = np.log10(np.abs(S) ** 2 + 1e-12)
    flux = np.maximum(np.diff(L, axis=1), 0).sum(0)
    pk, _ = signal.find_peaks(flux, height=np.percentile(flux, 85), distance=int(0.06 * SR / 256))
    return (t[1:][pk] * SR).astype(int) - 512

def best_match(m, ons, lo, hi):
    sos = signal.butter(4, [lo, hi], "band", fs=SR, output="sos")
    y = signal.sosfiltfilt(sos, m)
    segs = [y[o:o + int(0.04 * SR)] for o in ons if o > 200 and o + 2000 < len(y)]
    segs = [s / (np.linalg.norm(s) + 1e-20) for s in segs]
    best = []
    for i, a in enumerate(segs):
        b_ = 0
        for j, b in enumerate(segs):
            if i == j: continue
            c = signal.correlate(a, b, mode="full")[len(a) - 97:len(a) + 96]
            b_ = max(b_, np.abs(c).max())
        best.append(b_)
    return np.array(best)

for fn in ["groove-modelado-studio.flac", "groove-grabado-drskit.flac", "groove-grabado-crocellkit.flac", "frase-modelado-fieltro.flac"]:
    m = load(fn).mean(1)
    ons = onsets(m)
    for lo, hi in [(2000, 16000), (80, 1000)]:
        b = best_match(m, ons, lo, hi)
        print(f"{fn:32s} {lo}-{hi} Hz: n={len(b)} median best-match {np.median(b):.2f}, share >0.95: {np.mean(b>0.95)*100:.0f}%, >0.8: {np.mean(b>0.8)*100:.0f}%")
