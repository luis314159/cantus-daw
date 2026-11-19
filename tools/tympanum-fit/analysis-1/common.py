"""Shared helpers for the Tympanum drum analysis (audio only, no model code)."""
import numpy as np
import soundfile as sf
from scipy import signal

import os
# The listening material analysed (round 40: `tympanum-b3`, make_listen.py's files) — TYMP_LISTEN;
# results and figures — TYMP_ANALYSIS_OUT; the recorded kits' hit files — TYMP_MEAS.
BASE = os.path.join(os.environ.get("TYMP_LISTEN", "listen"), "")
OUT = os.path.join(os.environ.get("TYMP_ANALYSIS_OUT", "analysis-1-out"), "")
FIG = OUT + "figures/"
os.makedirs(FIG, exist_ok=True)
os.makedirs(OUT + "scripts", exist_ok=True)
SR = 48000

PIECES = ["kick", "snare", "rimshot", "sidestick", "tom_hi", "tom_mid", "tom_floor",
          "hat_closed", "hat_open", "hat_pedal", "crash", "ride", "ride_bell"]
VELS = [40, 80, 110, 127]


def load(name):
    x, sr = sf.read(BASE + name, always_2d=True)
    assert sr == SR
    return x.astype(np.float64)


def hit(x, piece, vel, length=3.9, pre=0.02):
    """Return the stereo segment of one hit of the 'golpes' file and its onset index."""
    i = PIECES.index(piece) * 4 + VELS.index(vel)
    s0 = int(i * 4 * SR)
    seg = x[s0:s0 + int(4 * SR)]
    mono = np.abs(seg).sum(1)
    thr = 0.05 * mono.max()
    on = int(np.argmax(mono > thr))
    a = max(0, on - int(pre * SR))
    return seg[a:a + int(length * SR)], on - a


def octave_bands():
    c = [63, 125, 250, 500, 1000, 2000, 4000, 8000, 16000]
    return [(f / np.sqrt(2), min(f * np.sqrt(2), 23500), f) for f in c]


def band_env(m, lo, hi, win=0.005, sr=SR):
    sos = signal.butter(4, [lo, hi], btype="band", fs=sr, output="sos")
    y = signal.sosfiltfilt(sos, m)
    n = int(win * sr)
    e = np.sqrt(np.convolve(y * y, np.ones(n) / n, mode="same") + 1e-30)
    return e


def schroeder_t(e2, sr, lo_db, hi_db):
    """Decay time to -60 dB from the Schroeder integral fitted between lo_db and hi_db."""
    s = np.cumsum(e2[::-1])[::-1]
    s = 10 * np.log10(s / s[0] + 1e-30)
    i0 = np.argmax(s <= lo_db)
    i1 = np.argmax(s <= hi_db)
    if i1 <= i0 or s[-1] > hi_db:
        return np.nan
    t = np.arange(len(s)) / sr
    p = np.polyfit(t[i0:i1], s[i0:i1], 1)
    return -60.0 / p[0]


def flatness(p, axis=0):
    p = np.maximum(p, 1e-20)
    return np.exp(np.mean(np.log(p), axis=axis)) / np.mean(p, axis=axis)
