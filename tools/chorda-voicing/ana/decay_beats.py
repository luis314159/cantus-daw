"""Across the keyboard (mf and ff): the two-stage decay (how much the note falls in its first
second against how fast it falls afterwards) and the depth of the beating nulls in the partials."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana'))
import numpy as np, csv, os
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from common import *
from notes_analysis import partials, envelope, TE

rows = []
for src in ('sal', 'c2', 'b14'):
    for dyn in ('mf', 'ff'):
        for key in KEYS:
            x, sr = note(src, dyn, key)
            m = x.mean(1)
            i0 = onset(x, sr, -50)
            # broadband envelope, 20 ms
            n = int(0.02 * sr)
            e = 10 * np.log10((m[i0:i0 + int(3.45 * sr) // n * n] ** 2).reshape(-1, n).mean(1) + 1e-30)
            t = np.arange(len(e)) * 0.02 + 0.01
            pk = e[:10].max()
            drop1 = pk - e[(t > 0.9) & (t < 1.1)].mean()
            late = np.polyfit(t[(t > 1.5) & (t < 3.4)], e[(t > 1.5) & (t < 3.4)], 1)[0]
            early = np.polyfit(t[(t > 0.05) & (t < 0.6)], e[(t > 0.05) & (t < 0.6)], 1)[0]
            # partial nulls
            f0, B, found, _ = partials(m, key)
            X = np.fft.rfft(m); Fx = np.fft.rfftfreq(len(m), 1 / sr)
            f0n = kfreq(key)
            half = min(0.4 * f0n, 12.0) if f0n < 200 else min(0.4 * f0n, 25.0)
            strong = sorted(found, key=lambda k: -found[k][1])[:8]
            depths, lates, prompts = [], [], []
            for nn in strong:
                env = envelope(X, Fx, found[nn][0], half)
                mm = (TE > 0.2) & (TE < 3.4) & (env > -200)
                if mm.sum() < 20:
                    continue
                # upper envelope (running max over 0.6 s) minus envelope = how deep the beats cut
                w = int(0.6 / (TE[1] - TE[0]))
                ev = env[mm]
                upper = np.array([ev[max(0, i - w // 2):i + w // 2 + 1].max() for i in range(len(ev))])
                depths.append(np.percentile(upper - ev, 95))
                prompts.append(np.polyfit(TE[(TE > 0.05) & (TE < 0.5)], env[(TE > 0.05) & (TE < 0.5)], 1)[0])
                lates.append(np.polyfit(TE[(TE > 1.5) & (TE < 3.4)], env[(TE > 1.5) & (TE < 3.4)], 1)[0])
            rows.append(dict(src=src, dyn=dyn, key=key, name=kname(key), drop_first_1s_db=round(drop1, 2),
                             early_dbs=round(early, 2), late_dbs=round(late, 2),
                             two_stage_ratio=round(early / late, 2) if late < -0.3 else np.nan,
                             null_depth_db=round(np.median(depths), 2) if depths else np.nan,
                             partial_prompt_dbs=round(np.median(prompts), 2) if prompts else np.nan,
                             partial_late_dbs=round(np.median(lates), 2) if lates else np.nan))
        print(src, dyn, flush=True)
with open(os.path.join(CSV, 'decay_beats.csv'), 'w', newline='') as fh:
    w = csv.DictWriter(fh, fieldnames=list(rows[0].keys())); w.writeheader(); w.writerows(rows)

fig, axs = plt.subplots(2, 3, figsize=(19, 9))
SRC = {'sal': 4, 'c2': 3, 'b14': 1}
for r_, dyn in enumerate(('mf', 'ff')):
    for src in ('sal', 'c2', 'b14'):
        d = [q for q in rows if q['src'] == src and q['dyn'] == dyn]
        k = [q['key'] for q in d]
        lw = .8 if src == 'b14' else 2
        axs[r_, 0].plot(k, [q['drop_first_1s_db'] for q in d], 'o-', ms=3, lw=lw, color=COL[SRC[src]], label=SHORT[SRC[src]])
        axs[r_, 1].plot(k, [q['late_dbs'] for q in d], 'o-', ms=3, lw=lw, color=COL[SRC[src]], label=SHORT[SRC[src]])
        axs[r_, 2].plot(k, [q['null_depth_db'] for q in d], 'o-', ms=3, lw=lw, color=COL[SRC[src]], label=SHORT[SRC[src]])
    axs[r_, 0].set_title(f'{dyn}: fall during the first second (dB, peak → 1 s)')
    axs[r_, 1].set_title(f'{dyn}: decay rate 1.5–3.4 s, broadband (dB/s)')
    axs[r_, 2].set_title(f'{dyn}: depth of the beating nulls, 8 strongest partials (dB, median)')
for a in axs.flat:
    a.grid(alpha=.3); a.legend(fontsize=8)
    ks = [21, 33, 45, 57, 69, 81, 93, 105]; a.set_xticks(ks); a.set_xticklabels([kname(q) for q in ks])
fig.tight_layout(); fig.savefig(os.path.join(PLOTS, 'note_two_stage_beats.png'), dpi=80); plt.close(fig)

REG = {'bass': (21, 47), 'middle': (48, 83), 'treble': (84, 108)}
for dyn in ('mf', 'ff'):
    for rn, (a, b) in REG.items():
        s = []
        for src in ('sal', 'c2', 'b14'):
            d = [q for q in rows if q['src'] == src and q['dyn'] == dyn and a <= q['key'] <= b]
            med = lambda c: np.nanmedian([q[c] for q in d])
            s.append(f"{src}: drop1s {med('drop_first_1s_db'):.1f} late {med('late_dbs'):.1f} nulls {med('null_depth_db'):.1f} pprompt {med('partial_prompt_dbs'):.1f} plate {med('partial_late_dbs'):.1f}")
        print(dyn, rn, ' | '.join(s))
