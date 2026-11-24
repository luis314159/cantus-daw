"""bowlib_feat.py - the per-note features of the Arcus reference study (Cantus, round 42).

Usage (a library): from bowlib_feat import analyse_arco, analyse_pizz, analyse_phrase
Each function takes a mono note (numpy float64), its sample rate and a dict `ctx` with
`fmin`, `fmax` (the family's f0 range) and optionally `pre` (a stretch of the recording's
background before the note, for the floor), and returns a JSON-ready dict.
Definitions of every feature are in refdb/README.md; validation in validate_synth.py.
"""
import numpy as np
from bowlib_core import (yin_track, refine_track, phase_of, project, resynth, rms_db,
                         band_powers, band_edges, median_filter)

NH = 40


def db(v):
    return 20 * np.log10(np.maximum(np.abs(v), 1e-12))


def _f0_steady(x, sr, fmin, fmax):
    t, f, ap = yin_track(x, sr, fmin, fmax, hop_s=0.01)
    if len(f) == 0:
        return None
    _, e = rms_db(x, sr, 0.04, 0.01)
    e = np.interp(t, np.arange(len(e)) * 0.01 + 0.02, e)
    good = (ap < 0.2) & (e > e.max() - 20)
    if good.sum() < 5:
        good = ap < np.percentile(ap, 30)
    return float(np.median(f[good]))


def _track(x, sr, f_med, hop_s=0.0025):
    lo, hi = f_med * 2 ** (-4 / 12), f_med * 2 ** (4 / 12)
    t, f, ap = yin_track(x, sr, lo, hi, hop_s=hop_s, win_periods=2, f_hint=f_med)
    if len(f) < 8:
        return t, f, ap
    bad = (ap > 0.3) | (np.abs(1200 * np.log2(f / f_med)) > 350)
    if bad.any() and (~bad).sum() > 4:
        f = f.copy()
        f[bad] = np.interp(t[bad], t[~bad], f[~bad])
    f = median_filter(f, 5)
    return t, f, ap


def steady_window(x, sr, t_on, t_end):
    """The steady part: after the attack (max(0.25 s, 20 % of the note)) to before the
    release (the last 15 %), at most 1.6 s, centred."""
    d = t_end - t_on
    a = t_on + max(0.25, 0.2 * d)
    b = t_end - max(0.1, 0.15 * d)
    if b - a < 0.15:
        a, b = t_on + 0.35 * d, t_on + 0.85 * d
    if b - a > 1.6:
        m = 0.5 * (a + b)
        a, b = m - 0.8, m + 0.8
    return a, b


