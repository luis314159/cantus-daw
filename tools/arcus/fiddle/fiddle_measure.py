"""fiddle_measure.py - what makes a fiddle sound like one (Cantus, round 42, the country fiddle).

Measures real country / old-time / bluegrass fiddle (`$CANTUS_REFS/fiddle`, catalogued as the
`fiddle` set), classical violin (`$CANTUS_REFS/arcus-upper`, `study/cnsm-violin`, `study/hsd-*`)
and Arcus's renders the same way, and writes per-file and per-group numbers:

    python -I tools/arcus/fiddle/fiddle_measure.py run [--only GROUP ...] [--out DIR]
    python -I tools/arcus/fiddle/fiddle_measure.py table [--out DIR]      # the markdown again

Results: `$CANTUS_WORK/arcus/fiddle/fiddle_measures.json` (per file, per group) and
`fiddle_measures.md`. The groups and files are `GROUPS` in fiddle_groups.py. The methods are in
fiddle_lib.py's header and, per measure, in the functions below; every one says what it cannot
see. Pre-1925 acoustic 78s (`pd78`) are band-limited (about 200-4000 Hz) with surface noise:
they are used for timing, pitch and rhythm only, never for timbre or noise.
"""
import json
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import fiddle_lib as fl  # noqa: E402

REG = [("G", 0, 293.7), ("D", 293.7, 440.0), ("A", 440.0, 659.3), ("E", 659.3, 5000.0)]


def sel(tr, a, b):
    return (tr.t >= a) & (tr.t < b)


def note_list(tr, ons):
    """Notes between consecutive onsets: (t_on, t_end, ioi, f_med Hz, voiced share)."""
    out = []
    for i, t0 in enumerate(ons):
        nxt = ons[i + 1] if i + 1 < len(ons) else t0 + 2.0
        k = sel(tr, t0, nxt) & tr.active
        if not k.any():
            continue
        t_end = float(tr.t[k][-1])
        dur = t_end - t0
        mid = sel(tr, t0 + 0.3 * dur, t0 + 0.9 * dur) & tr.voiced
        f = float(np.median(tr.f_raw[mid])) if mid.sum() >= 3 else 0.0
        out.append((float(t0), t_end, float(nxt - t0), f, float(mid.mean()) if mid.size else 0.0))
    return out


def file_nhr_ref(tr, notes):
    """The file's steady noise-to-harmonic ratio (2-8 kHz, dB): long notes' middles, else the
    25th percentile of every voiced frame."""
    v = []
    for t0, t1, ioi, f, vs in notes:
        if t1 - t0 >= 0.3 and f > 0:
            k = sel(tr, t0 + 0.4 * (t1 - t0), t1 - 0.05) & tr.voiced
            v.extend(tr.bNHR[k].tolist())
    if len(v) >= 20:
        return float(np.median(v))
    k = tr.voiced
    return float(np.percentile(tr.bNHR[k], 25)) if k.any() else 0.0


def mag_at(x, sr, t, n=2048):
    s = int(t * sr) - n // 2
    if s < 0 or s + n > len(x):
        return None
    return np.abs(np.fft.rfft(x[s:s + n] * np.hanning(n)))


def partial_sum(mag, sr, f, other, n=2048, nh=8, tol=35.0):
    freqs = np.fft.rfftfreq(n, 1 / sr)
    tot, used = 0.0, 0
    oth = other * np.arange(1, 3 * nh)
    for h in range(1, nh + 1):
        fh = f * h
        if fh > 5000:
            break
        if np.min(1200 * np.abs(np.log2(oth / fh))) < 2 * tol:
            continue
        lo = int(np.searchsorted(freqs, fh * 2 ** (-tol / 1200)))
        hi = int(np.searchsorted(freqs, fh * 2 ** (tol / 1200))) + 1
        tot += float(mag[lo:hi].max() ** 2)
        used += 1
    return tot, used


