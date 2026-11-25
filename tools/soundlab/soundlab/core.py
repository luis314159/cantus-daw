"""The signal core, ported unchanged in method from the Arcus reference study
(`tools/arcus/study/bowlib_core.py`, round 42), whose accuracy on synthetic notes is in its
`validate_synth.py` and the study's refdb README; the lab's own tests
(`tools/soundlab/tests/`) hold it again.

- rms_db(x, sr): frame RMS in dB.
- segment(x, sr): the notes of a file as (start, end) sample pairs (Iowa's runs hold several).
- yin_track(...): YIN f0 (de Cheveigne & Kawahara 2002, JASA 111(4)) per frame, parabolic
  refinement; returns (times, f0, aperiodicity d').
- refine_track(...): the phase-locked refinement (the strongest low harmonic's phase drift).
- phase_of(track): the instantaneous phase in cycles at every sample.
- project(...): complex amplitudes c_n(t) of harmonics n = 1..N by projection on
  exp(-j 2 pi n phi(t)) under a Blackman-Harris window of K periods (follows vibrato).
- resynth(...): the harmonic part from c_n(t); x - resynth = the residual (SMS, Serra & Smith
  1990): the noise between the harmonics.
- band_edges / band_powers: fractional-octave bands (IEC 61260 base-2 centres) by Welch.
"""
import numpy as np

BH = (0.35875, 0.48829, 0.14128, 0.01168)


def bh_window(n):
    k = np.arange(n) / max(n - 1, 1)
    a0, a1, a2, a3 = BH
    return a0 - a1 * np.cos(2 * np.pi * k) + a2 * np.cos(4 * np.pi * k) - a3 * np.cos(6 * np.pi * k)




def rms_db(x, sr, win_s=0.02, hop_s=0.005):
    """Frame RMS in dB (centres at hop multiples)."""
    w = max(int(win_s * sr), 8)
    h = max(int(hop_s * sr), 1)
    c = np.concatenate([[0.0], np.cumsum(x * x)])
    starts = np.arange(0, max(len(x) - w, 1), h)
    e = (c[starts + w] - c[starts]) / w
    t = (starts + w / 2) / sr
    return t, 10 * np.log10(e + 1e-20)


def segment(x, sr, thr_db=-32.0, min_gap=0.12, min_len=0.12, single=False):
    """Notes in a file as (start, end) sample indices. `single`: the file holds one note."""
    t, db = rms_db(x, sr, 0.02, 0.005)
    if len(db) < 4:
        return []
    floor = np.percentile(db, 5)
    peak = np.percentile(db, 99.5)
    if single:
        runs = [(int(np.argmax(db > max(peak + thr_db, floor + 6))), len(db) - 1)]
    else:
        act = db > max(peak + thr_db, floor + 10)
        runs, i, n = [], 0, len(act)
        while i < n:
            if act[i]:
                j = i
                while j + 1 < n and act[j + 1]:
                    j += 1
                runs.append([i, j])
                i = j + 1
            else:
                i += 1
        merged = []
        for r in runs:
            if merged and (t[r[0]] - t[merged[-1][1]]) < min_gap:
                merged[-1][1] = r[1]
            else:
                merged.append(list(r))
        runs = [r for r in merged if t[r[1]] - t[r[0]] >= min_len]
    out = []
    for a, b in runs:
        seg_peak = db[a:b + 1].max()
        lo = a
        while lo > 0 and db[lo - 1] > max(seg_peak - 45, floor + 4) and db[lo - 1] <= db[lo] + 1:
            lo -= 1
        hi = b
        while hi + 1 < len(db) and db[hi + 1] > max(seg_peak - 60, floor + 3):
            hi += 1
        s0 = max(int(t[lo] * sr) - int(0.01 * sr), 0)
        s1 = min(int(t[hi] * sr) + int(0.01 * sr), len(x))
        out.append((s0, s1))
    return out


