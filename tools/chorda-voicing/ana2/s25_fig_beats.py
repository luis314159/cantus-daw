"""Figure: beat depth (power-summed channels) and flanger-test coherence per key."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana2'))
import numpy as np, csv, os
import matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt
from common import *
d = list(csv.DictReader(open(os.path.join(CSV, 'beat_depth_powersum_mf.csv'))))
s = list(csv.DictReader(open(os.path.join(CSV, 'sweep_mf.csv'))))
fig, ax = plt.subplots(1, 2, figsize=(14, 4.5))
K = KEYS[:len(d)]
for src, c in [('c5', COL['c5']), ('sal', COL['sal'])]:
    ax[0].plot(K, [float(r[src + '_db']) for r in d], 'o-', color=c, label=NAME[src])
    ax[1].plot([int(r['key']) for r in s], [float(r[src]) for r in s], 'o-', color=c, label=NAME[src])
ax[0].set_title('Beat depth, mf: median over 8 strongest partials of the\nenvelope swing around its trend (dB, both channels summed)')
ax[1].set_title('Flanger test, mf: do the dips of partials 4–16 line up on t·n?\n(1 = one notch sweeping through the spectrum; 0 = independent)')
for a in ax:
    a.grid(alpha=.3); a.legend(); a.set_xlabel('MIDI key (21 = A0, 60 = C4)')
fig.tight_layout(); fig.savefig(os.path.join(FIG, 'beats_summary.png'), dpi=105); plt.close(fig)
