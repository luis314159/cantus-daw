"""Cymbal and hat measures of a hits file against a reference (round 40, tymp-engine).

The measures and their windows follow analysis-drums/scripts (s03, s11, s15), so the
numbers compare with analysis-drums/report.md's targets.

    python -I metrics.py <model hits.flac> <reference hits.flac> [piece ...]

A hits file is tympanum_measure's layout: 13 articulations x 4 velocities, 4 s each.
"""
import sys
import numpy as np
import soundfile as sf
from scipy import signal, ndimage

SR = 48000
PIECES = ["kick", "snare", "rimshot", "sidestick", "tom_hi", "tom_mid", "tom_floor",
          "hat_closed", "hat_open", "hat_pedal", "crash", "ride", "ride_bell"]
VELS = [40, 80, 110, 127]
CYM = ["hat_closed", "hat_open", "hat_pedal", "crash", "ride", "ride_bell"]
WIN = {"hat_closed": (0.01, 0.12), "hat_pedal": (0.01, 0.10), "hat_open": (0.1, 0.6),
       "crash": (0.15, 0.65), "ride": (0.15, 0.65), "ride_bell": (0.15, 0.65)}


def load(path):
    x, sr = sf.read(path, always_2d=True)
    assert sr == SR, sr
    return x.astype(np.float64)


def hit(x, piece, vel, length=3.9, pre=0.02):
    i = PIECES.index(piece) * 4 + VELS.index(vel)
    seg = x[int(i * 4 * SR):int((i + 1) * 4 * SR)]
    mono = np.abs(seg).sum(1)
    on = int(np.argmax(mono > 0.05 * mono.max()))
    a = max(0, on - int(pre * SR))
    return seg[a:a + int(length * SR)].mean(1), on - a


def band_env(m, lo, hi, win=0.005):
    sos = signal.butter(4, [lo, min(hi, 23500)], btype="band", fs=SR, output="sos")
    y = signal.sosfiltfilt(sos, m)
    n = max(1, int(win * SR))
    return np.sqrt(np.convolve(y * y, np.ones(n) / n, mode="same") + 1e-30)


def schroeder_t(e2, lo_db, hi_db):
    s = np.cumsum(e2[::-1])[::-1]
    s = 10 * np.log10(s / s[0] + 1e-30)
    i0, i1 = np.argmax(s <= lo_db), np.argmax(s <= hi_db)
    if i1 <= i0 or s[-1] > hi_db:
        return np.nan
    t = np.arange(len(s)) / SR
    return -60.0 / np.polyfit(t[i0:i1], s[i0:i1], 1)[0]


def flatness(p):
    p = np.maximum(p, 1e-20)
    return np.exp(np.mean(np.log(p))) / np.mean(p)


def tonal(m, on, piece, lo, hi):
    a, b = WIN[piece]
    w = m[on + int(a * SR):on + int(b * SR)]
    n = len(w)
    nfft = 1 << int(np.ceil(np.log2(n)) + 1)
    P = np.abs(np.fft.rfft(w * signal.windows.hann(n), nfft)) ** 2
    f = np.fft.rfftfreq(nfft, 1 / SR)
    d = 10 * np.log10(P + 1e-30)
    med = ndimage.median_filter(d, size=int(300 / f[1]) | 1)
    s = (f >= lo) & (f < hi)
    exc = d[s] - med[s]
    pk, _ = signal.find_peaks(exc, height=12, distance=max(1, int(20 / f[1])))
    return flatness(P[s]), len(pk) / ((hi - lo) / 1000), float(np.percentile(exc, 99.5))


