"""Low (<450 Hz) lines in cymbal hits: are they the kit's drums ringing sympathetically?"""
import sys
sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
import numpy as np
from scipy import signal
from common import *
for lab, fn in (("M", "golpes-modelado-studio.flac"), ("R", "golpes-grabado-drskit.flac")):
    X = load(fn)
    for p in ["snare", "tom_hi", "tom_floor", "kick", "crash", "ride", "hat_open"]:
        s, on = hit(X, p, 110)
        m = s.mean(1)
        w = m[on + int(0.3 * SR):on + int(2.3 * SR)]
        if len(w) < SR:
            w = m[on + int(0.05 * SR):on + int(0.5 * SR)]
        P = np.abs(np.fft.rfft(w * np.hanning(len(w)), 1 << 18)) ** 2
        f = np.fft.rfftfreq(1 << 18, 1 / SR)
        k = (f > 50) & (f < 450)
        pk, _ = signal.find_peaks(10 * np.log10(P[k] + 1e-30), distance=int(8 / f[1]))
        pk = pk[np.argsort(P[k][pk])[::-1]][:5]
        lowshare = 10 * np.log10(P[k].sum() / P[f > 50].sum())
        print(f"{lab} {p:9s}: <450 Hz share {lowshare:6.1f} dB; strongest lines: " + ", ".join(f"{f[k][i]:.0f}" for i in sorted(pk)))
