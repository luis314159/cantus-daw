"""Figures: per element, spectrograms (gen/rec) and first-30-ms waveforms + band envelopes, v80 and v127."""
import sys
sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
from feats import *
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt

DUR = {"bombo": 1.0, "caja": 0.8, "tom-medio": 1.5, "tom-de-piso": 1.5, "hihat-cerrado": 0.5,
       "hihat-abierto": 2.0, "ride": 3.0, "crash": 3.0}
BANDS = [(40, 200), (200, 1000), (1000, 4000), (4000, 10000), (10000, 20000)]


def spec(ax, m, on, dur, title):
    y = m[max(0, on - int(0.01 * SR)):on + int(dur * SR)]
    f, t, S = signal.stft(y, SR, nperseg=1024, noverlap=1024 - 128)
    P = 20 * np.log10(np.abs(S) + 1e-9)
    P -= P.max()
    ax.pcolormesh(t * 1000 - 10, f / 1000, P, vmin=-90, vmax=0, shading="auto", cmap="magma")
    ax.set_yscale("symlog", linthresh=1); ax.set_ylim(0.03, 22)
    ax.set_title(title, fontsize=8); ax.set_xlabel("ms", fontsize=7); ax.tick_params(labelsize=6)


def early(ax, ax2, m, on, title):
    m = np.pad(m, (200, 0)); on = on + 200
    t = (np.arange(-int(0.002 * SR), int(0.03 * SR))) / SR * 1000
    y = m[on - int(0.002 * SR):on + int(0.03 * SR)]
    ax.plot(t, y / np.abs(y).max(), lw=.6, color="k"); ax.set_title(title, fontsize=8); ax.tick_params(labelsize=6)
    cols = plt.cm.viridis(np.linspace(0, .9, len(BANDS)))
    pk = None
    for (lo, hi), c in zip(BANDS, cols):
        e = band_env(m, lo, hi, 0.0005)[on - int(0.002 * SR):on + int(0.03 * SR)]
        d = 20 * np.log10(e)
        if pk is None:
            pk = 20 * np.log10(np.abs(m).max())
        ax2.plot(t, d - pk, color=c, lw=.8, label=f"{lo/1000:g}-{hi/1000:g}k")
    ax2.set_ylim(-90, 0); ax2.grid(alpha=.3); ax2.legend(fontsize=5); ax2.tick_params(labelsize=6)


if __name__ == "__main__":
    for kit in KITS:
        for el in ELEMENTS:
            fig, axs = plt.subplots(4, 4, figsize=(16, 12))
            for r, vi in enumerate((1, 3)):
                for c, v in enumerate(("gen", "rec")):
                    m, on = element_hits(kit, el, v)[vi]
                    spec(axs[2 * r, 2 * c], m, on, DUR[el], f"{kit} {el} v{VELS[vi]} {v}")
                    early(axs[2 * r, 2 * c + 1], axs[2 * r + 1, 2 * c + 1], m, on, f"{v} first 30 ms")
                    # band T: envelope over the whole hit
                    ax = axs[2 * r + 1, 2 * c]
                    for (lo, hi), col in zip(BANDS, plt.cm.viridis(np.linspace(0, .9, len(BANDS)))):
                        e = band_env(m, lo, hi, 0.01)[on:on + int(DUR[el] * SR)]
                        ax.plot(np.arange(len(e)) / SR * 1000, 20 * np.log10(e / np.abs(m).max()), color=col, lw=.8)
                    ax.set_ylim(-100, 0); ax.grid(alpha=.3); ax.set_title(f"{v} band envelopes", fontsize=8)
                    ax.tick_params(labelsize=6)
            fig.tight_layout(); fig.savefig(FIG + f"{kit}_{el}.png", dpi=70); plt.close(fig)
            print(kit, el)
