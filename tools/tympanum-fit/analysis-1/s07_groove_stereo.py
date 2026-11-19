"""Groove: L/R correlation per 50 ms frame in three bands; what part of the kit makes the width."""
import sys
sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
import numpy as np
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from scipy import signal
from common import *
G = {"modelled": load("groove-modelado-studio.flac"), "DRS": load("groove-grabado-drskit.flac"), "Crocell": load("groove-grabado-crocellkit.flac")}
COL = {"modelled": "#c0392b", "DRS": "#2471a3", "Crocell": "#16a085"}
fig, axs = plt.subplots(3, 1, figsize=(14, 9), sharex=True)
for ax, (lo, hi) in zip(axs, [(150, 500), (500, 2000), (2000, 12000)]):
    for k, x in G.items():
        sos = signal.butter(4, [lo, hi], "band", fs=SR, output="sos")
        L = signal.sosfiltfilt(sos, x[:, 0]); R = signal.sosfiltfilt(sos, x[:, 1])
        n = 2400
        nf = len(L) // n
        Lf = L[:nf * n].reshape(nf, n); Rf = R[:nf * n].reshape(nf, n)
        c = (Lf * Rf).sum(1) / np.sqrt((Lf ** 2).sum(1) * (Rf ** 2).sum(1) + 1e-30)
        e = 10 * np.log10((Lf ** 2 + Rf ** 2).sum(1) + 1e-30)
        act = e > e.max() - 40
        ax.plot(np.arange(nf)[act] * n / SR, c[act], ".", ms=3, color=COL[k], label=k)
        w = 10 ** (e[act] / 10)
        print(f"{lo}-{hi} Hz {k:9s}: energy-weighted corr {np.sum(c[act]*w)/np.sum(w):+.2f}, median {np.median(c[act]):+.2f}, frames<0: {np.mean(c[act]<0)*100:.0f}%")
    ax.set_ylim(-1, 1); ax.grid(alpha=.3); ax.legend(fontsize=7); ax.set_title(f"L/R correlation per 50 ms, {lo}-{hi} Hz (frames within 40 dB of max)", fontsize=9)
axs[-1].set_xlabel("s")
fig.tight_layout(); fig.savefig(FIG + "groove_stereo_corr.png", dpi=80)
