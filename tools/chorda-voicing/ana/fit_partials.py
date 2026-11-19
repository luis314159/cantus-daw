"""Round 40: each key's partial voicing fitted to the reference — partials 2 ... 7 of every
measured key (A0 ... C8 every minor third, mf, the analyst's `notes_partials.csv`), each
against its own fundamental, candidate minus Salamander; the table moves each partial by
`-gain x difference` (clamped, smoothed across neighbouring keys, interpolated to every key)
on top of the table the candidate already had.

    python fit_partials.py <out dir of a try> <previous table or -> <new table> [gain] [clamp]
"""
import csv, os, sys
import numpy as np

out, prev, new = sys.argv[1], sys.argv[2], sys.argv[3]
gain = float(sys.argv[4]) if len(sys.argv) > 4 else 0.7
clamp = float(sys.argv[5]) if len(sys.argv) > 5 else 12.0
R = list(csv.DictReader(open(os.path.join(out, 'csv', 'notes_partials.csv'))))


def lv(src, key, dyn):
    d = {int(r['n']): float(r['lvl_db']) for r in R if r['src'] == src and int(r['key']) == key and r['dyn'] == dyn}
    return d


keys = sorted(set(int(r['key']) for r in R))
P = 6
diff = {}
for k in keys:
    row = []
    for n in range(2, 2 + P):
        ds = []
        for dyn in ('mf', 'ff'):
            s, c = lv('sal', k, dyn), lv('c2', k, dyn)
            if 1 in s and 1 in c and n in s and n in c and np.isfinite(s[n]) and np.isfinite(c[n]):
                ds.append((c[n] - c[1]) - (s[n] - s[1]))
        row.append(float(np.median(ds)) if ds else 0.0)
    diff[k] = np.array(row)
# Smooth across measured keys (1-2-1), then interpolate to every key.
ks = np.array(keys)
D = np.array([diff[k] for k in keys])
Ds = D.copy()
for i in range(len(keys)):
    lo, hi = max(0, i - 1), min(len(keys) - 1, i + 1)
    Ds[i] = (D[lo] + 2 * D[i] + D[hi]) / 4
table = np.zeros((88, P))
if prev != '-' and os.path.exists(prev):
    for line in open(prev):
        f = [float(x) for x in line.split(',')]
        table[int(f[0]) - 21] = f[1:]
for j in range(P):
    corr = np.interp(np.arange(21, 109), ks, Ds[:, j])
    table[:, j] = np.clip(table[:, j] - gain * corr, -clamp, clamp)
with open(new, 'w') as fh:
    for i in range(88):
        fh.write(','.join([str(21 + i)] + [f'{g:.2f}' for g in table[i]]) + '\n')
print('mean |difference| per partial (dB):', ' '.join(f'{v:.1f}' for v in np.abs(D).mean(0)))
print('table range:', f'{table.min():.1f} .. {table.max():.1f}')
