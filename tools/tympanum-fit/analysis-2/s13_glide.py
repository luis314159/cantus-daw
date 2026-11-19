"""Pitch glide of the fundamental: peak near the late fundamental in 8-40 ms vs 300-700 ms (cents); dominant low partial."""
import sys; sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
from feats import *
R = {"bombo": (40, 110), "caja": (150, 260), "tom-medio": (60, 130), "tom-de-piso": (45, 100)}
def pk(y, lo, hi):
    n = 1 << 18; X = np.abs(np.fft.rfft(y*np.hanning(len(y)), n)); f = np.fft.rfftfreq(n, 1/SR)
    k = np.nonzero((f>=lo)&(f<=hi))[0]; i = k[np.argmax(X[k])]; return f[i]
def glide(m, on, lo, hi):
    m = np.pad(m, (0, SR))
    fl = pk(m[on+int(0.3*SR):on+int(0.7*SR)], lo, hi)
    fe = pk(m[on+int(0.008*SR):on+int(0.040*SR)], fl*0.95, fl*1.5)
    fm = pk(m[on+int(0.04*SR):on+int(0.12*SR)], fl*0.95, fl*1.4)
    return fl, 1200*np.log2(fe/fl), 1200*np.log2(fm/fl)
for el, (lo, hi) in R.items():
    for kit in KITS:
        g = [glide(*element_hits(kit, el, "gen")[i], lo, hi) for i in range(4)]
        r = [glide(*element_hits(kit, el, "rec")[i], lo, hi) for i in range(4)]
        print(f"{kit:6s} {el:12s} f_late gen " + " ".join(f"{x[0]:6.1f}" for x in g) + " rec " + " ".join(f"{x[0]:6.1f}" for x in r)
              + " | glide8-40ms gen " + " ".join(f"{x[1]:5.0f}" for x in g) + " rec " + " ".join(f"{x[1]:5.0f}" for x in r)
              + " | 40-120ms gen " + " ".join(f"{x[2]:5.0f}" for x in g) + " rec " + " ".join(f"{x[2]:5.0f}" for x in r))
    print("    kits v110: " + " ".join(f"{p} {glide(*pack_hit(p, EL2P[el], 110), lo, hi)[0]:.0f}Hz/{glide(*pack_hit(p, EL2P[el], 110), lo, hi)[1]:.0f}c" for p in PACKS))
