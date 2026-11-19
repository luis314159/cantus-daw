"""Partial n's envelope (dB re its peak) every 0.1 s, cand vs ref.  python -I penv.py <cand notes dir> dyn key n [n2..]"""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '../..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys, os
sys.path.insert(0, tool_path('ana2'))
os.environ.setdefault('ANA2_OUT', _os.path.join(LAB, 'c16/out/pt'))
import common
common.NOTES['c5'] = sys.argv[1]
from common import note, onset, kname, SR
from partials import find_partials
import numpy as np
from scipy import signal
dyn, key = sys.argv[2], int(sys.argv[3])
ns = [int(a) for a in sys.argv[4:]] or [1]
for src in ['c5', 'sal']:
    x = note(src, dyn, key); on = onset(x, -30); y = x[on:]
    f0, B, _ = find_partials(y.mean(1), key, 0.25, 1.25)
    for n in ns:
        fn = n * f0 * np.sqrt(1 + B * n * n)
        t = np.arange(len(y)) / SR
        e = 0
        sos = signal.butter(4, min(0.3 * f0, 60) / (SR / 2), output='sos')
        for ch in range(2):
            z = y[:, ch] * np.exp(-2j * np.pi * fn * t)
            z = signal.sosfiltfilt(sos, z.real) + 1j * signal.sosfiltfilt(sos, z.imag)
            e = e + np.abs(z) ** 2
        d = 10 * np.log10(e + 1e-30)
        pts = [d[int(s * SR)] for s in np.arange(0.02, 3.0, 0.1)]
        m = max(pts)
        print(f'{kname(key)} {src:4s} p{n} ' + ' '.join(f'{v - m:5.0f}' for v in pts))
