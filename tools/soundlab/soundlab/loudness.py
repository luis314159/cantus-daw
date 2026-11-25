"""Loudness, ITU-R BS.1770-4 and EBU R128 (Tech 3341, 3342), for mono and stereo.

- `kweight_coeffs(sr)`: the K-weighting's two biquads at any rate, from the analog prototype
  (the shelf at 1681.97 Hz, +4.0 dB; the high-pass at 38.135 Hz); at 48 kHz they are
  BS.1770-4's Tables 1 and 2 to ~1e-12 (a test holds it).
- `measure(x, sr)`: integrated loudness (400 ms blocks, 75 % overlap, the -70 LKFS absolute
  and -10 LU relative gates), the momentary (400 ms) and short-term (3 s) maxima, the
  loudness range (Tech 3342: short-term 3 s blocks, here one every second as libebur128
  takes them, gates -70 LUFS and -20 LU, the 10th to 95th percentile), the sample peak and the true peak (4x oversampled,
  BS.1770-4 Annex 2's method by polyphase interpolation).
Channel weights are 1.0 for left and right (BS.1770's G for L, R, C); a mono file is one
channel (a dual-mono stereo copy reads +3.01 LU more, as a meter would).
"""
import numpy as np
from scipy import signal

ABS_GATE = -70.0
REL_GATE = -10.0
LRA_REL_GATE = -20.0


def kweight_coeffs(sr):
    """((b1, a1), (b2, a2)): the pre-filter (high shelf) and the RLB high-pass at `sr`."""
    f0, g, q = 1681.974450955533, 3.999843853973347, 0.7071752369554196
    k = np.tan(np.pi * f0 / sr)
    vh = 10 ** (g / 20)
    vb = vh ** 0.4996667741545416
    a0 = 1 + k / q + k * k
    b1 = [(vh + vb * k / q + k * k) / a0, 2 * (k * k - vh) / a0, (vh - vb * k / q + k * k) / a0]
    a1 = [1.0, 2 * (k * k - 1) / a0, (1 - k / q + k * k) / a0]
    f0, q = 38.13547087602444, 0.5003270373238773
    k = np.tan(np.pi * f0 / sr)
    a0 = 1 + k / q + k * k
    b2 = [1.0, -2.0, 1.0]
    a2 = [1.0, 2 * (k * k - 1) / a0, (1 - k / q + k * k) / a0]
    return (b1, a1), (b2, a2)


def kweight(x, sr):
    """K-weighted signal, each channel (x: frames or frames x channels)."""
    (b1, a1), (b2, a2) = kweight_coeffs(sr)
    return signal.lfilter(b2, a2, signal.lfilter(b1, a1, x, axis=0), axis=0)


def _blocks(y, sr, win_s, hop_s):
    """Mean square per channel over complete blocks of `win_s` every `hop_s`, summed over
    channels (G = 1): z[blocks]."""
    if y.ndim == 1:
        y = y[:, None]
    w, h = int(round(win_s * sr)), int(round(hop_s * sr))
    if len(y) < w:
        return np.zeros(0)
    c = np.concatenate([np.zeros((1, y.shape[1])), np.cumsum(y * y, axis=0)])
    starts = np.arange(0, len(y) - w + 1, h)
    return ((c[starts + w] - c[starts]) / w).sum(axis=1)


def _lk(z):
    return -0.691 + 10 * np.log10(np.maximum(z, 1e-30))


def integrated(z):
    """Gated integrated loudness of 400 ms block powers; None when every block is gated."""
    keep = z[_lk(z) > ABS_GATE]
    if not len(keep):
        return None
    rel = _lk(np.mean(keep)) + REL_GATE
    keep = keep[_lk(keep) > rel]
    return float(_lk(np.mean(keep))) if len(keep) else None


def lra(zs):
    """Loudness range of short-term (3 s) block powers, EBU Tech 3342; None if too few."""
    st = _lk(zs)
    st = st[st > ABS_GATE]
    if len(st) < 2:
        return None
    rel = _lk(np.mean(10 ** ((st + 0.691) / 10))) + LRA_REL_GATE
    st = np.sort(st[st > rel])
    if len(st) < 2:
        return None
    return float(np.percentile(st, 95) - np.percentile(st, 10))


def true_peak_db(x, over=4):
    """The 4x-oversampled peak, dBTP (polyphase low-pass interpolation)."""
    if x.size == 0:
        return -240.0
    y = signal.resample_poly(x, over, 1, axis=0, window=("kaiser", 10.0))
    return float(20 * np.log10(max(np.max(np.abs(y)), np.max(np.abs(x)), 1e-12)))


def lufs(x, sr):
    """Integrated loudness, LUFS (-120 for silence: what the kits' Rust meter returns)."""
    v = integrated(_blocks(kweight(x, sr), sr, 0.4, 0.1))
    return -120.0 if v is None else v


def measure(x, sr):
    """Every reading of one clip (x: frames or frames x channels)."""
    y = kweight(x, sr)
    zm = _blocks(y, sr, 0.4, 0.1)
    zs = _blocks(y, sr, 3.0, 0.1)
    i = integrated(zm)
    tp = true_peak_db(x)
    sp = float(20 * np.log10(max(np.max(np.abs(x)) if x.size else 0.0, 1e-12)))
    return {"integrated_lufs": i,
            "momentary_max_lufs": float(_lk(zm).max()) if len(zm) else None,
            "short_term_max_lufs": float(_lk(zs).max()) if len(zs) else None,
            "lra_lu": lra(zs[::10]) if len(zs) else None,
            "sample_peak_dbfs": sp, "true_peak_dbtp": tp,
            "crest_db": (tp - i) if i is not None else None}
