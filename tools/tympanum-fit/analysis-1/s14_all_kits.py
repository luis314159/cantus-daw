"""The key cymbal measures across the modelled kit and five recorded kits (builder's meas9 hit files,
same layout: 13 pieces x 4 velocities, 4 s each)."""
import sys
sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
import os
import numpy as np
import soundfile as sf
from scipy import signal, ndimage
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from common import *

D = __import__("os").path.join(__import__("os").environ.get("TYMP_MEAS", "meas"), "")
KITS = {"modelled": "model-studio-hits.flac", "DRS": "pack-drskit-tympanum-kit-hits.flac",
        "Crocell": "pack-crocellkit-tympanum-kit-hits.flac", "Muldjord": "pack-muldjordkit-tympanum-kit-hits.flac",
        "BigRusty": "pack-big-rusty-drums-kit-hits.flac", "Virtuosity": "pack-virtuosity-drums-kit-hits.flac"}


def measures(m, on, p):
    out = {}
    a, b = (0.01, 0.12) if p in ("hat_closed", "hat_pedal") else (0.15, 0.65)
    w = m[on + int(a * SR):on + int(b * SR)]
    if np.sum(w ** 2) < 1e-14:
        return None
    P = np.abs(np.fft.rfft(w * np.hanning(len(w)))) ** 2
    f = np.fft.rfftfreq(len(w), 1 / SR)
    s = (f >= 4000) & (f < 12000)
    out["flat4_12"] = float(flatness(P[s]))
    out["eff_per_khz"] = float(P[s].sum() ** 2 / (P[s] ** 2).sum() / 8)
    # top partial share above 1 kHz (+-4 bins)
    k = f > 1000
    pk, _ = signal.find_peaks(P * k, distance=8)
    top = np.sort(P[pk])[::-1][:3]
    shares = []
    for v in top:
        i = np.where(P == v)[0][0]
        shares.append(P[max(0, i - 4):i + 5].sum())
    out["top3_share"] = float(sum(shares) / P[k].sum() * 100)
    Pd = 10 * np.log10(P + 1e-30)
    med = ndimage.median_filter(Pd, size=int(300 / f[1]) | 1)
    out["p995_excess"] = float(np.percentile((Pd - med)[s], 99.5))
    if p not in ("hat_closed", "hat_pedal"):
        tl = []
        for lo, hi in [(500, 1000), (8000, 16000)]:
            e = band_env(m, lo, hi, win=0.01)[on:]
            tl.append(schroeder_t(e ** 2, SR, -5, -25))
        out["T60_8_16k"] = tl[1]
        out["T60_ratio_hf_lf"] = tl[1] / tl[0]
        e = band_env(m, 8000, 16000, win=0.005)[on:on + int(0.6 * SR)]
        out["t_peak_8_16k_ms"] = float(np.argmax(e) / SR * 1000)
        edb = 20 * np.log10(e + 1e-12)
        out["drop_8_16k_at_400ms"] = float(edb[int(0.4 * SR)] - edb.max())
        f2, P2 = signal.welch(m[on:on + int(2 * SR)], SR, nperseg=8192)
        out["share_below_450_dB"] = float(10 * np.log10(P2[(f2 > 40) & (f2 < 450)].sum() / P2[f2 > 40].sum()))
    # centroid 0-3 ms and 15-60 ms
    for nm, (x0, x1) in (("cen_0_3ms", (0, 0.003)), ("cen_15_60ms", (0.015, 0.06))):
        ww = m[on + int(x0 * SR):on + int(x1 * SR)]
        PP = np.abs(np.fft.rfft(ww * np.hanning(len(ww)), 4096)) ** 2
        ff = np.fft.rfftfreq(4096, 1 / SR)
        out[nm] = float((ff * PP).sum() / PP.sum())
    # fine-structure stability 4-16 kHz
    nfft = 4096
    ff = np.fft.rfftfreq(nfft, 1 / SR)
    ss = (ff >= 4000) & (ff < 16000)
    t0, t1 = (0.03, 0.25) if p in ("hat_closed", "hat_pedal") else (0.2, 1.2)
    cs = []
    t = t0
    while t + 0.05 + nfft / SR < t1:
        def fs(tt):
            i = on + int(tt * SR)
            X = 10 * np.log10(np.abs(np.fft.rfft(m[i:i + nfft] * np.hanning(nfft))) ** 2 + 1e-30)[ss]
            return X - ndimage.uniform_filter1d(X, 41)
        cs.append(np.corrcoef(fs(t), fs(t + 0.05))[0, 1])
        t += 0.05
    out["fine_stability"] = float(np.mean(cs))
    return out


rows = {}
for kit, fn in KITS.items():
    x, sr = sf.read(D + fn, always_2d=True)
    x = x.astype(np.float64)
    for p in ["hat_closed", "hat_open", "hat_pedal", "crash", "ride", "ride_bell"]:
        for v in (80, 110):
            seg, on = hit(x, p, v)
            r = measures(seg.mean(1), on, p)
            rows[(kit, p, v)] = r

keys = ["flat4_12", "eff_per_khz", "top3_share", "p995_excess", "fine_stability", "T60_8_16k", "T60_ratio_hf_lf",
        "t_peak_8_16k_ms", "drop_8_16k_at_400ms", "share_below_450_dB", "cen_0_3ms", "cen_15_60ms"]
for p in ["hat_closed", "hat_open", "hat_pedal", "crash", "ride", "ride_bell"]:
    print(f"\n=== {p} (v80 / v110) ===")
    print(f"{'measure':22s}" + "".join(f"{k:>18s}" for k in KITS))
    for key in keys:
        line = f"{key:22s}"
        any_ = False
        for kit in KITS:
            vals = []
            for v in (80, 110):
                r = rows[(kit, p, v)]
                vals.append(r.get(key, np.nan) if r else np.nan)
            if not all(np.isnan(vals)):
                any_ = True
            line += f"{vals[0]:9.3g}/{vals[1]:<8.3g}"
        if any_:
            print(line)

# summary figure: flatness and effective components per kHz, all kits, v110
fig, axs = plt.subplots(1, 4, figsize=(18, 4.5))
pieces = ["hat_closed", "hat_open", "hat_pedal", "crash", "ride", "ride_bell"]
for ax, key, title, log in [(axs[0], "flat4_12", "spectral flatness 4-12 kHz (white noise 0.56)", True),
                            (axs[1], "eff_per_khz", "effective components per kHz, 4-12 kHz", True),
                            (axs[2], "top3_share", "energy share of the 3 loudest partials >1 kHz (%)", False),
                            (axs[3], "fine_stability", "fine-structure stability 4-16 kHz (1 = frozen)", False)]:
    for j, kit in enumerate(KITS):
        vals = [rows[(kit, p, 110)][key] if rows[(kit, p, 110)] else np.nan for p in pieces]
        ax.plot(np.arange(len(pieces)) + (j - 2.5) * 0.08, vals, "o", ms=6 if kit == "modelled" else 4,
                color="#c0392b" if kit == "modelled" else plt.cm.Blues(0.4 + 0.12 * j), label=kit)
    ax.set_xticks(range(len(pieces)))
    ax.set_xticklabels(pieces, rotation=30, fontsize=8)
    if log:
        ax.set_yscale("log")
    ax.set_title(title + ", v110", fontsize=9)
    ax.grid(alpha=0.3)
axs[0].legend(fontsize=7)
fig.tight_layout()
fig.savefig(FIG + "all_kits_tonality.png", dpi=80)
print("ok")
