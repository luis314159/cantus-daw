"""Shared helpers for analysis-2 (independent listening analysis, Chorda candidate 5 vs the
Salamander Grand). Read-only on every input. Run every script with `python -I`.
"""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import os
import numpy as np
import soundfile as sf
from scipy import signal

ROOT = LAB  # the lab (round 40's voicing folder)
PREV = os.environ.get('ANA2_PREV', os.path.join(SHARE, 'chorda-preview-5'))
OUT = os.environ.get('ANA2_OUT', os.path.join(LAB, 'ana2', 'out'))
FIG = os.path.join(OUT, 'figures')
CSV = os.path.join(OUT, 'csv')
os.makedirs(FIG, exist_ok=True)
os.makedirs(CSV, exist_ok=True)

PHRASES = ['middle_c', 'soft_chords', 'fast_run', 'ballad', 'ff_chord', 'bass', 'top']
LABEL = {'b14': '1 Chorda 2 (beta.14)', 'c4': '2 candidato 4', 'c5': '3 candidato 5', 'sal': '4 Salamander'}
NAME = {'b14': 'beta.14', 'c4': 'cand 4', 'c5': 'Chorda c5', 'sal': 'Salamander'}
COL = {'b14': '#9a9a9a', 'c4': '#e3a33b', 'c5': '#2a6fdb', 'sal': '#c0392b'}
RAW = {'c5': os.environ.get('ANA2_RAW', os.path.join(LAB, 'lab4', 'wav', 'c5')),
       'sal': os.path.join(LAB, 'notes5', 'wav', 'salamander')}
NOTES = {'c5': os.environ.get('ANA2_NOTES', os.path.join(LAB, 'notes5', 'wav', 'c5')),
         'sal': os.path.join(LAB, 'notes5', 'wav', 'salamander')}
KEYS = list(range(21, 109, 3))
DYN = {'pp': 30, 'mf': 75, 'ff': 115}
EVERY, HELD = 6.0, 3.5
SR = 48000


def load(path):
    x, sr = sf.read(path, dtype='float64', always_2d=True)
    assert sr == SR
    if x.shape[1] == 1:
        x = np.repeat(x, 2, 1)
    return x


def phrase(ph, src):
    """Loudness-matched preview (−20 LUFS)."""
    return load(os.path.join(PREV, f'{ph} - {LABEL[src]}.flac'))


def raw(ph, src):
    """Un-normalised render (same engine, same gain staging for both)."""
    return load(os.path.join(RAW[src], ph + '.wav'))


_nc = {}


def note(src, dyn, key):
    p = os.path.join(NOTES[src], f'notes_{dyn}.wav')
    if p not in _nc:
        _nc[p] = load(p)
    x = _nc[p]
    i = KEYS.index(key)
    return x[int(i * EVERY * SR): int((i + 1) * EVERY * SR)]


def kfreq(key):
    return 440.0 * 2 ** ((key - 69) / 12)


NN = ['C', 'C#', 'D', 'D#', 'E', 'F', 'F#', 'G', 'G#', 'A', 'A#', 'B']


def kname(key):
    return f'{NN[key % 12]}{key // 12 - 1}'


def db(x, floor=1e-12):
    return 10 * np.log10(np.maximum(x, floor))


def db20(x, floor=1e-12):
    return 20 * np.log10(np.maximum(np.abs(x), floor))


# ---- BS.1770 ----
def kweight(x):
    b1 = [1.53512485958697, -2.69169618940638, 1.19839281085285]
    a1 = [1.0, -1.69065929318241, 0.73248077421585]
    b2 = [1.0, -2.0, 1.0]
    a2 = [1.0, -1.99004745483398, 0.99007225036621]
    return signal.lfilter(b2, a2, signal.lfilter(b1, a1, x, axis=0), axis=0)


def lufs(x):
    y = kweight(x)
    blk, hop = int(0.4 * SR), int(0.1 * SR)
    if len(y) < blk:
        return -70.0 + 0 * 1
    z = np.array([np.mean(y[i:i + blk] ** 2, 0).sum() for i in range(0, len(y) - blk + 1, hop)])
    l = -0.691 + 10 * np.log10(np.maximum(z, 1e-20))
    z1 = z[l > -70]
    if len(z1) == 0:
        return -70.0
    g = -0.691 + 10 * np.log10(z1.mean()) - 10
    z2 = z[l > g]
    return -0.691 + 10 * np.log10(z2.mean())


def short_term_rms(x, win=0.01, hop=0.002):
    """Frame RMS envelope (mono power sum of channels), dB; returns (t, env_db)."""
    m = (x ** 2).sum(1) if x.ndim == 2 else x ** 2
    w, h = int(win * SR), int(hop * SR)
    c = np.concatenate([[0], np.cumsum(m)])
    idx = np.arange(0, len(m) - w, h)
    e = (c[idx + w] - c[idx]) / w
    return (idx + w / 2) / SR, db(e)


def frac_oct_bands(n_per_oct, fmin=20, fmax=20000):
    k = np.arange(np.floor(n_per_oct * np.log2(fmin / 1000)), np.ceil(n_per_oct * np.log2(fmax / 1000)) + 1)
    fc = 1000 * 2 ** (k / n_per_oct)
    return fc


def ltas(x, nfft=16384):
    """Long-term power spectrum (Welch, summed channels), returns f, P (linear)."""
    m = x.mean(1) if x.ndim == 2 else x
    f, P = signal.welch(x, SR, nperseg=nfft, noverlap=nfft * 3 // 4, axis=0, window='hann')
    return f, P.sum(1) if P.ndim == 2 else P


def band_sum(f, P, fc, n_per_oct):
    lo, hi = fc * 2 ** (-0.5 / n_per_oct), fc * 2 ** (0.5 / n_per_oct)
    return np.array([P[(f >= a) & (f < b)].sum() for a, b in zip(lo, hi)])


def bandpass(x, lo, hi, order=4):
    sos = signal.butter(order, [lo, min(hi, SR / 2 * 0.98)], 'bandpass', fs=SR, output='sos')
    return signal.sosfiltfilt(sos, x, axis=0)


def onset(x, thr_db=-40):
    """First sample where |x| exceeds thr_db relative to the slot's peak."""
    a = np.abs(x).max(1) if x.ndim == 2 else np.abs(x)
    return int(np.argmax(a > a.max() * 10 ** (thr_db / 20)))
