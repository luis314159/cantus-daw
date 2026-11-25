"""The lab's features: every estimator documented, one registry (`FEATURES`), one `Note`.

A subject names the features that apply to its instrument (`"features": [...]`); each feature
says which kinds of note it reads (`sustained`: bowed, blown, sung — a steady part; `decaying`:
plucked, struck — no steady part) and returns a flat dict of scalars (floats) and vectors
(lists, with their axis in a `*_hz` / `*_n` key). What is not measurable on a note is left
out, never guessed. The note-level analyses are the Arcus study's (`notes.py`, validated on
synthetic notes); the others are new here and held by `tests/`.

| feature | kinds | what |
|---|---|---|
| loudness | all | BS.1770-4 integrated, momentary and short-term maxima, LRA, sample and true peak, crest |
| pitch | all | f0 (YIN, phase-refined; a decaying note's from the partials' fit), cents from the nominal note, the track's p10-p90 spread |
| harmonics | all | partials 1-20 dB re the strongest and re the fundamental (sustained: the steady window's median; decaying: each partial's peak) |
| inharmonicity | all | B from f_n = n f0 sqrt(1 + B n^2) fitted to the long FFT's peaks; sustained: also the partials' mode-locking deviation |
| centroid | all | the harmonic centroid (Hz, harmonic numbers) and the spectral centroid's trajectory after the onset (Hz at 20 ms ... 1 s, its slope in octaves per second, the note's energy-weighted mean) |
| noise | all | the noise between harmonics per third octave (HNR) and in the valid bands (sustained: the harmonic model's residual; decaying: the long FFT's between-partial bins) |
| attack | all | onset, 10-90 % rise, time to peak; sustained: rise to -3 dB of steady, build-up order, noise burst; decaying: the first 25 ms over 60-160 ms (burst) |
| t_steady | sustained | when the harmonic shape settles to the steady one (3 dB rms for 100 ms) |
| decay | all | decaying: each partial's T60 (and its first 0.3 s); sustained: the release's dB/s and T60 |
| band_decay | all | T60 per octave band 63 Hz-8 kHz from each band's envelope (membranes, bars, cymbals: no partials needed) |
| vibrato | sustained | rate, extent (sinusoid-equivalent cents), presence, AM per partial; cycle by cycle: peak-to-trough extent, rate and extent jitter, onset |
| stereo | all (2 ch) | left/right balance, side/mid energy, inter-channel correlation |
| room | all | the tail's Schroeder decay after the note ends (sustained) or from its peak (decaying): EDT, T20 |
"""
import numpy as np
from scipy import signal

from . import loudness as lmod
from .core import band_powers, band_edges, bh_window, rms_db
from .notes import (analyse_arco, analyse_pizz, _f0_steady, _track, detrend_at, onset_end, db)

KINDS = ("sustained", "decaying")
NP = 20  # partials reported


class Note:
    """One note: x (frames x channels, float64), its rate, its kind, the family's f0 range, its
    nominal MIDI note (or None), and the background before it (or None)."""

    def __init__(self, x, sr, kind, f_range, nominal=None, pre=None, hint=True):
        assert kind in KINDS, kind
        self.x = x if x.ndim == 2 else x[:, None]
        self.m = np.ascontiguousarray(self.x[:, 0])
        self.sr, self.kind, self.nominal, self.pre = int(sr), kind, nominal, pre
        # A known note narrows YIN's search to +-7 semitones of it (an octave error on a weak
        # fundamental — TinySOL's bass A1 read at 110 Hz — is then impossible; a note out of
        # tune by less than a fifth is still read as it is). `hint=False` keeps the family's.
        if nominal is not None and hint:
            f = 440.0 * 2 ** ((nominal - 69) / 12)
            f_range = (max(f_range[0], f * 2 ** (-7 / 12)) if f * 2 ** (7 / 12) > f_range[0] else f * 2 ** (-7 / 12),
                       f * 2 ** (7 / 12))
        self.f_range = f_range
        self._base = False

    def base(self):
        """The study's analysis of the note (sustained -> analyse_arco, decaying -> analyse_pizz)."""
        if self._base is False:
            ctx = {"fmin": self.f_range[0], "fmax": self.f_range[1], "pre": self.pre}
            f = analyse_arco if self.kind == "sustained" else analyse_pizz
            try:
                self._base = f(self.m, self.sr, ctx)
            except (ValueError, IndexError, np.linalg.LinAlgError):
                self._base = None
        return self._base

    def span(self):
        """(onset s, end s) by the study's rule: the first 2 ms frame over max(peak - 40,
        floor + 6) dB to the last over max(peak - 45, floor + 6)."""
        _, _, t_on, t_end, _ = onset_end(self.m, self.sr)
        return t_on, t_end


