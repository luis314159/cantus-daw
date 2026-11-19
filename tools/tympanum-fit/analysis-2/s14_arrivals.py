"""Discrete arrivals in the first 12 ms (2-16 kHz envelope at 0.1 ms): times and levels re the strongest."""
import sys; sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
from feats import *
for el in ("caja", "tom-medio", "tom-de-piso", "bombo", "ride", "crash", "hihat-cerrado"):
    for kit in KITS:
        out = []
        for v in ("gen", "rec"):
            m, on = element_hits(kit, el, v)[2]
            m = np.pad(m, (480, SR)); on += 480
            y = bandpass(m, 2000, 16000)
            a = np.abs(signal.hilbert(y[on-240:on+int(0.012*SR)]))
            e = 20*np.log10(np.convolve(a, np.ones(5)/5, mode="same")+1e-12)
            p, pr = signal.find_peaks(e, prominence=6, distance=24)
            o = np.argsort(e[p])[::-1][:4]
            mx = e[p].max()
            out.append(", ".join(f"{(p[i]-240)/SR*1000:5.1f}ms {e[p[i]]-mx:4.0f}" for i in sorted(o, key=lambda i: p[i])))
        print(f"{kit:6s} {el:13s} v110 gen: {out[0]:44s} | rec: {out[1]}")
