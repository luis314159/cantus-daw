"""Beat depth per key with both channels' power summed (no mono-sum artefacts): median over
the 8 strongest partials of the 3-97 % range of the detrended dB envelope, 0.15-3.3 s, mf."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana2'))
import numpy as np, csv, os
from common import *
from partials import find_partials, hetero_env
rows = []
for key in KEYS[:24]:
    r = [kname(key)]
    for src in ['c5', 'sal']:
        x = note(src, 'mf', key); on = onset(x, -30); y = x[on:on + int(3.4 * SR)]
        f0, B, pl = find_partials(y.mean(1), key, 0.25, 1.25)
        pl = sorted(pl, key=lambda p: -p[2])[:8]
        d = []
        for n, fn, lv in pl:
            ev = sum(10 ** (hetero_env(y[:, ch], fn, min(0.3 * f0, 25), 0.01)[1] / 10) for ch in range(2))
            t = np.arange(len(ev)) * 0.01
            ev = 10 * np.log10(ev + 1e-30); m = (t > 0.15) & (t < 3.3)
            res = ev[m] - np.polyval(np.polyfit(t[m], ev[m], 2), t[m])
            d.append(np.percentile(res, 97) - np.percentile(res, 3))
        r.append(round(float(np.median(d)), 1))
    rows.append(r); print(r, flush=True)
with open(os.path.join(CSV, 'beat_depth_powersum_mf.csv'), 'w', newline='') as fh:
    w = csv.writer(fh); w.writerow(['key', 'c5_db', 'sal_db']); w.writerows(rows)
