"""p1..p4 absolute levels (dB, both channels summed) over time windows for A4..A5, cand vs ref.
   python -I ptime.py <cand notes dir> [dyn]"""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '../..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys, os
sys.path.insert(0, tool_path('ana2'))
os.environ.setdefault('ANA2_OUT', _os.path.join(LAB, 'c16/out/pt'))
import common
common.NOTES['c5'] = sys.argv[1]
from common import note, onset, kname
from partials import spectrum, interp_peak, find_partials
import numpy as np
dyn = sys.argv[2] if len(sys.argv) > 2 else 'mf'
keys = [int(k) for k in (sys.argv[3] if len(sys.argv) > 3 else '69,72,75,78,81').split(',')]
W = [(0.0, 0.1), (0.1, 0.25), (0.25, 0.6), (0.6, 1.25), (1.25, 2.5)]
for key in keys:
    for src in ['c5', 'sal']:
        x = note(src, dyn, key); on = onset(x, -30); y = x[on:]
        f0, B, _ = find_partials(y.mean(1), key, 0.25, 1.25)
        rows = []
        for t0, t1 in W:
            P = 0
            for ch in range(2):
                f, Pc = spectrum(y[:, ch], t0, t1); P = P + Pc / (t1 - t0) ** 2
            lv = []
            for n in range(1, 5):
                fn = n * f0 * np.sqrt(1 + B * n * n)
                _, pp = interp_peak(f, P, fn * 0.985, fn * 1.015)
                lv.append(10 * np.log10(pp + 1e-30))
            rows.append(lv)
        r = np.array(rows)
        ref = r[0, 0]
        print(f'{kname(key):4s} {src:4s} ' + ' | '.join(' '.join(f'{v - ref:6.1f}' for v in row) for row in r))
