"""Cymbal/hat metrics: tonality (flatness, tonal-energy fraction, peaks/kHz), fine-structure
stability, envelope fluctuation, per-band decay, energy cascade and centroid track."""
import sys
sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
import json
import numpy as np
from scipy import signal, ndimage
from common import *

M = load("golpes-modelado-studio.flac")
R = load("golpes-grabado-drskit.flac")
CYM = ["hat_closed", "hat_open", "hat_pedal", "crash", "ride", "ride_bell"]
BANDS = [(500, 1000), (1000, 2000), (2000, 4000), (4000, 8000), (8000, 12000), (12000, 20000)]
# analysis window (s after onset) for the tonal measures
WIN = {"hat_closed": (0.01, 0.12), "hat_pedal": (0.01, 0.10), "hat_open": (0.1, 0.6),
       "crash": (0.15, 0.65), "ride": (0.15, 0.65), "ride_bell": (0.15, 0.65)}


def tonal(m, on, piece):
    a, b = WIN[piece]
    w = m[on + int(a * SR):on + int(b * SR)]
    n = len(w)
    nfft = 1 << int(np.ceil(np.log2(n)) + 1)
    P = np.abs(np.fft.rfft(w * signal.windows.hann(n), nfft)) ** 2
    f = np.fft.rfftfreq(nfft, 1 / SR)
    df = f[1]
    # running median over 300 Hz as the "noise floor" reference
    k = int(300 / df) | 1
    Pdb = 10 * np.log10(P + 1e-30)
    med = ndimage.median_filter(Pdb, size=k)
    res = {}
    for lo, hi in BANDS:
        s = (f >= lo) & (f < hi)
        flat = float(flatness(P[s]))
        exc = Pdb[s] - med[s]
        ton_frac = float(P[s][exc > 10].sum() / P[s].sum())
        # peaks standing >= 12 dB above the local median, at least 20 Hz apart
        pk, _ = signal.find_peaks(exc, height=12, distance=max(1, int(20 / df)))
        dens = len(pk) / ((hi - lo) / 1000)
        p99 = float(np.percentile(exc, 99.5))
        res[f"{lo}-{hi}"] = dict(flat=flat, tonal=ton_frac, peaks_per_khz=dens, p995=p99)
    return res


def stability(m, on, piece):
    """Correlation of fine-structure (log spectrum minus smoothed) between frames 100 ms apart, 4-16 kHz."""
    nfft = 4096
    f = np.fft.rfftfreq(nfft, 1 / SR)
    s = (f >= 4000) & (f < 16000)
    a, b = (0.03, 0.25) if piece in ("hat_closed", "hat_pedal") else (0.2, 1.2)
    cs = []
    t = a
    hop = 0.05
    while t + hop + nfft / SR < b:
        def fs(tt):
            i = on + int(tt * SR)
            X = 10 * np.log10(np.abs(np.fft.rfft(m[i:i + nfft] * np.hanning(nfft))) ** 2 + 1e-30)
            X = X[s]
            return X - ndimage.uniform_filter1d(X, 41)
        x1, x2 = fs(t), fs(t + hop)
        cs.append(np.corrcoef(x1, x2)[0, 1])
        t += hop
    return float(np.mean(cs)) if cs else np.nan


def env_fluct(m, on, piece):
    """Residual std (dB) of 4-8 and 8-16 kHz envelopes around a smooth (3rd-order) log decay."""
    a, b = (0.02, 0.2) if piece in ("hat_closed", "hat_pedal") else (0.1, 1.5)
    out = {}
    for lo, hi in [(2000, 4000), (4000, 8000), (8000, 16000)]:
        e = band_env(m, lo, hi, win=0.002)
        seg = 20 * np.log10(e[on + int(a * SR):on + int(b * SR)] + 1e-12)[::48]
        t = np.arange(len(seg))
        p = np.polyfit(t, seg, 3)
        out[f"{lo}-{hi}"] = float(np.std(seg - np.polyval(p, t)))
    return out


def decays(m, on):
    out = {}
    for lo, hi in [(250, 500), (500, 1000), (1000, 2000), (2000, 4000), (4000, 8000), (8000, 16000), (16000, 22000)]:
        e = band_env(m, lo, hi, win=0.01)[on:]
        out[f"{lo}-{hi}"] = dict(edt=schroeder_t(e ** 2, SR, 0, -10), t20=schroeder_t(e ** 2, SR, -5, -25))
    return out


def cascade(m, on):
    out = {}
    for lo, hi in [(1000, 2000), (2000, 4000), (4000, 8000), (8000, 12000), (12000, 20000)]:
        e = band_env(m, lo, hi, win=0.005)[on:on + int(0.5 * SR)]
        edb = 20 * np.log10(e + 1e-12)
        out[f"{lo}-{hi}"] = dict(t_peak_ms=float(np.argmax(e) / SR * 1000),
                                 at5_rel=float(edb[int(0.005 * SR)] - edb.max()),
                                 at100_rel=float(edb[int(0.1 * SR)] - edb.max()),
                                 at400_rel=float(edb[int(0.4 * SR)] - edb.max()))
    return out


ALL = {}
for p in CYM:
    for v in VELS:
        for lab, X in (("M", M), ("R", R)):
            seg, on = hit(X, p, v)
            m = seg.mean(1)
            ALL[f"{p}|{v}|{lab}"] = dict(tonal=tonal(m, on, p), stab=stability(m, on, p),
                                         fluct=env_fluct(m, on, p), dec=decays(m, on), casc=cascade(m, on))
json.dump(ALL, open(OUT + "scripts/cymbal_metrics.json", "w"), indent=1, default=float)

for p in CYM:
    print(f"\n=== {p} ===")
    for v in VELS:
        a, b = ALL[f"{p}|{v}|M"], ALL[f"{p}|{v}|R"]
        print(f" v{v}: fine-structure stability M {a['stab']:.2f} R {b['stab']:.2f}")
        for bd in a["tonal"]:
            x, y = a["tonal"][bd], b["tonal"][bd]
            print(f"   {bd:12s} flat M {x['flat']:.3f} R {y['flat']:.3f} | tonal-frac M {x['tonal']:.2f} R {y['tonal']:.2f}"
                  f" | peaks/kHz M {x['peaks_per_khz']:5.1f} R {y['peaks_per_khz']:5.1f} | p99.5 M {x['p995']:4.1f} R {y['p995']:4.1f}")
        print("   fluct dB M", {k: round(x, 2) for k, x in a["fluct"].items()}, "R", {k: round(x, 2) for k, x in b["fluct"].items()})
        print("   T20 M", {k: round(x["t20"], 2) for k, x in a["dec"].items()})
        print("   T20 R", {k: round(x["t20"], 2) for k, x in b["dec"].items()})
        print("   EDT M", {k: round(x["edt"], 2) for k, x in a["dec"].items()})
        print("   EDT R", {k: round(x["edt"], 2) for k, x in b["dec"].items()})
        print("   casc M", {k: (round(x["t_peak_ms"]), round(x["at5_rel"], 1), round(x["at100_rel"], 1), round(x["at400_rel"], 1)) for k, x in a["casc"].items()})
        print("   casc R", {k: (round(x["t_peak_ms"]), round(x["at5_rel"], 1), round(x["at100_rel"], 1), round(x["at400_rel"], 1)) for k, x in b["casc"].items()})
