"""Round 40, candidate 6: each key's partial voicing refitted on the second analysis's
per-channel partial levels (both channels' power summed — the A-B pair's mono comb left
out): partials 2..7 against the fundamental, mf and ff, candidate minus Salamander, every
measured key (A0 ... C8 every minor third); the table (in dB, applied at partials_amount 1)
moves by -gain x difference, smoothed 1-2-1 across measured keys and interpolated to every
key, clamped.

    python -I fit_partials2.py <notes dir of the try> <old table> <old amount> <new table> [gain] [clamp] [lo key] [hi key]
"""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana2'))
import os
import numpy as np
import common
from common import *
from partials import spectrum, interp_peak, find_partials

notes, old, amount, new = sys.argv[1], sys.argv[2], float(sys.argv[3]), sys.argv[4]
gain = float(sys.argv[5]) if len(sys.argv) > 5 else 0.6
clamp = float(sys.argv[6]) if len(sys.argv) > 6 else 18.0
lo = int(sys.argv[7]) if len(sys.argv) > 7 else 21
hi = int(sys.argv[8]) if len(sys.argv) > 8 else 96
common.NOTES['c5'] = notes


def levels(src, dyn, key):
    x = note(src, dyn, key); on = onset(x, -30); y = x[on:]
    f0, B, pl = find_partials(y.mean(1), key, 0.25, 1.25)
    P = 0
    for ch in range(2):
        f, Pc = spectrum(y[:, ch], 0.25, 1.25); P = P + Pc
    lv = []
    for n in range(1, 8):
        fn = n * f0 * np.sqrt(1 + B * n * n)
        if fn > 16000:
            lv.append(np.nan); continue
        fp, pp = interp_peak(f, P, fn * 0.99, fn * 1.01)
        lv.append(10 * np.log10(pp + 1e-30))
    lv = np.array(lv)
    return lv[1:] - lv[0]


ks = [k for k in KEYS if lo <= k <= hi]
D = []
for k in ks:
    ds = []
    for dyn in ('mf', 'ff'):
        c, s = levels('c5', dyn, k), levels('sal', dyn, k)
        d = c - s
        # A partial the reference hardly has (under -45 dB re its fundamental) is noise there.
        d[(s < -45) | ~np.isfinite(d)] = 0.0
        ds.append(np.clip(d, -25, 25))
    D.append(np.mean(ds, 0))
    print(kname(k), ' '.join(f'{v:+5.1f}' for v in D[-1]), flush=True)
D = np.array(D)
Ds = D.copy()
for i in range(len(ks)):
    a, b = max(0, i - 1), min(len(ks) - 1, i + 1)
    Ds[i] = (D[a] + 2 * D[i] + D[b]) / 4
T = np.zeros((88, 6))
for line in open(old):
    f = [float(x) for x in line.split(',')]
    T[int(f[0]) - 21] = np.array(f[1:]) * amount
K = np.arange(21, 109)
for j in range(6):
    corr = np.interp(K, ks, Ds[:, j])
    corr[(K < lo) | (K > hi)] = 0.0
    T[:, j] = np.clip(T[:, j] - gain * corr, -clamp, clamp)
with open(new, 'w') as fh:
    for i in range(88):
        fh.write(','.join([str(21 + i)] + [f'{g:.2f}' for g in T[i]]) + '\n')
print('written', new, 'range %.1f .. %.1f' % (T.min(), T.max()))
