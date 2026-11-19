"""Groove: long-term 1/3-oct balance, stereo width per band, HF tonality over time, spectrograms."""
import sys
sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
import numpy as np
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from scipy import signal
from common import *

G = {"modelled": load("groove-modelado-studio.flac"),
     "DRS": load("groove-grabado-drskit.flac"),
     "Crocell": load("groove-grabado-crocellkit.flac")}
COL = {"modelled": "#c0392b", "DRS": "#2471a3", "Crocell": "#16a085"}

fc = 1000 * 2 ** (np.arange(-14, 14) / 3)
fc = fc[(fc > 30) & (fc < 20000)]
print("1/3-oct band level (dB re total, mid) and side/mid (dB), coherence L-R")
rows = {}
for k, x in G.items():
    mid = (x[:, 0] + x[:, 1]) / 2
    side = (x[:, 0] - x[:, 1]) / 2
    f, Pm = signal.welch(mid, SR, nperseg=16384)
    _, Ps = signal.welch(side, SR, nperseg=16384)
    _, C = signal.coherence(x[:, 0], x[:, 1], SR, nperseg=4096)
    fC = np.fft.rfftfreq(4096, 1 / SR)
    tot = Pm.sum()
    lv, sm, co = [], [], []
    for c in fc:
        s = (f >= c / 2 ** (1 / 6)) & (f < c * 2 ** (1 / 6))
        lv.append(10 * np.log10(Pm[s].sum() / tot))
        sm.append(10 * np.log10(Ps[s].sum() / Pm[s].sum()))
        sc = (fC >= c / 2 ** (1 / 6)) & (fC < c * 2 ** (1 / 6))
        co.append(C[sc].mean() if sc.any() else np.nan)
    rows[k] = (np.array(lv), np.array(sm), np.array(co))
print("   fc   " + "  ".join(f"{k:>22s}" for k in G))
for i, c in enumerate(fc):
    print(f"{c:7.0f} " + "  ".join(f"{rows[k][0][i]:6.1f} {rows[k][1][i]:6.1f} {rows[k][2][i]:5.2f}   " for k in G))

fig, axs = plt.subplots(1, 3, figsize=(17, 4.5))
for k in G:
    axs[0].semilogx(fc, rows[k][0], color=COL[k], label=k)
    axs[1].semilogx(fc, rows[k][1], color=COL[k], label=k)
    axs[2].semilogx(fc, rows[k][2], color=COL[k], label=k)
axs[0].set_title("groove: 1/3-oct level re total (mid)", fontsize=9)
axs[1].set_title("groove: side/mid per band (dB) = stereo width", fontsize=9)
axs[2].set_title("groove: L-R coherence per band", fontsize=9)
for a in axs:
    a.grid(alpha=0.3)
    a.legend(fontsize=8)
    a.set_xlabel("Hz")
fig.tight_layout()
fig.savefig(FIG + "groove_balance_width.png", dpi=80)
plt.close(fig)

# broad band balance
print("\nbroad bands (dB re total): sub<60, kick 60-150, body 150-500, mid 0.5-2k, presence 2-6k, air-cym 6-12k, top 12-20k")
edges = [20, 60, 150, 500, 2000, 6000, 12000, 20000]
for k, x in G.items():
    mid = x.mean(1)
    f, P = signal.welch(mid, SR, nperseg=16384)
    tot = P.sum()
    print(f" {k:9s}", " ".join(f"{10*np.log10(P[(f>=a)&(f<b)].sum()/tot):6.1f}" for a, b in zip(edges[:-1], edges[1:])))

# HF tonality over time: flatness of 6-16 kHz per 46 ms frame + effective components
print("\n6-16 kHz: median spectral flatness per 2048 frame; median participation ratio per kHz (8192 frames)")
fig, ax = plt.subplots(figsize=(14, 4))
for k, x in G.items():
    mid = x.mean(1)
    f, t, S = signal.stft(mid, SR, nperseg=2048, noverlap=1024)
    P = np.abs(S) ** 2
    s = (f >= 6000) & (f < 16000)
    fl = flatness(P[s], axis=0)
    e = P[s].sum(0)
    ok = e > np.percentile(e, 30)
    f2, t2, S2 = signal.stft(mid, SR, nperseg=8192, noverlap=4096)
    P2 = np.abs(S2) ** 2
    s2 = (f2 >= 6000) & (f2 < 16000)
    pr = P2[s2].sum(0) ** 2 / (P2[s2] ** 2).sum(0) / 10
    e2 = P2[s2].sum(0)
    ok2 = e2 > np.percentile(e2, 30)
    print(f" {k:9s} flatness {np.median(fl[ok]):.3f}  eff comps/kHz {np.median(pr[ok2]):.1f}")
    ax.plot(t, fl, color=COL[k], lw=0.8, label=k)
ax.set_title("groove: spectral flatness 6-16 kHz per 43 ms frame (1 = white noise)", fontsize=9)
ax.set_xlabel("s")
ax.legend()
ax.grid(alpha=0.3)
fig.tight_layout()
fig.savefig(FIG + "groove_hf_flatness.png", dpi=80)
plt.close(fig)

# spectrograms
fig, axs = plt.subplots(3, 1, figsize=(14, 11))
for ax, (k, x) in zip(axs, G.items()):
    f, t, S = signal.stft(x.mean(1), SR, nperseg=2048, noverlap=2048 - 256)
    P = 20 * np.log10(np.abs(S) + 1e-12)
    P -= P.max()
    ax.pcolormesh(t, f[1:], P[1:], vmin=-90, vmax=0, cmap="magma", shading="auto")
    ax.set_yscale("log")
    ax.set_ylim(40, 24000)
    ax.set_title(f"groove {k}", fontsize=9)
fig.tight_layout()
fig.savefig(FIG + "groove_spectrograms.png", dpi=80)
plt.close(fig)

# crest / dynamics
print("\ncrest factor (peak/RMS dB), short-term (100 ms) loudness range p95-p10 dB")
for k, x in G.items():
    m = x.mean(1)
    cr = 20 * np.log10(np.abs(m).max() / np.sqrt(np.mean(m * m)))
    r = 10 * np.log10(np.convolve(m * m, np.ones(4800) / 4800, "valid")[::480] + 1e-20)
    print(f" {k:9s} crest {cr:5.1f}  range {np.percentile(r,95)-np.percentile(r,10):5.1f}")
