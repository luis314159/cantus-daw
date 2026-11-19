"""Cymbal figures + effective partial density (participation ratio) per kHz."""
import sys
sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
import numpy as np
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from scipy import signal, ndimage
from common import *

M = load("golpes-modelado-studio.flac")
R = load("golpes-grabado-drskit.flac")
CM, CR = "#c0392b", "#2471a3"


def mono(X, p, v):
    s, on = hit(X, p, v)
    return s.mean(1), on


# ---- 1. fine spectra, 1 s window from 150 ms, zoom 6-7 kHz and 12-13 kHz
fig, axs = plt.subplots(4, 2, figsize=(14, 12))
for r, p in enumerate(["crash", "ride", "hat_open", "ride_bell"]):
    for c, (lo, hi) in enumerate([(6000, 6500), (12000, 12500)]):
        ax = axs[r, c]
        for X, col, lab in ((R, CR, "recorded DRS"), (M, CM, "modelled")):
            m, on = mono(X, p, 110)
            w = m[on + int(0.15 * SR):on + int(1.15 * SR)]
            P = np.abs(np.fft.rfft(w * np.hanning(len(w)), 4 * len(w))) ** 2
            f = np.fft.rfftfreq(4 * len(w), 1 / SR)
            s = (f >= lo) & (f < hi)
            d = 10 * np.log10(P[s] + 1e-30)
            ax.plot(f[s], d - np.percentile(d, 99.9), color=col, lw=0.7, label=lab)
        ax.set_ylim(-70, 3)
        ax.set_title(f"{p} v110, 0.15-1.15 s, {lo/1000:.1f}-{hi/1000:.1f} kHz (0 dB = band max)", fontsize=9)
        ax.legend(fontsize=8, loc="lower right")
fig.tight_layout()
fig.savefig(FIG + "cym_fine_spectra.png", dpi=80)
plt.close(fig)

# ---- 2. effective partial density (participation ratio per kHz) in 0.15-0.65 s
print("effective components per kHz (participation ratio of 0.5-s periodogram; white noise ~ 500/kHz)")
for p in ["crash", "ride", "ride_bell", "hat_open"]:
    for v in [80, 127]:
        row = []
        for X in (M, R):
            m, on = mono(X, p, v)
            w = m[on + int(0.15 * SR):on + int(0.65 * SR)]
            P = np.abs(np.fft.rfft(w * np.hanning(len(w)))) ** 2
            f = np.fft.rfftfreq(len(w), 1 / SR)
            vals = []
            for lo, hi in [(1000, 2000), (2000, 4000), (4000, 8000), (8000, 12000), (12000, 20000)]:
                s = (f >= lo) & (f < hi)
                pr = P[s].sum() ** 2 / (P[s] ** 2).sum()
                vals.append(pr / ((hi - lo) / 1000))
            row.append(vals)
        print(f" {p:9s} v{v}: M " + " ".join(f"{x:6.1f}" for x in row[0]) + " | R " + " ".join(f"{x:6.1f}" for x in row[1]))

# ---- 3. band envelopes 0-1.5 s (cascade)
bands = [(1000, 2000), (2000, 4000), (4000, 8000), (8000, 12000), (12000, 20000)]
cols = plt.cm.viridis(np.linspace(0, 0.9, len(bands)))
for p, tmax in [("crash", 1.5), ("ride", 1.5), ("ride_bell", 1.5), ("hat_open", 1.5), ("hat_closed", 0.3), ("hat_pedal", 0.3)]:
    fig, axs = plt.subplots(2, 2, figsize=(13, 8), sharey=True)
    for c, (X, lab) in enumerate(((M, "modelled"), (R, "recorded DRS"))):
        for r, v in enumerate([80, 127]):
            m, on = mono(X, p, v)
            ax = axs[r, c]
            ref = None
            for (lo, hi), col in zip(bands, cols):
                e = band_env(m, lo, hi, win=0.005)[on:on + int(tmax * SR)]
                d = 20 * np.log10(e + 1e-12)
                ax.plot(np.arange(len(d)) / SR * 1000, d - d.max(), color=col, lw=1, label=f"{lo/1000:g}-{hi/1000:g} kHz")
            ax.set_ylim(-50, 2)
            ax.set_title(f"{p} v{v} {lab}: each band vs its own max", fontsize=9)
            ax.set_xlabel("ms after onset")
            ax.grid(alpha=0.3)
            ax.legend(fontsize=7)
    fig.tight_layout()
    fig.savefig(FIG + f"cascade_{p}.png", dpi=80)
    plt.close(fig)