def yin_track(x, sr, fmin, fmax, hop_s=0.0025, win_periods=None, f_hint=None, thr=0.15):
    """YIN f0 per frame. Returns (times s, f0 Hz, aperiodicity d'). With `f_hint` and
    `win_periods`, the integration window is that many periods of f_hint."""
    if win_periods and f_hint:
        W = int(np.ceil(win_periods * sr / f_hint))
    else:
        W = int(np.ceil(2 * sr / fmin))
    tmax = int(np.ceil(sr / fmin)) + 2
    tmin = max(int(np.floor(sr / fmax)) - 1, 2)
    hop = max(int(hop_s * sr), 1)
    L = W + tmax
    if len(x) < L + 2:
        return np.array([]), np.array([]), np.array([])
    starts = np.arange(0, len(x) - L, hop)
    nfft = 1 << int(np.ceil(np.log2(2 * L)))
    times, f0s, aps = [], [], []
    c2 = np.concatenate([[0.0], np.cumsum(x * x)])
    taus = np.arange(tmax + 1)
    for blk in range(0, len(starts), 256):
        st = starts[blk:blk + 256]
        idx = st[:, None] + np.arange(L)[None, :]
        fr = x[idx]
        head = np.zeros_like(fr)
        head[:, :W] = fr[:, :W]
        acf = np.fft.irfft(np.fft.rfft(fr, nfft) * np.conj(np.fft.rfft(head, nfft)), nfft)[:, :tmax + 1]
        e1 = (c2[st + W] - c2[st])[:, None]
        e2 = c2[st[:, None] + taus[None, :] + W] - c2[st[:, None] + taus[None, :]]
        d = e1 + e2 - 2 * acf
        d[:, 0] = 0
        cm = np.cumsum(d[:, 1:], axis=1)
        dp = np.ones_like(d)
        dp[:, 1:] = d[:, 1:] * np.arange(1, tmax + 1)[None, :] / np.maximum(cm, 1e-20)
        for k in range(len(st)):
            row = dp[k]
            seg = row[tmin:tmax]
            below = np.nonzero(seg < thr)[0]
            if len(below):
                i = below[0] + tmin
                while i + 1 < tmax and row[i + 1] < row[i]:
                    i += 1
            else:
                i = int(np.argmin(seg)) + tmin
            if 1 <= i < tmax:
                a, b, c = row[i - 1], row[i], row[i + 1]
                den = a - 2 * b + c
                off = 0.5 * (a - c) / den if abs(den) > 1e-12 else 0.0
                off = float(np.clip(off, -1, 1))
            else:
                off = 0.0
            times.append((st[k] + W / 2) / sr)
            f0s.append(sr / (i + off))
            aps.append(float(row[i]))
    return np.array(times), np.array(f0s), np.array(aps)


def median_filter(v, k):
    k = max(int(k) | 1, 1)
    if k == 1 or len(v) < k:
        return v.copy()
    p = k // 2
    vp = np.pad(v, p, mode="edge")
    sw = np.lib.stride_tricks.sliding_window_view(vp, k)
    return np.median(sw, axis=1)


def phase_of(times, f0, n, sr, t0=0.0):
    """Instantaneous phase (cycles) at samples 0..n-1 of a signal that starts at t0 s, from a
    track (times, f0) held constant beyond its ends."""
    ts = t0 + np.arange(n) / sr
    f = np.interp(ts, times, f0) if len(times) > 1 else np.full(n, f0[0] if len(f0) else 100.0)
    return np.concatenate([[0.0], np.cumsum(f[:-1]) / sr]), f


