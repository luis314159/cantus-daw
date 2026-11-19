"""Tympanum probe (round 40, voicing-drums): The highs of named pieces in two hit files side by side (band envelopes over time, every velocity).
    python -I hf.py <hits A> <hits B> <piece> ...
"""
import sys; sys.path.insert(0, __import__('os').path.dirname(__import__('os').path.dirname(__import__('os').path.abspath(__file__))))
import numpy as np
from metrics import load, hit, SR, VELS, band_env
xs = [load(f) for f in sys.argv[1:3]]
for p in sys.argv[3:]:
    out = []
    for v in (80, 110):
        r = []
        for x in xs:
            m, on = hit(x, p, v)
            e = band_env(m, 6000, 16000, win=0.01)[on:on + SR // 4]
            r.append(10 * np.log10(np.mean(e ** 2) + 1e-30))
        out.append(f"v{v}: {r[0]:6.1f}/{r[1]:6.1f} ({r[0]-r[1]:+.1f})")
    print(f"{p:10s}", "  ".join(out))
