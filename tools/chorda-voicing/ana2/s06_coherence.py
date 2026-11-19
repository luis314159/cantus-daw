"""How much the partials' beating moves together (coherent = the whole tone pulses or a notch
sweeps through the spectrum, like a flanger) versus independently (a real piano's shimmer).
Per key at mf: mean pairwise correlation of the partials' detrended dB envelopes, and the
depth of the broadband level's own fluctuation."""
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
rows = []
for key in KEYS[:22]:  # up to A5 — above, too few partials
    r = {'key': key}
    for src in ['c5', 'sal']:
        x = note(src, dyn, key)
        on = onset(x, -30)
        y = x[on:on + int(3.4 * SR)]
        m = y.mean(1)
        f0, B, pl = find_partials(m, key, 0.25, 1.25)
        pl = sorted(pl, key=lambda p: -p[2])[:12]
        res = []
        for n, fn, lv in pl:
            # both channels separately, summed in power: avoids the A-B pair's mono comb
            evs = []
            for ch in range(2):
                t, ev = hetero_env(y[:, ch], fn, min(0.3 * f0, 25), hop=0.01)
                evs.append(10 ** (ev / 10))
            ev = 10 * np.log10(evs[0] + evs[1] + 1e-30)
            mk = (t > 0.15) & (t < 3.3)
            tt, vv = t[mk], ev[mk]
            res.append(vv - np.polyval(np.polyfit(tt, vv, 2), tt))
        R = np.array(res)
        C = np.corrcoef(R)
        iu = np.triu_indices(len(R), 1)
        r[src + '_coh'] = float(np.mean(C[iu]))
        t, e = short_term_rms(y, 0.05, 0.01)
        mk = (t > 0.15) & (t < 3.3)
        res = e[mk] - np.polyval(np.polyfit(t[mk], e[mk], 2), t[mk])
        r[src + '_bb'] = float(np.percentile(res, 97) - np.percentile(res, 3))
    rows.append(r)
    print(f"{kname(key):4s} coherence c5 {r['c5_coh']:.2f} sal {r['sal_coh']:.2f} | broadband pulsing c5 {r['c5_bb']:.1f} dB sal {r['sal_bb']:.1f} dB", flush=True)

with open(os.path.join(CSV, f'coherence_{dyn}.csv'), 'w', newline='') as fh:
    w = csv.DictWriter(fh, rows[0].keys()); w.writeheader(); w.writerows(rows)
K = [r['key'] for r in rows]
fig, ax = plt.subplots(1, 2, figsize=(13, 4.5))
for src in ['c5', 'sal']:
    ax[0].plot(K, [r[src + '_coh'] for r in rows], 'o-', color=COL[src], label=NAME[src])
    ax[1].plot(K, [r[src + '_bb'] for r in rows], 'o-', color=COL[src], label=NAME[src])
ax[0].set_title('Do the partials beat together? mean pairwise correlation\nof the 12 strongest partials\' detrended envelopes (0 = independent)')
ax[1].set_title('Broadband level fluctuation (whole note swelling/fading),\n3–97 % range of the detrended 50-ms level (dB)')
for a in ax:
    a.grid(alpha=.3); a.legend(); a.set_xlabel('MIDI key')
fig.tight_layout(); fig.savefig(os.path.join(FIG, f'beat_coherence_{dyn}.png'), dpi=105); plt.close(fig)