def _f(v):
    return None if v is None or not np.isfinite(v) else float(v)


def _vec(v, n=NP):
    return [None if (x is None or not np.isfinite(x)) else float(x) for x in list(v)[:n]]


# ---- loudness, pitch, harmonics, inharmonicity

def f_loudness(n):
    return lmod.measure(n.x, n.sr)


def f_pitch(n):
    b = n.base()
    if not b:
        return {}
    out = {"f0_hz": float(b["f0_hz"])}
    if n.nominal is not None:
        out["cents_from_nominal"] = float(1200 * np.log2(b["f0_hz"] / (440.0 * 2 ** ((n.nominal - 69) / 12))))
    if n.kind == "sustained":
        a, e = b["steady_s"]
        seg = n.m[int(a * n.sr):int(e * n.sr)]
        t, f, _ = _track(seg, n.sr, b["f0_hz"])
        if len(f) > 10:
            c = 1200 * np.log2(f / b["f0_hz"])
            out["spread_cents_p10_p90"] = float(np.percentile(c, 90) - np.percentile(c, 10))
    return out


def f_harmonics(n):
    b = n.base()
    if not b:
        return {}
    if n.kind == "sustained":
        return {"h_db_re_max": _vec(b["h_db_re_max"]), "h_db_re_f1": _vec(b["h_db_re_f1"]),
                "f1_re_max_db": float(b["f1_re_max_db"])}
    pk = b.get("h_peak_db_re_max") or []
    full = [None] * NP
    for k, v in zip(b["partials"], pk):
        if k <= NP:
            full[k - 1] = float(v)
    return {"h_db_re_max": full}


def fit_b(x, sr, a0, length, f_med, nmax=40, fmax=10000.0):
    """f0 and B from the long FFT's peaks of x[a0:a0+length] (Blackman-Harris, 8x zero-padded):
    each partial looked for at n f0 sqrt(1 + B n^2) within +-f0/4, fitted by least squares on
    f_n^2 = a n^2 + b n^4 once four are found (the study's pizzicato fit). (f0, B, partials)."""
    w = bh_window(length)
    nfft = 1 << int(np.ceil(np.log2(length * 8)))
    S = np.abs(np.fft.rfft(x[a0:a0 + length] * w, nfft))
    fr = np.fft.rfftfreq(nfft, 1 / sr)
    Sd = db(S)
    fn, ns, B, f0 = [], [], 0.0, f_med
    for k in range(1, nmax + 1):
        pred = k * f0 * np.sqrt(1 + B * k * k)
        if pred > min(fmax, 0.45 * sr):
            break
        band = (fr > pred - 0.25 * f_med) & (fr < pred + 0.25 * f_med)
        if not band.any():
            break
        i = np.nonzero(band)[0][np.argmax(S[band])]
        if Sd[i] < Sd.max() - 60 or i < 1 or i + 1 >= len(Sd):
            continue
        al, be, ga = Sd[i - 1], Sd[i], Sd[i + 1]
        den = al - 2 * be + ga
        off = 0.5 * (al - ga) / den if den != 0 else 0
        fn.append(fr[i] + off * (fr[1] - fr[0]))
        ns.append(k)
        if len(ns) >= 4:
            M = np.array([[q * q, q ** 4] for q in ns], float)
            sol = np.linalg.lstsq(M, np.array(fn) ** 2, rcond=None)[0]
            if sol[0] > 0:
                f0 = float(np.sqrt(sol[0]))
                B = float(max(sol[1] / sol[0], 0.0))
    return f0, B, ns


def f_inharmonicity(n):
    b = n.base()
    if not b:
        return {}
    if n.kind == "decaying":
        return {"B": float(b["B"]), "n_partials_fit": int(b["n_partials_fit"])}
    a, e = b["steady_s"]
    a0, length = int(a * n.sr), int((e - a) * n.sr)
    out = {}
    if length > int(8 * n.sr / b["f0_hz"]):
        _, B, ns = fit_b(n.m, n.sr, a0, length, b["f0_hz"])
        out.update({"B": B, "n_partials_fit": len(ns)})
    dev = [abs(v) for v in b.get("partial_dev_cents") or [] if v is not None]
    if dev:
        out["mode_lock_dev_cents_median"] = float(np.median(dev))
    return out