def detrend_at(v, dt, rate):
    """Remove the slow drift with a moving average two vibrato periods long: its response is
    zero at the rate and its harmonics, so the vibrato itself is untouched."""
    k = max(int(round(2.0 / rate / dt)), 3)
    if len(v) <= k + 2:
        return v - v.mean()
    ker = np.ones(k) / k
    tr = np.convolve(np.pad(v, (k // 2, k - 1 - k // 2), mode="edge"), ker, mode="valid")
    return v - tr


def vibrato_stats(t, f, f_med, a_tracks=None, tc=None, win_s=None):
    """Rate (Hz), extent (+-cents, sinusoid-equivalent, corrected for YIN's 2-period window),
    and each partial's AM depth (+-dB at the rate) and its phase against the FM."""
    if len(t) < 40:
        return None
    dt = np.median(np.diff(t))
    c = 1200 * np.log2(f / f_med)
    tn = (t - t.mean()) / max(np.ptp(t), 1e-9)
    v0 = c - np.polyval(np.polyfit(tn, c, 2), tn)
    n0 = len(v0)
    sp0 = np.abs(np.fft.rfft(v0 * np.hanning(n0), 1 << int(np.ceil(np.log2(n0 * 8)))))
    fr0 = np.fft.rfftfreq(1 << int(np.ceil(np.log2(n0 * 8))), dt)
    b0 = (fr0 >= 3.0) & (fr0 <= 9.0)
    r0 = float(fr0[b0][np.argmax(sp0[b0])]) if b0.any() else 6.0
    v = detrend_at(c, dt, r0)
    n = len(v)
    win = np.hanning(n)
    nfft = 1 << int(np.ceil(np.log2(n * 8)))
    sp = np.abs(np.fft.rfft(v * win, nfft))
    fr = np.fft.rfftfreq(nfft, dt)
    band = (fr >= 3.0) & (fr <= 9.0)
    if not band.any():
        return None
    i = np.nonzero(band)[0][np.argmax(sp[band])]
    rate = float(fr[i])
    ref = np.exp(-2j * np.pi * rate * t)
    fm = 2 * np.sum(v * ref * win) / np.sum(win)
    ext = abs(fm)
    T = win_s or 2.0 / f_med
    corr = np.sinc(rate * T)  # YIN integrates over 2 periods
    ext_c = ext / corr if corr > 0.3 else ext
    prom = sp[i] / (np.median(sp[(fr > 0.5) & (fr < 20)]) + 1e-12)
    out = {"rate_hz": rate, "extent_cents": float(ext_c), "prominence": float(prom),
           "present": bool(ext_c > 4.0 and prom > 6.0 and 3.5 <= rate <= 8.5)}
    if a_tracks is not None and tc is not None and len(tc) > 20:
        am, ph = [], []
        fm_c = np.interp(tc, t, v)
        ref2 = np.exp(-2j * np.pi * rate * tc)
        w2 = np.hanning(len(tc))
        fm2 = 2 * np.sum(fm_c * ref2 * w2) / np.sum(w2)
        for row in a_tracks:
            r = detrend_at(row, np.median(np.diff(tc)), rate)
            z = 2 * np.sum(r * ref2 * w2) / np.sum(w2)
            am.append(float(abs(z)))
            ph.append(float(np.angle(z / fm2)) if abs(fm2) > 0 else 0.0)
        out["am_db"] = am
        out["am_phase_rad"] = ph
    return out


def harmonic_part(x, sr, t, f, f_med, a, b, nh_res=None, periods=6.0):
    """Project the steady window [a, b] s; returns centres (s), c[n, k], residual, h."""
    s0, s1 = int(a * sr), int(b * sr)
    seg = x[s0:s1]
    tt = t - a
    ff = refine_track(seg, sr, tt, f, f_med, periods=periods)
    phi, _ = phase_of(tt, ff, len(seg), sr)
    nh = nh_res or max(1, int(min(16000.0, 0.47 * sr) / f_med))
    cen, c = project(seg, phi, sr, f_med, nh, periods=periods)
    h = resynth(len(seg), phi, cen, c)
    valid = slice(cen[0], cen[-1]) if len(cen) else slice(0, 0)
    return cen / sr + a, c, seg[valid] - h[valid], h[valid], ff, tt + a


def spectrum_feats(c, f_med, sr):
    amps = np.median(np.abs(c), axis=1)
    nh = len(amps)
    a_db = db(amps)
    ref = a_db[0]
    top = a_db.max()
    k = min(NH, nh)
    fr = f_med * np.arange(1, nh + 1)
    p = amps ** 2
    lim = fr <= min(10000, 0.45 * sr)
    cent = float(np.sum(fr[lim] * p[lim]) / np.sum(p[lim]))
    return {"h_db_re_f1": [float(v) for v in a_db[:k] - ref],
            "h_db_re_max": [float(v) for v in a_db[:k] - top],
            "h_db_all_re_max": [float(v) for v in a_db - top],
            "centroid_hz": cent, "centroid_harm": cent / f_med,
            "f1_re_max_db": float(ref - top)}


ABSORB = 0.54  # share of f0 per harmonic that the projection takes from noise (exp3, validation)


def noise_feats(res, h, sr, f_med, pre=None):
    fc, pr = band_powers(res, sr, 3)
    _, ph = band_powers(h, sr, 3)
    lo, hi = fc * 2 ** (-1 / 6), fc * 2 ** (1 / 6)
    nh_max = int(min(16000.0, 0.47 * sr) / f_med)
    n_in = np.array([max(0, min(int(np.floor(b / f_med)), nh_max) - int(np.ceil(a / f_med)) + 1) for a, b in zip(lo, hi)])
    frac = np.minimum(ABSORB * f_med * n_in / (hi - lo), 0.9)
    pr = pr / (1 - frac)
    tot_h = ph.sum()
    valid = (fc >= 300) & (fc >= 5 * f_med) & (fc <= 0.42 * sr)
    out = {"bands_hz": [float(v) for v in fc],
           "res_band_db_re_harm_total": [float(v) for v in 10 * np.log10(pr / tot_h + 1e-20)],
           "hnr_band_db": [float(v) for v in 10 * np.log10((ph + 1e-20) / (pr + 1e-20))],
           "res_total_db_re_harm": float(10 * np.log10(pr.sum() / tot_h + 1e-20)),
           "band_valid": [bool(v) for v in valid],
           "res_valid_db_re_harm": float(10 * np.log10(pr[valid].sum() / tot_h + 1e-20)) if valid.any() else None}
    if pre is not None and len(pre) > int(0.08 * sr):
        _, pb = band_powers(pre, sr, 3)
        out["floor_band_db_re_harm_total"] = [float(v) for v in 10 * np.log10(pb / tot_h + 1e-20)]
        out["res_over_floor_db"] = [float(v) for v in 10 * np.log10((pr + 1e-20) / (pb + 1e-20))]
    return out
