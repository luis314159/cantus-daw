"""Upper-middle colour: level of partials 2..8 relative to the fundamental for A4..A5, both
channels' power summed (no mono comb), 0.25-1.25 s; mf and ff."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana2'))
import numpy as np
from common import *
from partials import spectrum, interp_peak, find_partials
for dyn in ['mf', 'ff']:
    print('==', dyn, ' partial n: level re p1 (dB)  c5 / sal')
    acc = {'c5': [], 'sal': []}
    for key in [int(k) for k in (sys.argv[1].split(",") if len(sys.argv) > 1 else "69,72,75,78,81".split(","))]:
        line = f'{kname(key):4s}'
        for src in ['c5', 'sal']:
            x = note(src, dyn, key); on = onset(x, -30); y = x[on:]
            f0, B, pl = find_partials(y.mean(1), key, 0.25, 1.25)
            P = 0
            for ch in range(2):
                f, Pc = spectrum(y[:, ch], 0.25, 1.25); P = P + Pc
            lv = []
            for n in range(1, 9):
                fn = n * f0 * np.sqrt(1 + B * n * n)
                fp, pp = interp_peak(f, P, fn * 0.99, fn * 1.01)
                lv.append(10 * np.log10(pp + 1e-30))
            lv = np.array(lv) - lv[0]
            acc[src].append(lv)
        line += ' ' + ' '.join(f'{a:6.1f}/{b:<6.1f}' for a, b in zip(acc['c5'][-1][1:], acc['sal'][-1][1:]))
        print(line)
    d = np.array(acc['c5']) - np.array(acc['sal'])
    print('mean c5 - sal per partial n=2..8:', ' '.join(f'{v:+.1f}' for v in d.mean(0)[1:]))
