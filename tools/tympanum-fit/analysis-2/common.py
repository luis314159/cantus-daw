"""Shared helpers for the Tympanum b9 element-by-element analysis (audio only)."""
import numpy as np
import soundfile as sf
from scipy import signal

import os
# The listening material analysed (round 40: `tympanum-b9`, make_listen.py's files) — TYMP_LISTEN;
# results and figures — TYMP_ANALYSIS_OUT; the recorded kits' hit files — TYMP_MEAS.
BASE = os.path.join(os.environ.get("TYMP_LISTEN", "listen"), "")
OUT = os.path.join(os.environ.get("TYMP_ANALYSIS_OUT", "analysis-2-out"), "")
FIG = OUT + "figures/"
os.makedirs(FIG, exist_ok=True)
os.makedirs(OUT + "scripts", exist_ok=True)
MEAS = os.path.join(os.environ.get("TYMP_MEAS", "meas"), "")
SR = 48000

ELEMENTS = ["bombo", "caja", "tom-medio", "tom-de-piso", "hihat-cerrado", "hihat-abierto", "ride", "crash"]
KITS = ["studio", "jazz"]
VERS = {"prev": "1-antes", "gen": "2-nuevo", "rec": "3-grabado"}
VELS = [40, 80, 110, 127]  # assumed order of the four hits (checked by level)

# meas9 hit-files layout (13 pieces x 4 velocities x 4 s, stereo)
MPIECES = ["kick", "snare", "rimshot", "sidestick", "tom_hi", "tom_mid", "tom_floor",
           "hat_closed", "hat_open", "hat_pedal", "crash", "ride", "ride_bell"]
EL2P = {"bombo": "kick", "caja": "snare", "tom-medio": "tom_mid", "tom-de-piso": "tom_floor",
        "hihat-cerrado": "hat_closed", "hihat-abierto": "hat_open", "ride": "ride", "crash": "crash"}
PACKS = {"DRS": "pack-drskit-tympanum-kit-hits.flac", "Crocell": "pack-crocellkit-tympanum-kit-hits.flac",
         "Muldjord": "pack-muldjordkit-tympanum-kit-hits.flac", "BigRusty": "pack-big-rusty-drums-kit-hits.flac",
         "Virtuosity": "pack-virtuosity-drums-kit-hits.flac"}

_cache = {}


def load(path):
    if path not in _cache:
        x, sr = sf.read(path, always_2d=True)
        assert sr == SR, (path, sr)
        _cache[path] = x.astype(np.float64)
    return _cache[path]


def element(kit, el, ver):
    return load(f"{BASE}{kit}-{el}-{VERS[ver]}.flac")


def onsets_equal(x, n=4):
    """Split a file of n equally spaced hits; return list of (segment_mono, onset_index)."""
    m = x.mean(1)
    L = len(m) // n
    out = []
    for i in range(n):
        seg = m[i * L:(i + 1) * L]
        a = np.abs(seg)
        thr = 0.02 * a.max()
        on = int(np.argmax(a > thr))
        # back off to the first sample above -50 dB re peak within 3 ms before
        k0 = max(0, on - int(0.003 * SR))
        thr2 = 10 ** (-50 / 20) * a.max()
        pre = np.nonzero(a[k0:on + 1] > thr2)[0]
        if len(pre):
            on = k0 + int(pre[0])
        out.append((seg, on))
    return out


def element_hits(kit, el, ver):
    return onsets_equal(element(kit, el, ver), 4)


def pack_hit(pack, piece, vel, length=3.9, pre=0.0):
    x = load(MEAS + PACKS[pack])
    i = MPIECES.index(piece) * 4 + VELS.index(vel)
    s0 = int(i * 4 * SR)
    seg = x[s0:s0 + int(4 * SR)].mean(1)
    a = np.abs(seg)
    on = int(np.argmax(a > 0.02 * a.max()))
    k0 = max(0, on - int(0.003 * SR))
    pre_ = np.nonzero(a[k0:on + 1] > 10 ** (-50 / 20) * a.max())[0]
    if len(pre_):
        on = k0 + int(pre_[0])
    return seg, on


def thirdoct_centres(lo=31.5, hi=20000):
    c = 1000 * 2 ** (np.arange(-15, 14) / 3)
    return c[(c >= lo) & (c <= hi)]


def band_env(m, lo, hi, win=0.005, sr=SR):
    hi = min(hi, sr / 2 * 0.98)
    sos = signal.butter(4, [lo, hi], btype="band", fs=sr, output="sos")
    y = signal.sosfiltfilt(sos, m)
    n = max(1, int(win * sr))
    return np.sqrt(np.convolve(y * y, np.ones(n) / n, mode="same") + 1e-30)


def bandpass(m, lo, hi, sr=SR, order=4):
    hi = min(hi, sr / 2 * 0.98)
    sos = signal.butter(order, [lo, hi], btype="band", fs=sr, output="sos")
    return signal.sosfiltfilt(sos, m)


def schroeder_t60(y, sr=SR, lo_db=-5, hi_db=-25):
    e2 = y * y
    s = np.cumsum(e2[::-1])[::-1]
    s = 10 * np.log10(s / (s[0] + 1e-30) + 1e-30)
    i0 = int(np.argmax(s <= lo_db))
    i1 = int(np.argmax(s <= hi_db))
    if i1 <= i0 + 10 or s[-1] > hi_db:
        return np.nan
    t = np.arange(len(s)) / sr
    p = np.polyfit(t[i0:i1], s[i0:i1], 1)
    return -60.0 / p[0]


def thirdoct_spec(m, sr=SR, nfft=None):
    """Energy per 1/3 octave (dB) of a segment via FFT."""
    n = len(m)
    if n < 64:
        m = np.pad(m, (0, 64 - n))
        n = 64
    w = np.hanning(n)
    X = np.abs(np.fft.rfft(m * w)) ** 2
    f = np.fft.rfftfreq(n, 1 / sr)
    c = thirdoct_centres()
    out = []
    for fc in c:
        k = (f >= fc / 2 ** (1 / 6)) & (f < fc * 2 ** (1 / 6))
        out.append(10 * np.log10(X[k].sum() + 1e-30) if k.any() else np.nan)
    return c, np.array(out)


def flatness(p, axis=0):
    p = np.maximum(p, 1e-20)
    return np.exp(np.mean(np.log(p), axis=axis)) / np.mean(p, axis=axis)


def centroid(m, sr=SR):
    X = np.abs(np.fft.rfft(m * np.hanning(len(m)))) ** 2
    f = np.fft.rfftfreq(len(m), 1 / sr)
    return float((X * f).sum() / (X.sum() + 1e-30))


def db(x):
    return 20 * np.log10(np.abs(x) + 1e-15)


def loudness_weights(c):
    """Rough equal-loudness weighting per band (A-weighting in dB) for a listen-proxy."""
    f2 = c ** 2
    ra = 12194 ** 2 * f2 ** 2 / ((f2 + 20.6 ** 2) * np.sqrt((f2 + 107.7 ** 2) * (f2 + 737.9 ** 2)) * (f2 + 12194 ** 2))
    return 20 * np.log10(ra) + 2.0
