"""Partial-table variant: start from a base table (default candidate 16's; round 40 started
from candidate 15's), put rows lo..hi of a source table (round 40: candidate 6b's) scaled by s
into chosen columns (1 = partial 2 ... 6 = partial 7).
   python -I mkpart.py out.csv lo hi cols scale <src table> [base table]"""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '../..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys
out, lo, hi, cols, s = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), [int(c) for c in sys.argv[4].split(',')], float(sys.argv[5])
src = sys.argv[6]
base = [l.strip().split(',') for l in open(sys.argv[7] if len(sys.argv) > 7 else _os.path.join(C16, 'partials.csv')) if l.strip()]
sr = {int(r[0]): r for r in (l.strip().split(',') for l in open(src) if l.strip())}
for r in base:
    k = int(r[0])
    if lo <= k <= hi:
        for c in cols:
            r[c] = f'{float(sr[k][c]) * s:.2f}'
open(out, 'w').write('\n'.join(','.join(r) for r in base) + '\n')