def fast_notes(tr, ons, src, notes, fast=0.3):
    """Item 3: per onset whose note is short (IOI < `fast`; the report also pools IOI < 0.2 s
    apart), the bow-change measures."""
    ref = file_nhr_ref(tr, notes)
    rows = []
    by_t = {round(n[0], 4): n for n in notes}
    for i in range(1, len(ons) - 1):
        t0, t1 = float(ons[i]), float(ons[i + 1])
        ioi, ioi_p = t1 - t0, t0 - float(ons[i - 1])
        if not (0.05 <= ioi < fast) or ioi_p > 0.6:
            continue
        n = by_t.get(round(t0, 4))
        if n is None or n[3] <= 0:
            continue
        st = sel(tr, t0 + max(0.5 * ioi, 0.04), t1 - 0.008) & tr.active & tr.band_ok
        if st.sum() < 4:
            continue
        r = {"t": t0, "ioi": ioi, "f0": n[3], "cue": int(src[i]),
             "dyad_share": float(tr.dyad[sel(tr, t0, t1)].mean()) if sel(tr, t0, t1).any() else 0.0}
        n_st = float(np.median(tr.bN[st]))
        nhr_st = float(np.median(tr.bNHR[st]))
        w = sel(tr, t0 - 0.010, t0 + 0.040) & tr.band_ok
        if w.any():
            iw = np.flatnonzero(w)
            j = iw[int(np.argmax(tr.bN[iw]))]
            r["burst_db"] = float(tr.bN[j] - n_st)
            r["burst_nhr_db"] = float(np.max(tr.bNHR[iw]) - nhr_st)
            a = j
            while a > 0 and tr.bN[a - 1] > n_st + 3 and tr.t[a - 1] > t0 - 0.05:
                a -= 1
            b = j
            while b + 1 < len(tr.t) and tr.bN[b + 1] > n_st + 3 and tr.t[b + 1] < t1:
                b += 1
            r["burst_ms"] = float((tr.t[b] - tr.t[a]) * 1000 + fl.HOP * 1000) if tr.bN[j] > n_st + 3 else 0.0
        r["nhr_steady_db"] = nhr_st
        r["nhr_steady_rel_db"] = nhr_st - ref
        # pitch settling (cents against the note's own steady pitch)
        ps = st & tr.voiced
        if ps.sum() >= 3:
            fn = float(np.median(tr.f_raw[ps]))
            ww = sel(tr, t0, min(t0 + 0.06, t1)) & (tr.ap < 0.4) & (tr.f_raw > 0)
            c = 1200 * np.log2(np.maximum(tr.f_raw, 1.0) / fn)
            tt = tr.t - t0
            k0 = ww & (tt < 0.02)
            if k0.sum() >= 2:
                # an octave (or more) off in the first 20 ms: the tracker locked on a doubled
                # period (a raucous, subharmonic start) or on another string's note
                r["octave_start"] = float(np.mean(np.abs(c[k0]) > 600) >= 0.5)
            ww &= np.abs(c) < 600
            for lo, hi, key in ((0.0, 0.02, "dev0_20"), (0.02, 0.04, "dev20_40")):
                kk = ww & (tt >= lo) & (tt < hi)
                if kk.sum() >= 2:
                    r[key] = float(np.median(np.abs(c[kk])))
                    r[key + "_signed"] = float(np.median(c[kk]))
            kk = np.flatnonzero(sel(tr, t0, t1))
            ok = (np.abs(c[kk]) < 20) & (tr.ap[kk] < 0.4)
            # (a frame an octave off counts as not settled)
            for m in range(len(kk) - 3):
                if ok[m:m + 4].all():
                    r["settle_ms"] = float((tr.t[kk[m]] - t0) * 1000)
                    break
            else:
                r["settle_ms"] = float(ioi * 1000)
            # the steady share: periodic and no noisier than the file's steady + 3 dB
            nk = sel(tr, t0, t1) & tr.active
            good = (tr.ap[nk] < 0.15) & (tr.bNHR[nk] < ref + 3)
            r["steady_share"] = float(good.mean()) if nk.any() else 0.0
        # the level dip at the change
        pre = sel(tr, t0 - 0.5 * ioi_p, t0 - 0.02) & tr.active
        post = sel(tr, t0 + 0.02, t0 + 0.6 * ioi) & tr.active
        dw = sel(tr, t0 - 0.03, t0 + 0.03)
        if pre.sum() >= 2 and post.sum() >= 2 and dw.any():
            lp, lq = float(np.median(tr.lvl[pre])), float(np.median(tr.lvl[post]))
            idw = np.flatnonzero(dw)
            j = idw[int(np.argmin(tr.lvl[idw]))]
            r["dip_db"] = float(0.5 * (lp + lq) - tr.lvl[j])
            thr = min(lp, lq) - 3
            a = j
            while a > 0 and tr.lvl[a - 1] < thr and tr.t[a - 1] > t0 - 0.06:
                a -= 1
            b = j
            while b + 1 < len(tr.t) and tr.lvl[b + 1] < thr and tr.t[b + 1] < t0 + 0.06:
                b += 1
            r["dip_ms"] = float((tr.t[b] - tr.t[a]) * 1000 + fl.HOP * 1000) if tr.lvl[j] < thr else 0.0
        # the previous pitch ringing under the new note
        prev = by_t.get(round(float(ons[i - 1]), 4))
        if prev is not None and prev[3] > 0 and abs(1200 * np.log2(prev[3] / n[3])) > 60:
            m0 = mag_at(tr.x, tr.sr, t0 - 0.03)
            if m0 is not None:
                e0, used = partial_sum(m0, tr.sr, prev[3], n[3])
                if used >= 2 and e0 > 0:
                    for dt, key in ((0.03, "ring30_db"), (0.06, "ring60_db")):
                        if dt < ioi + 0.005:
                            m1 = mag_at(tr.x, tr.sr, t0 + dt)
                            if m1 is not None:
                                e1, _ = partial_sum(m1, tr.sr, prev[3], n[3])
                                r[key] = float(10 * np.log10((e1 + 1e-20) / e0))
        rows.append(r)
    return rows, ref