# ---- 4. T20 per third-octave band vs frequency
fc = 1000 * 2 ** (np.arange(-9, 14) / 3)
fc = fc[(fc > 150) & (fc < 18000)]
fig, axs = plt.subplots(1, 4, figsize=(17, 4.5))
for ax, p in zip(axs, ["crash", "ride", "ride_bell", "hat_open"]):
    for X, col, lab in ((M, CM, "modelled"), (R, CR, "recorded DRS")):
        for v, ls in ((80, "--"), (127, "-")):
            m, on = mono(X, p, v)
            if p == "hat_open":
                m = m[:on + int(2.9 * SR)]
            t = []
            for c in fc:
                e = band_env(m, c / 2 ** (1 / 6), c * 2 ** (1 / 6), win=0.01)[on:]
                t.append(schroeder_t(e ** 2, SR, -5, -25))
            ax.semilogx(fc, t, ls, color=col, label=f"{lab} v{v}")
    ax.set_title(f"{p}: T60 from T20 (Schroeder) per 1/3 oct", fontsize=9)
    ax.set_xlabel("Hz")
    ax.set_ylabel("s")
    ax.set_ylim(0, 16)
    ax.grid(alpha=0.3)
    ax.legend(fontsize=7)
fig.tight_layout()
fig.savefig(FIG + "cym_decay_vs_freq.png", dpi=80)
plt.close(fig)

# ---- 5. spectral centroid over time (20 ms frames)
fig, axs = plt.subplots(1, 4, figsize=(17, 4.5))
for ax, p in zip(axs, ["crash", "ride", "ride_bell", "hat_open"]):
    for X, col, lab in ((M, CM, "modelled"), (R, CR, "recorded DRS")):
        for v, ls in ((40, ":"), (80, "--"), (127, "-")):
            m, on = mono(X, p, v)
            f, t, S = signal.stft(m[on:on + int(2.5 * SR)], SR, nperseg=1024, noverlap=1024 - 480)
            P = np.abs(S) ** 2
            k = f > 200
            cen = (f[k, None] * P[k]).sum(0) / (P[k].sum(0) + 1e-30)
            ax.plot(t, cen, ls, color=col, lw=1, label=f"{lab} v{v}")
    ax.set_title(f"{p}: spectral centroid (>200 Hz)", fontsize=9)
    ax.set_xlabel("s")
    ax.set_ylabel("Hz")
    ax.grid(alpha=0.3)
    ax.legend(fontsize=7)
fig.tight_layout()
fig.savefig(FIG + "cym_centroid.png", dpi=80)
plt.close(fig)

# ---- 6. long-term spectra (1/6-oct smoothed) per cymbal, 0-2 s, both levels absolute
fig, axs = plt.subplots(2, 3, figsize=(16, 8))
for ax, p in zip(axs.flat, ["hat_closed", "hat_open", "hat_pedal", "crash", "ride", "ride_bell"]):
    for X, col, lab in ((M, CM, "modelled"), (R, CR, "recorded DRS")):
        for v, ls in ((40, ":"), (80, "--"), (127, "-")):
            m, on = mono(X, p, v)
            w = m[on:on + int(2 * SR)]
            f, P = signal.welch(w, SR, nperseg=8192)
            # 1/6-octave smoothing
            Ps = np.array([P[(f >= x / 2 ** (1 / 12)) & (f < x * 2 ** (1 / 12))].mean() for x in f[5:]])
            ax.semilogx(f[5:], 10 * np.log10(Ps + 1e-30), ls, color=col, lw=1, label=f"{lab} v{v}")
    ax.set_xlim(40, 22000)
    ax.set_title(f"{p}: long-term spectrum 0-2 s (1/6 oct)", fontsize=9)
    ax.grid(alpha=0.3)
    ax.legend(fontsize=6)
fig.tight_layout()
fig.savefig(FIG + "cym_ltas.png", dpi=80)
plt.close(fig)
print("figs ok")