def stability(m, on, piece):
    nfft = 4096
    f = np.fft.rfftfreq(nfft, 1 / SR)
    s = (f >= 4000) & (f < 16000)
    a, b = (0.03, 0.25) if piece in ("hat_closed", "hat_pedal") else (0.2, 1.2)

    def fs(tt):
        i = on + int(tt * SR)
        X = 10 * np.log10(np.abs(np.fft.rfft(m[i:i + nfft] * np.hanning(nfft))) ** 2 + 1e-30)[s]
        return X - ndimage.uniform_filter1d(X, 41)
    cs, t = [], a
    while t + 0.05 + nfft / SR < b:
        cs.append(np.corrcoef(fs(t), fs(t + 0.05))[0, 1])
        t += 0.05
    return float(np.mean(cs)) if cs else np.nan


def partials(m, on, piece):
    """Top-1 and top-3 partials' share of the energy above 1 kHz (s11's way)."""
    a, b = (0.01, 0.12) if piece in ("hat_closed", "hat_pedal") else (0.1, 1.1)
    w = m[on + int(a * SR):on + int(b * SR)]
    n = len(w)
    P = np.abs(np.fft.rfft(w * np.hanning(n), 4 * n)) ** 2
    f = np.fft.rfftfreq(4 * n, 1 / SR)
    d = 10 * np.log10(P + 1e-30)
    k = f > 1000
    pk, _ = signal.find_peaks(d * k, distance=max(1, int(30 / f[1])))
    pk = pk[np.argsort(d[pk])[::-1]][:8]
    tot = P[k].sum()
    sh = sorted((P[max(0, i - 8):i + 9].sum() / tot * 100 for i in pk), reverse=True)
    return sh[0], sum(sh[:3])


def centroid(x):
    P = np.abs(np.fft.rfft(x * np.hanning(len(x)))) ** 2
    f = np.fft.rfftfreq(len(x), 1 / SR)
    return float((P * f).sum() / (P.sum() + 1e-30))


