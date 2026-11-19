"""Octave-band long-term spectrum differences (source - Salamander) per phrase, from csv/phrase_bands.csv."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana'))
import numpy as np, csv, os
from common import *
B = list(csv.DictReader(open(os.path.join(CSV, 'phrase_bands.csv'))))
oct_c = [31.5, 63, 125, 250, 500, 1000, 2000, 4000, 8000, 16000]
rows = []
for ph in PHRASES:
    lv = {}
    for s in ('beta.14', 'cand2', 'Salamander'):
        d = [r for r in B if r['phrase'] == ph and r['source'] == s]
        f = np.array([float(r['band_hz']) for r in d]); v = 10 ** (np.array([float(r['mid_db']) for r in d]) / 10)
        lv[s] = np.array([10 * np.log10(v[(f >= c / 1.42) & (f < c * 1.42)].sum()) for c in oct_c])
    for s in ('beta.14', 'cand2'):
        r = dict(phrase=ph, source=s); r.update({f'{c:g}': round(x, 1) for c, x in zip(oct_c, lv[s] - lv['Salamander'])}); rows.append(r)
with open(os.path.join(CSV, 'phrase_octave_diff.csv'), 'w', newline='') as fh:
    w = csv.DictWriter(fh, fieldnames=list(rows[0].keys())); w.writeheader(); w.writerows(rows)
print('phrase      source   ' + ' '.join(f'{c:>6g}' for c in oct_c))
for r in rows:
    print(f"{r['phrase']:11s} {r['source']:8s} " + ' '.join(f"{r[f'{c:g}']:>6}" for c in oct_c))
