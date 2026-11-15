"""A drum's attack, model against recording: octave-band levels (dB re the hit's peak
octave in the first 100 ms) in short windows after the onset.

    python -I attack.py <model hits> <reference hits> [piece ...]
"""
import sys
import numpy as np
from scipy import signal
sys.path.insert(0, __file__.rsplit("/", 1)[0].rsplit("\\", 1)[0])
from metrics import SR, VELS, load, hit

OCT = [63, 125, 250, 500, 1000, 2000, 4000, 8000, 16000]
WINS = [(0, 3), (3, 10), (10, 30), (30, 100), (100, 250)]


def bands(m, on):
    out = []
    for c in OCT:
        sos = signal.butter(3, [c / 2 ** 0.5, min(c * 2 ** 0.5, 23000)], btype="band", fs=SR, output="sos")
        # zero-phase but causal-ish: forward only, the onset is what matters
        y = signal.sosfilt(sos, m)
        out.append(y * y)
    return np.array(out)


def table(fm, fr, pieces):
    xm, xr = load(fm), load(fr)
    for p in pieces:
        print(f"\n=== {p}: dB per octave band (63 Hz..16 kHz), model / recording, re each hit's loudest cell")
        for v in VELS:
            rows = []
            for x in (xm, xr):
                m, on = hit(x, p, v)
                pre = int(0.002 * SR)
                b = bands(m, on)
                cells = np.array([[b[k, on - pre + int(a * SR / 1000) + pre:on + int(e * SR / 1000)].mean()
                                   for k in range(len(OCT))] for a, e in WINS])
                rows.append(10 * np.log10(cells + 1e-30))
            ref = max(rows[0].max(), -999), rows[1].max()
            for w, (a, e) in enumerate(WINS):
                dm = rows[0][w] - ref[0]
                dr = rows[1][w] - ref[1]
                print(f"v{v:3d} {a:3d}-{e:3d}ms " + " ".join(f"{u:5.0f}/{r:<4.0f}" for u, r in zip(dm, dr)))


if __name__ == "__main__":
    files = [a for a in sys.argv[1:] if a.endswith((".flac", ".wav"))]
    table(files[0], files[1], [a for a in sys.argv[1:] if not a.endswith((".flac", ".wav"))])