# ---- centroid, noise

def centroid_track(m, sr, nfft=2048, hop=256, fmin=20.0, fmax=16000.0):
    """The spectral centroid per frame (Hann, power-weighted over fmin..min(fmax, 0.45 sr)):
    (times s at frame centres, centroid Hz, frame power dB)."""
    if len(m) < nfft:
        return np.array([]), np.array([]), np.array([])
    f, t, Z = signal.stft(m, sr, window="hann", nperseg=nfft, noverlap=nfft - hop, boundary=None, padded=False)
    P = np.abs(Z) ** 2
    sel = (f >= fmin) & (f <= min(fmax, 0.45 * sr))
    p = P[sel]
    tot = p.sum(axis=0)
    c = (f[sel][:, None] * p).sum(axis=0) / np.maximum(tot, 1e-30)
    return t, c, 10 * np.log10(tot + 1e-30)


CENTROID_AT = (0.02, 0.05, 0.1, 0.2, 0.5, 1.0)


def f_centroid(n):
    out = {}
    b = n.base()
    if b and n.kind == "sustained":
        out["harmonic_centroid_hz"] = float(b["centroid_hz"])
        out["harmonic_centroid_n"] = float(b["centroid_harm"])
    t_on, t_end = n.span()
    t, c, p = centroid_track(n.m, n.sr)
    if not len(t):
        return out
    live = (p > p.max() - 40) & (t >= t_on) & (t <= t_end)
    if live.sum() < 3:
        return out
    w = 10 ** (p[live] / 10)
    out["mean_hz"] = float(np.sum(c[live] * w) / np.sum(w))
    out["at_s"] = list(CENTROID_AT)
    out["at_hz"] = [float(np.interp(t_on + d, t[live], c[live])) if t_on + d <= t_end else None for d in CENTROID_AT]
    first = live & (t <= t_on + 1.0)
    if first.sum() >= 4 and np.ptp(t[first]) > 0.1:
        out["slope_oct_per_s"] = float(np.polyfit(t[first], np.log2(np.maximum(c[first], 1.0)), 1)[0])
    return out


def between_partials(m, sr, a0, length, f0, B, per_oct=3, half_width=0.2):
    """HNR per third octave from a long FFT: bins within +-half_width f0 of a predicted
    partial n f0 sqrt(1 + B n^2) are the partials', the rest the noise; each band's noise
    density, measured on its between-partial bins, is taken out of its partial bins too.
    (bands Hz, hnr dB per band or None, valid mask)."""
    w = np.hanning(length)
    X = np.fft.rfft(m[a0:a0 + length] * w)
    P = np.abs(X) ** 2
    fr = np.fft.rfftfreq(length, 1 / sr)
    near = np.zeros(len(fr), bool)
    k = 1
    while True:
        fk = k * f0 * np.sqrt(1 + B * k * k)
        if fk > 0.47 * sr:
            break
        near |= np.abs(fr - fk) <= half_width * f0
        k += 1
    fc, lo, hi = band_edges(25.0, min(16000.0, 0.45 * sr), per_oct)
    hnr, valid = [], []
    for c, a, e in zip(fc, lo, hi):
        sel = (fr >= a) & (fr < e)
        nb, hb = sel & ~near, sel & near
        if nb.sum() < 3 or hb.sum() < 1:
            hnr.append(None)
            valid.append(False)
            continue
        dens = P[nb].mean()
        h = P[hb].sum() - dens * hb.sum()
        nz = dens * sel.sum()
        hnr.append(float(10 * np.log10(max(h, 1e-30) / max(nz, 1e-30))))
        valid.append(bool(c >= 300 and c >= 2 * f0 and c <= 0.42 * sr))
    return [float(v) for v in fc], hnr, valid


