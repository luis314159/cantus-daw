"""Felt vs plastic beater in the phrase: kick onsets (40-120 Hz), attack band levels (first 8 ms)."""
import sys
sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
import numpy as np
from scipy import signal
from common import *
F = load("frase-modelado-fieltro.flac").mean(1); P = load("frase-modelado-plastico.flac").mean(1)
d = F - P
print("whole-file difference re felt: %.1f dB" % (10 * np.log10(np.sum(d ** 2) / np.sum(F ** 2))))
e = band_env(F, 40, 120, win=0.005)
pk, _ = signal.find_peaks(e, height=e.max() * 0.3, distance=int(0.15 * SR))
print("kick onsets s:", np.round(pk / SR, 2))
rows = []
for i in pk:
    on = i - int(0.012 * SR)
    seg = slice(on, on + int(0.008 * SR))
    r = []
    for lo, hi in [(40, 120), (500, 1500), (1500, 4000), (4000, 10000)]:
        sos = signal.butter(4, [lo, hi], "band", fs=SR, output="sos")
        a = signal.sosfiltfilt(sos, F)[seg]; b = signal.sosfiltfilt(sos, P)[seg]
        r.append(10 * np.log10(np.sum(b ** 2) / (np.sum(a ** 2) + 1e-30) + 1e-30))
    rows.append(r)
rows = np.array(rows)
print("plastic minus felt (dB), first 8 ms around each kick, bands 40-120 / 0.5-1.5k / 1.5-4k / 4-10k: median",
      np.round(np.median(rows, 0), 1), " min", np.round(rows.min(0), 1), " max", np.round(rows.max(0), 1))
