"""top phrase: level of the string tone (>1.5 kHz, so the knock and the reference's rumble
are out) in the gaps after the staccato notes and after the held ones, re the phrase peak."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana2'))
import numpy as np
from common import *
pts = [0.25, 0.55, 0.85, 1.2, 2.0, 2.7, 3.4, 4.1, 4.35, 6.2, 6.6, 7.0, 7.4]
for s in ['c5', 'sal']:
    x = bandpass(phrase('top', s), 1500, 20000)
    t, e = short_term_rms(x, 0.05, 0.01)
    pk = e.max()
    print(NAME[s].ljust(10), ' '.join(f'{p}s:{e[np.searchsorted(t, p)] - pk:6.1f}' for p in pts))
    xl = bandpass(phrase('top', s), 20, 400)
    t, e2 = short_term_rms(xl, 0.05, 0.01)
    print('   <400 Hz', ' '.join(f'{p}s:{e2[np.searchsorted(t, p)] - pk:6.1f}' for p in pts))