def f_noise(n):
    b = n.base()
    if not b:
        return {}
    if n.kind == "sustained":
        z = b["noise"]
        # Only the valid bands (>= 300 Hz, >= 5 f0, <= 0.42 fs: under 5 f0 the vibrato's AM
        # leaks into the residual — the study's rule); the others are left out (None).
        hnr = [v if ok else None for v, ok in zip(_vec(z["hnr_band_db"], 99), z["band_valid"])]
        return {"bands_hz": z["bands_hz"], "hnr_band_db": hnr,
                "res_total_db_re_harm": float(z["res_total_db_re_harm"]),
                "res_valid_db_re_harm": _f(z.get("res_valid_db_re_harm"))}
    a0 = int((b["t_on_s"] + 0.05) * n.sr)
    length = min(int(0.5 * n.sr), len(n.m) - a0)
    if length < int(8 * n.sr / b["f0_hz"]):
        return {}
    fc, hnr, valid = between_partials(n.m, n.sr, a0, length, b["f0_hz"], b["B"])
    vv = [h for h, v in zip(hnr, valid) if v and h is not None]
    return {"bands_hz": fc, "hnr_band_db": [h if v else None for h, v in zip(hnr, valid)],
            "hnr_valid_median_db": float(np.median(vv)) if vv else None}


# ---- attack, steady, decay

def envelope_db(m, sr, win_s=0.005, hop_s=0.001):
    return rms_db(m, sr, win_s, hop_s)


def f_attack(n):
    b = n.base()
    t_on, t_end = n.span()
    # A 5 ms RMS, or three periods where the pitch is known (a shorter window ripples by
    # +-0.5 dB on a low note and crosses the 90 % line early); a pluck's peak needs the 5 ms.
    win = max(0.005, 3.0 / b["f0_hz"]) if b and n.kind == "sustained" else 0.005
    et, ed = envelope_db(n.m, n.sr, win)
    sel = (et >= t_on - 0.01) & (et <= min(t_end, t_on + 1.5))
    if sel.sum() < 5:
        return {}
    tt, ee = et[sel], ed[sel]
    if n.kind == "sustained" and b:
        top = float(b["level_db_steady"])
    else:
        top = float(ee.max())
    out = {"onset_s": float(t_on), "t_peak_ms": float(1000 * (tt[np.argmax(ee)] - t_on))}
    # 10 % and 90 % of the top's amplitude: -20 dB and -0.92 dB
    i10 = np.nonzero(ee >= top - 20.0)[0]
    i90 = np.nonzero(ee >= top + 20 * np.log10(0.9))[0]
    if len(i10) and len(i90) and i90[0] >= i10[0]:
        out["rise_10_90_ms"] = float(1000 * (tt[i90[0]] - tt[i10[0]]))
    if n.kind == "sustained" and b and b.get("attack"):
        a = b["attack"]
        out.update({"rise_3db_ms": None if a["t_rise_3db_s"] is None else float(1000 * a["t_rise_3db_s"]),
                    "build_order_rho": _f(a["build_order_rho"]),
                    "noise_burst_db_over_steady": _f(a["noise_burst_db_over_steady"])})
    s0 = int(t_on * n.sr)
    early, late = n.m[s0:s0 + int(0.025 * n.sr)], n.m[s0 + int(0.06 * n.sr):s0 + int(0.16 * n.sr)]
    if len(late) > int(0.05 * n.sr) and len(early) > 10:
        out["burst_db"] = float(10 * np.log10((np.mean(early ** 2) + 1e-30) / (np.mean(late ** 2) + 1e-30)))
    return out


def f_t_steady(n):
    b = n.base()
    if not b or n.kind != "sustained" or not b.get("attack"):
        return {}
    v = b["attack"]["t_steady_s"]
    return {} if v is None else {"t_steady_s": float(v)}


def f_decay(n):
    b = n.base()
    if not b:
        return {}
    if n.kind == "decaying":
        t60 = [None] * NP
        early = [None] * NP
        for k, v, e in zip(b["partials"], b["t60_s"], b["t60_early_s"]):
            if k <= NP:
                t60[k - 1], early[k - 1] = _f(v), _f(e)
        return {"t60_s": t60, "t60_early_s": early, "t60_f1_s": t60[0]}
    r = b.get("release") or {}
    if not r.get("released") or r.get("truncated") or r.get("decay_db_s") is None:
        return {}
    return {"release_db_per_s": float(r["decay_db_s"]), "release_t60_s": _f(r.get("t60_s"))}


def release_start(n):
    """A sustained note's release: where its level first falls 6 dB under the steady level
    after the steady window (the study's `t_release_s`); None if it never does."""
    b = n.base()
    r = (b or {}).get("release") or {}
    return r.get("t_release_s") if r.get("released") else None


OCTAVES = (63.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0)