# ---------------------------------------------------------------- item 5: double stops

JUST = [0.0, 111.7, 203.9, 315.6, 386.3, 498.0, 590.2, 702.0, 813.7, 884.4, 968.8, 996.1, 1088.3, 1200.0]
CLASS = {1: "m2/M7", 11: "m2/M7", 2: "M2/m7", 10: "M2/m7", 3: "m3/M6", 9: "m3/M6",
         4: "M3/m6", 8: "M3/m6", 5: "P4/P5", 7: "P4/P5", 6: "tritone"}


def smooth(v, k):
    k = min(max(int(k), 1), len(v))
    if k < 2:
        return v
    return np.convolve(v, np.ones(k) / k, mode="same")


def double_stops(x, sr, onsets_t=None, hop=0.010):
    """Share of sounding time with two pitches, their intervals (octave-complement classes, so
    a wrong octave on one voice does not move a dyad between classes; octaves and unisons are
    invisible), and per dyad segment (>= 150 ms of one stable pair): the two voices' partial
    envelopes' correlation (dB, raw and with the 100 ms trend removed), whether their dips
    coincide, and when a segment starts from silence for both, their onset offset."""
    d = fl.dyads(x, sr, hop_s=hop)
    act = d[:, 4] > 0
    dy = d[:, 1] > 0
    res = {"frames": int(act.sum()), "dyad_share": float(dy.sum() / max(act.sum(), 1))}
    semis = np.round(12 * np.log2(d[dy, 2] / d[dy, 1])).astype(int)
    cls = {}
    for s in semis:
        c = CLASS.get(int(s) % 12, "other")
        cls[c] = cls.get(c, 0) + 1
    res["classes"] = {k: v / max(len(semis), 1) for k, v in sorted(cls.items())}
    res["semitones"] = {int(k): int(v) for k, v in zip(*np.unique(semis, return_counts=True))}
    # segments of a stable pair
    segs, i, n = [], 0, len(d)
    while i < n:
        if not dy[i]:
            i += 1
            continue
        j = i
        while j + 1 < n and dy[j + 1] and abs(1200 * np.log2(d[j + 1, 1] / d[j, 1])) < 60 \
                and abs(1200 * np.log2(d[j + 1, 2] / d[j, 2])) < 60:
            j += 1
        if (j - i + 1) * hop >= 0.15:
            segs.append((float(d[i, 0]), float(d[j, 0]), float(np.median(d[i:j + 1, 1])),
                         float(np.median(d[i:j + 1, 2]))))
        i = j + 1
    res["segments"] = len(segs)
    rows = []
    N = 2048
    for t0, t1, fa, fb in segs:
        a = max(t0 - 0.12, 0.0)
        xx = x[int(a * sr):int((t1 + 0.02) * sr)]
        if len(xx) < N + 10:
            continue
        tt, S = fl.stft_mag(xx, sr, N, 0.005)
        freqs = np.fft.rfftfreq(N, 1 / sr)
        ea = fl.voice_env(S, freqs, np.full(len(S), fa), np.full(len(S), fb))
        eb = fl.voice_env(S, freqs, np.full(len(S), fb), np.full(len(S), fa))
        tt = tt + a
        da, db = 20 * np.log10(ea + 1e-9), 20 * np.log10(eb + 1e-9)
        ins = (tt >= t0) & (tt <= t1)
        if ins.sum() < 20 or not (ea[ins] > 0).all() or not (eb[ins] > 0).all():
            continue
        r = {"t0": t0, "dur": t1 - t0, "semis": int(round(12 * np.log2(fb / fa)))}
        A, B = da[ins], db[ins]
        r["level_lo_minus_hi_db"] = float(np.median(A - B))
        # the interval as played (refined pitches, 10 ms frames): against equal temperament
        # and against the nearest just ratio, its wobble, and the voices' shared pitch motion
        fr = (d[:, 0] >= t0) & (d[:, 0] <= t1) & (d[:, 6] > 0) & (d[:, 7] > 0)
        if fr.sum() >= 10:
            cl = 1200 * np.log2(d[fr, 6] / 440.0)
            ch = 1200 * np.log2(d[fr, 7] / 440.0)
            iv = ch - cl
            ivm = float(np.median(iv))
            r["interval_c"] = ivm
            r["et_dev_c"] = float(ivm - 100 * np.round(ivm / 100))
            red = ivm % 1200
            r["just_dev_c"] = float(min((red - j for j in JUST), key=abs))
            r["interval_sd_c"] = float(np.std(iv - ivm))
            kk = max(int(0.1 / hop), 3)
            ul, uh = cl - smooth(cl, kk * 2), ch - smooth(ch, kk * 2)
            if ul.std() > 0.5 and uh.std() > 0.5:
                r["pitch_corr"] = float(np.corrcoef(ul, uh)[0, 1])
        r["corr_raw"] = float(np.corrcoef(A, B)[0, 1])
        k = int(0.1 / 0.005)
        Af, Bf = A - smooth(A, k), B - smooth(B, k)
        cut = slice(k // 2, len(Af) - k // 2)
        if len(Af[cut]) >= 10 and Af[cut].std() > 0 and Bf[cut].std() > 0:
            r["corr_fast"] = float(np.corrcoef(Af[cut], Bf[cut])[0, 1])
        # dips (>= 4 dB under the +-40 ms neighbourhood) and whether the other voice dips too
        def dips(v):
            out = []
            for m in range(8, len(v) - 8):
                if v[m] == v[m - 8:m + 9].min() and max(v[m - 8:m].max(), v[m + 1:m + 9].max()) - v[m] >= 4:
                    out.append(m)
            return out
        pa_, pb_ = dips(A), dips(B)
        if pa_ or pb_:
            both = [p for p in pa_ if any(abs(p - q) <= 3 for q in pb_)]
            r["dips"] = len(pa_) + len(pb_)
            r["dips_shared"] = float(2 * len(both) / max(len(pa_) + len(pb_), 1))
        # a joint onset: both voices rise from >= 12 dB under their median inside the window
        ma, mb = np.median(A), np.median(B)
        pre = tt < t0
        if pre.sum() >= 4 and da[pre].min() < ma - 12 and db[pre].min() < mb - 12:
            def rise(v, m):
                lo = np.flatnonzero(v < m - 12)
                if not len(lo):
                    return None
                after = np.flatnonzero((np.arange(len(v)) > lo[0]) & (v >= m - 6))
                return float(tt[after[0]]) if len(after) else None
            ta, tb = rise(da, ma), rise(db, mb)
            if ta is not None and tb is not None:
                r["onset_lo_minus_hi_ms"] = (ta - tb) * 1000
        rows.append(r)
    res["rows"] = rows
    return res, d


# ---------------------------------------------------------------- items 1-2: timbre and noise

def third_octaves():
    fc = 1000.0 * 2 ** (np.arange(-10, 13) / 3.0)        # 100 Hz .. 16 kHz
    return fc, fc * 2 ** (-1 / 6), fc * 2 ** (1 / 6)


def timbre(tr, n=4096):
    x, sr = tr.x, tr.sr
    t, S = fl.stft_mag(x, sr, n, 0.02)
    P = S.astype(np.float64) ** 2
    lv = np.interp(t, tr.t, tr.lvl)
    act = lv > tr.peak - 30
    if act.sum() < 5:
        return {}
    freqs = np.fft.rfftfreq(n, 1 / sr)
    L = P[act].mean(axis=0)
    tot = L[freqs >= 80].sum()
    fc, lo, hi = third_octaves()
    bands = [float(10 * np.log10(L[(freqs >= a) & (freqs < b)].sum() / tot + 1e-20)) for a, b in zip(lo, hi)]
    cen_f = (P[act] * freqs).sum(axis=1) / (P[act].sum(axis=1) + 1e-20)
    f0 = np.interp(t[act], tr.t, np.where(tr.voiced, tr.f_raw, np.nan))
    out = {"centroid_hz": float((L * freqs).sum() / L.sum()),
           "centroid_frames_hz": [float(v) for v in np.percentile(cen_f, [25, 50, 75])],
           "share_above_2k_db": float(10 * np.log10(L[freqs >= 2000].sum() / tot)),
           "share_above_4k_db": float(10 * np.log10(L[freqs >= 4000].sum() / tot)),
           "ltas_third_oct_db": dict(zip([f"{v:.0f}" for v in fc], bands))}
    ok = np.isfinite(f0)
    if ok.sum() > 10:
        out["centroid_over_f0"] = float(np.median(cen_f[ok] / f0[ok]))
    return out


def noise_by_register(tr):
    """Item 2: in steady voiced frames (pitch moving < 30 cents per 10 ms), the broadband HNR
    (YIN: (1 - d') / d') and the 2-8 kHz HNR (the comb residual), per string's range."""
    dm = np.abs(np.gradient(tr.midi)) * (0.01 / fl.HOP) * 100
    st = tr.voiced & (dm < 30) & (tr.lvl > tr.peak - 25) & ~tr.dyad
    out = {}
    for name, a, b in REG:
        k = st & (tr.f_raw >= a) & (tr.f_raw < b)
        if k.sum() < 40:
            continue
        ap = np.clip(tr.ap[k], 1e-4, 0.9999)
        out[name] = {"frames": int(k.sum()),
                     "hnr_bb_db": [float(v) for v in np.percentile(10 * np.log10((1 - ap) / ap), [25, 50, 75])],
                     "hnr_2_8k_db": [float(v) for v in np.percentile(-tr.bNHR[k], [25, 50, 75])]}
    return out


# ---------------------------------------------------------------- items 6-7: vibrato, intonation

def vibrato_and_pitch(tr, notes):
    vib, slides, cents = [], [], []
    for idx, (t0, t1, ioi, f, vs) in enumerate(notes):
        if f <= 0:
            continue
        dur = t1 - t0
        k = sel(tr, t0, t1) & tr.voiced
        c = 1200 * np.log2(np.maximum(tr.f_raw, 1) / f)
        if dur >= 0.12:
            cents.append((f, dur))
        if dur >= 0.3:
            m = sel(tr, t0 + 0.08, t1 - 0.04) & tr.voiced
            if m.sum() >= int(0.3 / fl.HOP):
                cc = c[m]
                tt = tr.t[m]
                det = cc - smooth(cc, int(0.25 / fl.HOP))
                core = det[int(0.06 / fl.HOP):-int(0.06 / fl.HOP) or None]
                if len(core) >= 20:
                    ext = float(np.percentile(core, 90) - np.percentile(core, 10))
                    zc = np.sum(np.diff(np.sign(core)) != 0)
                    rate = zc / 2.0 / (len(core) * fl.HOP)
                    vib.append({"dur": dur, "extent_c": ext, "rate_hz": float(rate),
                                "has": bool(ext >= 20 and 3.5 <= rate <= 9.0)})
        # a slide into the note: the first voiced 10 ms >= 50 cents off, reaching +-20 within 150 ms
        if dur >= 0.12:
            kk = np.flatnonzero(k & (tr.t < t0 + 0.15))
            if len(kk) >= 6:
                c0 = float(np.median(c[kk[:4]]))
                prev = notes[idx - 1][3] if idx > 0 else 0
                from_prev = prev > 0 and abs(1200 * np.log2(f * 2 ** (c0 / 1200) / prev)) < 40
                if abs(c0) > 400:
                    slides.append({"from_c": 0.0, "ms": 0.0, "octave": True})
                elif abs(c0) >= 50 and not from_prev:
                    reach = np.flatnonzero(np.abs(c[kk]) < 20)
                    slides.append({"from_c": c0, "ms": float((tr.t[kk[reach[0]]] - tr.t[kk[0]]) * 1000) if len(reach) else 150.0})
                else:
                    slides.append({"from_c": c0 if abs(c0) >= 50 else 0.0, "ms": 0.0})
    return vib, slides, cents


KK_MAJOR = np.array([6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88])


def intonation(tr, cents_notes):
    """Item 7: the tonic from the duration-weighted pitch-class profile (Krumhansl-Kessler,
    major; fiddle tunes are major or mixolydian), then each note's cents from equal temperament
    by its degree; thirds (3, 4) and sevenths (10, 11) reported, with how many sit between."""
    if len(cents_notes) < 12:
        return {}
    m = np.array([69 + 12 * np.log2(f / 440) - tr.tuning for f, _ in cents_notes])
    w = np.array([d for _, d in cents_notes])
    pc = np.zeros(12)
    for v, ww in zip(m, w):
        pc[int(np.round(v)) % 12] += ww
    sc = [np.corrcoef(np.roll(KK_MAJOR, k), pc)[0, 1] for k in range(12)]
    ton = int(np.argmax(sc))
    rel = (m - ton) % 12
    out = {"tonic_pc": ton, "key_corr": float(max(sc)), "tuning_c": float(100 * tr.tuning)}
    for deg, lo, hi in (("third", 3.0, 4.0), ("seventh", 10.0, 11.0)):
        k = (rel > lo - 0.5) & (rel < hi + 0.5)
        if k.sum() >= 3:
            v = (rel[k] - lo) * 100   # 0 = minor, 100 = major
            out[deg] = {"n": int(k.sum()), "cents_above_minor": [float(q) for q in np.percentile(v, [25, 50, 75])],
                        "neutral_share": float(np.mean((v > 25) & (v < 75)))}
    dev = (m - np.round(m)) * 100
    out["dev_from_et_c_sd"] = float(np.std(dev))
    return out


# ---------------------------------------------------------------- items 4, 8, 9: rhythm, room

def note_peaks(tr, ons):
    pk = []
    for i, t0 in enumerate(ons):
        t1 = ons[i + 1] if i + 1 < len(ons) else t0 + 0.2
        k = sel(tr, t0, t0 + min(0.6 * (t1 - t0), 0.08))
        pk.append(float(tr.lvl[k].max()) if k.any() else -200.0)
    return np.array(pk)


def rhythm(tr, ons):
    """Runs of even notes (IOIs 0.07-0.35 s, each within 0.55-1.8 x the run's median): the
    long/short alternation (the swing ratio, long first; on a solo fiddle the beat's phase is
    not known, so 'long' stands for 'on the beat'), the IOI spread left after it, the accent
    alternation (peak level of the long-IOI notes minus the short ones) and the level spread."""
    ioi = np.diff(ons)
    pk = note_peaks(tr, ons)
    runs, i = [], 0
    while i < len(ioi):
        j = i
        while j < len(ioi) and 0.07 <= ioi[j] <= 0.35:
            j += 1
        if j - i >= 6:
            seg = np.arange(i, j)
            med = np.median(ioi[seg])
            ok = (ioi[seg] > 0.55 * med) & (ioi[seg] < 1.8 * med)
            if ok.mean() > 0.8:
                runs.append(seg)
        i = max(j, i + 1)
    rows = []
    for seg in runs:
        v = ioi[seg]
        lv = np.log(v)
        a, b = lv[0::2].mean(), lv[1::2].mean()
        long_first = a >= b
        sw = float(np.exp(abs(a - b)))
        par = np.arange(len(v)) % 2
        mean_par = np.where(par == 0, np.exp(a), np.exp(b))
        resid = (v - mean_par) * 1000
        p = pk[seg]
        good = p > -150
        L = p[(par == (0 if long_first else 1)) & good]
        S = p[(par == (1 if long_first else 0)) & good]
        lvl_res = p[good] - np.where(par[good] == 0, p[(par == 0) & good].mean(), p[(par == 1) & good].mean())
        rows.append({"t0": float(ons[seg[0]]), "notes": int(len(seg)), "eighth_s": float(np.median(v)),
                     "swing": sw, "ioi_sd_ms": float(np.std(resid)),
                     "accent_long_minus_short_db": float(L.mean() - S.mean()) if len(L) and len(S) else None,
                     "level_sd_db": float(np.std(lvl_res))})
    return rows


def decays(tr):
    """Item 9: decays after the sound stops (the level falls under the file's peak - 35 dB and
    stays there 0.25 s): T60 extrapolated from the -5 to -25 dB fall. It mixes the string's own
    free decay with the room's; a reverberant tail shows as a long T60 and a slow late slope."""
    out = []
    a = tr.active
    ends = np.flatnonzero(a[:-1] & ~a[1:])
    hold = int(0.25 / fl.HOP)
    for e in ends:
        if e + hold >= len(a) or a[e + 1:e + hold].any():
            continue
        s0 = max(e - int(0.15 / fl.HOP), 0)
        j0 = s0 + int(np.argmax(tr.lvl[s0:e + 1]))
        L0 = tr.lvl[j0]
        seg = tr.lvl[j0:j0 + int(2.0 / fl.HOP)]
        i5 = np.flatnonzero(seg < L0 - 5)
        i25 = np.flatnonzero(seg < L0 - 25)
        if len(i5) and len(i25) and i25[0] > i5[0]:
            out.append({"t": float(tr.t[j0]), "t60_s": float(3 * (i25[0] - i5[0]) * fl.HOP),
                        "edt_ms": float(i5[0] * fl.HOP * 1000)})
    return out


# ---------------------------------------------------------------- one file

def analyse(path, kind="fiddle", t0=None, t1=None, extra=None, truth=None, ds_only=False):
    """kind: 'fiddle' / 'classical' / 'ours' (every measure); 'pd78' (timing and pitch only)."""
    x, sr = fl.load(path, t0, t1)
    for p in extra or []:
        y, _ = fl.load(p, t0, t1)
        n = min(len(x), len(y))
        x = x[:n] + y[:n]
    res = {"path": str(path), "kind": kind, "sr": sr, "dur_s": len(x) / sr}
    d = None
    if kind != "pd78":
        res["double"], d = double_stops(x, sr)
    if ds_only:
        return res
    tr = fl.Track(x, sr)
    # two pitches sounding (10 ms frames) mapped onto the track: the single-voice measures
    # (noise, HNR, the steady share) leave those frames out or report them apart
    tr.dyad = np.zeros(len(tr.t), bool)
    if d is not None and len(d):
        j = np.clip(np.searchsorted(d[:, 0], tr.t), 0, len(d) - 1)
        tr.dyad = d[j, 1] > 0
    ons, src = fl.onsets(tr)
    notes = note_list(tr, ons)
    res["onsets"] = int(len(ons))
    res["cue_share"] = {"flux": float(np.mean(src == 0)), "pitch": float(np.mean(src == 1))} if len(src) else {}
    fr, ref = fast_notes(tr, ons, src, notes)
    res["fast"] = fr
    res["nhr_ref_db"] = ref
    vib, slides, cn = vibrato_and_pitch(tr, notes)
    res["vibrato"], res["slides"] = vib, slides
    res["intonation"] = intonation(tr, cn)
    res["rhythm"] = rhythm(tr, ons)
    iois = np.diff(ons)
    res["ioi_q"] = [float(v) for v in np.percentile(iois, [10, 25, 50, 75])] if len(iois) > 4 else []
    res["fast_share"] = float(np.mean(iois < 0.2)) if len(iois) else 0.0
    res["decays"] = decays(tr)
    if kind != "pd78":
        res["timbre"] = timbre(tr)
        res["noise"] = noise_by_register(tr)
    if truth is not None:
        tt = np.array([n[0] for n in truth])
        lag = []
        hit = 0
        for v in tt:
            j = np.argmin(np.abs(ons - v)) if len(ons) else None
            if j is not None and abs(ons[j] - v) < 0.035:
                hit += 1
                lag.append(ons[j] - v)
        fp = sum(1 for o in ons if np.min(np.abs(tt - o)) >= 0.035)
        res["detector_vs_truth"] = {"recall": hit / max(len(tt), 1), "precision": 1 - fp / max(len(ons), 1),
                                    "lag_ms": float(np.median(lag) * 1000) if lag else None}
        lagv = float(np.median(lag)) if lag else 0.0
        tons = np.unique(np.round(tt + lagv, 4))
        tnotes = note_list(tr, tons)
        res["fast_truth"], _ = fast_notes(tr, tons, np.zeros(len(tons), int), tnotes)
    return res


# ---------------------------------------------------------------- our two stems, exactly

def ours_stems(stem1, stem2, cantus, names):
    """Our double stops as rendered: the two stems are separate instruments, so their level
    envelopes, their 2-8 kHz noise envelopes and their onsets can be compared directly
    (a real double stop is one bow: one envelope, one noise)."""
    import fiddle_truth as ft
    x1, sr = fl.load(stem1)
    x2, _ = fl.load(stem2)
    n = min(len(x1), len(x2))
    a, b = fl.Track(x1[:n], sr), fl.Track(x2[:n], sr)
    both = a.active & b.active & (a.lvl > a.peak - 25) & (b.lvl > b.peak - 25)
    out = {"both_share": float(both.mean())}
    if both.sum() > 50:
        k = int(0.1 / fl.HOP)
        for key, va, vb in (("level", a.lvl, b.lvl), ("noise_2_8k", a.bN, b.bN)):
            A, B = va - smooth(va, k), vb - smooth(vb, k)
            out[f"{key}_corr_raw"] = float(np.corrcoef(va[both], vb[both])[0, 1])
            out[f"{key}_corr_fast"] = float(np.corrcoef(A[both], B[both])[0, 1])
    ns, _ = ft.notes(cantus)
    n1, n2 = ns[names[0]], ns[names[1]]
    dt = []
    for p in n2:
        q = min(n1, key=lambda r: abs(r[0] - p[0]))
        if abs(q[0] - p[0]) < 0.06:
            dt.append((p[0] - q[0]) * 1000)
    out["asked_onset_lo_minus_hi_ms"] = [float(v) for v in np.percentile(dt, [25, 50, 75])] if dt else []
    out["asked_onset_abs_ms_median"] = float(np.median(np.abs(dt))) if dt else None
    out["pairs"] = len(dt)
    return out


def main(argv):
    import argparse
    import hashlib
    import fiddle_groups as fg
    import fiddle_report as fr
    import fiddle_truth as ft
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=["run", "table"])
    ap.add_argument("--only", nargs="*")
    ap.add_argument("--out", default=str(fg.cp.work("arcus", "fiddle")))
    ap.add_argument("--fresh", action="store_true")
    a = ap.parse_args(argv)
    out = Path(a.out)
    (out / "files").mkdir(parents=True, exist_ok=True)
    G = fg.groups()
    if a.cmd == "run":
        for gname, items in G.items():
            if a.only and gname not in a.only:
                continue
            for path, kind, opt in items:
                key = hashlib.sha1(json.dumps([path, kind, sorted(opt.items(), key=str)], default=str)
                                   .encode()).hexdigest()[:12]
                cache = out / "files" / f"{gname}-{Path(path).stem[:40]}-{key}.json"
                if cache.exists() and not a.fresh:
                    continue
                truth = None
                if "truth" in opt and not opt.get("ds_only"):
                    ns, _ = ft.notes(opt["truth"][0])
                    truth = ns[opt["truth"][1][0]]
                print(f"[{gname}] {Path(path).name}", flush=True)
                r = analyse(path, kind, opt.get("t0"), opt.get("t1"), opt.get("extra"), truth, opt.get("ds_only", False))
                r["group"] = gname
                cache.write_text(json.dumps(r, default=float), encoding="utf-8")
        if not a.only or "ours-stems" in a.only:
            cache = out / "files" / "ours-stems.json"
            if not cache.exists() or a.fresh:
                rows = {}
                for path, kind, opt in G["ours-dstops"]:
                    nm = opt["extra"][0]
                    cant = path.split("-stems")[0] + ".cantus"
                    t1 = Path(path).stem.split(" ", 1)[1]
                    t2 = Path(nm).stem.split(" ", 1)[1]
                    print(f"[ours-stems] {Path(cant).name}", flush=True)
                    rows[Path(cant).stem] = ours_stems(path, nm, cant, [t1, t2])
                cache.write_text(json.dumps(rows, default=float), encoding="utf-8")
    fr.write(out)


if __name__ == "__main__":
    main(sys.argv[1:])
