"""Partial excess (cand - ref, each key normalised to its partials' total) mapped to frequency,
mean over keys lo..hi in 1/3-octave bands.   python -I pfreq.py <cand keys_mf.wav> [lo hi]"""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '../..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys
import numpy as np, soundfile as sf
SR = 48000; N = 1 << 17
def prof(x, k):
    a = int(((k - 21) * 4 + 0.05) * SR); y = x[a:a + SR]
    P = sum(np.abs(np.fft.rfft(y[:, c] * np.hanning(len(y)), N)) ** 2 for c in range(2)); F = np.fft.rfftfreq(N, 1 / SR)
    f0 = 440 * 2 ** ((k - 69) / 12); out = []
    for n in range(1, 25):
        g = n * f0 * (1 + 0.0004 * n * n * (k / 60) ** 2) ** 0.5
        if g > 8000: break
        m = (F > g * 0.985) & (F < g * 1.03); out.append((g, P[m].max()))
    v = np.array([p for _, p in out]); return [(g, 10 * np.log10(p / v.sum())) for (g, p) in out]
ref = sf.read(_os.path.join(LAB, 'keys/wav/salamander/keys_mf.wav'), always_2d=True)[0]
x = sf.read(sys.argv[1], always_2d=True)[0]
lo, hi = (int(sys.argv[2]), int(sys.argv[3])) if len(sys.argv) > 3 else (48, 76)
bands = 250 * 2 ** (np.arange(0, 16) / 3); acc = {i: [] for i in range(len(bands))}
for k in range(lo, hi + 1):
    for (g, da), (_, db) in zip(prof(x, k), prof(ref, k)):
        acc[int(np.argmin(abs(np.log2(bands / g))))].append(da - db)
v = [np.mean(acc[i]) for i in range(len(bands)) if acc[i]]
print(' '.join('%d:%+.1f' % (bands[i], np.mean(acc[i])) for i in range(len(bands)) if acc[i]), ' rms %.2f' % np.sqrt(np.mean(np.square(v))))
