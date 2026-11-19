"""Tympanum probe (round 40, voicing-drums): the 6-16 kHz band's effective components (participation ratio of its STFT bins, /10; the first analysis's "components per kHz") per piece and velocity, two hit files side by side (the hats over 0-0.25 s, the rest 0-1 s).
    python -I pr.py <hits A> <hits B> <piece> ...
"""
import sys; sys.path.insert(0, __import__('os').path.dirname(__import__('os').path.dirname(__import__('os').path.abspath(__file__))))
import numpy as np
from scipy import signal
from metrics import load, hit, SR, VELS
files = sys.argv[1:3]; pieces = sys.argv[3:]
xs = [load(f) for f in files]
def pr(m, on, a, b):
    seg = m[on + int(a * SR):on + int(b * SR)]
    f, t, S = signal.stft(seg, SR, nperseg=8192, noverlap=4096)
    P = np.abs(S) ** 2
    s = (f >= 6000) & (f < 16000)
    v = P[s].sum(0) ** 2 / (P[s] ** 2).sum(0) / 10
    return float(np.median(v))
for p in pieces:
    win = (0.0, 0.25) if p in ("hat_closed", "hat_pedal") else (0.0, 1.0)
    print(p, " ".join(f"v{v}: " + "/".join(f"{pr(*hit(x, p, v), *win):5.1f}" for x in xs) for v in VELS))
