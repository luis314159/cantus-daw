"""Every key's partials 2..4 re p1 (dB), window t0-t1, from keys_mf (key k at (k-21)*4 s).
   python -I pkeys.py <cand keys_mf.wav> lo hi [t0 t1]"""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '../..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys
sys.path.insert(0, tool_path('ana2'))
import os; os.environ.setdefault('ANA2_OUT', _os.path.join(LAB, 'c16/out/pt'))
from common import load, onset, SR, kname
from partials import spectrum, interp_peak, find_partials
import numpy as np
SAL = _os.path.join(LAB, 'keys/wav/salamander/keys_mf.wav')
lo, hi = int(sys.argv[2]), int(sys.argv[3])
t0, t1 = (float(sys.argv[4]), float(sys.argv[5])) if len(sys.argv) > 5 else (0.25, 1.25)
X = {'c': load(sys.argv[1]), 's': load(SAL)}
D = []
for k in range(lo, hi + 1):
    out = {}
    for s, x in X.items():
        y = x[int((k - 21) * 4 * SR): int((k - 20) * 4 * SR)]
        y = y[onset(y, -30):]
        f0, B, _ = find_partials(y.mean(1), k, 0.25, 1.25)
        P = 0
        for ch in range(2):
            f, Pc = spectrum(y[:, ch], t0, t1); P = P + Pc
        lv = []
        for n in range(1, 5):
            fn = n * f0 * np.sqrt(1 + B * n * n)
            _, pp = interp_peak(f, P, fn * 0.985, fn * 1.015)
            lv.append(10 * np.log10(pp + 1e-30))
        out[s] = np.array(lv) - lv[0]
    d = out['c'] - out['s']
    D.append(d[1:])
    print(f'{kname(k):4s} ' + ' '.join(f'{a:6.1f}/{b:<6.1f}' for a, b in zip(out['c'][1:], out['s'][1:])) + '   diff ' + ' '.join(f'{v:+5.1f}' for v in d[1:]))
D = np.array(D)
print('mean |diff| /5 (P10-like):', f'{np.mean(np.abs(D)) / 5:.2f}', ' mean diff', ' '.join(f'{v:+.1f}' for v in D.mean(0)))
