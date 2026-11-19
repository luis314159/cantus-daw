"""Phrase spectrograms (felt vs plastic beater) with onset marks."""
import sys
sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
import numpy as np
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from scipy import signal
from common import *
fig, axs = plt.subplots(2, 1, figsize=(16, 9), sharex=True)
for ax, fn in zip(axs, ["frase-modelado-fieltro.flac", "frase-modelado-plastico.flac"]):
    x = load(fn).mean(1)
    f, t, S = signal.stft(x, SR, nperseg=1024, noverlap=1024 - 128)
    P = 20 * np.log10(np.abs(S) + 1e-12); P -= P.max()
    ax.pcolormesh(t, f[1:], P[1:], vmin=-90, vmax=0, cmap="magma", shading="auto")
    ax.set_yscale("log"); ax.set_ylim(40, 24000); ax.set_title(fn, fontsize=9)
    ax.set_xticks(np.arange(0, 13.5, 0.5))
    ax.grid(alpha=.25, color="w")
fig.tight_layout(); fig.savefig(FIG + "phrase_spectrograms.png", dpi=80)
