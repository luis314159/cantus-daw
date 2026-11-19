"""Sanity check of s21: C5 and F#5 mf, partial 1..3 level in L, R, mono sum, power sum."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana2'))
import numpy as np
from common import *
from partials import spectrum, interp_peak
for key in [72, 78]:
    for src in ['c5', 'sal']:
        x = note(src, 'mf', key); on = onset(x, -30); y = x[on:]
        f0 = kfreq(key)
        res = {}
        for nm, sig in [('L', y[:, 0]), ('R', y[:, 1]), ('mono', y.mean(1))]:
            f, P = spectrum(sig, 0.25, 1.25)
            res[nm] = [10 * np.log10(interp_peak(f, P, n * f0 * 0.97, n * f0 * 1.03)[1]) for n in (1, 2, 3)]
        print(kname(key), src, {k: [round(v - res['L'][0], 1) for v in vals] for k, vals in res.items()})
