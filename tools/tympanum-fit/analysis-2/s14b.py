import sys; sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
from feats import *
for kit, el in (("studio","caja"),("studio","tom-de-piso"),("studio","ride"),("jazz","caja")):
    for v in ("gen","rec"):
        m, on = element_hits(kit, el, v)[2]; m = np.pad(m,(480,SR)); on+=480
        for lo,hi in ((2000,16000),(100,1000)):
            y = bandpass(m, lo, hi); e = np.sqrt(np.convolve(y*y, np.ones(12)/12, mode="same"))
            seg = 20*np.log10(e[on-48:on+int(0.010*SR)]+1e-12); seg -= seg.max()
            print(f"{kit} {el:11s} {v} {lo}-{hi}: " + " ".join(f"{x:4.0f}" for x in seg[::12]))
