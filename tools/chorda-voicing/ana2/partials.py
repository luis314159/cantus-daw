"""Partial tracking for single notes: peak-picked inharmonic series, B fit, heterodyne
envelopes per partial. Shared by the per-note scripts."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import numpy as np
from scipy import signal
from common import SR, kfreq


def spectrum(m, t0, t1, zp=4):
    seg = m[int(t0 * SR):int(t1 * SR)]
    n = len(seg)
    w = signal.windows.hann(n)
    N = int(2 ** np.ceil(np.log2(n * zp)))
    F = np.fft.rfft(seg * w, N)
    f = np.fft.rfftfreq(N, 1 / SR)
    return f, np.abs(F) ** 2


def interp_peak(f, P, lo, hi):
    i0, i1 = np.searchsorted(f, lo), np.searchsorted(f, hi)
    if i1 - i0 < 3:
        return None, None
    i = i0 + int(np.argmax(P[i0:i1]))
    if i <= 0 or i >= len(P) - 1:
        return f[i], P[i]
    a, b, c = np.log(P[i - 1] + 1e-30), np.log(P[i] + 1e-30), np.log(P[i + 1] + 1e-30)
    d = 0.5 * (a - c) / (a - 2 * b + c) if (a - 2 * b + c) != 0 else 0
    return f[i] + d * (f[1] - f[0]), P[i]


def find_partials(m, key, t0=0.25, t1=1.25, fmax=16000, nmax=60):
    """Returns (f0_fit, B, list of (n, f_n, power_db)). m: mono slot from the onset."""
    f, P = spectrum(m, t0, t1)
    fn0 = kfreq(key)
    Pmax = P[(f > fn0 * 0.5) & (f < fmax)].max()
    found = []
    f0, B = fn0, 0.0
    n = 1
    while n <= nmax:
        pred = n * f0 * np.sqrt(1 + B * n * n)
        if pred > fmax:
            break
        tol = max(0.012 * pred, 1.5) if n < 4 else max(0.006 * pred, 1.5)
        tol = min(tol, 0.35 * fn0)
        fp, pp = interp_peak(f, P, pred - tol, pred + tol)
        if fp is not None and pp > Pmax * 1e-8:
            # require local prominence over the neighbourhood between partials
            nb = (f > pred - 0.5 * fn0) & (f < pred + 0.5 * fn0)
            if pp > 30 * np.median(P[nb]):
                found.append((n, fp, 10 * np.log10(pp / Pmax)))
        if len(found) >= 4 and n >= 4:
            nn = np.array([a for a, _, _ in found], float)
            ff = np.array([b for _, b, _ in found])
            # (f/n)^2 = f0^2 + f0^2 B n^2
            A = np.vstack([np.ones_like(nn), nn ** 2]).T
            w = np.ones_like(nn)
            c, *_ = np.linalg.lstsq(A * w[:, None], (ff / nn) ** 2 * w, rcond=None)
            if c[0] > 0:
                f0 = np.sqrt(c[0]); B = max(c[1] / c[0], 0.0)
        n += 1
    return f0, B, found


def hetero_env(m, fc, bw, hop=0.005):
    """Envelope (dB) of the component at fc: complex demodulation + zero-phase low-pass of
    cut-off bw (Hz). Returns t (s), env_db."""
    t = np.arange(len(m)) / SR
    z = m * np.exp(-2j * np.pi * fc * t)
    sos = signal.butter(4, bw, 'low', fs=SR, output='sos')
    zr = signal.sosfiltfilt(sos, z.real)
    zi = signal.sosfiltfilt(sos, z.imag)
    a = 2 * np.sqrt(zr ** 2 + zi ** 2)
    h = int(hop * SR)
    return t[::h], 20 * np.log10(a[::h] + 1e-12)


def slope(t, e, a, b):
    m = (t >= a) & (t <= b)
    if m.sum() < 5:
        return np.nan
    return np.polyfit(t[m], e[m], 1)[0]
