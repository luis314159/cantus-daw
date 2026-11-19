"""The beats of a chord's intervals: each pair's lowest coinciding partials (from tune.py's
measure of keys_mf), their frequency difference (Hz) for candidate and reference.
    python -I chordbeat.py <cand keys_mf> <ref keys_mf> k1,k2,...   """
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '../..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys, itertools
from fractions import Fraction
import numpy as np, soundfile as sf
sys.path.insert(0, tool_path('probes/c7'))
from tune import partials

c, _ = sf.read(sys.argv[1], always_2d=True); r, _ = sf.read(sys.argv[2], always_2d=True)
keys = [int(k) for k in sys.argv[3].split(',')]
P = {s: {k: partials(x, k) for k in keys} for s, x in (('c', c), ('r', r))}
RAT = {0: (1, 1), 3: (6, 5), 4: (5, 4), 5: (4, 3), 7: (3, 2), 8: (8, 5), 9: (5, 3), 12: (2, 1), 15: (12, 5),
       16: (5, 2), 19: (3, 1), 24: (4, 1), 17: (8, 3), 21: (10, 3), 22: (7, 2), 14: (9, 4), 10: (7, 4), 11: (15, 8), 2: (9, 8), 1: (16, 15)}
for a, b in itertools.combinations(sorted(keys), 2):
    iv = b - a
    if iv not in RAT:
        continue
    q, p = RAT[iv]  # partial q of the lower meets partial p of the upper
    if q > 6 or p > 6:
        continue
    row = []
    for s in ('c', 'r'):
        fa = q * 440 * 2 ** ((a - 69) / 12) * 2 ** (P[s][a][q - 1] / 1200)
        fb = p * 440 * 2 ** ((b - 69) / 12) * 2 ** (P[s][b][p - 1] / 1200)
        row.append(fa - fb)
    print(f'{a}-{b} ({iv:2d}) partials {q}:{p}  beat cand {row[0]:+6.2f} Hz  ref {row[1]:+6.2f} Hz')
