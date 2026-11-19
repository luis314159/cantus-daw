"""Tail shape of drums: level (re peak, 10 ms RMS) at 100/300/600 ms and the slope 300-600 ms (dB/s)."""
import sys
sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
import numpy as np
from common import *
M = load("golpes-modelado-studio.flac"); R = load("golpes-grabado-drskit.flac")
print("piece      vel |  M @100 @300 @600 slope300-600 |  R @100 @300 @600 slope300-600")
for p in ["kick", "snare", "rimshot", "tom_hi", "tom_floor"]:
    for v in (80, 127):
        row = []
        for X in (M, R):
            s, on = hit(X, p, v); m = s.mean(1)
            e = np.sqrt(np.convolve(m * m, np.ones(480) / 480, "same") + 1e-30)
            d = 20 * np.log10(e / e.max())
            a = [d[on + int(t * SR)] for t in (0.1, 0.3, 0.6)]
            row.append(a + [(a[2] - a[1]) / 0.3])
        print(f"{p:10s} {v:3d} | " + "  ".join(f"{x:6.0f}" for x in row[0]) + " | " + "  ".join(f"{x:6.0f}" for x in row[1]))