def measures(m, on, piece):
    r = {}
    r["rms"] = 10 * np.log10(np.mean(m[on:on + SR // 2] ** 2) + 1e-30)
    r["top1"], r["top3"] = partials(m, on, piece)
    r["flat4-8"], r["pk4-8"], _ = tonal(m, on, piece, 4000, 8000)
    r["flat8-12"], r["pk8-12"], _ = tonal(m, on, piece, 8000, 12000)
    r["peakdB"] = tonal(m, on, piece, 4000, 12000)[2]
    r["stab"] = stability(m, on, piece)
    r["c3ms"] = centroid(m[on:on + 144]) / 1000
    r["c300"] = centroid(m[on:on + int(0.3 * SR)]) / 1000
    hi = band_env(m, 6000, 20000, win=0.001)[on:on + SR // 2]
    lo = band_env(m, 200, 1500, win=0.001)[on:on + SR // 2]
    r["hi/lo3"] = 20 * np.log10(hi[:144].max() / lo[:144].max())
    e = band_env(m, 8000, 16000)[on:on + SR // 2]
    edb = 20 * np.log10(e + 1e-12)
    r["pk8k_ms"] = np.argmax(e) / SR * 1000
    r["at400"] = edb[int(0.4 * SR)] - edb.max()
    seg = m[on:on + 2 * SR]
    sp = np.abs(np.fft.rfft(seg)) ** 2
    f = np.fft.rfftfreq(len(seg), 1 / SR)
    r["<450"] = 10 * np.log10(sp[f < 450].sum() / sp.sum() + 1e-30)
    for lo_, hi_ in [(500, 1000), (2000, 4000), (8000, 16000)]:
        ee = band_env(m, lo_, hi_, win=0.01)[on:]
        r[f"T{lo_ // 1000 if lo_ >= 1000 else lo_}"] = schroeder_t(ee ** 2, -5, -25)
    return r


KEYS = ["rms", "top1", "top3", "peakdB", "flat4-8", "pk4-8", "flat8-12", "pk8-12", "stab", "c3ms",
        "c300", "hi/lo3", "pk8k_ms", "at400", "<450", "T500", "T2", "T8"]


def table(files, pieces):
    xs = [load(f) for f in files]
    out = []
    for p in pieces:
        out.append(f"\n=== {p} ===   " + "  ".join(f"[{i}] {f.split('/')[-1]}" for i, f in enumerate(files)))
        out.append("vel src " + " ".join(f"{k:>8s}" for k in KEYS))
        for v in VELS:
            for i, x in enumerate(xs):
                m, on = hit(x, p, v)
                r = measures(m, on, p)
                out.append(f"{v:3d} [{i}] " + " ".join(f"{r[k]:8.2f}" for k in KEYS))
    return "\n".join(out)


if __name__ == "__main__" and not any(a in ("kick", "snare", "rimshot", "sidestick", "tom_hi", "tom_mid", "tom_floor") for a in sys.argv[1:]):
    files = [a for a in sys.argv[1:] if a.endswith(".flac") or a.endswith(".wav")]
    pieces = [a for a in sys.argv[1:] if a in PIECES] or CYM
    print(table(files, pieces))


DRUMS = ["kick", "snare", "rimshot", "sidestick", "tom_hi", "tom_mid", "tom_floor"]


def low_peak(m, on, a, b, lo=30, hi=400):
    w = m[on + int(a * SR):on + int(b * SR)]
    n = len(w)
    P = np.abs(np.fft.rfft(w * np.hanning(n), 16 * n)) ** 2
    f = np.fft.rfftfreq(16 * n, 1 / SR)
    k = (f >= lo) & (f <= hi)
    return float(f[k][np.argmax(P[k])])


def drum_measures(m, on, piece):
    """analysis-drums s05's measures: the level law, the low end's T60, how the body dies
    (dB re peak at 300 and 600 ms, the slope between), the glide, the attack's colour."""
    r = {}
    r["rms"] = 10 * np.log10(np.mean(m[on:on + SR // 2] ** 2) + 1e-30)
    e = band_env(m, 20, 20000, win=0.01)[on:]
    edb = 20 * np.log10(e + 1e-12)
    pk = edb[:SR // 10].max()
    r["at300"] = edb[int(0.3 * SR)] - pk
    r["at600"] = edb[int(0.6 * SR)] - pk
    r["slope"] = (edb[int(0.6 * SR)] - edb[int(0.3 * SR)]) / 0.3
    lo_band = (40, 100) if piece == "kick" else (40, 250)
    ee = band_env(m, *lo_band, win=0.01)[on:]
    r["T60low"] = schroeder_t(ee ** 2, -5, -25)
    ew = band_env(m, 2000, 8000, win=0.01)[on:]
    r["T60hi"] = schroeder_t(ew ** 2, -5, -25)
    late = low_peak(m, on, 0.15, 0.45)
    early = low_peak(m, on, 0.01, 0.06, late * 0.9, late * 1.3)
    r["f_late"] = late
    r["glide"] = 1200 * np.log2(early / late)
    r["c3ms"] = centroid(m[on:on + 144]) / 1000
    r["c15_60"] = centroid(m[on + 720:on + 2880]) / 1000
    return r


DKEYS = ["rms", "at300", "at600", "slope", "T60low", "T60hi", "f_late", "glide", "c3ms", "c15_60"]


def drum_table(files, pieces):
    xs = [load(f) for f in files]
    out = []
    for p in pieces:
        out.append(f"\n=== {p} ===")
        out.append("vel src " + " ".join(f"{k:>8s}" for k in DKEYS))
        for v in VELS:
            for i, x in enumerate(xs):
                m, on = hit(x, p, v)
                r = drum_measures(m, on, p)
                out.append(f"{v:3d} [{i}] " + " ".join(f"{r[k]:8.2f}" for k in DKEYS))
    return "\n".join(out)


if __name__ == "__main__" and any(a in DRUMS for a in sys.argv[1:]):
    files = [a for a in sys.argv[1:] if a.endswith(".flac") or a.endswith(".wav")]
    print(drum_table(files, [a for a in sys.argv[1:] if a in DRUMS]))
