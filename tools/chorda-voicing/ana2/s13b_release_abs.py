"""Release noise below the note (20 Hz .. 0.6 f0) in absolute terms: its peak 5-ms level in the
150 ms after key-up relative to the note's whole level just before key-up; and its duration
above -20 dB of that peak. mf, C4..D#6."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana2'))
import numpy as np
from common import *
print('key  | peak re note (dB) | duration ms   (c5 / sal)')
for key in [48, 54, 60, 66, 72, 78, 84, 87]:
    o = []
    for src in ['c5', 'sal']:
        x = note(src, 'mf', key)
        on = onset(x, -30); ku = on + int(HELD * SR)
        seg = x[ku - int(0.3 * SR):ku + int(0.5 * SR)]
        t, etot = short_term_rms(seg, 0.005, 0.001)
        pre = etot[(t > 0.2) & (t < 0.29)].mean()
        y = bandpass(seg, 20, 0.6 * kfreq(key))
        t, e = short_term_rms(y, 0.005, 0.001)
        post = (t > 0.3) & (t < 0.45)
        pk = e[post].max()
        dur = ((e[post] > pk - 20).sum()) * 1.0
        o.append((round(pk - pre, 1), dur))
    print(kname(key), o)
