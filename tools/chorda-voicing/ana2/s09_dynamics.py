"""Velocity -> level and brightness, and key-to-key evenness, from csv/notes.csv (raw renders:
both pianos through the same engine; absolute offsets between pianos do not matter, the
spacing between dynamics and between neighbouring keys does)."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana2'))
import numpy as np, csv, os
import matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt
from common import *

R = list(csv.DictReader(open(os.path.join(CSV, 'notes.csv'))))
g = lambda src, dyn, col: np.array([float(r[col]) for r in R if r['src'] == src and r['dyn'] == dyn])
K = np.array(KEYS)
fig, ax = plt.subplots(2, 2, figsize=(14, 9))
# align each piano's mf curve to the same mean over A0..C6 so key evenness is comparable
off = {s: np.mean(g(s, 'mf', 'level_1s')[:22]) for s in ['c5', 'sal']}
for s in ['c5', 'sal']:
    for dyn, ls in [('pp', ':'), ('mf', '-'), ('ff', '--')]:
        ax[0, 0].plot(K, g(s, dyn, 'level_1s') - off[s], ls, marker='o', ms=3, color=COL[s], label=f'{NAME[s]} {dyn}')
    ax[0, 1].plot(K, g(s, 'mf', 'level_1s') - g(s, 'pp', 'level_1s'), 'o-', color=COL[s], label=f'{NAME[s]} mf − pp')
    ax[0, 1].plot(K, g(s, 'ff', 'level_1s') - g(s, 'mf', 'level_1s'), 's--', color=COL[s], label=f'{NAME[s]} ff − mf')
    ax[1, 0].plot(K, g(s, 'ff', 'cent_0_50') / g(s, 'pp', 'cent_0_50'), 'o-', color=COL[s], label=f'{NAME[s]} attack centroid ff/pp')
    ax[1, 0].plot(K, g(s, 'ff', 'cent_300_1000') / g(s, 'pp', 'cent_300_1000'), 's--', color=COL[s], label=f'{NAME[s]} sustain centroid ff/pp')
    ax[1, 1].plot(K, g(s, 'mf', 'cent_300_1000'), 'o-', color=COL[s], label=f'{NAME[s]} mf sustain centroid')
ax[0, 0].set_title('Level (K-weighted, first second), each piano aligned on its mf mean A0–C6')
ax[0, 1].set_title('Steps between dynamics (dB)')
ax[1, 0].set_title('Brightness growth pp → ff (centroid ratio)')
ax[1, 1].set_title('Sustain brightness at mf: spectral centroid 0.3–1.0 s (Hz)'); ax[1, 1].set_yscale('log')
for a in ax.ravel():
    a.grid(alpha=.3); a.legend(fontsize=7); a.set_xlabel('MIDI key')
fig.tight_layout(); fig.savefig(os.path.join(FIG, 'dynamics_evenness.png'), dpi=100); plt.close(fig)

for s in ['c5', 'sal']:
    for dyn in ['pp', 'mf', 'ff']:
        L = g(s, dyn, 'level_1s') - off[s]
        # evenness: deviation from a 5-point running quadratic trend
        dev = []
        for i in range(len(L)):
            lo, hi = max(0, i - 3), min(len(L), i + 4)
            idx = [j for j in range(lo, hi) if j != i]
            p = np.polyfit(K[idx], L[idx], 2)
            dev.append(L[i] - np.polyval(p, K[i]))
        dev = np.array(dev)
        print(f'{NAME[s]:10s} {dyn}: key-to-key deviation rms {np.sqrt(np.mean(dev[:26]**2)):.1f} dB (A0..A7), worst',
              ', '.join(f'{kname(K[i])} {dev[i]:+.1f}' for i in np.argsort(-np.abs(dev))[:4]))
    mp = g(s, 'mf', 'level_1s') - g(s, 'pp', 'level_1s'); fm = g(s, 'ff', 'level_1s') - g(s, 'mf', 'level_1s')
    for nm, a, b in [('A0-B2', 21, 47), ('C3-B4', 48, 71), ('C5-B6', 72, 95), ('C7-C8', 96, 108)]:
        m = (K >= a) & (K <= b)
        print(f'   {NAME[s]:10s} {nm}: mf-pp {mp[m].mean():.1f} dB, ff-mf {fm[m].mean():.1f} dB, ff-pp {(mp+fm)[m].mean():.1f}')
