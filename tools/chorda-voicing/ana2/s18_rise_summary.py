"""Register summary of the attack rise (-20 dB -> -3 dB of the 1-ms envelope peak, ms) and of
the attack's 2-10 kHz share, per dynamic, from csv/notes.csv."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana2'))
import numpy as np, csv, os
from common import *
R = list(csv.DictReader(open(os.path.join(CSV, 'notes.csv'))))
REG = [('A0-B1', 21, 35), ('C2-B2', 36, 47), ('C3-B3', 48, 59), ('C4-B4', 60, 71), ('C5-B5', 72, 83), ('C6-B6', 84, 95), ('C7-C8', 96, 108)]
for col in ['rise_20_3_ms', 'att_2k10k_frac_db', 'cent_0_50']:
    print('==', col)
    for dyn in ['pp', 'mf', 'ff']:
        line = f'{dyn}: '
        for nm, a, b in REG:
            v = {s: np.median([float(r[col]) for r in R if r['src'] == s and r['dyn'] == dyn and a <= int(r['key']) <= b]) for s in ['c5', 'sal']}
            line += f'{nm} {v["c5"]:.1f}/{v["sal"]:.1f}  '
        print(line)
