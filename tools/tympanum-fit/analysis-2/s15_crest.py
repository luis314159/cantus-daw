"""Crest of the first 50 ms (peak re rms, dB) and the stick spike: peak of the first 1 ms re peak of 2-50 ms."""
import sys; sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
from feats import *
def cr(m, on):
    m = np.pad(m, (0, SR)); y = m[on:on+int(0.05*SR)]
    hp = bandpass(m, 3000, 20000)
    sp = db(np.abs(hp[on:on+int(0.0015*SR)]).max()/np.abs(hp[on+int(0.002*SR):on+int(0.05*SR)]).max())
    return db(np.abs(y).max()/np.sqrt(np.mean(y**2))), sp
for el in ELEMENTS:
    for kit in KITS:
        g=[cr(*element_hits(kit,el,"gen")[i]) for i in range(4)]; r=[cr(*element_hits(kit,el,"rec")[i]) for i in range(4)]
        print(f"{kit:6s} {el:14s} crest gen "+" ".join(f"{x[0]:5.1f}" for x in g)+" | rec "+" ".join(f"{x[0]:5.1f}" for x in r)
              +"   spike(3k+ first 1.5ms re 2-50ms) gen "+" ".join(f"{x[1]:5.1f}" for x in g)+" | rec "+" ".join(f"{x[1]:5.1f}" for x in r))
    print("    kits v110 crest/spike: "+" ".join(f"{p} {cr(*pack_hit(p,EL2P[el],110))[0]:.1f}/{cr(*pack_hit(p,EL2P[el],110))[1]:.1f}" for p in PACKS))
