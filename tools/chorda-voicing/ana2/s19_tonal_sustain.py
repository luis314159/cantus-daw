"""Treble sustain measured on the string tone only: energy within +-0.6 % of each partial
(first 6) minus the noise density measured beside it, in 0.3-s windows; removes the
Salamander's recording noise from the decay figures. mf and pp, F#4..C8."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana2'))
import numpy as np, csv, os
import matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt
from common import *
from partials import find_partials

TW = [0.05, 0.25, 0.5, 1.0, 1.5, 2.0, 2.5, 3.1]
out = {}
for dyn in ['mf', 'pp']:
    for src in ['c5', 'sal']:
        for key in [k for k in KEYS if k >= 66]:
            x = note(src, dyn, key)
            on = onset(x, -30)
            y = x[on:]
            f0, B, pl = find_partials(y.mean(1), key, 0.1, 0.6)
            if not pl:
                pl = [(1, kfreq(key), 0)]
            fl = [fn for n, fn, lv in pl if n <= 6]
            lv = []
            for t0 in TW:
                seg = y[int(t0 * SR):int((t0 + 0.3) * SR)]
                f, P = signal.periodogram(seg * signal.windows.hann(len(seg))[:, None], SR, axis=0)
                P = P.sum(1)
                df = f[1] - f[0]
                E = 0
                for fn in fl:
                    w = max(0.006 * fn, 3 * df)
                    on_ = np.abs(f - fn) < w
                    side = (np.abs(f - fn) > 2 * w) & (np.abs(f - fn) < 6 * w)
                    E += max(P[on_].sum() - np.median(P[side]) * on_.sum(), 0)
                lv.append(E)
            lv = db(np.array(lv) + 1e-30)
            out[(dyn, src, key)] = lv - lv.max()
K = [k for k in KEYS if k >= 66]
fig, axs = plt.subplots(1, 2, figsize=(14, 4.8))
for i, dyn in enumerate(['mf', 'pp']):
    for src in ['c5', 'sal']:
        for j, (ti, ls) in enumerate([(3, '-.'), (5, '--'), (7, '-')]):
            axs[i].plot(K, [out[(dyn, src, k)][ti] for k in K], ls, marker='o', ms=3, color=COL[src], label=f'{NAME[src]} {TW[ti] + 0.15:.2f} s')
    axs[i].set_title(f'String tone only (partials 1–6, noise removed), {dyn}: level re its max'); axs[i].grid(alpha=.3)
    axs[i].set_ylim(-90, 2); axs[i].set_xlabel('MIDI key'); axs[i].legend(fontsize=7, ncol=2)
fig.tight_layout(); fig.savefig(os.path.join(FIG, 'treble_tonal_sustain.png'), dpi=100); plt.close(fig)
for dyn in ['mf', 'pp']:
    print('==', dyn, 'level at', [round(t + 0.15, 2) for t in TW])
    for k in K:
        a, b = out[(dyn, 'c5', k)], out[(dyn, 'sal', k)]
        print(f'{kname(k):4s} c5 ' + ' '.join(f'{v:6.1f}' for v in a) + ' | sal ' + ' '.join(f'{v:6.1f}' for v in b))
