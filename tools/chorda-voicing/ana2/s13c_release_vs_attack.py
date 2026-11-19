"""Release noise peak (5-ms level, whole band) after key-up relative to the note's own attack
peak (5-ms level), mf — how loud the key-up event is next to the key-down event."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana2'))
import numpy as np
from common import *
for key in [60, 66, 72, 78, 84, 87]:
    o = []
    for src in ['c5', 'sal']:
        x = note(src, 'mf', key)
        on = onset(x, -30); ku = on + int(HELD * SR)
        t, e = short_term_rms(x[on:], 0.005, 0.001)
        att = e[t < 0.2].max()
        y = bandpass(x[on:], 20, 0.6 * kfreq(key))
        t2, e2 = short_term_rms(y, 0.005, 0.001)
        rel = e2[(t2 > HELD) & (t2 < HELD + 0.15)].max()
        o.append(round(rel - att, 1))
    print(kname(key), 'release noise under the note re attack peak: c5 %.1f dB, sal %.1f dB' % tuple(o))