def project(x, phi, sr, f_ref, nh, periods=6.0, hop_s=None, window="bh"):
    """Complex harmonic amplitudes. Returns (centres in samples, c[nh, frames]).
    x ~ Re sum_n c_n e^{j 2 pi n phi}."""
    L = int(round(periods * sr / f_ref)) | 1
    hop = int(round(hop_s * sr)) if hop_s else max(int(round(sr / f_ref)), 1)
    if len(x) < L:
        return np.array([], dtype=int), np.zeros((nh, 0), complex)
    w = bh_window(L) if window == "bh" else np.hanning(L)
    g = 2.0 / w.sum()
    centres = np.arange(L // 2, len(x) - L // 2, hop)
    ns = np.arange(1, nh + 1)[:, None]
    out = np.zeros((nh, len(centres)), complex)
    for k, c in enumerate(centres):
        sl = slice(c - L // 2, c - L // 2 + L)
        ph = phi[sl]
        z = np.exp(-2j * np.pi * (ph - ph[L // 2]))
        E = np.cumprod(np.broadcast_to(z, (nh, L)), axis=0)
        out[:, k] = (E @ (w * x[sl])) * g * np.exp(-2j * np.pi * ns[:, 0] * ph[L // 2])
    return centres, out


def resynth(n, phi, centres, c):
    """The harmonic part at samples 0..n-1 from project()'s output (linear interpolation of
    each complex amplitude between frame centres; zero outside the first/last centre)."""
    h = np.zeros(n)
    if c.shape[1] < 2:
        return h
    s = np.arange(centres[0], centres[-1] + 1)
    ph = phi[s]
    for i in range(c.shape[0]):
        re = np.interp(s, centres, c[i].real)
        im = np.interp(s, centres, c[i].imag)
        h[s] += re * np.cos(2 * np.pi * (i + 1) * ph) - im * np.sin(2 * np.pi * (i + 1) * ph)
    return h


def refine_track(x, sr, times, f0, f_ref, iters=3, nref_max=8, periods=6.0):
    """Phase-locked refinement of an f0 track: project the low harmonics with the track's own
    phase, take the strongest one's phase drift per frame, correct the track; the last pass
    uses a 4-period Hann window so a vibrato's residual error is followed. Returns the new
    f0 at `times`."""
    f = f0.copy()
    n = len(x)
    for it in range(iters):
        last = it == iters - 1
        phi, _ = phase_of(times, f, n, sr)
        cen, c = project(x, phi, sr, f_ref, nref_max, periods=4.0 if last else periods,
                         window="hann" if last else "bh")
        if c.shape[1] < 4:
            break
        m = int(np.argmax(np.median(np.abs(c), axis=1)))
        th = np.unwrap(np.angle(c[m]))
        tc = cen / sr
        df = np.gradient(th, tc) / (2 * np.pi * (m + 1))
        df = median_filter(df, 3)
        f = f + np.interp(times, tc, df)
    return f


def band_edges(fc_lo=25.0, fc_hi=16000.0, per_oct=3):
    k0 = int(np.floor(per_oct * np.log2(fc_lo / 1000.0)))
    k1 = int(np.ceil(per_oct * np.log2(fc_hi / 1000.0)))
    fc = 1000.0 * 2.0 ** (np.arange(k0, k1 + 1) / per_oct)
    return fc, fc * 2 ** (-0.5 / per_oct), fc * 2 ** (0.5 / per_oct)


def band_powers(x, sr, per_oct=3, fc_lo=25.0, fc_hi=16000.0, nfft=None):
    """Welch power in fractional-octave bands (linear power per band)."""
    from scipy import signal
    nper = nfft or min(len(x), 1 << int(np.ceil(np.log2(sr * 0.1))))
    nper = max(min(nper, len(x)), 64)
    f, p = signal.welch(x, sr, window="hann", nperseg=nper, noverlap=nper // 2, scaling="density")
    df = f[1] - f[0]
    fc, lo, hi = band_edges(fc_lo, min(fc_hi, sr * 0.45), per_oct)
    out = np.array([p[(f >= a) & (f < b)].sum() * df for a, b in zip(lo, hi)])
    return fc, out
