"""Strongest partials of crash/ride/bell/open hat (v110): frequency, prominence over local floor,
own T60, frequency drift 0.1->1.5 s and amplitude-envelope roughness."""
import sys
sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
import numpy as np
from scipy import signal, ndimage
from common import *
M = load("golpes-modelado-studio.flac"); R = load("golpes-grabado-drskit.flac")
for p in ["crash", "ride", "ride_bell", "hat_open"]:
    for lab, X in (("M", M), ("R", R)):
        s, on = hit(X, p, 110)
        m = s.mean(1)
        w = m[on + int(0.1 * SR):on + int(1.1 * SR)]
        N = len(w)
        P = np.abs(np.fft.rfft(w * np.hanning(N), 4 * N)) ** 2
        f = np.fft.rfftfreq(4 * N, 1 / SR)
        d = 10 * np.log10(P + 1e-30)
        med = ndimage.median_filter(d, size=int(300 / (f[1])) | 1)
        k = f > 1000
        pk, pr = signal.find_peaks(d * k, distance=int(30 / f[1]))
        pk = pk[np.argsort(d[pk])[::-1]][:8]
        out = []
        tot = P[k].sum()
        for i in sorted(pk, key=lambda i: f[i]):
            fr = f[i]
            # partial envelope via narrow band (+-6 Hz) STFT tracking
            ff, tt, S = signal.stft(m[on:on + int(3.0 * SR)], SR, nperseg=8192, noverlap=8192 - 1024)
            j = np.argmin(np.abs(ff - fr))
            A = np.abs(S[j - 1:j + 2]).max(0)
            Adb = 20 * np.log10(A + 1e-12)
            sel = (tt > 0.2) & (tt < 2.8)
            sl = np.polyfit(tt[sel], Adb[sel], 1)[0]
            t60 = -60 / sl if sl < 0 else np.inf
            rough = np.std(Adb[sel] - np.polyval(np.polyfit(tt[sel], Adb[sel], 1), tt[sel]))
            share = P[max(0, i - 8):i + 9].sum() / tot * 100
            out.append(f"{fr:7.0f}Hz +{d[i]-med[i]:4.1f}dB {share:4.1f}% T60 {t60:4.1f}s rough {rough:3.1f}")
        print(f"{p:9s} {lab}: " + " | ".join(out))
