"""Sustain length per key: broadband (K-weighted) level at 0.25/1/2/3.3 s after the onset,
relative to the note's peak, at mf; plus the prompt/after-sound split (level drop in the
first 0.5 s vs the slope after 1.5 s)."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana2'))
import numpy as np, csv, os
import matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt
from common import *

dyn = sys.argv[1] if len(sys.argv) > 1 else 'mf'
T = [0.25, 1.0, 2.0, 3.3]
res = {s: [] for s in ['c5', 'sal']}
slopes = {s: [] for s in ['c5', 'sal']}
for s in ['c5', 'sal']:
    for key in KEYS:
        x = note(s, dyn, key)
        on = onset(x, -30)
        t, e = short_term_rms(kweight(x[on:on + int(3.45 * SR)]), 0.05, 0.01)
        pk = e.max()
        res[s].append([e[np.searchsorted(t, tt)] - pk for tt in T])
        m = (t > 1.5) & (t < 3.3)
        slopes[s].append(np.polyfit(t[m], e[m], 1)[0])
K = np.array(KEYS)
fig, ax = plt.subplots(1, 2, figsize=(14, 4.8))
for s in ['c5', 'sal']:
    R = np.array(res[s])
    for j, (tt, ls) in enumerate(zip(T, [':', '-.', '--', '-'])):
        ax[0].plot(K, R[:, j], ls, color=COL[s], marker='o', ms=2.5, label=f'{NAME[s]} at {tt} s')
    ax[1].plot(K, slopes[s], 'o-', color=COL[s], label=NAME[s])
ax[0].set_title(f'Single notes {dyn}: level re peak (K-weighted 50 ms)'); ax[0].legend(fontsize=6, ncol=2)
ax[1].set_title(f'After-sound slope 1.5–3.3 s (dB/s), {dyn}')
for a in ax:
    a.grid(alpha=.3); a.set_xlabel('MIDI key')
fig.tight_layout(); fig.savefig(os.path.join(FIG, f'sustain_{dyn}.png'), dpi=100); plt.close(fig)
for nm, a, b in [('A0-B1', 21, 35), ('C2-B2', 36, 47), ('C3-B3', 48, 59), ('C4-B4', 60, 71), ('C5-B5', 72, 83), ('C6-B6', 84, 95), ('C7-C8', 96, 108)]:
    m = (K >= a) & (K <= b)
    c = np.array(res['c5'])[m].mean(0); s_ = np.array(res['sal'])[m].mean(0)
    print(f'{nm}: c5 ' + ' '.join(f'{v:6.1f}' for v in c) + ' | sal ' + ' '.join(f'{v:6.1f}' for v in s_) +
          f' | late slope c5 {np.mean(np.array(slopes["c5"])[m]):6.1f} sal {np.mean(np.array(slopes["sal"])[m]):6.1f} dB/s')
