"""Treble: decay of the fundamental partial alone (robust to the reference's low-frequency floor)."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana'))
import numpy as np, csv, os
from common import *
from notes_analysis import partials, envelope, TE
rows = []
for dyn in ('mf', 'ff'):
    for key in range(66, 109, 3):
        r = dict(dyn=dyn, key=key, name=kname(key))
        for src in ('sal', 'c2'):
            x, sr = note(src, dyn, key); m = x.mean(1)
            f0n = kfreq(key)
            X = np.fft.rfft(m); Fx = np.fft.rfftfreq(len(m), 1 / sr)
            f0, B, found, _ = partials(m, key)
            f1 = found[1][0] if 1 in found else f0n
            env = envelope(X, Fx, f1, min(0.4 * f0n, 25.0))
            pk = env[(TE < 0.1)].max()
            for a, b, nm in ((0.05, 0.5, 'prompt'), (0.5, 1.5, 'mid'), (1.5, 3.4, 'late')):
                mm = (TE > a) & (TE < b)
                r[f'{src}_{nm}_dbs'] = round(np.polyfit(TE[mm], env[mm], 1)[0], 1)
            r[f'{src}_at3s_re_peak'] = round(env[(TE > 2.9) & (TE < 3.1)].mean() - pk, 1)
            r[f'{src}_after_keyup_0.5s_dbs'] = round(np.polyfit(TE[(TE > 3.55) & (TE < 4.0)], env[(TE > 3.55) & (TE < 4.0)], 1)[0], 1)
        rows.append(r)
with open(os.path.join(CSV, 'treble_fundamental_decay.csv'), 'w', newline='') as fh:
    w = csv.DictWriter(fh, fieldnames=list(rows[0].keys())); w.writeheader(); w.writerows(rows)
for r in rows: print(r)
