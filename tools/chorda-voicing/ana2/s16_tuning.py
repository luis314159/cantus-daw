"""Tuning curve: each key's effective pitch (median cents of its strongest low partials against
equal temperament n*f_ET) and the inharmonicity B per key, mf."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana2'))
import numpy as np, csv, os
import matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt
from common import *
from partials import find_partials

res = {}
for src in ['c5', 'sal']:
    cents, Bs = [], []
    for key in KEYS:
        x = note(src, 'mf', key)
        on = onset(x, -30)
        m = x[on:].mean(1)
        f0, B, pl = find_partials(m, key, 0.3, 1.8)
        lo, hi = (2, 8) if key < 48 else (1, 4)
        c = [1200 * np.log2(fn / (n * kfreq(key))) for n, fn, lv in pl if lo <= n <= hi and lv > -50]
        cents.append(np.median(c) if c else np.nan)
        Bs.append(B if B > 0 else np.nan)
    res[src] = (np.array(cents), np.array(Bs))
K = np.array(KEYS)
fig, ax = plt.subplots(1, 2, figsize=(14, 4.5))
for s in ['c5', 'sal']:
    ax[0].plot(K, res[s][0], 'o-', color=COL[s], label=NAME[s])
    ax[1].semilogy(K, res[s][1], 'o-', color=COL[s], label=NAME[s])
ax[0].set_title('Effective pitch vs equal temperament (cents)\n(median of partials 2–8 below C3, 1–4 above)')
ax[1].set_title('Inharmonicity coefficient B')
for a in ax:
    a.grid(alpha=.3); a.legend(); a.set_xlabel('MIDI key')
fig.tight_layout(); fig.savefig(os.path.join(FIG, 'tuning_inharmonicity.png'), dpi=100); plt.close(fig)
d = res['c5'][0] - res['sal'][0]
for k, a, b in zip(K, res['c5'][0], res['sal'][0]):
    print(f'{kname(k):4s} c5 {a:+6.1f}  sal {b:+6.1f}  diff {a-b:+5.1f}')
print('rms diff all keys %.1f cents' % np.sqrt(np.nanmean(d ** 2)))
rB = np.log10(res['c5'][1] / res['sal'][1])
print('B ratio c5/sal median %.2f' % 10 ** np.nanmedian(rB))
