"""Spectrograms, modelled vs recorded, for every piece at velocities 80 and 127 (full) and 127 (0-500 ms zoom)."""
import sys
sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
import numpy as np
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from scipy import signal
from common import *

M = load("golpes-modelado-studio.flac")
R = load("golpes-grabado-drskit.flac")


def spec(ax, m, nfft, hop, tmax, title, vmin=-90):
    f, t, S = signal.stft(m, SR, nperseg=nfft, noverlap=nfft - hop, window="hann", boundary=None)
    P = 20 * np.log10(np.abs(S) + 1e-12)
    P -= P.max()
    k = t <= tmax
    ax.pcolormesh(t[k], f[1:], P[1:, k], vmin=vmin, vmax=0, cmap="magma", shading="auto")
    ax.set_yscale("log")
    ax.set_ylim(40, 24000)
    ax.set_title(title, fontsize=9)
    ax.set_xlabel("s")
    ax.set_ylabel("Hz")


for p in PIECES:
    fig, axs = plt.subplots(3, 2, figsize=(13, 11))
    for col, (X, lab) in enumerate([(M, "modelled"), (R, "recorded DRS")]):
        for row, v in enumerate([80, 127]):
            seg, on = hit(X, p, v)
            spec(axs[row, col], seg.mean(1), 2048, 256, 3.5, f"{p} v{v} {lab}")
        seg, on = hit(X, p, 127)
        spec(axs[2, col], seg.mean(1), 512, 32, 0.5, f"{p} v127 {lab} 0-500 ms (512-pt)")
    fig.tight_layout()
    fig.savefig(FIG + f"spec_{p}.png", dpi=80)
    plt.close(fig)
print("ok")
