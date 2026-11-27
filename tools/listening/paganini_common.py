"""The Paganini comparison's shared readings (round 42): the recording, its pitch track, its
onsets and its loudness — what `paganini_notes.py` aligns and transcribes from and what
`paganini_compare.py` measures. Run every script with `python -I` (it puts tools/soundlab on
the path itself).

The recording: Caprice No. 24, Queen Mary University of London's Performance Lab, 2014 —
Internet Archive item `Paganini_Caprice24`, Public Domain Mark 1.0 (catalog.toml's
`paganini` set), take 03 on the C414 pair (`C414_L03.aif`, `C414_R03.aif`, 96 kHz).
"""
import os
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "soundlab"))
from soundlab import audio, core, loudness  # noqa: E402

REFS = os.environ.get("CANTUS_REFS", r"F:\projects\cantus-refs")
SET = os.path.join(REFS, "paganini", "archive")
SR = 48000
NAMES = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"]


def name(k):
    k = int(round(k))
    return f"{NAMES[k % 12]}{k // 12 - 1}"


def recording(take="03"):
    """The take's pair at 48 kHz: (stereo [frames, 2], sr)."""
    l, _ = audio.read(os.path.join(SET, f"C414_L{take}.aif"), sr=SR, channels=1)
    r, _ = audio.read(os.path.join(SET, f"C414_R{take}.aif"), sr=SR, channels=1)
    n = min(len(l), len(r))
    return np.concatenate([l[:n], r[:n]], axis=1), SR


def pitch_track(m, sr, hop_s=0.005, fmin=180.0, fmax=2700.0):
    """YIN on a 24 kHz copy: (t, MIDI key as a float, aperiodicity d')."""
    x = audio.resample(m, sr, 24000)
    t, f0, ap = core.yin_track(x, 24000, fmin, fmax, hop_s=hop_s, thr=0.12)
    key = 69 + 12 * np.log2(np.maximum(f0, 1.0) / 440.0)
    return t, key, ap


def flux_onsets(m, sr, hop_s=0.005, thr=1.6, min_gap=0.045):
    """Onsets from the spectral flux (log magnitude, positive part, 250 Hz – 8 kHz), each a
    peak over `thr` × its local median: (times s, the flux curve's t, the curve)."""
    from scipy import signal
    n = 2048
    h = int(hop_s * sr)
    f, t, z = signal.stft(m, sr, nperseg=n, noverlap=n - h, boundary=None, padded=False)
    band = (f >= 250) & (f <= 8000)
    a = np.log1p(1000 * np.abs(z[band]))
    d = np.maximum(np.diff(a, axis=1), 0).sum(axis=0)
    d = np.concatenate([[0.0], d])
    med = core.median_filter(d, int(0.3 / hop_s)) + 1e-9
    r = d / med
    peaks, _ = signal.find_peaks(r, height=thr, distance=max(int(min_gap / hop_s), 1))
    return t[peaks], t, r


def env_db(m, sr, win_s=0.01, hop_s=0.002):
    return core.rms_db(m, sr, win_s, hop_s)


def lufs(x, sr):
    return loudness.lufs(x, sr)


def to_lufs(x, sr, target=-23.0):
    """`x` scaled to `target` LUFS integrated; (scaled, gain dB)."""
    g = target - loudness.lufs(x, sr)
    return x * 10 ** (g / 20), g


def write16(path, x, sr):
    """16-bit WAV (TPDF-dithered), peaks held under -1 dBFS by a plain gain if needed."""
    pk = np.max(np.abs(x))
    if pk > 0.891:
        x = x * (0.891 / pk)
    rng = np.random.default_rng(1)
    d = (rng.random(x.shape) - rng.random(x.shape)) / 32768.0
    audio.write(path, x + d, sr, subtype="PCM_16")


