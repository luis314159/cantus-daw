"""body.py - the harmonic-peak estimate of an instrument's body response (Arcus study).

Usage (a library): fc, H, H_upper, n = body_estimate(notes); marks = body_marks(fc, H, family)

The luthiers' harmonic-peak method: every bowed note's harmonics sample the instrument's
transfer function at n*f0. A Helmholtz motion's bridge force falls as 1/n (a sawtooth), so
a_n + 20 log10(n) is the response at n*f0 up to a level per note. Over many notes those
samples are fitted as  y = c_note + H(f)  on a 1/12-octave grid by alternating medians (robust
to the β notches at n = k/β and to weak partials). `H_upper` is the 85th percentile per bin
(the "envelope of the peaks"). What it measures is body x microphone x the source's own
high-frequency roll-off (corner rounding, bow width): compare a model with the references
**through the same estimator**, never with a measured admittance directly.
"""
import numpy as np

GRID_LO, GRID_HI, PER_OCT = 40.0, 12000.0, 12
WINDOWS = {  # Hz: A0 search, signature (main wood) modes, bridge hill
    "cello": ((80, 140), (140, 700), (700, 2500)),
    "bass": ((45, 90), (85, 450), (350, 1600)),
    "viola": ((180, 280), (280, 1000), (1100, 3500)),
    "violin": ((240, 330), (330, 1100), (1700, 4200)),
}


def body_estimate(rows, iters=12):
    k0 = int(np.floor(PER_OCT * np.log2(GRID_LO / 1000)))
    k1 = int(np.ceil(PER_OCT * np.log2(GRID_HI / 1000)))
    fc = 1000 * 2.0 ** (np.arange(k0, k1 + 1) / PER_OCT)
    pts = []
    for j, r in enumerate(rows):
        a = np.asarray(r.get("h_db_all_re_max") or r["h_db_re_max"], float)
        n = np.arange(1, len(a) + 1)
        f = n * r["f0_hz"]
        ok = (a > -70) & (f >= GRID_LO) & (f <= GRID_HI)
        for fi, ai, ni in zip(f[ok], a[ok], n[ok]):
            b = int(np.round(PER_OCT * np.log2(fi / 1000))) - k0
            pts.append((j, b, ai + 20 * np.log10(ni)))
    if not pts:
        return fc.tolist(), [None] * len(fc), [None] * len(fc), 0
    P = np.array(pts)
    note = P[:, 0].astype(int)
    b = P[:, 1].astype(int)
    y = P[:, 2]
    c = np.zeros(note.max() + 1)
    H = np.full(len(fc), np.nan)
    for _ in range(iters):
        r = y - c[note]
        for k in range(len(fc)):
            s = r[b == k]
            H[k] = np.median(s) if len(s) >= 3 else np.nan
        Hb = np.nan_to_num(H[b], nan=np.nanmedian(H))
        for j in range(len(c)):
            s = (y - Hb)[note == j]
            c[j] = np.median(s) if len(s) else 0.0
    r = y - c[note]
    Hu = np.array([np.percentile(r[b == k], 85) if (b == k).sum() >= 3 else np.nan for k in range(len(fc))])
    # smooth over 3 bins (a quarter octave), keep gaps
    def sm(v):
        out = v.copy()
        for k in range(1, len(v) - 1):
            w = v[k - 1:k + 2]
            if np.isfinite(w).sum() >= 2:
                out[k] = np.nanmean(w)
        return out
    H, Hu = sm(H), sm(Hu)
    ref = np.nanmean(H[(fc >= 200) & (fc <= 4000)])
    H, Hu = H - ref, Hu - ref
    cnt = [int((b == k).sum()) for k in range(len(fc))]
    tolist = lambda v: [None if not np.isfinite(x) else round(float(x), 2) for x in v]  # noqa: E731
    return fc.round(2).tolist(), tolist(H), tolist(Hu), cnt


def body_marks(fc, H, family):
    fc = np.asarray(fc, float)
    H = np.array([np.nan if v is None else v for v in H], float)
    a0w, ww, hw = WINDOWS[family]
    out = {}
    s = (fc >= a0w[0]) & (fc <= a0w[1]) & np.isfinite(H)
    if s.any():
        i = np.nonzero(s)[0][np.nanargmax(H[s])]
        out["A0_hz"], out["A0_db"] = float(fc[i]), float(H[i])
    s = np.nonzero((fc >= ww[0]) & (fc <= ww[1]) & np.isfinite(H))[0]
    peaks = []
    for i in s:
        if 0 < i < len(H) - 1 and H[i] >= H[i - 1] and H[i] >= H[i + 1]:
            lo = np.nanmin(H[max(i - 4, 0):i]) if i > 0 else H[i]
            hi = np.nanmin(H[i + 1:i + 5])
            prom = H[i] - max(lo, hi)
            if prom >= 1.0:
                peaks.append((float(fc[i]), float(H[i]), float(prom)))
    peaks.sort(key=lambda p: -p[1])
    out["wood_peaks"] = [{"hz": p[0], "db": round(p[1], 2), "prominence_db": round(p[2], 2)} for p in peaks[:4]]
    k = 4  # one-third octave = 4 bins on the 1/12 grid
    Hs = np.array([np.nanmean(H[max(i - k // 2, 0):i + k // 2 + 1]) if np.isfinite(H[max(i - k // 2, 0):i + k // 2 + 1]).any() else np.nan for i in range(len(H))])
    s = (fc >= hw[0]) & (fc <= hw[1]) & np.isfinite(Hs)
    base = np.nanmean(H[(fc >= ww[0]) & (fc <= ww[1])])
    if s.any():
        i = np.nonzero(s)[0][np.nanargmax(Hs[s])]
        out["bridge_hill_hz"] = float(fc[i])
        out["bridge_hill_db_re_wood"] = float(Hs[i] - base)
    # the hill against the falling trend: the estimate falls ~4 dB/oct faster than the bridge
    # admittance (source roll-off x radiation; cnsm.py), which hides the hill. Fit a line in
    # log2 f from the wood region to twice the hill window and take the largest bump in the
    # hill window above it (1/3-octave smoothed).
    t = (fc >= ww[0]) & (fc <= 2 * hw[1]) & np.isfinite(Hs)
    if t.sum() > 6:
        k = np.polyfit(np.log2(fc[t]), Hs[t], 1)
        d = Hs - np.polyval(k, np.log2(fc))
        s = (fc >= hw[0]) & (fc <= hw[1]) & np.isfinite(d)
        if s.any():
            i = np.nonzero(s)[0][np.nanargmax(d[s])]
            out["hill_detrended_hz"] = float(fc[i])
            out["hill_detrended_db"] = float(d[i])
            out["trend_db_per_oct"] = float(k[0])
    hf = (fc >= 2 * hw[1]) & np.isfinite(H)
    if hf.any():
        out["above_hill_db_re_wood"] = float(np.nanmean(H[hf]) - base)
    return out
