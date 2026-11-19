"""Tympanum probe (round 40, voicing-drums): One piece and velocity, recording vs model: each one's onset and peak, and the octave bands' levels at fixed times after the onset.
    python -I diag2.py <recorded hits> <model hits> <piece> <velocity>
"""
import sys; sys.path.insert(0, __import__('os').path.dirname(__import__('os').path.dirname(__import__('os').path.abspath(__file__))))
import numpy as np
from metrics import *
R = load(sys.argv[1]); S = load(sys.argv[2]); piece = sys.argv[3]; vel = int(sys.argv[4])
i = PIECES.index(piece) * 4 + VELS.index(vel)
a0 = int(i * 4 * SR)
for X, l in ((R, 'R'), (S, 'S')):
    seg = X[a0:a0 + 4 * SR].mean(1)
    m, on = hit(X, piece, vel)
    mono = np.abs(X[a0:a0 + 4 * SR]).sum(1); ona = int(np.argmax(mono > 0.05 * mono.max()))
    print(l, 'onset at', ona, 'max at', int(np.argmax(mono)))
    base = int(np.argmax(np.abs(R[a0:a0 + 4 * SR]).sum(1) > 0.05 * np.abs(R[a0:a0 + 4 * SR]).sum(1).max()))
    for lo, hi in ((200, 1500), (1500, 6000), (6000, 20000)):
        e = band_env(seg, lo, hi, win=0.001)
        ts = [0, 1, 2, 3, 5, 8, 12, 20, 30, 45, 70, 100]
        print('  %5d-%5d' % (lo, hi), ' '.join('%6.1f' % (20 * np.log10(e[base + int(t * SR / 1000)] + 1e-12)) for t in ts))
