"""Onset shape per note: lead-in (time from -50 dB to -20 dB of the note's peak), time to the
peak of the 1-ms envelope, and the first-cycles spike (peak of the first 4 ms after -20 dB vs
the peak of the following 10-30 ms)."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana'))
import numpy as np, csv, os
from common import *
rows = []
for src in ('sal', 'c2'):
    for dyn in DYN:
        for key in KEYS:
            x, sr = note(src, dyn, key); m = np.abs(x.mean(1))[:int(0.5 * sr)]
            pk = m.max()
            i50 = np.argmax(m > pk * 10 ** (-50 / 20)); i20 = np.argmax(m > pk * 10 ** (-20 / 20))
            e = np.sqrt(np.convolve(x.mean(1)[:int(0.5 * sr)] ** 2, np.ones(48) / 48, 'same'))
            ip = np.argmax(e)
            a = m[i20:i20 + int(0.004 * sr)].max(); b = m[i20 + int(0.010 * sr):i20 + int(0.030 * sr)].max()
            rows.append(dict(src=src, dyn=dyn, key=key, lead_in_ms=round((i20 - i50) / sr * 1000, 2),
                             t_peak_ms=round((ip - i20) / sr * 1000, 1), spike_db=round(20 * np.log10(a / b), 2)))
with open(os.path.join(CSV, 'onset_shape.csv'), 'w', newline='') as fh:
    w = csv.DictWriter(fh, fieldnames=list(rows[0].keys())); w.writeheader(); w.writerows(rows)
for dyn in DYN:
    for rn, (lo, hi) in {'bass': (21, 47), 'middle': (48, 83), 'treble': (84, 108)}.items():
        s = []
        for src in ('sal', 'c2'):
            d = [q for q in rows if q['src'] == src and q['dyn'] == dyn and lo <= q['key'] <= hi]
            s.append(f"{src}: lead-in {np.median([q['lead_in_ms'] for q in d]):.1f} ms, t_peak {np.median([q['t_peak_ms'] for q in d]):.1f} ms, spike {np.median([q['spike_db'] for q in d]):+.1f} dB")
        print(dyn, rn, ' | '.join(s))