def band_t60(y, sr, t_from, span_db=(5.0, 35.0), min_range=10.0):
    """T60 of one band's envelope: from its peak after t_from, a straight line fitted to the
    dB envelope (10 ms RMS) from peak - 5 dB down to peak - 35 dB or floor + 10, whichever is
    higher; None when less than `min_range` dB is available."""
    t, e = rms_db(y, sr, 0.01, 0.005)
    sel = t >= t_from
    if sel.sum() < 10:
        return None
    t, e = t[sel], e[sel]
    ip = int(np.argmax(e[: max(int(0.5 / 0.005), 2)]))
    floor = np.percentile(e, 5)
    hi, lo = e[ip] - span_db[0], max(e[ip] - span_db[1], floor + 10.0)
    if hi - lo < min_range:
        return None
    after = np.arange(len(e)) > ip
    i_hi = np.nonzero(after & (e <= hi))[0]
    i_lo = np.nonzero(after & (e <= lo))[0]
    if not len(i_hi) or not len(i_lo) or i_lo[0] - i_hi[0] < 5:
        return None
    s = slice(i_hi[0], i_lo[0] + 1)
    sl = np.polyfit(t[s], e[s], 1)[0]
    return float(-60.0 / sl) if sl < 0 else None


def f_band_decay(n):
    t_on, t_end = n.span()
    t_from = release_start(n) if n.kind == "sustained" else t_on
    if t_from is None:
        return {}
    out = []
    for fc in OCTAVES:
        lo, hi = fc / np.sqrt(2), min(fc * np.sqrt(2), 0.45 * n.sr)
        if lo >= hi:
            out.append(None)
            continue
        sos = signal.butter(4, [lo, hi], btype="bandpass", fs=n.sr, output="sos")
        out.append(band_t60(signal.sosfilt(sos, n.m), n.sr, t_from))
    return {"bands_hz": list(OCTAVES), "t60_s": out}


# ---- vibrato (the study's spectral estimate, and the pass-5 cycle-by-cycle reading)

def _extrema(v, dt, rate):
    gap = max(int(0.33 / rate / dt), 2)
    out, i = [], 1
    while i < len(v) - 1:
        lo, hi = max(i - gap, 0), min(i + gap + 1, len(v))
        if v[i] == v[lo:hi].max() and v[i] > 0:
            kind = 1
        elif v[i] == v[lo:hi].min() and v[i] < 0:
            kind = -1
        else:
            i += 1
            continue
        if out and out[-1][1] == kind:
            if (kind == 1 and v[i] > v[out[-1][0]]) or (kind == -1 and v[i] < v[out[-1][0]]):
                out[-1] = (i, kind)
        else:
            out.append((i, kind))
        i += gap
    return out


def vibrato_cycles(t, v, rate):
    """Each cycle peak to peak: (centre s, period s, extent peak to trough cents)
    (`tools/arcus/pass5/vibrato.py`'s cutting at alternate extrema)."""
    ex = _extrema(v, np.median(np.diff(t)), rate)
    peaks = [k for k, (i, s) in enumerate(ex) if s == 1]
    out = []
    for a, b in zip(peaks, peaks[1:]):
        if b - a != 2:
            continue
        i0, i1, i2 = ex[a][0], ex[a + 1][0], ex[b][0]
        per = t[i2] - t[i0]
        if 0.08 < per < 0.4:
            out.append((0.5 * (t[i0] + t[i2]), per, 0.5 * (v[i0] + v[i2]) - v[i1]))
    return out


