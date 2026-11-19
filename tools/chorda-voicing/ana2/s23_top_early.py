"""top phrase staccato notes (0, 0.3, 0.6, 0.9 s): >2 kHz energy at 5-80 ms and 80-195 ms after
each onset relative to the first 5 ms after it, and side-minus-mid there (does the treble
sound distant and smeared?)."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana2'))
import numpy as np
from common import *
for s in ['c5', 'sal']:
    x = bandpass(phrase('top', s), 2000, 20000)
    r1, r2, sm = [], [], []
    for t0 in [0.0, 0.3, 0.6, 0.9]:
        seg = x[int(t0 * SR):int((t0 + 0.25) * SR)]
        a = np.abs(seg).max(1); i0 = int(np.argmax(a > a.max() * 0.1))
        e = (seg ** 2).sum(1)
        E = lambda a_, b_: e[i0 + int(a_ * SR / 1000): i0 + int(b_ * SR / 1000)].mean()
        r1.append(db(E(5, 80) / E(0, 5))); r2.append(db(E(80, 195) / E(0, 5)))
        w = seg[i0 + 240:i0 + 3840]
        sm.append(db(np.mean((w[:, 0] - w[:, 1]) ** 2) / np.mean((w[:, 0] + w[:, 1]) ** 2)))
    print(NAME[s].ljust(10), '5-80 ms %.1f dB, 80-195 ms %.1f dB re first 5 ms; S-M 5-80 ms %.1f dB' % (np.mean(r1), np.mean(r2), np.mean(sm)))
