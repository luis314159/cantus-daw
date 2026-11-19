"""Figures: partials 1-6 of A4..A5 per channel (power sum) and the treble knock-to-tone ratio."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana2'))
import numpy as np, csv, os
import matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt
from common import *
from partials import spectrum, interp_peak, find_partials
fig, axs = plt.subplots(1, 5, figsize=(17, 3.8), sharey=True)
for i, key in enumerate([69, 72, 75, 78, 81]):
    for src in ['c5', 'sal']:
        x = note(src, 'mf', key); on = onset(x, -30); y = x[on:]
        f0, B, pl = find_partials(y.mean(1), key, 0.25, 1.25)
        P = sum(spectrum(y[:, ch], 0.25, 1.25)[1] for ch in range(2)); f = spectrum(y[:, 0], 0.25, 1.25)[0]
        lv = [10 * np.log10(interp_peak(f, P, n * f0 * np.sqrt(1 + B * n * n) * 0.99, n * f0 * np.sqrt(1 + B * n * n) * 1.01)[1] + 1e-30) for n in range(1, 7)]
        lv = np.array(lv) - lv[0]
        axs[i].plot(range(1, 7), lv, 'o-', color=COL[src], label=NAME[src])
    axs[i].set_title(f'{kname(key)} mf'); axs[i].grid(alpha=.3); axs[i].set_xlabel('partial n')
    axs[i].axhline(0, color='k', lw=.5)
axs[0].set_ylabel('dB re fundamental (L+R power)'); axs[0].legend(); axs[0].set_ylim(-50, 15)
fig.suptitle('Upper-middle register: Chorda\'s 2nd–4th partials are as loud as the fundamental; the Salamander\'s sit 12–30 dB below it')
fig.tight_layout(); fig.savefig(os.path.join(FIG, 'upper_mid_partials.png'), dpi=100); plt.close(fig)
R = list(csv.DictReader(open(os.path.join(CSV, 'treble_knock.csv'))))
fig, ax = plt.subplots(figsize=(9, 4))
for dyn, ls in [('pp', '-'), ('mf', '--'), ('ff', ':')]:
    rr = [r for r in R if r['dyn'] == dyn]
    ax.plot([r['key'] for r in rr], [float(r['c5_knock_to_tone_db']) for r in rr], ls, marker='o', color=COL['c5'], label=f'Chorda c5 {dyn}')
    ax.plot([r['key'] for r in rr], [float(r['sal_knock_to_tone_db']) for r in rr], ls, marker='o', color=COL['sal'], label=f'Salamander {dyn}')
ax.axhline(0, color='k', lw=.5); ax.grid(alpha=.3); ax.legend(fontsize=7, ncol=3)
ax.set_title('Treble, first 80 ms: knock (below 0.6·f0) re string tone (dB); > 0 = more thud than pitch')
fig.tight_layout(); fig.savefig(os.path.join(FIG, 'treble_knock.png'), dpi=100); plt.close(fig)
