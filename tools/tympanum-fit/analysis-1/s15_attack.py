"""First 30 ms of the cymbals: waveform and band envelopes (1 ms), modelled vs DRS, v110."""
import sys
sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
import numpy as np
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from common import *
M = load("golpes-modelado-studio.flac"); R = load("golpes-grabado-drskit.flac")
bands = [(200, 1000), (1000, 3000), (3000, 6000), (6000, 12000), (12000, 20000)]
cols = plt.cm.viridis(np.linspace(0, .9, len(bands)))
fig, axs = plt.subplots(4, 4, figsize=(18, 13))
for r, p in enumerate(["crash", "ride", "hat_open", "hat_closed"]):
    for c, (X, lab) in enumerate(((M, "modelled"), (R, "DRS"))):
        s, on = hit(X, p, 110, pre=0.005)
        m = s.mean(1)
        t = (np.arange(len(m)) - on) / SR * 1000
        k = (t > -3) & (t < 30)
        axs[r, 2 * c].plot(t[k], m[k] / np.abs(m[k]).max(), lw=.6, color="k")
        axs[r, 2 * c].set_title(f"{p} v110 {lab}: waveform", fontsize=9)
        ref = None
        for (lo, hi), col in zip(bands, cols):
            e = band_env(m, lo, hi, win=0.001)
            d = 20 * np.log10(e + 1e-12)
            if ref is None: ref = None
            axs[r, 2 * c + 1].plot(t[k], d[k], color=col, lw=1, label=f"{lo/1000:g}-{hi/1000:g}k")
        axs[r, 2 * c + 1].set_title(f"{p} v110 {lab}: band envelopes (abs dB)", fontsize=9)
        axs[r, 2 * c + 1].legend(fontsize=6); axs[r, 2 * c + 1].grid(alpha=.3)
        axs[r, 2 * c + 1].set_ylim(-110, -20)
        for a in axs[r, 2 * c:2 * c + 2]: a.set_xlabel("ms")
fig.tight_layout(); fig.savefig(FIG + "cym_attack_30ms.png", dpi=75)
# numeric: energy 6-20 kHz in first 3 ms relative to its max over 0-500 ms
from scipy import signal
for p in ["crash", "ride", "ride_bell", "hat_open", "hat_closed"]:
    o = []
    for X in (M, R):
        s, on = hit(X, p, 110)
        m = s.mean(1)
        e = band_env(m, 6000, 20000, win=0.001)[on:on + int(0.5 * SR)]
        e2 = band_env(m, 200, 1500, win=0.001)[on:on + int(0.5 * SR)]
        o.append((20 * np.log10(e[:144].max() / e.max()), 20 * np.log10(e[:144].max() / e2[:144].max())))
    print(f"{p:10s} 6-20k first 3 ms re its own 0-500 ms max: M {o[0][0]:6.1f} R {o[1][0]:6.1f} dB | 6-20k vs 0.2-1.5k in first 3 ms: M {o[0][1]:6.1f} R {o[1][1]:6.1f} dB")
