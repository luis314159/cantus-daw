"""Shared helpers for the Chorda-vs-Salamander analysis (round 40). Read-only on every input."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import os
import numpy as np
import soundfile as sf
from scipy import signal

V = LAB
PREV = os.environ.get('ANA_PREV', os.path.join(SHARE, 'chorda-preview-2'))
OUT = os.environ.get('ANA_OUT', os.path.join(LAB, 'analysis'))
PLOTS = os.path.join(OUT, 'plots')
CSV = os.path.join(OUT, 'csv')
os.makedirs(PLOTS, exist_ok=True)
os.makedirs(CSV, exist_ok=True)

PHRASES = ['ff_chord', 'bass', 'middle_c', 'ballad', 'top', 'soft_chords', 'fast_run']
LABELS = {1: '1 Chorda 2 (beta.14)', 2: '2 candidato 1', 3: '3 candidato 2', 4: '4 Salamander'}
SHORT = {1: 'beta.14', 2: 'cand1', 3: 'cand2', 4: 'Salamander'}
COL = {1: '#9a9a9a', 2: '#e3a33b', 3: '#2a6fdb', 4: '#c0392b'}

# raw (not loudness-normalised) renders of the same phrases
RAW = {1: os.path.join(V, 'eng40', 'wav', 'chorda2'), 2: os.path.join(V, 'lab4', 'wav', os.environ.get('ANA_SLOT2', 'candB')),
       3: os.environ.get('ANA_RAW3', os.path.join(V, 'lab4', 'wav', 'c2')), 4: os.path.join(V, 'eng40', 'wav', 'salamander')}
# single notes A0..C8 every minor third, one every 6 s, held 3.5 s, at pp 30 / mf 75 / ff 115
NOTES = {'sal': os.path.join(V, 'notes5', 'wav', 'salamander'),
         'c2': os.environ.get('ANA_NOTES3', os.path.join(V, 'notes5', 'wav', 'c2')),
         'b14': os.path.join(V, 'notes5', 'wav', 'b14')}
KEYS = list(range(21, 109, 3))
DYN = {'pp': 30, 'mf': 75, 'ff': 115}
EVERY, HELD = 6.0, 3.5


def load(path):
    x, sr = sf.read(path, dtype='float64', always_2d=True)
    if x.shape[1] == 1:
        x = np.repeat(x, 2, 1)
    return x, sr


def phrase(ph, k):
    return load(os.path.join(PREV, f'{ph} - {LABELS[k]}.flac'))


def raw_phrase(ph, k):
    return load(os.path.join(RAW[k], ph + '.wav'))


_notes_cache = {}


def note(src, dyn, key):
    """One single note's 6-s slot (stereo)."""
    p = os.path.join(NOTES[src], f'notes_{dyn}.wav')
    if p not in _notes_cache:
        _notes_cache[p] = load(p)
    x, sr = _notes_cache[p]
    i = KEYS.index(key)
    return x[int(i * EVERY * sr): int((i + 1) * EVERY * sr)], sr


def kfreq(key):
    return 440.0 * 2 ** ((key - 69) / 12)


NAMES = ['C', 'C#', 'D', 'D#', 'E', 'F', 'F#', 'G', 'G#', 'A', 'A#', 'B']


def kname(key):
    return f'{NAMES[key % 12]}{key // 12 - 1}'


# ---- BS.1770 loudness ----
def _kw(sr):
    # pre-filter (high shelf) + RLB high-pass, coefficients for 48 kHz
    assert sr == 48000
    b1 = [1.53512485958697, -2.69169618940638, 1.19839281085285]
    a1 = [1.0, -1.69065929318241, 0.73248077421585]
    b2 = [1.0, -2.0, 1.0]
    a2 = [1.0, -1.99004745483398, 0.99007225036621]
    return (b1, a1), (b2, a2)


def kweight(x, sr):
    (b1, a1), (b2, a2) = _kw(sr)
    return signal.lfilter(b2, a2, signal.lfilter(b1, a1, x, axis=0), axis=0)


def lufs(x, sr=48000):
    y = kweight(x, sr)
    n, h = int(0.4 * sr), int(0.1 * sr)
    if len(y) < n:
        return -70.0
    ms = np.array([(y[i:i + n] ** 2).mean(0).sum() for i in range(0, len(y) - n + 1, h)])
    l = -0.691 + 10 * np.log10(ms + 1e-30)
    g = ms[l > -70]
    if len(g) == 0:
        return -70.0
    rel = -0.691 + 10 * np.log10(g.mean()) - 10
    g2 = ms[(l > -70) & (l > rel)]
    return -0.691 + 10 * np.log10(g2.mean())


def short_term(x, sr=48000, win=0.4, hop=0.05):
    y = kweight(x, sr)
    n, h = int(win * sr), int(hop * sr)
    t, l = [], []
    for i in range(0, max(1, len(y) - n + 1), h):
        t.append((i + n / 2) / sr)
        l.append(-0.691 + 10 * np.log10((y[i:i + n] ** 2).mean(0).sum() + 1e-30))
    return np.array(t), np.array(l)


# ---- spectra ----
THIRDS = 1000 * 2 ** (np.arange(-17, 14) / 3)  # 20 Hz .. 20 kHz


def psd(x, sr, nper=16384):
    f, p = signal.welch(x, sr, nperseg=min(nper, len(x)), noverlap=min(nper, len(x)) * 3 // 4, axis=0)
    return f, p


def bands(f, p, centres=THIRDS, frac=3):
    """Band powers (dB) of a PSD; p may be 1-D or 2-D (freq x ch)."""
    out = []
    for c in centres:
        lo, hi = c * 2 ** (-0.5 / frac), c * 2 ** (0.5 / frac)
        m = (f >= lo) & (f < hi)
        out.append(p[m].sum(0) if m.any() else np.zeros(p.shape[1:]))
    return 10 * np.log10(np.array(out) + 1e-30)


def smooth_oct(f, pdb, frac=12):
    """Fractional-octave smoothing of a dB spectrum (power-domain)."""
    pw = 10 ** (pdb / 10)
    out = np.empty_like(pw)
    c = np.cumsum(pw)
    for i, fi in enumerate(f):
        if fi <= 0:
            out[i] = pw[i]
            continue
        lo, hi = np.searchsorted(f, fi * 2 ** (-0.5 / frac)), np.searchsorted(f, fi * 2 ** (0.5 / frac))
        hi = max(hi, lo + 1)
        out[i] = (c[hi - 1] - (c[lo - 1] if lo > 0 else 0)) / (hi - lo)
    return 10 * np.log10(out + 1e-30)


def onset(x, sr, thr_db=-50):
    e = np.abs(x).max(1)
    pk = e.max()
    i = np.argmax(e > pk * 10 ** (thr_db / 20))
    return i


def db(v):
    return 10 * np.log10(np.maximum(v, 1e-30))


def tilt(centres, levels, lo=100, hi=10000):
    m = (centres >= lo) & (centres <= hi)
    a = np.polyfit(np.log2(centres[m]), levels[m], 1)
    return a[0]
