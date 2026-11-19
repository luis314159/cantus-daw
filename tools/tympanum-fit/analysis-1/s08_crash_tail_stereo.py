"""Stereo of the groove's final crash tail (10.0-11.5 s) per octave band; mono-sum loss."""
import sys
sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
import numpy as np
from scipy import signal
from common import *
for k, fn in [("modelled", "groove-modelado-studio.flac"), ("DRS", "groove-grabado-drskit.flac"), ("Crocell", "groove-grabado-crocellkit.flac")]:
    x = load(fn)
    for (a, b, nm) in [(10.0, 11.5, "crash tail"), (4.0, 7.0, "ride section")]:
        seg = x[int(a * SR):int(b * SR)]
        out = []
        for lo, hi in [(250, 500), (500, 1000), (1000, 2000), (2000, 4000), (4000, 8000), (8000, 16000)]:
            sos = signal.butter(4, [lo, hi], "band", fs=SR, output="sos")
            L = signal.sosfiltfilt(sos, seg[:, 0]); R = signal.sosfiltfilt(sos, seg[:, 1])
            c = np.sum(L * R) / np.sqrt(np.sum(L * L) * np.sum(R * R))
            loss = 10 * np.log10(np.sum(((L + R) / 2) ** 2) / ((np.sum(L * L) + np.sum(R * R)) / 2))
            out.append(f"{c:+.2f}/{loss:+.1f}")
        print(f"{k:9s} {nm:13s} corr/mono-sum gain dB per oct 250..16k: " + "  ".join(out))
