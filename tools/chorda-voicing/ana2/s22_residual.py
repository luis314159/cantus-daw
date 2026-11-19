"""What the Salamander's late release tail (0.7-1.6 s after key-up) is made of: C4's partials
or noise? middle_c raw renders; energy on partials (+-1 %) vs total, per band."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana2'))
import numpy as np
from common import *
for s in ['sal', 'c5']:
    x = raw('middle_c', s)
    for a, b in [(3.7, 4.6), (2.0, 2.9)]:
        seg = x[int(a * SR):int(b * SR)]
        f, P = signal.periodogram(seg * signal.windows.hann(len(seg))[:, None], SR, axis=0); P = P.sum(1)
        on = np.zeros_like(f, bool)
        for n in range(1, 40):
            on |= np.abs(f - n * kfreq(60) * np.sqrt(1 + 2.9e-4 * n * n)) < max(0.01 * n * kfreq(60), 3)
        tot = P[f > 20].sum()
        print(s, f'{a}-{b}s total {db(tot * (f[1]-f[0])):.1f} dB, on-partials share {db(P[on].sum() / tot):.1f} dB, <200 Hz share {db(P[(f > 20) & (f < 200)].sum() / tot):.1f} dB')
