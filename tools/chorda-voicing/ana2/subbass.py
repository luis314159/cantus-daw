"""Sub-bass of single notes (mf, 0.25-1.25 s, L+R power): the first partial against the
second and third, peak-picked near the key's harmonics.  python -I subbass.py <notes dir>"""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana2'))
import numpy as np
import common
from common import *
common.NOTES['c5'] = sys.argv[1]
for key in (21, 24, 27, 30, 33, 36):
    out = []
    for src in ('c5', 'sal'):
        x = note(src, 'mf', key); on = onset(x, -30); y = x[on + int(0.25 * SR): on + int(1.25 * SR)]
        f = np.fft.rfftfreq(len(y), 1 / SR)
        P = sum(np.abs(np.fft.rfft(y[:, c] * np.hanning(len(y)))) ** 2 for c in range(2))
        f0 = kfreq(key)
        lv = [10 * np.log10(P[(f > n * f0 * 0.97) & (f < n * f0 * 1.03)].max() + 1e-30) for n in (1, 2, 3)]
        out.append(f'{lv[1]-lv[0]:+6.1f} {lv[2]-lv[0]:+6.1f}')
    print(kname(key), ' p2-p1 p3-p1  cand', out[0], ' sal', out[1])
