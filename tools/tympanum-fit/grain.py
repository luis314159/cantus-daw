"""How grainy a hit is: per octave band, the fluctuation of its envelope around its
smooth course (round 40, drums-gen) — a snare's wires rattle in bursts, a cymbal's wash
crackles; stationary noise under a smooth envelope fluctuates only as noise does.

    python -I grain.py <model hits> <reference hits> [piece ...]

Per band (1, 2, 4, 8 kHz octaves): the envelope (|analytic signal|, 0.5 ms smoothing) in
dB, less its 15-ms running median; the standard deviation (dB) in a window, and the share
of that fluctuation's power below 200 Hz (slow bursts) — noise's own sits higher.
"""
import sys
import numpy as np
from scipy import signal, ndimage
sys.path.insert(0, __file__.rsplit("/", 1)[0].rsplit("\\", 1)[0])
from metrics import load, hit, SR, VELS

OCT = [1000, 2000, 4000, 8000]
WIN = {"snare": (0.01, 0.15), "hat_closed": (0.005, 0.08), "hat_open": (0.05, 0.5),
       "crash": (0.1, 0.8), "ride": (0.1, 0.8), "rimshot": (0.01, 0.15)}


def grain(m, on, piece):
    a, b = WIN.get(piece, (0.01, 0.15))
    out = []
    for c in OCT:
        sos = signal.butter(3, [c / 2 ** 0.5, min(c * 2 ** 0.5, 23000)], btype="band", fs=SR, output="sos")
        y = signal.sosfiltfilt(sos, m)
        e = np.abs(signal.hilbert(y))
        e = ndimage.uniform_filter1d(e, int(0.0005 * SR))
        d = 20 * np.log10(e + 1e-12)
        r = d - ndimage.median_filter(d, int(0.015 * SR) | 1)
        seg = r[on + int(a * SR):on + int(b * SR)]
        P = np.abs(np.fft.rfft(seg - seg.mean())) ** 2
        f = np.fft.rfftfreq(len(seg), 1 / SR)
        out.append((float(seg.std()), float(P[(f > 5) & (f < 200)].sum() / P[f > 5].sum())))
    return out


if __name__ == "__main__":
    files = [a for a in sys.argv[1:] if a.endswith((".flac", ".wav"))]
    pieces = [a for a in sys.argv[1:] if not a.endswith((".flac", ".wav"))] or ["snare"]
    xs = [load(f) for f in files]
    for p in pieces:
        print(f"=== {p}: per octave (1, 2, 4, 8 kHz) std dB / slow share; model | recording")
        for v in VELS:
            row = []
            for x in xs:
                m, on = hit(x, p, v)
                row.append("  ".join(f"{s:4.1f}/{q:.2f}" for s, q in grain(m, on, p)))
            print(f"v{v:3d}  " + "  |  ".join(row))
