"""Drums (kick, snare, toms, rim, sidestick): pitch + glide, per-band decay, attack, snare wires."""
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
DR = ["kick", "snare", "rimshot", "sidestick", "tom_hi", "tom_mid", "tom_floor"]
CM, CR = "#c0392b", "#2471a3"


def f0_track(m, on, fmax):
    """Strongest peak below fmax, 4096-pt frames with 8x zero padding, every 10 ms, 0-600 ms."""
    out = []
    for t in np.arange(0.01, 0.6, 0.01):
        i = on + int(t * SR)
        w = m[i:i + 4096] * np.hanning(4096)
        P = np.abs(np.fft.rfft(w, 32768))
        f = np.fft.rfftfreq(32768, 1 / SR)
        s = (f > 35) & (f < fmax)
        out.append(f[s][np.argmax(P[s])])
    return np.array(out)


def band_decays(m, on):
    res = []
    for lo, hi in [(40, 100), (100, 250), (250, 500), (500, 1000), (1000, 2000), (2000, 4000), (4000, 8000), (8000, 16000)]:
        e = band_env(m, lo, hi, win=0.005)[on:]
        res.append(schroeder_t(e ** 2, SR, -5, -25))
    return res


def attack(m, on):
    e = np.sqrt(np.convolve(m * m, np.ones(48) / 48, "same"))[max(0, on - 480):on + 4800]
    pk = np.argmax(e)
    rise = (pk - np.argmax(e > 0.1 * e[pk])) / SR * 1000
    # share of energy above 2 kHz in the first 5 ms vs in 5-50 ms
    sos = signal.butter(4, 2000, "high", fs=SR, output="sos")
    h = signal.sosfiltfilt(sos, m)
    a = np.sum(h[on:on + 240] ** 2) / np.sum(m[on:on + 240] ** 2)
    b = np.sum(h[on + 240:on + 2400] ** 2) / np.sum(m[on + 240:on + 2400] ** 2)
    return rise, 10 * np.log10(a + 1e-12), 10 * np.log10(b + 1e-12)


fmax = {"kick": 120, "snare": 400, "rimshot": 400, "sidestick": 1200, "tom_hi": 300, "tom_mid": 250, "tom_floor": 200}
print("piece vel | f0@20ms M/R | glide20->400ms cents M/R | rise ms M/R | HF share first 5 ms dB M/R | 5-50 ms M/R")
for p in DR:
    for v in VELS:
        r = []
        for X in (M, R):
            s, on = hit(X, p, v)
            m = s.mean(1)
            tr = f0_track(m, on, fmax[p])
            gl = 1200 * np.log2(tr[1] / np.median(tr[35:40]))
            r.append((tr[1], gl) + attack(m, on))
        print(f"{p:9s} {v:3d} | {r[0][0]:6.1f} {r[1][0]:6.1f} | {r[0][1]:6.0f} {r[1][1]:6.0f} | {r[0][2]:5.1f} {r[1][2]:5.1f} | {r[0][3]:6.1f} {r[1][3]:6.1f} | {r[0][4]:6.1f} {r[1][4]:6.1f}")

print("\nT60 from T20 per band [40-100,100-250,250-500,0.5-1k,1-2k,2-4k,4-8k,8-16k] s")
for p in DR:
    for v in (80, 127):
        for lab, X in (("M", M), ("R", R)):
            s, on = hit(X, p, v)
            d = band_decays(s.mean(1), on)
            print(f"{p:9s} {v:3d} {lab}: " + " ".join(f"{x:5.2f}" for x in d))

# snare: wires (2-8 kHz) vs head (150-300 Hz) envelopes; flatness of 2-8 kHz in 30-150 ms
fig, axs = plt.subplots(2, 4, figsize=(17, 7), sharey=True)
for c, v in enumerate(VELS):
    for r, (X, lab) in enumerate(((M, "modelled"), (R, "recorded DRS"))):
        s, on = hit(X, "snare", v)
        m = s.mean(1)
        ax = axs[r, c]
        for (lo, hi), col, nm in [((150, 300), "k", "head 150-300"), ((2000, 8000), "tab:orange", "wires 2-8k"), ((8000, 16000), "tab:green", "8-16k")]:
            e = band_env(m, lo, hi, win=0.003)[on:on + int(1.0 * SR)]
            ax.plot(np.arange(len(e)) / SR * 1000, 20 * np.log10(e + 1e-12), color=col, lw=0.8, label=nm)
        ax.set_title(f"snare v{v} {lab}", fontsize=9)
        ax.set_ylim(-130, -10)
        ax.grid(alpha=0.3)
        ax.legend(fontsize=7)
fig.tight_layout()
fig.savefig(FIG + "snare_wires.png", dpi=80)
plt.close(fig)

print("\nsnare wire band 2-8k: flatness 30-150 ms, level re head at 100 ms")
for v in VELS:
    o = []
    for X in (M, R):
        s, on = hit(X, "snare", v)
        m = s.mean(1)
        w = m[on + int(0.03 * SR):on + int(0.15 * SR)]
        f, P = signal.periodogram(w * np.hanning(len(w)), SR)
        k = (f > 2000) & (f < 8000)
        eh = band_env(m, 150, 300)[on + int(0.1 * SR)]
        ew = band_env(m, 2000, 8000)[on + int(0.1 * SR)]
        o.append((flatness(P[k]), 20 * np.log10(ew / eh)))
    print(f" v{v}: flat M {o[0][0]:.3f} R {o[1][0]:.3f} | wires-head @100ms M {o[0][1]:.1f} R {o[1][1]:.1f} dB")

# pitch-track figure, kick + toms + snare
fig, axs = plt.subplots(1, 5, figsize=(18, 4))
for ax, p in zip(axs, ["kick", "snare", "tom_hi", "tom_mid", "tom_floor"]):
    for X, col, lab in ((M, CM, "modelled"), (R, CR, "recorded")):
        for v, ls in ((80, "--"), (127, "-")):
            s, on = hit(X, p, v)
            tr = f0_track(s.mean(1), on, fmax[p])
            ax.plot(np.arange(0.01, 0.6, 0.01) * 1000, tr, ls, color=col, label=f"{lab} v{v}")
    ax.set_title(f"{p}: strongest low peak", fontsize=9)
    ax.set_xlabel("ms")
    ax.set_ylabel("Hz")
    ax.legend(fontsize=7)
    ax.grid(alpha=0.3)
fig.tight_layout()
fig.savefig(FIG + "drum_pitch.png", dpi=80)
plt.close(fig)