def two_voice(m, sr, a, b, hi_key, lo_key, nh=4):
    """Two notes sounding together over [a, b) (s): the upper one's harmonics 1–nh against
    the lower one's (each harmonic's power within ±1.5 % of where it should be, from a
    Hann-windowed FFT zero-padded to 1 Hz), and each one's frequency in cents from its
    tempered key, from its 2nd and 3rd harmonics' peaks (parabolic). Returns
    (balance dB: upper minus lower, upper cents, lower cents, interval cents minus the
    tempered interval)."""
    seg = m[int(a * sr):int(b * sr)]
    if len(seg) < int(0.03 * sr):
        return None
    w = seg * np.hanning(len(seg))
    n = 1 << int(np.ceil(np.log2(max(sr, len(seg)))))
    X = np.abs(np.fft.rfft(w, n)) ** 2
    fr = np.fft.rfftfreq(n, 1 / sr)

    def power(key):
        f0 = 440.0 * 2 ** ((key - 69) / 12)
        return sum(X[(fr > h * f0 * 0.985) & (fr < h * f0 * 1.015)].sum() for h in range(1, nh + 1))

    def cents(key):
        f0 = 440.0 * 2 ** ((key - 69) / 12)
        out = []
        for h in (2, 3):
            sel = np.nonzero((fr > h * f0 * 0.985) & (fr < h * f0 * 1.015))[0]
            if len(sel) < 3:
                continue
            i = sel[np.argmax(X[sel])]
            y0, y1, y2 = np.log(X[i - 1] + 1e-30), np.log(X[i] + 1e-30), np.log(X[i + 1] + 1e-30)
            den = y0 - 2 * y1 + y2
            off = 0.5 * (y0 - y2) / den if abs(den) > 1e-12 else 0.0
            f = (fr[i] + off * (fr[1] - fr[0])) / h
            out.append(1200 * np.log2(f / f0))
        return float(np.median(out)) if out else None

    ph, pl = power(hi_key), power(lo_key)
    ch, cl = cents(hi_key), cents(lo_key)
    iv = None if ch is None or cl is None else ch - cl
    return float(10 * np.log10((ph + 1e-30) / (pl + 1e-30))), ch, cl, iv


def crossing_beat(m, sr, a, b, hi_key, lo_key, nh=4):
    """A beat that alternates an upper note with its lower neighbour (hi lo hi lo) over
    [a, b) s: each note's harmonics 1-nh (power within +-1.5 % of each) followed in time
    (43 ms Hann, 5 ms hop); the frames where the lower note leads most (the top 30 % of
    lo - hi) are where the lower note is played. Returns dict:
      balance_db  the upper note's level there against the lower's (median, dB);
      ring_db     how far the upper note falls there under its own peak in the beat
                  (one string: it stops, 15-30 dB; two strings: it rings on, a few dB);
      hi_cents, lo_cents, interval_cents
                  each note's pitch from a Hann FFT over the whole beat (2nd and 3rd
                  harmonics' peaks), and the interval against the tempered one."""
    from scipy import signal as sg
    seg = m[int(a * sr):int(b * sr)]
    if len(seg) < int(0.15 * sr):
        return None
    n = 2048
    h = int(0.005 * sr)
    fr, _, z = sg.stft(seg, sr, nperseg=n, noverlap=n - h, boundary=None, padded=False)
    P = np.abs(z) ** 2

    def level(key):
        f0 = 440.0 * 2 ** ((key - 69) / 12)
        sel = np.zeros(len(fr), bool)
        for k_ in range(1, nh + 1):
            sel |= (fr > k_ * f0 * 0.985) & (fr < k_ * f0 * 1.015)
        return 10 * np.log10(P[sel].sum(axis=0) + 1e-20)

    hi, lo = level(hi_key), level(lo_key)
    d = lo - hi
    top = d >= np.percentile(d, 70)
    r = two_voice(m, sr, a, b, hi_key, lo_key, nh)
    return dict(balance_db=float(np.median(hi[top] - lo[top])),
                ring_db=float(hi.max() - np.median(hi[top])),
                hi_cents=r[1], lo_cents=r[2], interval_cents=r[3])
