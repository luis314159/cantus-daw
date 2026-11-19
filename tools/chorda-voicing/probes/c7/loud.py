"""Each phrase's loudness (BS.1770) of a render against the reference; the mean gap.
    python -I loud.py <dir> [<ref dir>]"""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '../..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys, os
sys.path.insert(0, tool_path(''))
import numpy as np, soundfile as sf
from heard import lufs
d = sys.argv[1]; r = sys.argv[2] if len(sys.argv) > 2 else _os.path.join(LAB, 'eng40/wav/salamander')
g = []
for ph in ['middle_c', 'soft_chords', 'fast_run', 'ballad', 'ff_chord', 'bass', 'top', 'triad']:
    a = lufs(sf.read(os.path.join(d, ph + '.wav'), always_2d=True)[0]); b = lufs(sf.read(os.path.join(r, ph + '.wav'), always_2d=True)[0])
    g.append(a - b); print(f'{ph:12s} {a:6.1f} {b:6.1f} {a-b:+5.1f}')
print(f'mean gap {np.mean(g):+.2f} dB')
