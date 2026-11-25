"""One note's analysis, ported unchanged in method from the Arcus reference study
(`tools/arcus/study/bowlib_feat.py` and `bowlib_note.py`, round 42; the definitions are its
refdb README's, the measured accuracy its `validate_synth.py`'s). The study named the two
analyses for the bow; they are instrument-agnostic and the lab calls them by what they need:

- `analyse_arco` -> a **sustained** note (bowed, blown, sung): the steady window, the f0 track,
  the phase-tracked harmonics, the residual's noise per band, the vibrato and its AM per
  partial, the attack (t_steady, rise, build-up order, noise burst), the release.
- `analyse_pizz` -> a **decaying** note (plucked, struck): f0 and inharmonicity B from the
  long FFT's peaks, each partial's T60 (and its first 0.3 s), the partials' peak levels, the
  thump (the first 25 ms over 60-160 ms per third octave), the LTAS.

`ctx`: {"fmin", "fmax"} the family's f0 range, and optionally "pre" (background before the
note, for the floor). Each returns a JSON-ready dict, or None when the note cannot be read.
"""
import numpy as np
from scipy.stats import spearmanr

from .core import (yin_track, refine_track, phase_of, project, resynth, rms_db, band_powers,
                   band_edges, median_filter, bh_window)

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


def onset_end(x, sr):
    t, e = rms_db(x, sr, 0.01, 0.002)
    peak = np.percentile(e, 99)
    floor = np.percentile(e, 3)
    on_i = int(np.argmax(e > max(peak - 40, floor + 6)))
    above = np.nonzero(e > max(peak - 45, floor + 6))[0]
    end_i = int(above[-1]) if len(above) else len(e) - 1
    return t, e, float(t[on_i]), float(t[end_i]), float(floor)


def ltas(x, sr, per_oct=6):
    fc, p = band_powers(x, sr, per_oct, 25.0, 16000.0)
    return [float(v) for v in fc], [float(v) for v in 10 * np.log10(p / p.sum() + 1e-20)]


def attack_feats(x, sr, t_on, f_med, tmpl_db, L_s, res_steady_db, e_t, e_db):
    a = max(t_on - 0.02, 0.0)
    b = min(t_on + 1.0, len(x) / sr)
    s0, s1 = int(a * sr), int(b * sr)
    seg = x[s0:s1]
    nh = len(tmpl_db)
    phi = np.arange(len(seg)) * f_med / sr
    cen, c = project(seg, phi, sr, f_med, nh, periods=4.0, hop_s=0.002, window="hann")
    if c.shape[1] < 10:
        return None
    tc = cen / sr + a - t_on
    A = db(np.abs(c))
    use = tmpl_db > tmpl_db.max() - 30
    dlt = A[use] - tmpl_db[use, None]
    lvl = np.mean(dlt, axis=0)
    dist = np.sqrt(np.mean((dlt - lvl[None, :]) ** 2, axis=0))
    ok = (dist < 3.0) & (lvl > -6.0)
    t_steady = None
    hold = int(0.1 / 0.002)
    for k in range(len(ok)):
        if tc[k] >= 0 and ok[k:k + hold].all() and k + hold <= len(ok):
            t_steady = float(tc[k])
            break
    sel = (e_t >= t_on) & (e_t <= t_on + 1.0)
    rise = e_t[sel][np.argmax(e_db[sel] > L_s - 3)] - t_on if (e_db[sel] > L_s - 3).any() else None
    build = []
    for n in range(min(8, nh)):
        if tmpl_db[n] < tmpl_db.max() - 30:
            build.append(None)
            continue
        hit = np.nonzero((A[n] > tmpl_db[n] - 6) & (tc >= -0.005))[0]
        build.append(float(tc[hit[0]]) if len(hit) else None)
    pairs = [(n + 1, v) for n, v in enumerate(build) if v is not None]
    rho = None
    if len(pairs) >= 4:
        tv = [p[1] for p in pairs]
        rho = float(spearmanr([p[0] for p in pairs], tv).statistic) if np.ptp(tv) > 0 else 0.0
    # the noise burst: residual of a constant-f0 harmonic model in the first 50 ms
    h = resynth(len(seg), phi, cen, c)
    i0, i1 = int((t_on - a) * sr), int((t_on - a + 0.05) * sr)
    i0 = max(i0, cen[0])
    i1 = min(i1, cen[-1])
    burst = None
    if i1 - i0 > int(0.02 * sr):
        r = seg[i0:i1] - h[i0:i1]
        burst = float(10 * np.log10(np.sum(r * r) / (np.sum(h[i0:i1] ** 2) + 1e-20) + 1e-20) - res_steady_db)
    return {"t_steady_s": t_steady, "t_rise_3db_s": float(rise) if rise is not None else None,
            "build_up_s": build, "build_order_rho": rho, "noise_burst_db_over_steady": burst,
            "window_ms": 4000.0 / f_med}


