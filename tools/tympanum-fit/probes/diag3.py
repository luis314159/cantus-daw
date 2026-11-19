"""Tympanum probe (round 40, voicing-drums): One band of one hit: the recording's envelope at fixed times beside the fit's synthesis (synth.py) of its modes and noise, and the fit's modes in the band.
    python -I diag3.py <recorded hits> <fit.json> <piece> <velocity> <lo Hz> <hi Hz>
"""
import sys; sys.path.insert(0, __import__('os').path.dirname(__import__('os').path.dirname(__import__('os').path.abspath(__file__))))
import json, numpy as np, synth
from metrics import *
R = load(sys.argv[1]); t = json.load(open(sys.argv[2])); piece = sys.argv[3]; vel = int(sys.argv[4])
lo, hi = float(sys.argv[5]), float(sys.argv[6])
TS = [1, 3, 8, 20, 50, 100, 150, 200, 300, 400, 600, 1000]
m, on = hit(R, piece, vel)
e = band_env(m, lo, hi, win=0.01)
print('%-11s' % 'ref', ' '.join('%6.1f' % (20 * np.log10(e[on + int(ms * SR / 1000)] + 1e-12)) for ms in TS))
lay = [l for l in t['pieces'][piece] if l['vel'] == vel][0]
for name, L in (('all', lay), ('noise', dict(lay, modes=[], stick=[0, 0])), ('modes', dict(lay, env=(np.array(lay['env']) - 200).tolist(), stick=[0, 0]))):
    tab = dict(t, pieces=dict(x=[dict(L, vel=vel)]))
    y = synth.render(tab, 'x', vel, int(1.1 * SR), np.random.default_rng(1))
    e = band_env(y, lo, hi, win=0.01)
    print('%-11s' % name, ' '.join('%6.1f' % (20 * np.log10(e[int(ms * SR / 1000)] + 1e-12)) for ms in TS))
md = np.array(lay['modes']); k = (md[:, 0] > lo) & (md[:, 0] < hi)
print('modes in band', k.sum(), 'rise med %.3f direct med %.3f t60 med %.2f' % tuple(np.median(md[k][:, [3, 4, 2]], axis=0)))
