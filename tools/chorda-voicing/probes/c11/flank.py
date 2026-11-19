"""The flanger test (s07_sweep) on every key from keys_mf (key k at (k-21)*4 s): partials 4..16
envelopes aligned on t*n, mean pairwise correlation. Prints keys whose candidate reads > 0.2.
   python -I flank.py <cand keys_mf.wav> [ref keys_mf.wav] [lo hi]"""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '../..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana2'))
import numpy as np
from common import load, onset, SR
from partials import find_partials, hetero_env
NS = list(range(4, 17)); tau = np.linspace(0.15 * NS[-1], 3.3 * NS[0], 600)
def coh(x, key):
    y = x[int((key - 21) * 4 * SR):int((key - 21) * 4 * SR + 3.9 * SR)]
    on = onset(y, -30); y = y[on:on + int(3.4 * SR)]
    f0, B, pl = find_partials(y.mean(1), key, 0.25, 1.25)
    d = {n: fn for n, fn, lv in pl}; W = []
    for n in NS:
        if n not in d: continue
        evs = []
        for ch in range(2):
            t, ev = hetero_env(y[:, ch], d[n], min(0.3 * f0, 25), hop=0.005); evs.append(10 ** (ev / 10))
        ev = 10 * np.log10(evs[0] + evs[1] + 1e-30); mk = (t > 0.15) & (t < 3.3)
        res = ev[mk] - np.polyval(np.polyfit(t[mk], ev[mk], 2), t[mk]); W.append(np.interp(tau, t[mk] * n, res))
    if len(W) < 5: return np.nan
    C = np.corrcoef(np.array(W)); iu = np.triu_indices(len(W), 1); return float(np.nanmean(C[iu]))
if __name__ == '__main__':
    lo, hi = (int(sys.argv[3]), int(sys.argv[4])) if len(sys.argv) > 4 else (21, 72)
    X = load(sys.argv[1]); R = load(sys.argv[2]) if len(sys.argv) > 2 and sys.argv[2] != '-' else None
    NM = ['C','C#','D','D#','E','F','F#','G','G#','A','A#','B']
    out = []
    for k in range(lo, hi + 1):
        c = coh(X, k); r = coh(R, k) if R is not None else np.nan
        out.append((k, c, r))
        print(f'{NM[k%12]}{k//12-1:<2} {k:3d} cand {c:5.2f} ref {r:5.2f}' + ('  <-- ' if c > 0.2 else ''), flush=True)
    print('outliers (>0.2):', ' '.join(f'{k}:{c:.2f}' for k, c, r in out if c > 0.2))