def release_feats(e_t, e_db, t_end_steady, L_s, floor, sr, x, f_med):
    sel = e_t > t_end_steady
    tt, ee = e_t[sel], e_db[sel]
    if len(tt) < 20:
        return None
    i6 = np.nonzero(ee < L_s - 6)[0]
    if not len(i6):
        return {"released": False}
    t_rel = tt[i6[0]]
    lo = max(L_s - 40, floor + 8)
    fit = (tt >= t_rel) & (ee <= L_s - 6) & (ee >= lo)
    hit = np.nonzero(ee < lo)[0]
    truncated = not len(hit) or (len(x) / sr - tt[hit[0]] < 0.02)
    if fit.sum() < 8:
        return {"released": True, "t_release_s": float(t_rel), "decay_db_s": None, "truncated": bool(truncated)}
    sl = np.polyfit(tt[fit], ee[fit], 1)[0]
    return {"released": True, "t_release_s": float(t_rel), "decay_db_s": float(-sl),
            "t60_s": float(60 / -sl) if sl < 0 else None, "range_db": float(L_s - 6 - lo),
            "truncated": bool(truncated)}


def analyse_arco(x, sr, ctx):
    e_t, e_db, t_on, t_end, floor = onset_end(x, sr)
    if t_end - t_on < 0.3:
        return None
    f_med = _f0_steady(x[int(t_on * sr):int(t_end * sr)], sr, ctx["fmin"], ctx["fmax"])
    if not f_med:
        return None
    a, b = steady_window(x, sr, t_on, t_end)
    s0, s1 = int((a - 0.05) * sr), int((b + 0.05) * sr)
    seg = x[max(s0, 0):s1]
    t, f, ap = _track(seg, sr, f_med)
    if len(f) < 20:
        return None
    t = t + max(s0, 0) / sr
    f_med = float(np.median(f[(t >= a) & (t <= b)]))
    cen_t, c, res, h, ff, tt = harmonic_part(x, sr, t, f, f_med, a, b)
    if c.shape[1] < 4:
        return None
    sel = (e_t >= a) & (e_t <= b)
    L_s = float(np.median(e_db[sel]))
    out = {"f0_hz": f_med, "t_on_s": t_on, "t_end_s": t_end, "steady_s": [a, b],
           "level_db_steady": L_s, "floor_db": floor}
    out.update(spectrum_feats(c, f_med, sr))
    pre = ctx.get("pre")
    if pre is None and t_on > 0.15:
        pre = x[: int((t_on - 0.03) * sr)]
    out["noise"] = noise_feats(res, h, sr, f_med, pre)
    nam = min(20, c.shape[0])
    vib = vibrato_stats(t[(t >= a) & (t <= b)], f[(t >= a) & (t <= b)], f_med,
                        db(np.abs(c[:nam])), cen_t)
    out["vibrato"] = vib
    # mode locking: each partial's frequency against n*f0 from its phase drift (cents)
    dev = []
    strong = db(np.median(np.abs(c[:20]), axis=1))
    for n in range(1, min(20, c.shape[0]) + 1):
        if strong[n - 1] < strong.max() - 30:
            dev.append(None)
            continue
        th = np.unwrap(np.angle(c[n - 1]))
        sl = np.polyfit(cen_t, th, 1)[0] / (2 * np.pi)
        dev.append(float(1200 * np.log2(1 + sl / (n * f_med))))
    out["partial_dev_cents"] = dev
    out["ltas_fc"], out["ltas_db"] = ltas(x[int(t_on * sr):int(t_end * sr)], sr)
    tmpl = db(np.median(np.abs(c[:12]), axis=1))
    out["attack"] = attack_feats(x, sr, t_on, f_med, tmpl, L_s, out["noise"]["res_total_db_re_harm"], e_t, e_db)
    out["release"] = release_feats(e_t, e_db, b, L_s, floor, sr, x, f_med)
    return out


