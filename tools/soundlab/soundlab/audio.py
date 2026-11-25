"""Audio in and out: every clip is float64, shape (frames, channels)."""
from math import gcd

import numpy as np
import soundfile as sf
from scipy import signal


def read(path, sr=None, channels=None):
    """(x[frames, ch], sr). `sr`: resample to it (polyphase, scipy). `channels`: 1 = the first
    channel (a spaced pair summed would comb-filter: the study's rule), 2 = stereo (a mono
    file duplicated)."""
    x, rate = sf.read(str(path), dtype="float64", always_2d=True)
    if channels == 1:
        x = x[:, :1]
    elif channels == 2:
        x = np.repeat(x[:, :1], 2, axis=1) if x.shape[1] == 1 else x[:, :2]
    if sr and rate != sr:
        x = resample(x, rate, sr)
        rate = sr
    return np.ascontiguousarray(x), int(rate)


def resample(x, sr_from, sr_to):
    g = gcd(int(sr_from), int(sr_to))
    return signal.resample_poly(x, sr_to // g, sr_from // g, axis=0)


def mono(x):
    """The first channel (see `read`)."""
    return x[:, 0] if x.ndim == 2 else x


def write(path, x, sr, subtype="PCM_24"):
    """WAV or FLAC by the extension; 24-bit by default, clipped to [-1, 1]."""
    sf.write(str(path), np.clip(x, -1.0, 1.0), int(sr), subtype=subtype)


def db(v, floor=1e-12):
    return 20 * np.log10(np.maximum(np.abs(v), floor))


def peak_db(x):
    return float(db(np.max(np.abs(x)) if x.size else 0.0))
