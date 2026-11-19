"""Per-key measures for the draw search on a keys_mf render: the flanger (flank.coh) and
partials 8/9 at 0 and 1.5 s against the reference (p89's measure, any key).
   python -I drawq.py <cand keys_mf.wav> <k1,k2,..>"""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '../..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('probes/c11')); sys.path.insert(0, tool_path('ana2'))
import numpy as np, soundfile as sf
from scipy import signal
import importlib
SR = 48000
REF = _os.path.join(LAB, 'keys/wav/salamander/keys_mf.wav')
def p89(x, key):
    f0 = 440 * 2 ** ((key - 69) / 12)
    y = x[(key - 21) * 4 * SR:((key - 21) * 4 + 3) * SR]
    on = int(np.argmax(np.abs(y).sum(1) > 0.05 * np.abs(y).sum(1).max())); y = y[on:]
    f, t, Z = signal.stft(y.T, SR, nperseg=4800, noverlap=4200); P = (np.abs(Z) ** 2).sum(0)
    tot = 10 * np.log10(P[(f > 50) & (f < 5000)].sum(0) + 1e-30).max()
    def env(n):
        g = n * f0 * (1 + 0.0004 * n * n * (key / 60) ** 2) ** 0.5; b = (f > g * 0.985) & (f < g * 1.025)
        return 10 * np.log10(P[b].max(0) + 1e-30) - tot
    return env(8)[1], env(8)[120], env(9)[1], env(9)[120]
sys.argv_saved = sys.argv; keys = [int(k) for k in sys.argv[2].split(',')]
X = sf.read(sys.argv[1], always_2d=True)[0]; R = sf.read(REF, always_2d=True)[0]
import flank
def dec(x, key):
    y = x[(key - 21) * 4 * SR:((key - 21) * 4 + 3) * SR].mean(1)
    on = int(np.argmax(np.abs(y) > 0.05 * np.abs(y).max())); y = y[on:]
    L = lambda a, b: 10 * np.log10(np.mean(y[int(a * SR):int(b * SR)] ** 2) + 1e-30)
    return L(0.1, 0.3) - L(0.9, 1.1), (L(2.6, 3.0) - L(1.5, 1.9)) / 1.1
for k in keys:
    c = flank.coh(X, k)
    a = np.array(p89(X, k)); b = np.array(p89(R, k))
    dc, dr = dec(X, k), dec(R, k)
    print(f'{k} drop1s {dc[0]:5.1f}/{dr[0]:5.1f} late {dc[1]:5.1f}/{dr[1]:5.1f}  ', end='')
    print(f'{k} flanger {c:5.2f}  p8 {a[0]-b[0]:+5.1f}/{a[1]-b[1]:+5.1f}  p9 {a[2]-b[2]:+5.1f}/{a[3]-b[3]:+5.1f}  (dB re ref at 0 / 1.5 s)', flush=True)
