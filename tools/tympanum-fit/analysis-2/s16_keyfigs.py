"""Key figures: (1) attack smear — 2-16 kHz envelope, first 10 ms, studio v110; (2) cymbal tail texture spectra 2-3 s."""
import sys; sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
from feats import *
import matplotlib; matplotlib.use("Agg"); import matplotlib.pyplot as plt
fig, axs = plt.subplots(2, 3, figsize=(15, 7))
for ax, (kit, el) in zip(axs.flat, (("studio","caja"),("studio","tom-medio"),("studio","tom-de-piso"),("studio","ride"),("jazz","caja"),("jazz","tom-de-piso"))):
    for v, col in (("gen","C3"),("rec","k")):
        m, on = element_hits(kit, el, v)[2]; m = np.pad(m,(480,SR)); on += 480
        y = bandpass(m, 2000, 16000); e = np.sqrt(np.convolve(y*y, np.ones(12)/12, mode="same"))
        seg = 20*np.log10(e[on-48:on+int(0.012*SR)]+1e-12); seg -= seg.max()
        ax.plot((np.arange(len(seg))-48)/SR*1000, seg, color=col, lw=1, label=v)
    ax.set_ylim(-50, 2); ax.grid(alpha=.3); ax.set_title(f"{kit} {el} v110: 2-16 kHz envelope", fontsize=9); ax.set_xlabel("ms after first sound"); ax.legend(fontsize=7)
fig.tight_layout(); fig.savefig(FIG+"key_attack_smear.png", dpi=80); plt.close(fig)
fig, axs = plt.subplots(2, 2, figsize=(15, 8))
for ax, (kit, el) in zip(axs.flat, (("studio","crash"),("jazz","crash"),("studio","ride"),("jazz","hihat-abierto"))):
    for v, col in (("gen","C3"),("rec","k")):
        m, on = element_hits(kit, el, v)[3]; m = np.pad(m,(0,4*SR))
        a, b = (2.0, 3.3) if "hihat" not in el else (1.6, 2.4)
        y = m[on+int(a*SR):on+int(b*SR)]; n = len(y)
        X = 20*np.log10(np.abs(np.fft.rfft(y*np.hanning(n)))+1e-12); f = np.fft.rfftfreq(n, 1/SR)
        k = (f > 3000) & (f < 16000)
        ax.plot(f[k]/1000, X[k]-X[k].max(), color=col, lw=.4, label=v, alpha=.8)
    ax.set_ylim(-80, 2); ax.set_title(f"{kit} {el} v127 tail {a}-{b} s spectrum (gen red, rec black)", fontsize=9); ax.set_xlabel("kHz"); ax.legend(fontsize=7)
fig.tight_layout(); fig.savefig(FIG+"key_cymbal_tails.png", dpi=80); plt.close(fig)