def f_vibrato(n):
    b = n.base()
    if not b or n.kind != "sustained":
        return {}
    out = {}
    v = b.get("vibrato")
    if v:
        out = {"rate_hz": float(v["rate_hz"]), "extent_cents": float(v["extent_cents"]),
               "present": 1.0 if v["present"] else 0.0}
        if v.get("am_db"):
            out["am_db"] = _vec(v["am_db"], 10)
    t_on, t_end = b["t_on_s"], b["t_end_s"]
    seg = n.m[int(t_on * n.sr):int(t_end * n.sr)]
    t, f, _ = _track(seg, n.sr, b["f0_hz"])
    if len(f) < 60:
        return out
    c = 1200 * np.log2(f / b["f0_hz"])
    dt = float(np.median(np.diff(t)))
    rate = out.get("rate_hz", 6.0)
    cy = vibrato_cycles(t, detrend_at(c, dt, rate), rate)
    if len(cy) >= 3:
        per = np.array([q[1] for q in cy])
        ext = np.array([q[2] for q in cy])
        full = float(np.median(np.sort(ext)[len(ext) // 2:]))
        out["extent_pp_cents"] = full
        out["cycles"] = float(len(cy))
        if len(per) >= 3:
            out["rate_jitter"] = float(np.mean(np.abs(np.diff(per))) / np.mean(per))
            out["extent_jitter"] = float(np.mean(np.abs(np.diff(ext))) / max(np.mean(ext), 1e-9))
        ct = np.array([q[0] for q in cy])
        k25 = np.nonzero((ext >= 0.25 * full) & (ct >= t[0] + 0.1))[0]
        if len(k25) and full >= 8.0:
            out["onset_s"] = float(ct[k25[0]] - 0.5 * per[k25[0]])
    return out


# ---- stereo, room

def f_stereo(n):
    if n.x.shape[1] < 2:
        return {}
    t_on, t_end = n.span()
    s = slice(int(t_on * n.sr), int(max(t_end, t_on + 0.05) * n.sr))
    l, r = n.x[s, 0], n.x[s, 1]
    el, er = np.sum(l * l), np.sum(r * r)
    mid, side = 0.5 * (l + r), 0.5 * (l - r)
    corr = np.sum(l * r) / np.sqrt(max(el * er, 1e-30))
    return {"balance_lr_db": float(10 * np.log10((el + 1e-30) / (er + 1e-30))),
            "side_mid_db": float(10 * np.log10((np.sum(side ** 2) + 1e-30) / (np.sum(mid ** 2) + 1e-30))),
            "correlation": float(corr)}


def schroeder(y, sr, t_from):
    """Schroeder's backward-integrated energy decay (dB re its start) from t_from s, the
    noise floor's share taken out first (Lundeby's simple form: the last 10 % as floor)."""
    seg = y[int(t_from * sr):]
    if len(seg) < int(0.1 * sr):
        return None, None
    e = seg * seg
    floor = np.mean(e[-max(len(e) // 10, 1):])
    e = np.maximum(e - floor, 0.0)
    edc = np.cumsum(e[::-1])[::-1]
    if edc[0] <= 0:
        return None, None
    return np.arange(len(seg)) / sr, 10 * np.log10(np.maximum(edc / edc[0], 1e-30))


def _decay_time(t, d, lo, hi):
    sel = (d <= lo) & (d >= hi)
    if sel.sum() < 10:
        return None
    sl = np.polyfit(t[sel], d[sel], 1)[0]
    return float(-60.0 / sl) if sl < 0 else None


def f_room(n):
    t_on, t_end = n.span()
    if n.kind == "sustained":
        t_from = release_start(n)
        if t_from is None:
            return {}
    else:
        et, ed = envelope_db(n.m, n.sr)
        sel = et >= t_on
        t_from = float(et[sel][np.argmax(ed[sel])]) if sel.any() else t_on
    t, d = schroeder(n.m, n.sr, t_from)
    if t is None:
        return {}
    out = {"edt_s": _decay_time(t, d, 0.0, -10.0), "t20_s": _decay_time(t, d, -5.0, -25.0)}
    return {k: v for k, v in out.items() if v is not None}


# ---- the registry

FEATURES = {
    "loudness": (KINDS, f_loudness),
    "pitch": (KINDS, f_pitch),
    "harmonics": (KINDS, f_harmonics),
    "inharmonicity": (KINDS, f_inharmonicity),
    "centroid": (KINDS, f_centroid),
    "noise": (KINDS, f_noise),
    "attack": (KINDS, f_attack),
    "t_steady": (("sustained",), f_t_steady),
    "decay": (KINDS, f_decay),
    "band_decay": (KINDS, f_band_decay),
    "vibrato": (("sustained",), f_vibrato),
    "stereo": (KINDS, f_stereo),
    "room": (KINDS, f_room),
}


def analyse(note, names):
    """{feature: {key: value}} for the named features that read this note's kind; a feature
    that fails on a note is left out of it (and said in `_errors`)."""
    out, errors = {}, {}
    for name in names:
        kinds, fn = FEATURES[name]
        if note.kind not in kinds:
            continue
        try:
            r = fn(note)
        except (ValueError, IndexError, ZeroDivisionError, np.linalg.LinAlgError) as e:
            errors[name] = f"{type(e).__name__}: {e}"
            continue
        if r:
            out[name] = r
    if errors:
        out["_errors"] = errors
    return out
