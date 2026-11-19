"""Sub-bass and low-mid per element in the first 50 ms and 50-300 ms: gen - rec (dB re each hit's broadband)."""
import sys; sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
from feats import *
B = [(20, 40), (40, 60), (60, 100), (100, 200)]
for kit in KITS:
    for el in ELEMENTS:
        row = []
        for vi in range(4):
            d = []
            for v in ("gen", "rec"):
                m, on = element_hits(kit, el, v)[vi]
                m = np.pad(m, (0, SR))
                tot = 10*np.log10(np.mean(m[on:on+int(0.05*SR)]**2)+1e-20)
                d.append([10*np.log10(np.mean(bandpass(m, lo, hi, order=2)[on:on+int(0.05*SR)]**2)+1e-20)-tot for lo, hi in B])
            row.append(np.array(d[0]) - np.array(d[1]))
        r = np.array(row)
        print(f"{kit:6s} {el:14s} gen-rec 0-50ms 20-40/40-60/60-100/100-200 Hz per vel: " + " | ".join(" ".join(f"{x:4.0f}" for x in rr) for rr in r))
