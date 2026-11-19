"""Sub-fundamental energy (20-55 Hz, below every drum's fundamental except the kick) in 0-80 ms, re the spectrum max; and its waveform pulse."""
import sys; sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
from feats import *
def sub(m, on, lo=20, hi=55):
    m = np.pad(m,(0,SR)); y=m[on:on+int(0.08*SR)]; n=1<<16
    X=np.abs(np.fft.rfft(y*np.hanning(len(y)),n))**2; f=np.fft.rfftfreq(n,1/SR)
    k=(f>=lo)&(f<hi); return 10*np.log10(X[k].mean()/X.max())
for el in ("caja","tom-medio","tom-de-piso","hihat-cerrado","hihat-abierto","ride","crash"):
    for kit in KITS:
        g=[sub(*element_hits(kit,el,"gen")[i]) for i in range(4)]; r=[sub(*element_hits(kit,el,"rec")[i]) for i in range(4)]
        print(f"{kit:6s} {el:14s} gen "+" ".join(f"{x:5.0f}" for x in g)+" | rec "+" ".join(f"{x:5.0f}" for x in r))
    print("   kits v110: "+" ".join(f"{pk} {sub(*pack_hit(pk,EL2P[el],110)):5.0f}" for pk in PACKS))
