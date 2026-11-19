"""Tympanum probe (round 40, voicing-drums): One piece and velocity, recording vs model: the tonal share of each band (peaks over 10 dB above the local median) and the modal against the noise power, with the fit.
    python -I diag.py <recorded hits> <model hits> <fit.json> <piece> <velocity>
"""
import sys; sys.path.insert(0, __import__('os').path.dirname(__import__('os').path.dirname(__import__('os').path.abspath(__file__))))
import json, numpy as np
from scipy import signal, ndimage
from metrics import *
R = load(sys.argv[1]); S = load(sys.argv[2]); t = json.load(open(sys.argv[3]))
piece = sys.argv[4]; vel = int(sys.argv[5])
for X, l in ((R, 'R'), (S, 'S')):
    m, on = hit(X, piece, vel)
    w = m[on + int(.15 * SR):on + int(.65 * SR)]; n = len(w)
    P = np.abs(np.fft.rfft(w * np.hanning(n), 4 * n)) ** 2; f = np.fft.rfftfreq(4 * n, 1 / SR)
    d = 10 * np.log10(P + 1e-30); med = ndimage.median_filter(d, size=int(300 / f[1]) | 1)
    for lo, hi in ((1000, 4000), (4000, 8000), (8000, 12000)):
        s = (f > lo) & (f < hi); e = d[s] - med[s]
        print(l, lo, 'tonal frac>10dB %.2f' % (P[s][e > 10].sum() / P[s].sum()), 'exc p50 %.1f p90 %.1f p99 %.1f' % tuple(np.percentile(e, [50, 90, 99])))
lay = [x for x in t['pieces'][piece] if x['vel'] == vel][0]; md = np.array(lay['modes'])
C = np.array(t['centres']); env = np.array(lay['env']); bi = t['bp_ms'].index(300)
for lo, hi in ((1000, 4000), (4000, 8000), (8000, 12000)):
    k = (md[:, 0] > lo) & (md[:, 0] < hi)
    mp = (0.5 * md[k, 1] ** 2 * np.exp(-2 * 6.9 * 0.3 / md[k, 2])).sum()
    nb = (C > lo) & (C < hi)
    print(lo, 'modal pow %.3g noise pow %.3g' % (mp, (10 ** (env[nb, bi] / 10)).sum()), 'n modes', k.sum())
