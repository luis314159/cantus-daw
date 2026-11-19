"""Same p2..p4 re p1 check on the earlier renders (beta.14, candidate 4) to date the change."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana2'))
import numpy as np, os
import common
from common import *
from partials import spectrum, interp_peak, find_partials
common.NOTES['b14'] = os.path.join(LAB, 'notes5', 'wav', 'b14')
common.NOTES['c4'] = os.path.join(LAB, 'notes5', 'wav', 'c4')
for src in ['b14', 'c4', 'c5', 'sal']:
    out = []
    for key in [69, 72, 75, 78, 81]:
        x = note(src, 'mf', key); on = onset(x, -30); y = x[on:]
        f0, B, pl = find_partials(y.mean(1), key, 0.25, 1.25)
        P = sum(spectrum(y[:, ch], 0.25, 1.25)[1] for ch in range(2)); f = spectrum(y[:, 0], 0.25, 1.25)[0]
        lv = [10 * np.log10(interp_peak(f, P, n * f0 * np.sqrt(1 + B * n * n) * 0.99, n * f0 * np.sqrt(1 + B * n * n) * 1.01)[1] + 1e-30) for n in (1, 2, 3)]
        out.append(f'{kname(key)} p2 {lv[1]-lv[0]:+5.1f} p3 {lv[2]-lv[0]:+5.1f}')
    print(src.ljust(4), ' | '.join(out))
