"""Per-partial summaries from csv/partials.csv: decay (early/late), beat depth, beat
coherence across partials, partial balance. Figures + printed tables."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana2'))
import numpy as np, csv, os
import matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt
from common import *

P = list(csv.DictReader(open(os.path.join(CSV, 'partials.csv'))))
for p in P:
    for k in ['key', 'n']:
        p[k] = int(p[k])
    for k in ['f', 'rel_db', 'dec_early', 'dec_mid', 'dec_late', 'lvl_3s', 'beat_depth', 'beat_hz']:
        try:
            p[k] = float(p[k])
        except Exception:
            p[k] = np.nan


def sel(src, dyn, key, nmax=99, top=None):
    q = [p for p in P if p['src'] == src and p['dyn'] == dyn and p['key'] == key and p['n'] <= nmax]
    if top:
        q = sorted(q, key=lambda p: -p['rel_db'])[:top]
    return q


dyn = sys.argv[1] if len(sys.argv) > 1 else 'mf'
print(f'--- {dyn}: per key (c5 / sal) ---')
print('key name | dec_early p1 | dec_late p1 | lvl3s p1 | med beat depth top8 | med dec_early top8 | med dec_late top8 | beat/n CV top8 | p2-p1 dB | p3-p1')
tab = []
for key in KEYS:
    line = f'{key:3d} {kname(key):4s}'
    vals = {}
    for src in ['c5', 'sal']:
        q = sel(src, dyn, key, top=8)
        p1 = [p for p in sel(src, dyn, key) if p['n'] == 1]
        p2 = [p for p in sel(src, dyn, key) if p['n'] == 2]
        p3 = [p for p in sel(src, dyn, key) if p['n'] == 3]
        v = dict(
            de1=p1[0]['dec_early'] if p1 else np.nan,
            dl1=p1[0]['dec_late'] if p1 else np.nan,
            l31=p1[0]['lvl_3s'] if p1 else np.nan,
            bd=np.nanmedian([p['beat_depth'] for p in q]) if q else np.nan,
            de=np.nanmedian([p['dec_early'] for p in q]) if q else np.nan,
            dl=np.nanmedian([p['dec_late'] for p in q]) if q else np.nan,
            r21=(p2[0]['rel_db'] - p1[0]['rel_db']) if (p1 and p2) else np.nan,
            r31=(p3[0]['rel_db'] - p1[0]['rel_db']) if (p1 and p3) else np.nan,
        )
        bn = np.array([p['beat_hz'] / p['n'] for p in q if p['beat_depth'] > 3])
        v['cv'] = np.std(bn) / np.mean(bn) if len(bn) >= 3 else np.nan
        vals[src] = v
    tab.append((key, vals))
    f = lambda k: f"{vals['c5'][k]:6.1f}/{vals['sal'][k]:<6.1f}"
    line += ' | ' + ' | '.join(f(k) for k in ['de1', 'dl1', 'l31', 'bd', 'de', 'dl', 'cv', 'r21', 'r31'])
    print(line)

K = np.array(KEYS)
fig, ax = plt.subplots(2, 2, figsize=(13, 8))
for src in ['c5', 'sal']:
    ax[0, 0].plot(K, [v[src]['de'] for _, v in tab], 'o-', color=COL[src], label=NAME[src])
    ax[0, 1].plot(K, [v[src]['dl'] for _, v in tab], 'o-', color=COL[src], label=NAME[src])
    ax[1, 0].plot(K, [v[src]['bd'] for _, v in tab], 'o-', color=COL[src], label=NAME[src])
    ax[1, 1].plot(K, [v[src]['l31'] for _, v in tab], 'o-', color=COL[src], label=NAME[src])
ax[0, 0].set_title('Early decay 0.05–0.5 s, median of 8 strongest partials (dB/s)')
ax[0, 1].set_title('Late decay 1.5–3.3 s, median of 8 strongest partials (dB/s)')
ax[1, 0].set_title('Beat/fluctuation depth (dB, 3–97 % range of detrended envelope)')
ax[1, 1].set_title('Fundamental level at 3.2 s re its peak (dB)')
for a in ax.ravel():
    a.grid(alpha=.3); a.legend(); a.set_xlabel('MIDI key')
fig.suptitle(f'Single notes at {dyn}: partial decay and beating')
fig.tight_layout(); fig.savefig(os.path.join(FIG, f'partials_{dyn}.png'), dpi=105); plt.close(fig)
