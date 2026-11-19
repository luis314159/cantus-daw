"""Cymbal tails: 4-12 kHz flatness, 6-16k effective components, stability, 8-16k level re hit peak, in windows 0.15-0.65 / 1-2 / 2-3.3 s."""
import sys; sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
from feats import *
from scipy import ndimage
def tail(m, on, a, b):
    m = np.pad(m, (0, 4*SR)); y = m[on+int(a*SR):on+int(b*SR)]
    f, t, S = signal.stft(y, SR, nperseg=4096, noverlap=2048); P = np.abs(S)**2
    k = (f>=4000)&(f<12000); fl = np.median(flatness(P[k], axis=0))
    k3 = (f>=6000)&(f<16000); p = P[k3]/P[k3].sum(0); eff = np.median(1/np.sum(p**2,0)/10)
    Lg = 10*np.log10(P+1e-20); k2=(f>=4000)&(f<16000); X = Lg[k2]-ndimage.uniform_filter1d(Lg[k2],41,axis=0)
    st = np.nanmean([np.corrcoef(X[:,i],X[:,i+2])[0,1] for i in range(X.shape[1]-2)])
    e = band_env(m, 8000, 16000, 0.02); lv = 20*np.log10(np.mean(e[on+int(a*SR):on+int(b*SR)])/e[on:on+int(0.5*SR)].max())
    e2 = band_env(m, 1000, 4000, 0.02); lv2 = 20*np.log10(np.mean(e2[on+int(a*SR):on+int(b*SR)])/e2[on:on+int(0.5*SR)].max())
    return fl, eff, st, lv, lv2
W = {"crash": ((0.15,0.65),(1,2),(2,3.3)), "ride": ((0.15,0.65),(1,2),(2,3.3)), "hihat-abierto": ((0.15,0.65),(0.8,1.6),(1.6,2.4))}
for el, wins in W.items():
    for kit in KITS:
        for vi in (1, 3):
            s = []
            for (a,b) in wins:
                g = tail(*element_hits(kit, el, "gen")[vi], a, b); r = tail(*element_hits(kit, el, "rec")[vi], a, b)
                s.append(f"{a}-{b}s fl {g[0]:.2f}/{r[0]:.2f} eff {g[1]:4.1f}/{r[1]:4.1f} st {g[2]:.2f}/{r[2]:.2f} 8-16k {g[3]:5.1f}/{r[3]:5.1f} 1-4k {g[4]:5.1f}/{r[4]:5.1f}")
            print(f"{kit:6s} {el:13s} v{VELS[vi]:3d} " + " || ".join(s))
    for pk in PACKS:
        s=[]
        for (a,b) in wins:
            r = tail(*pack_hit(pk, EL2P[el], 110), a, b)
            s.append(f"fl {r[0]:.2f} eff {r[1]:4.1f} st {r[2]:.2f} 8-16k {r[3]:5.1f} 1-4k {r[4]:5.1f}")
        print(f"     {pk:10s} v110 " + " || ".join(s))
