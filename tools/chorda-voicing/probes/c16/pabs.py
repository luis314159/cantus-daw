"""Absolute partial levels cand - ref (dB), p1..p6, window t0-t1, plus the note's broadband level diff.
   python -I pabs.py <cand notes dir> dyn t0 t1 [keys]"""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '../..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys, os
sys.path.insert(0, tool_path('ana2'))
os.environ.setdefault('ANA2_OUT', _os.path.join(LAB, 'c16/out/pt'))
import common
common.NOTES['c5'] = sys.argv[1]
from common import note, onset, kname, SR
from partials import spectrum, interp_peak, find_partials
import numpy as np
dyn, t0, t1 = sys.argv[2], float(sys.argv[3]), float(sys.argv[4])
keys = [int(k) for k in (sys.argv[5] if len(sys.argv) > 5 else '60,63,66,69,72,75,78,81,84').split(',')]
acc = []
for key in keys:
    L = {}
    for src in ['c5', 'sal']:
        x = note(src, dyn, key); on = onset(x, -30); y = x[on:]
        f0, B, _ = find_partials(y.mean(1), key, 0.25, 1.25)
        P = 0
        for ch in range(2):
            f, Pc = spectrum(y[:, ch], t0, t1); P = P + Pc
        lv = []
        for n in range(1, 7):
            fn = n * f0 * np.sqrt(1 + B * n * n)
            _, pp = interp_peak(f, P, fn * 0.985, fn * 1.015)
            lv.append(10 * np.log10(pp + 1e-30))
        tot = 10 * np.log10(np.sum(y[int(t0 * SR):int(t1 * SR)] ** 2))
        L[src] = (np.array(lv), tot)
    d = L['c5'][0] - L['sal'][0]
    acc.append(d)
    print(f'{kname(key):4s} total {L["c5"][1] - L["sal"][1]:+5.1f}  partials ' + ' '.join(f'{v:+6.1f}' for v in d))
