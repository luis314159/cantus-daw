"""The first analysis's P1 (two-stage beats) from an ANA_OUT dir holding csv/decay_beats.csv, with each term."""
import sys
sys.path.insert(0, tool_path('ana'))
import numpy as np, accept
d = sys.argv[1]
R = accept.rows(d, 'decay_beats.csv')
p1 = []
for dyn in ('mf', 'ff'):
    for reg, (lo, hi) in accept.REG.items():
        g = {s: [r for r in R if r['src'] == s and r['dyn'] == dyn and lo <= int(r['key']) <= hi] for s in ('sal', 'c2')}
        m = {s: {c: accept.med([accept.f(r[c]) for r in g[s]]) for c in ('drop_first_1s_db', 'null_depth_db', 'partial_prompt_dbs', 'partial_late_dbs')} for s in g}
        t = [abs(m['c2']['drop_first_1s_db'] - m['sal']['drop_first_1s_db']) / 3,
             max(0.0, m['c2']['null_depth_db'] - m['sal']['null_depth_db']) / 3,
             abs(m['c2']['partial_prompt_dbs'] - m['sal']['partial_prompt_dbs']) / 6]
        if reg != 'treble':
            t.append(abs(m['c2']['partial_late_dbs'] - m['sal']['partial_late_dbs']) / 2)
        p1 += t
        print(f'{dyn} {reg:7s} drop {m["c2"]["drop_first_1s_db"]:5.1f}/{m["sal"]["drop_first_1s_db"]:5.1f} nulls {m["c2"]["null_depth_db"]:5.1f}/{m["sal"]["null_depth_db"]:5.1f} '
              f'prompt {m["c2"]["partial_prompt_dbs"]:6.1f}/{m["sal"]["partial_prompt_dbs"]:6.1f} late {m["c2"]["partial_late_dbs"]:5.1f}/{m["sal"]["partial_late_dbs"]:5.1f}  terms ' + ' '.join(f'{v:.2f}' for v in t))
print(f'P1 {np.nanmean(p1):.3f}')