def analyse_pizz(x, sr, ctx):
    e_t, e_db, t_on, t_end, floor = onset_end(x, sr)
    if t_end - t_on < 0.15:
        return None
    s_on = int(t_on * sr)
    f_med = _f0_steady(x[s_on + int(0.03 * sr): s_on + int(0.6 * sr)], sr, ctx["fmin"], ctx["fmax"])
    if not f_med:
        return None
    # partial frequencies from a long FFT after the attack -> f0 and B (f_n^2 = a n^2 + b n^4)
    a0 = s_on + int(0.03 * sr)
    L = min(int(0.6 * sr), len(x) - a0)
    if L < int(8 * sr / f_med):
        return None
    w = bh_window(L)
    nfft = 1 << int(np.ceil(np.log2(L * 8)))
    S = np.abs(np.fft.rfft(x[a0:a0 + L] * w, nfft))
    fr = np.fft.rfftfreq(nfft, 1 / sr)
    Sd = db(S)
    fn, ns, B, f0 = [], [], 0.0, f_med
    for n in range(1, 41):
        pred = n * f0 * np.sqrt(1 + B * n * n)
        if pred > min(10000, 0.45 * sr):
            break
        band = (fr > pred - 0.25 * f_med) & (fr < pred + 0.25 * f_med)
        if not band.any():
            break
        i = np.nonzero(band)[0][np.argmax(S[band])]
        if Sd[i] < Sd.max() - 60:
            continue
        al, be, ga = Sd[i - 1], Sd[i], Sd[i + 1]
        off = 0.5 * (al - ga) / (al - 2 * be + ga) if (al - 2 * be + ga) != 0 else 0
        fn.append(fr[i] + off * (fr[1] - fr[0]))
        ns.append(n)
        if len(ns) >= 4:
            M = np.array([[k * k, k ** 4] for k in ns], float)
            sol = np.linalg.lstsq(M, np.array(fn) ** 2, rcond=None)[0]
            if sol[0] > 0:
                f0 = float(np.sqrt(sol[0]))
                B = float(max(sol[1] / sol[0], 0.0))
    out = {"f0_hz": f0, "B": B, "n_partials_fit": len(ns), "t_on_s": t_on, "t_end_s": t_end}
    # per-partial decay: fixed-frequency projection, hop 10 ms, window 8 periods
    Lw = int(8 * sr / f0) | 1
    wb = bh_window(Lw)
    g = 2.0 / wb.sum()
    hop = int(0.01 * sr)
    cens = np.arange(s_on + Lw // 2, min(len(x), s_on + int(4 * sr)) - Lw // 2, hop)
    tc = (cens - s_on) / sr
    t60, early, peaks = [], [], []
    for n in ns[:20]:
        fq = n * f0 * np.sqrt(1 + B * n * n)
        ph = np.exp(-2j * np.pi * fq * np.arange(Lw) / sr)
        amp = np.array([abs(np.dot(x[c - Lw // 2:c - Lw // 2 + Lw] * wb, ph)) * g for c in cens])
        A = db(amp)
        ip = int(np.argmax(A[: max(int(0.3 / 0.01), 2)]))
        peaks.append(float(A[ip]))
        fl = np.percentile(A, 2)
        fit = (tc >= tc[ip] + 0.02) & (A > fl + 10) & (A < A[ip] - 1)
        idx = np.nonzero(fit)[0]
        if len(idx) >= 10:
            last = idx[0] + np.argmax(~fit[idx[0]:]) if (~fit[idx[0]:]).any() else len(fit)
            use = np.arange(idx[0], last)
            if len(use) >= 10:
                sl = np.polyfit(tc[use], A[use], 1)[0]
                t60.append(float(-60 / sl) if sl < 0 else None)
                e_use = use[tc[use] <= tc[use[0]] + 0.3]
                el = np.polyfit(tc[e_use], A[e_use], 1)[0] if len(e_use) >= 6 else None
                early.append(float(-60 / el) if el is not None and el < 0 else None)
                continue
        t60.append(None)
        early.append(None)
    out["partials"] = ns[:20]
    out["t60_s"] = t60
    out["t60_early_s"] = early
    pk = np.array(peaks)
    out["h_peak_db_re_max"] = [float(v) for v in pk - pk.max()] if len(pk) else []
    # the thump: 1/3-octave excess of the first 25 ms over 60-160 ms (equal-length normalised)
    i25 = x[s_on: s_on + int(0.025 * sr)]
    late = x[s_on + int(0.06 * sr): s_on + int(0.16 * sr)]
    if len(late) > int(0.05 * sr):
        fc, p1 = band_powers(i25, sr, 3, 25, 16000, nfft=len(i25))
        _, p2 = band_powers(late, sr, 3, 25, 16000, nfft=len(i25))
        out["thump_fc"] = [float(v) for v in fc]
        out["thump_excess_db"] = [float(v) for v in 10 * np.log10((p1 + 1e-20) / (p2 + 1e-20))]
    out["ltas_fc"], out["ltas_db"] = ltas(x[s_on:int(t_end * sr)], sr)
    return out
