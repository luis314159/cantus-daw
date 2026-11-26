"""fiddle_lib.py - the signal core of the fiddle study (Cantus, round 42, the country fiddle).

A library (nothing runs on import); `fiddle_measure.py` is the tool. Run with `python -I` from
the study venv (numpy, scipy, soundfile; no librosa). What is here:

- load(path): mono float64 (the channels' mean), sr. MP3 and Ogg through libsndfile >= 1.1.
- Track: per 2.5 ms frame the f0 (the study's YIN, `bowlib_core.yin_track`), its aperiodicity
  (YIN d' at the chosen lag: the share of the broadband energy that is not periodic), the
  level (RMS, 10 ms) and, in the 2-8 kHz band, the energy and its non-periodic share by a
  comb residual: the band-passed signal against itself one period later, the lag refined to
  1/4 sample around YIN's (`band_aperiodicity`). r = residual / (both copies' energy) is the
  noise's share of the band (pure noise -> 1, a strict period -> 0), so the band's noise is
  E + 10 log10 r and its harmonic part E + 10 log10 (1 - r). The window is 10 ms, so a
  length in ms is good to about +-5 ms.
- onsets(...): note starts from two cues merged within 30 ms: a spectral-flux peak (a bow
  change or a re-articulation) and a held change of the rounded semitone (a slur, a finger).
- dyads(...): two simultaneous pitches per 10 ms frame by a harmonic sieve on the spectral
  peaks (4096-point frames): the single f0 that explains most peak energy, then the best
  second f0 among the candidates by the energy it alone explains. Octaves and unisons cannot
  be told from one note (every partial of the upper note is a partial of the lower), so they
  are never reported; a fifth's virtual fundamental an octave under the lower note is ruled
  out by requiring each note's own first or second partial.
"""
import sys
from pathlib import Path

import numpy as np
import soundfile as sf
from scipy import signal

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "study"))
from bowlib_core import yin_track, median_filter  # noqa: E402

HOP = 0.0025
FMIN, FMAX = 180.0, 2600.0


def load(path, t0=None, t1=None):
    x, sr = sf.read(str(path), dtype="float64", always_2d=True)
    x = x.mean(axis=1)
    if t0 is not None or t1 is not None:
        a = int((t0 or 0) * sr)
        b = int(t1 * sr) if t1 else len(x)
        x = x[a:b]
    return np.ascontiguousarray(x), int(sr)


