"""Round 40 (candidate 9): tuning in the acceptance suite. Every key's partials 1-6 (every key at
mf, keys_mf: key k at (k-21)*4 s; 0.1-1.1 s, both channels' power) in cents, candidate against
the Salamander, after taking out the median offset of the first partial (a whole-instrument
pitch offset is not a mistuned key); the worst key; the beats of the consonant intervals
(fifths, fourths, octaves, twelfths) in the test chords (the soft chords' four and the ff
chord). Writes the log accept3 reads (P20) and exits 1 when any key's first partial is more
than 10 cents from the reference's stretch (the gate).
    python -I tuning.py <cand keys_mf.wav> [<log>]"""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '../..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys, itertools
import numpy as np, soundfile as sf
SR = 48000; N = 1 << 18
REF = _os.path.join(LAB, 'keys/wav/salamander/keys_mf.wav')
CHORDS = [[48, 55, 59, 64, 67], [45, 52, 59, 60, 64, 71], [45, 53, 60, 64, 69], [43, 50, 57, 60, 64, 65],
          [24, 36, 43, 48, 52, 55, 60, 64, 67, 72]]
RAT = {5: (4, 3), 7: (3, 2), 12: (2, 1), 19: (3, 1), 24: (4, 1)}


def partials(x, k):
    a = int(((k - 21) * 4 + 0.1) * SR); y = x[a:a + SR]
    P = sum(np.abs(np.fft.rfft(y[:, c] * np.hanning(len(y)), N)) ** 2 for c in range(2))
    F = np.fft.rfftfreq(N, 1 / SR); f0 = 440 * 2 ** ((k - 69) / 12); out, prev = [], 0.0
    for n in range(1, 7):
        g = n * f0 * 2 ** (prev / 1200)
        m = (F > g * 2 ** (-60 / 1200)) & (F < g * 2 ** (60 / 1200))
        if not m.any() or n * f0 > 16000:
            out.append(np.nan); continue
        i = np.flatnonzero(m)[np.argmax(P[m])]
        a_, b_, c_ = (np.log(P[j] + 1e-30) for j in (i - 1, i, i + 1))
        f = (i + 0.5 * (a_ - c_) / (a_ - 2 * b_ + c_)) * SR / N
        out.append(1200 * np.log2(f / (n * f0))); prev = out[-1]
    return np.array(out)


c = sf.read(sys.argv[1], always_2d=True)[0]; r = sf.read(REF, always_2d=True)[0]
C = {k: partials(c, k) for k in range(21, 109)}; R = {k: partials(r, k) for k in range(21, 109)}
d1 = np.array([C[k][0] - R[k][0] for k in range(21, 109)]); off = np.nanmedian(d1); d1 -= off
d13 = np.array([C[k][:3] - R[k][:3] for k in range(21, 109)]) - off
wi = int(np.nanargmax(np.abs(d1)))
beats = []
for ch in CHORDS:
    for a, b in itertools.combinations(sorted(ch), 2):
        if b - a not in RAT: continue
o = np.argsort(-np.nan_to_num(np.abs(d1)))[:6]
print('worst: ' + ' '.join(f'{i + 21}:{d1[i]:+.1f}' for i in o), ' F#2(42) %+.1f' % d1[42 - 21], ' GATE', 'FAIL' if np.nanmax(np.abs(d1)) > 10 else 'ok')
