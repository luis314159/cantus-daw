"""Where the 4-6 kHz excess sits: single notes, L+R power, band 4-6.3 kHz against the note's
own 200 Hz-2 kHz energy, in the attack (0-50 ms), the bloom (50-300 ms) and the sustain
(0.3-1.5 s), candidate minus Salamander, by octave.  python -I hf2.py <notes dir> <dyn>"""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana2'))
import numpy as np
import common
from common import *
common.NOTES['c5'] = sys.argv[1]
dyn = sys.argv[2] if len(sys.argv) > 2 else 'mf'
W = [(0.0, 0.05), (0.05, 0.3), (0.3, 1.5)]
def band(y, lo, hi):
    f = np.fft.rfftfreq(len(y), 1 / SR)
    P = sum(np.abs(np.fft.rfft(y[:, c] * np.hanning(len(y)))) ** 2 for c in range(2))
    return P[(f >= lo) & (f < hi)].sum()
rows = {}
for k in KEYS:
    for src in ('c5', 'sal'):
        x = note(src, dyn, k); on = onset(x, -30)
        r = []
        for a, b in W:
            y = x[on + int(a * SR): on + int(b * SR)]
            ref = band(y, 200, 2000) + band(y, 2000, 4000)
            r.append(10 * np.log10(band(y, 4000, 6300) / ref + 1e-30))
        rows[(k, src)] = r
oct_ = [(21, 47, 'A0-B2'), (48, 59, 'C3-B3'), (60, 71, 'C4-B4'), (72, 83, 'C5-B5'), (84, 95, 'C6-B6'), (96, 108, 'C7-C8')]
print(dyn, 'band 4-6.3 kHz re 0.2-4 kHz, cand - sal (dB): attack 0-50 ms | bloom 50-300 ms | sustain 0.3-1.5 s')
for a, b, nm in oct_:
    ks = [k for k in KEYS if a <= k <= b]
    d = np.array([np.array(rows[(k, 'c5')]) - np.array(rows[(k, 'sal')]) for k in ks])
    print(f'{nm:6s} ' + ' | '.join(f'{v:+6.1f}' for v in d.mean(0)))