def frame_rms_db(x, sr, times, win_s=0.010):
    w = max(int(win_s * sr), 8)
    c = np.concatenate([[0.0], np.cumsum(x * x)])
    s = np.clip((times * sr).astype(int) - w // 2, 0, len(x) - w - 1)
    return 10 * np.log10((c[s + w] - c[s]) / w + 1e-20)


def band_aperiodicity(y, sr, times, f0, win_s=0.010, up=4, span=0.015):
    """Per frame: (band energy dB, noise share r) of `y` (already band-passed) by the comb
    residual at the period 1/f0 refined within +-span."""
    yu = signal.resample_poly(y, up, 1)
    sru = sr * up
    W = int(win_s * sru)
    c2 = np.concatenate([[0.0], np.cumsum(yu * yu)])
    E = np.full(len(times), -200.0)
    R = np.ones(len(times))
    for k, (t, f) in enumerate(zip(times, f0)):
        s = int(t * sru) - W // 2
        if not (f > 0) or s < 0:
            continue
        T = sru / f
        lags = np.arange(int(T * (1 - span)), int(T * (1 + span)) + 2)
        if s + W + lags[-1] >= len(yu):
            continue
        a = yu[s:s + W]
        idx = s + lags[:, None] + np.arange(W)[None, :]
        b = yu[idx]
        cross = b @ a
        ea = c2[s + W] - c2[s]
        eb = c2[s + lags + W] - c2[s + lags]
        d = ea + eb - 2 * cross
        i = int(np.argmin(d))
        if 0 < i < len(d) - 1:
            den = d[i - 1] - 2 * d[i] + d[i + 1]
            dmin = d[i] - 0.125 * (d[i - 1] - d[i + 1]) ** 2 / den if den > 1e-30 else d[i]
        else:
            dmin = d[i]
        E[k] = 10 * np.log10(ea / W + 1e-20)
        R[k] = float(np.clip(max(dmin, 0.0) / max(ea + eb[i], 1e-30), 1e-4, 1.0))
    return E, R


class Track:
    """The per-frame track of one file (2.5 ms hop)."""

    def __init__(self, x, sr, band=(2000.0, 8000.0)):
        self.x, self.sr = x, sr
        t, f, ap = yin_track(x, sr, FMIN, FMAX, hop_s=HOP, thr=0.15)
        self.t, self.f_raw, self.ap = t, f, ap
        self.lvl = frame_rms_db(x, sr, t)
        self.peak = float(np.percentile(self.lvl, 99))
        self.active = self.lvl > self.peak - 35
        self.voiced = self.active & (ap < 0.35) & (f > FMIN) & (f < FMAX)
        m = 69 + 12 * np.log2(np.maximum(f, 1.0) / 440.0)
        self.midi = median_filter(m, 5)
        # the file's tuning (a 78 runs off speed): the circular mean of the voiced cents
        ph = 2 * np.pi * (self.midi[self.voiced] % 1.0)
        self.tuning = float(np.angle(np.mean(np.exp(1j * ph))) / (2 * np.pi)) if len(ph) else 0.0
        sos = signal.butter(6, [band[0], min(band[1], 0.45 * sr)], "bandpass", fs=sr, output="sos")
        y = signal.sosfiltfilt(sos, x)
        # the comb needs a period in every sounding frame: an unvoiced one (an onset, a
        # scrape) takes the nearest voiced f0 within 60 ms, and comes out noisy as it should
        fb = np.where(self.voiced, f, np.nan)
        idx = np.flatnonzero(np.isfinite(fb))
        if len(idx):
            near = idx[np.clip(np.searchsorted(idx, np.arange(len(fb))), 0, len(idx) - 1)]
            near2 = idx[np.clip(np.searchsorted(idx, np.arange(len(fb))) - 1, 0, len(idx) - 1)]
            pick = np.where(np.abs(near - np.arange(len(fb))) <= np.abs(near2 - np.arange(len(fb))), near, near2)
            ok = self.active & (np.abs(pick - np.arange(len(fb))) * HOP <= 0.06)
            fb = np.where(ok, f[pick], 0.0)
        else:
            fb = np.zeros(len(f))
        self.band_ok = fb > 0
        self.bE, self.bR = band_aperiodicity(y, sr, t, fb)
        r = np.clip(self.bR, 1e-4, 0.9999)
        self.bN = self.bE + 10 * np.log10(r)          # band noise, dB
        self.bH = self.bE + 10 * np.log10(1 - r)      # band harmonic part, dB
        self.bNHR = self.bN - self.bH

    def at(self, tt):
        return int(np.clip(np.searchsorted(self.t, tt), 0, len(self.t) - 1))


def stft_mag(x, sr, n, hop_s):
    h = max(int(hop_s * sr), 1)
    w = signal.get_window("hann", n)
    nfr = max((len(x) - n) // h, 0)
    out = np.zeros((nfr, n // 2 + 1), dtype=np.float32)
    for b in range(0, nfr, 512):
        idx = (np.arange(b, min(b + 512, nfr)) * h)[:, None] + np.arange(n)[None, :]
        out[b:b + idx.shape[0]] = np.abs(np.fft.rfft(x[idx] * w, axis=1))
    t = (np.arange(nfr) * h + n / 2) / sr
    return t, out


def band_flux(x, sr, lag=4):
    """Spectral flux on ~1/4-octave bands (150 Hz - 10 kHz) of log energy, differenced over
    `lag` frames (10 ms): bands wide enough that vibrato moving a partial inside its band
    does not count as an onset."""
    t, S = stft_mag(x, sr, 1024, HOP)
    freqs = np.fft.rfftfreq(1024, 1 / sr)
    edges = 150.0 * 2 ** (np.arange(0, 26) / 4.0)
    edges = edges[edges < min(10000.0, 0.45 * sr)]
    B = np.stack([(S[:, (freqs >= a) & (freqs < b)].astype(np.float64) ** 2).sum(axis=1)
                  for a, b in zip(edges[:-1], edges[1:])], axis=1)
    L = 10 * np.log10(B + B.max() * 1e-7)
    d = np.zeros_like(L)
    d[lag:] = L[lag:] - L[:-lag]
    flux = np.maximum(d, 0).mean(axis=1)
    return t, flux


def onsets(tr, min_gap=0.05, delta=3.5):
    """Note starts (s): band-flux peaks (dB per band, `delta` over the 150 ms median) and held
    semitone changes, merged within 30 ms."""
    x, sr = tr.x, tr.sr
    t, flux = band_flux(x, sr)
    k = int(0.15 / HOP) | 1
    thr = signal.medfilt(flux, k) + delta
    lv = np.interp(t, tr.t, tr.lvl)
    pk, _ = signal.find_peaks(flux, height=thr, distance=int(min_gap / HOP))
    fo = [float(t[i]) - 0.005 for i in pk if lv[i] > tr.peak - 30]
    # held semitone changes on the voiced track
    semi = np.where(tr.voiced, np.round(tr.midi - tr.tuning), np.nan)
    hold = int(0.03 / HOP)
    po, last = [], np.nan
    for i in range(len(semi) - hold):
        s = semi[i]
        if np.isnan(s):
            continue
        if not np.isnan(last) and s != last and np.all(semi[i:i + hold] == s):
            po.append(float(tr.t[i]) - 0.01)
            last = s
        elif np.isnan(last):
            last = s
    allo = sorted([(v, 0) for v in fo] + [(v, 1) for v in po])
    out = []
    for v, src in allo:
        if out and v - out[-1][0] < 0.03:
            if src == 0 and out[-1][1] == 1:
                out[-1] = (v, 0)
            continue
        out.append((v, src))
    # a voiced start after silence is an onset too
    on = np.flatnonzero(np.diff(tr.active.astype(int)) == 1)
    for i in on:
        v = float(tr.t[i + 1])
        if not any(abs(v - o[0]) < 0.04 for o in out):
            out.append((v, 2))
    out.sort()
    return np.array([o[0] for o in out]), np.array([o[1] for o in out])


# ---------------------------------------------------------------- two pitches at once

GRID = 440.0 * 2 ** ((np.arange(-38 * 10, 30 * 10) / 10.0) / 12.0)   # 10-cent grid
GRID = GRID[(GRID >= 185.0) & (GRID <= 1600.0)]


def frame_peaks(mag, freqs, fmax=5000.0, floor_db=-45.0):
    """Spectral peaks (Hz, linear magnitude) above floor_db re the frame's maximum."""
    top = mag.max()
    if top <= 0:
        return np.array([]), np.array([])
    lim = int(np.searchsorted(freqs, fmax))
    m = mag[:lim]
    pk, _ = signal.find_peaks(m, height=top * 10 ** (floor_db / 20))
    pk = pk[(pk > 0) & (pk < lim - 1)]
    if not len(pk):
        return np.array([]), np.array([])
    a, b, c = np.log(m[pk - 1] + 1e-12), np.log(m[pk] + 1e-12), np.log(m[pk + 1] + 1e-12)
    den = a - 2 * b + c
    off = np.where(np.abs(den) > 1e-9, 0.5 * (a - c) / den, 0.0)
    df = freqs[1] - freqs[0]
    return freqs[pk] + np.clip(off, -0.5, 0.5) * df, m[pk]


def sieve(pf, pa, f0s, tol_c=30.0, nh=12):
    """(len(f0s), len(pf)) boolean: peak j is a partial of candidate i."""
    h = np.arange(1, nh + 1)
    hf = f0s[:, None, None] * h[None, :, None]
    c = 1200 * np.abs(np.log2(pf[None, None, :] / hf))
    return (c < tol_c).any(axis=1), (c[:, :2, :] < tol_c).any(axis=(1, 2))


def dyad_frame(pf, pa, min_unique=0.12, min_gain=0.10, keep=30):
    """One frame: ((f_lo, f_hi, share explained by both) or None, f_single, its share)."""
    if len(pf) < 3:
        return None, 0.0, 0.0
    if len(pf) > keep:
        top = np.sort(np.argsort(-pa)[:keep])
        pf, pa = pf[top], pa[top]
    w = pa ** 1.0
    W = w.sum()
    M, low = sieve(pf, pa, GRID)
    expl = (M * w[None, :]).sum(axis=1) / W
    expl = np.where(low, expl, 0.0)
    i1 = int(np.argmax(expl))
    e1 = expl[i1]
    # the strongest candidates, at least 60 cents from the first and not its octaves
    order = np.argsort(-expl)[:60]
    best = None
    for i2 in order:
        if not low[i2] or expl[i2] < 0.05:
            continue
        r = GRID[i2] / GRID[i1]
        cents = 1200 * np.log2(r)
        if abs(cents) < 60:
            continue
        oc = abs(cents) / 1200.0
        if abs(oc - round(oc)) * 1200 < 40:
            continue
        both = (M[i1] | M[i2])
        uniq2 = ((M[i2] & ~M[i1]) * w).sum() / W
        uniq1 = ((M[i1] & ~M[i2]) * w).sum() / W
        e2 = (both * w).sum() / W
        if uniq2 < min_unique or uniq1 < min_unique * 0.5 or e2 - e1 < min_gain:
            continue
        if best is None or e2 > best[2]:
            best = (min(GRID[i1], GRID[i2]), max(GRID[i1], GRID[i2]), e2, e1)
    if best is not None:
        best = best + (refine(pf, pa, best[0], best[1]), refine(pf, pa, best[1], best[0]))
    return best, float(GRID[i1]), float(e1)


def refine(pf, pa, f, other, tol_c=30.0):
    """f0 from the peaks that are partials of f and not of `other` (weighted by magnitude),
    to about a cent: each partial's frequency over its number."""
    h = np.round(pf / f)
    ok = (h >= 1) & (h <= 12) & (1200 * np.abs(np.log2(pf / (np.maximum(h, 1) * f))) < tol_c)
    ho = np.round(pf / other)
    ok &= ~((ho >= 1) & (1200 * np.abs(np.log2(pf / (np.maximum(ho, 1) * other))) < tol_c))
    if not ok.any():
        return f
    return float(np.sum(pa[ok] * pf[ok] / h[ok]) / np.sum(pa[ok]))


def dyads(x, sr, hop_s=0.010, n=4096, active=None):
    """Per frame (t, f_lo or 0, f_hi or 0, f_single, e1, e2, f_lo refined, f_hi refined)."""
    t, S = stft_mag(x, sr, n, hop_s)
    freqs = np.fft.rfftfreq(n, 1 / sr)
    rms = 20 * np.log10(S.sum(axis=1) + 1e-12)
    peak = np.percentile(rms, 99)
    out = np.zeros((len(t), 8))
    out[:, 0] = t
    for k in range(len(t)):
        if rms[k] < peak - 30:
            continue
        pf, pa = frame_peaks(S[k], freqs)
        if len(pf) < 3:
            continue
        d, f1, e1 = dyad_frame(pf, pa)
        out[k, 3], out[k, 4] = f1, e1
        if d:
            out[k, 1], out[k, 2], out[k, 5] = d[0], d[1], d[2]
            out[k, 6], out[k, 7] = d[4], d[5]
    return out


def voice_env(S, freqs, f_a, f_b, nh=10, tol_c=35.0):
    """Per frame the magnitude sum of the partials of f_a that are not partials of f_b."""
    env = np.zeros(len(S))
    for k in range(len(S)):
        fa, fb = f_a[k], f_b[k]
        if not (fa > 0 and fb > 0):
            continue
        hs = fa * np.arange(1, nh + 1)
        hs = hs[hs < 5000]
        other = fb * np.arange(1, 2 * nh + 1)
        keep = [h for h in hs if np.min(1200 * np.abs(np.log2(other / h))) > 2 * tol_c]
        tot = 0.0
        for h in keep:
            lo = int(np.searchsorted(freqs, h * 2 ** (-tol_c / 1200)))
            hi = int(np.searchsorted(freqs, h * 2 ** (tol_c / 1200))) + 1
            tot += float(S[k, lo:hi].max()) if hi > lo else 0.0
        env[k] = tot
    return env
