"""The 'sweeping notch' test: if every partial n beats at n times one rate (strings detuned by
a constant number of cents), the envelope dips fall on t = k/n — a notch sweeping down
through the spectrum like a flanger. Resample each partial's detrended envelope on tau = t*n
and measure how alike they are (mean pairwise correlation, partials 4..16), at mf."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana2'))
import numpy as np, csv, os
import matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt
from common import *
from partials import find_partials, hetero_env

dyn = sys.argv[1] if len(sys.argv) > 1 else 'mf'
NS = list(range(4, 17))
tau = np.linspace(0.15 * NS[-1], 3.3 * NS[0], 600)
rows = []
for key in KEYS[:18]:  # A0..F#4 (enough partials)
    r = {'key': key}
    for src in ['c5', 'sal']:
        x = note(src, dyn, key)
        on = onset(x, -30)
        y = x[on:on + int(3.4 * SR)]
        f0, B, pl = find_partials(y.mean(1), key, 0.25, 1.25)
        d = {n: fn for n, fn, lv in pl}
        W = []
        for n in NS:
            if n not in d:
                continue
            evs = []
            for ch in range(2):
                t, ev = hetero_env(y[:, ch], d[n], min(0.3 * f0, 25), hop=0.005)
                evs.append(10 ** (ev / 10))
            ev = 10 * np.log10(evs[0] + evs[1] + 1e-30)
            mk = (t > 0.15) & (t < 3.3)
            res = ev[mk] - np.polyval(np.polyfit(t[mk], ev[mk], 2), t[mk])
            W.append(np.interp(tau, t[mk] * n, res))
        W = np.array(W)
        if len(W) >= 5:
            C = np.corrcoef(W)
            iu = np.triu_indices(len(W), 1)
            r[src] = float(np.nanmean(C[iu]))
        else:
            r[src] = np.nan
    rows.append(r)
    print(f"{kname(key):4s} sweep-coherence c5 {r['c5']:.2f}  sal {r['sal']:.2f}", flush=True)
K = [r['key'] for r in rows]
fig, ax = plt.subplots(figsize=(8, 4))
for s in ['c5', 'sal']:
    ax.plot(K, [r[s] for r in rows], 'o-', color=COL[s], label=NAME[s])
ax.set_title('Flanger test: partials 4–16 envelopes aligned on t·n\n(1 = one notch sweeping through all partials; 0 = independent beats)')
ax.set_xlabel('MIDI key'); ax.grid(alpha=.3); ax.legend()
fig.tight_layout(); fig.savefig(os.path.join(FIG, f'sweep_coherence_{dyn}.png'), dpi=105); plt.close(fig)
with open(os.path.join(CSV, f'sweep_{dyn}.csv'), 'w', newline='') as fh:
    w = csv.DictWriter(fh, ['key', 'c5', 'sal']); w.writeheader(); w.writerows(rows)
