"""Groove over time: per 1-s window, 6-16 kHz flatness, effective components/kHz, fine-structure stability, 4-8k and 8-16k level gen-rec."""
import sys; sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
from feats import *
from s05_groove import groove
from scipy import ndimage
for kit in KITS:
    print(f"== {kit}: per second  [flat gen/rec | eff gen/rec | stab gen/rec | 1-4k, 4-8k, 8-16k gen-rec dB]")
    G = {v: groove(kit, v).mean(1) for v in ("gen", "rec")}
    for t in range(14):
        row = []
        for v in ("gen", "rec"):
            y = G[v][int(t*SR):int((t+1)*SR)]
            f, tt, S = signal.stft(y, SR, nperseg=4096, noverlap=2048)
            P = np.abs(S)**2; k = (f >= 6000) & (f < 16000)
            fl = np.median(flatness(P[k], axis=0)); p = P[k]/P[k].sum(0); eff = np.median(1/np.sum(p**2, 0)/10)
            Lg = 10*np.log10(P+1e-20); k2 = (f>=4000)&(f<16000)
            X = Lg[k2] - ndimage.uniform_filter1d(Lg[k2], 41, axis=0)
            st = np.nanmean([np.corrcoef(X[:, i], X[:, i+1])[0,1] for i in range(X.shape[1]-1)])
            lv = [10*np.log10(P[(f>=a)&(f<b)].sum()+1e-20) for a, b in ((1000,4000),(4000,8000),(8000,16000))]
            row.append((fl, eff, st, lv))
        g, r = row
        print(f"  {t:2d}s  {g[0]:.2f}/{r[0]:.2f} | {g[1]:5.1f}/{r[1]:5.1f} | {g[2]:.2f}/{r[2]:.2f} | " + " ".join(f"{a-b:5.1f}" for a, b in zip(g[3], r[3])))
