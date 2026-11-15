"""The tenth listen's acceptance (round 40, drums-gen 2): the gap analysis's fifteen ranked
cues (`analysis-drums-2/report.md`), each measured as the analyst measured it, against its
numeric target, per kit — PASS / FAIL with the recording's value beside ours.

    python -I accept_b10.py <studio hits> <jazz hits> [--rec <drskit hits> <virtuosity hits>]
        [--groove <studio groove> <jazz groove> <drskit groove> <virtuosity groove>] [--cues 1,2,…]

Hits: `tympanum_measure hits` (13 articulations x 4 velocities x 4 s). The recordings
default to `tmp/meas9`'s. Where the analyst's absolute target is beyond the recording itself
(a measure the recording fails), the check passes within a stated margin of the recording:
the target is the recording, not a number it does not reach.
"""
import sys
import numpy as np
import soundfile as sf
from scipy import signal, ndimage

SR = 48000
PIECES = ["kick", "snare", "rimshot", "sidestick", "tom_hi", "tom_mid", "tom_floor",
          "hat_closed", "hat_open", "hat_pedal", "crash", "ride", "ride_bell"]
VELS = [40, 80, 110, 127]
MEAS = "F:/projects/daw-r40/tmp/meas9/"
REC = {"studio": MEAS + "pack-drskit-tympanum-kit-hits.flac",
       "jazz": MEAS + "pack-virtuosity-drums-kit-hits.flac"}


def db(x):
    return 20 * np.log10(np.abs(x) + 1e-15)


def load(p):
    x, sr = sf.read(p, always_2d=True)
    assert sr == SR, (p, sr)
    return x.astype(np.float64)


def hit(x, piece, vel):
    """The analyst's segmentation (common.pack_hit): mono, onset at the first sample over
    -50 dB re the peak within 3 ms before the 2 %-of-peak crossing; padded by 1 s."""
    i = PIECES.index(piece) * 4 + VELS.index(vel)
    seg = x[int(i * 4 * SR):int((i + 1) * 4 * SR)].mean(1)
    a = np.abs(seg)
    on = int(np.argmax(a > 0.02 * a.max()))
    k0 = max(0, on - int(0.003 * SR))
    pre = np.nonzero(a[k0:on + 1] > 10 ** (-50 / 20) * a.max())[0]
    if len(pre):
        on = k0 + int(pre[0])
    return np.pad(seg, (480, SR)), on + 480


def bandpass(m, lo, hi, order=4):
    hi = min(hi, SR / 2 * 0.98)
    return signal.sosfiltfilt(signal.butter(order, [lo, hi], btype="band", fs=SR, output="sos"), m)


def band_env(m, lo, hi, win):
    y = bandpass(m, lo, hi)
    n = max(1, int(win * SR))
    return np.sqrt(np.convolve(y * y, np.ones(n) / n, mode="same") + 1e-30)


def flatness(p, axis=0):
    p = np.maximum(p, 1e-20)
    return np.exp(np.mean(np.log(p), axis=axis)) / np.mean(p, axis=axis)


def centroid(m):
    X = np.abs(np.fft.rfft(m * np.hanning(len(m)))) ** 2
    f = np.fft.rfftfreq(len(m), 1 / SR)
    return float((X * f).sum() / (X.sum() + 1e-30))


def schroeder_t60(y, lo_db=-5, hi_db=-25):
    s = np.cumsum((y * y)[::-1])[::-1]
    s = 10 * np.log10(s / (s[0] + 1e-30) + 1e-30)
    i0, i1 = int(np.argmax(s <= lo_db)), int(np.argmax(s <= hi_db))
    if i1 <= i0 + 10 or s[-1] > hi_db:
        return np.nan
    t = np.arange(len(s)) / SR
    return -60.0 / np.polyfit(t[i0:i1], s[i0:i1], 1)[0]


def e_db(m, on, lo, hi, a, b):
    return 10 * np.log10(np.mean(bandpass(m, lo, hi)[on + int(a * SR / 1000):on + int(b * SR / 1000)] ** 2) + 1e-30)


def flat_band(m, on, a, b, lo, hi, nfft=2048):
    y = m[on + int(a * SR / 1000):on + int(b * SR / 1000)]
    f, _, S = signal.stft(y, SR, nperseg=min(nfft, len(y)), noverlap=min(nfft, len(y)) // 2)
    k = (f >= lo) & (f <= hi)
    return float(np.mean(flatness(np.abs(S[k]) ** 2, axis=0)))


# --- the analyst's measures (s14, s15, s09, s08, s12, s07c, s02, s04) ------------------

def arrival(m, on):
    """s14: the strongest 2-16 kHz arrival in the first 12 ms, ms after the first sound."""
    y = bandpass(m, 2000, 16000)
    a = np.abs(signal.hilbert(y[on - 240:on + int(0.012 * SR)]))
    e = 20 * np.log10(np.convolve(a, np.ones(5) / 5, mode="same") + 1e-12)
    p, _ = signal.find_peaks(e, prominence=6, distance=24)
    if not len(p):
        return np.nan
    return (p[np.argmax(e[p])] - 240) / SR * 1000


def spike(m, on):
    """s15: 3 kHz+ in the first 1.5 ms re the 2-50 ms max (dB)."""
    hp = bandpass(m, 3000, 20000)
    return db(np.abs(hp[on:on + int(0.0015 * SR)]).max() / np.abs(hp[on + int(0.002 * SR):on + int(0.05 * SR)]).max())


def crest(m, on):
    y = m[on:on + int(0.05 * SR)]
    return db(np.abs(y).max() / np.sqrt(np.mean(y ** 2)))


def cep(m, on):
    """s09: the largest real-cepstrum peak at 0.7-15 ms quefrency in the first 80 ms (z)."""
    y = m[on:on + int(0.08 * SR)]
    L = np.log(np.abs(np.fft.rfft(y * np.hanning(len(y)), 1 << 14)) + 1e-9)
    c = np.fft.irfft(L - L.mean())
    q = np.arange(len(c)) / SR * 1000
    k = (q > 0.7) & (q < 15)
    return float(np.abs(c[k]).max() / np.std(c[k]))


def punch(m, on):
    lp = signal.sosfiltfilt(signal.butter(4, 1500, fs=SR, output="sos"), m)
    return db(np.abs(lp[on:on + int(0.006 * SR)]).max() / np.abs(lp[on + int(0.006 * SR):on + int(0.06 * SR)]).max())


def tail(m, on, a, b):
    """s12: 4-12 kHz flatness, 6-16 kHz components/kHz, stability, 8-16 kHz re the hit."""
    y = m[on + int(a * SR):on + int(b * SR)]
    f, _, S = signal.stft(y, SR, nperseg=4096, noverlap=2048)
    P = np.abs(S) ** 2
    k = (f >= 4000) & (f < 12000)
    fl = np.median(flatness(P[k], axis=0))
    k3 = (f >= 6000) & (f < 16000)
    p = P[k3] / P[k3].sum(0)
    eff = np.median(1 / np.sum(p ** 2, 0) / 10)
    Lg = 10 * np.log10(P + 1e-20)
    k2 = (f >= 4000) & (f < 16000)
    X = Lg[k2] - ndimage.uniform_filter1d(Lg[k2], 41, axis=0)
    st = np.nanmean([np.corrcoef(X[:, i], X[:, i + 2])[0, 1] for i in range(X.shape[1] - 2)])
    e = band_env(m, 8000, 16000, 0.02)
    lv = 20 * np.log10(np.mean(e[on + int(a * SR):on + int(b * SR)]) / e[on:on + int(0.5 * SR)].max())
    return fl, eff, st, lv


def noise_share(m, on, t):
    """The noise's share of 4-16 kHz at `t` s: power in bins within 6 dB of the 300-Hz local
    median (a pure noise reads ~0.77; a spectrum of lines, ~0)."""
    i = on + int(t * SR)
    y = m[i:i + 4096] * np.hanning(4096)
    P = np.abs(np.fft.rfft(y)) ** 2
    f = np.fft.rfftfreq(4096, 1 / SR)
    L = 10 * np.log10(P + 1e-30)
    med = ndimage.median_filter(L, size=27)
    k = (f >= 4000) & (f < 16000)
    return float(P[k][L[k] <= med[k] + 6].sum() / P[k].sum())


def sub(m, on):
    """s07c: 20-55 Hz in 0-80 ms re the spectrum's max (dB)."""
    y = m[on:on + int(0.08 * SR)]
    X = np.abs(np.fft.rfft(y * np.hanning(len(y)), 1 << 16)) ** 2
    f = np.fft.rfftfreq(1 << 16, 1 / SR)
    k = (f >= 20) & (f < 55)
    return 10 * np.log10(X[k].mean() / X.max())


def at500(m, on, lo, hi):
    env = band_env(m, lo, hi, 0.02)
    return 20 * np.log10(env[on + int(0.5 * SR)] / env[on:on + int(0.1 * SR)].max())


def t60(m, on, lo, hi, a=-15, b=-35):
    return schroeder_t60(bandpass(m, lo, hi)[on:on + int(3.9 * SR)], a, b)


def c3(m, on):
    return centroid(m[on:on + int(0.003 * SR)]) / 1000


def tonal(m, on, a, b):
    """s04: 4-12 kHz flatness and peaks per kHz standing 12 dB over the 300-Hz median."""
    w = m[on + int(a * SR):on + int(b * SR)]
    nfft = 1 << int(np.ceil(np.log2(len(w))) + 1)
    P = np.abs(np.fft.rfft(w * np.hanning(len(w)), nfft)) ** 2
    f = np.fft.rfftfreq(nfft, 1 / SR)
    df = f[1]
    Pdb = 10 * np.log10(P + 1e-30)
    med = ndimage.median_filter(Pdb, size=int(300 / df) | 1)
    s = (f >= 4000) & (f < 12000)
    pk, _ = signal.find_peaks(Pdb[s] - med[s], height=12, distance=max(1, int(20 / df)))
    return float(flatness(P[s])), len(pk) / 8.0


def stability(m, on, a, b):
    nfft = 4096
    f = np.fft.rfftfreq(nfft, 1 / SR)
    s = (f >= 4000) & (f < 16000)

    def fs(tt):
        i = on + int(tt * SR)
        X = 10 * np.log10(np.abs(np.fft.rfft(m[i:i + nfft] * np.hanning(nfft))) ** 2 + 1e-30)[s]
        return X - ndimage.uniform_filter1d(X, 41)
    cs, t = [], a
    while t + 0.05 + nfft / SR < b:
        cs.append(np.corrcoef(fs(t), fs(t + 0.05))[0, 1])
        t += 0.05
    return float(np.mean(cs)) if cs else np.nan


def kick_pitch(m, on):
    """The dominant low partial 250-600 ms (s02) (Hz) and the strongest 47-57 Hz line re it (dB)."""
    y = m[on + int(0.25 * SR):on + int(0.6 * SR)]
    X = np.abs(np.fft.rfft(y * np.hanning(len(y)), 1 << 18))
    f = np.fft.rfftfreq(1 << 18, 1 / SR)
    k = (f >= 35) & (f <= 120)
    i = np.nonzero(k)[0][np.argmax(X[k])]
    k2 = (f >= 47) & (f <= 57)
    return float(f[i]), float(db(X[k2].max() / X[i]))


# --- the checks -----------------------------------------------------------------------

ROWS = []


def check(cue, kit, name, g, r, ok, target):
    ROWS.append((cue, kit, name, g, r, ok, target))


def fmt(v):
    if isinstance(v, (list, tuple)):
        return " ".join(fmt(x) for x in v)
    return "  nan" if v is None or (isinstance(v, float) and np.isnan(v)) else f"{v:6.2f}"


def per_vel(fn, G, R, piece, vels=VELS):
    g = [fn(*hit(G, piece, v)) for v in vels]
    r = [fn(*hit(R, piece, v)) for v in vels]
    return g, r


def cues(kit, G, R, only):
    S = kit == "studio"
    drums = ["snare", "tom_mid", "tom_floor"]
    loud = [80, 110, 127]
    if 1 in only:
        for p in drums + ["ride"]:
            g, r = per_vel(arrival, G, R, p, loud)
            check(1, kit, f"{p} 2-16k arrival ms", g, r, all(x <= max(1.5, y + 0.5) for x, y in zip(g, r)), "<= 1.5 (or rec+.5)")
            g, r = per_vel(spike, G, R, p, loud)
            check(1, kit, f"{p} 3k+ first 1.5ms dB", g, r,
                  all(x >= min(-5, y - 2) for x, y in zip(g, r)), ">= -5 (or rec-2)")
        for p in ["kick"] + drums:
            g, r = per_vel(cep, G, R, p, [80, 127])
            check(1, kit, f"{p} cepstral z", g, r, all(x <= 4.5 for x in g), "<= 4.5")
    if 10 in only or 1 in only:
        for p, tgt in (("snare", 0.0), ("tom_mid", 1.5), ("tom_floor", 1.5)):
            g, r = per_vel(punch, G, R, p, loud)
            check(10, kit, f"{p} punch dB", g, r,
                  all(x >= min(tgt, y - 1) for x, y in zip(g, r)), f">= {tgt} (or rec-1)")
    if 2 in only:
        for p, (a, b) in (("crash", (2.0, 3.3)), ("ride", (2.0, 3.3)), ("hat_open", (1.6, 2.4))):
            if p == "hat_open" and S:
                continue
            for v in (80, 127):
                g = tail(*hit(G, p, v), a, b)
                r = tail(*hit(R, p, v), a, b)
                check(2, kit, f"{p} v{v} {a}-{b}s flat", g[0], r[0], g[0] <= max(0.06, r[0] + 0.02), "<= max(.06, rec+.02)")
                check(2, kit, f"{p} v{v} {a}-{b}s comps/kHz", g[1], r[1], g[1] <= max(5, r[1] + 1.5), "<= max(5, rec+1.5)")
                check(2, kit, f"{p} v{v} {a}-{b}s stability", g[2], r[2], g[2] >= min(0.45, r[2] - 0.05), ">= min(.45, rec-.05)")
                check(2, kit, f"{p} v{v} {a}-{b}s 8-16k dB", g[3], r[3], g[3] <= r[3] + 2, "<= rec+2")
            for t in (0.3, 1.5):
                g, r = per_vel(lambda m, on: noise_share(m, on, t), G, R, p, [80, 127])
                check(2, kit, f"{p} noise share 4-16k @{t}s", g, r,
                      all(x <= y + 0.1 for x, y in zip(g, r)), "<= rec+0.1")
    if 3 in only:
        g, r = per_vel(lambda m, on: e_db(m, on, 4000, 12000, 50, 300) - e_db(m, on, 40, 400, 50, 300), G, R, "snare")
        check(3, kit, "snare wires 4-12k re body dB", g, r, all(x >= (-12 if S else -7) for x in g[1:]),
              ">= -12" if S else ">= -7")
        g, r = per_vel(lambda m, on: flat_band(m, on, 20, 200, 3000, 10000), G, R, "snare")
        check(3, kit, "snare 3-10k flatness 20-200ms", g, r, all(x >= 0.38 for x in g[1:]), ">= 0.38")
        g, r = per_vel(lambda m, on: centroid(m[on:on + int(0.1 * SR)]) / 1000, G, R, "snare")
        check(3, kit, "snare centroid 0-100ms kHz", g, r, all(x >= (0.45 if S else 1.0) for x in g[1:]),
              ">= 0.45" if S else ">= 1.0")
    if 4 in only:
        for p in ["snare", "tom_hi", "tom_mid", "tom_floor", "hat_closed", "ride", "crash"]:
            g, r = per_vel(sub, G, R, p)
            check(4, kit, f"{p} 20-55Hz 0-80ms dB", g, r,
                  all(x <= max(-40, y + 3) for x, y in zip(g, r)), "<= -40 (or rec+3)")
    if 5 in only:
        for p in ["kick", "tom_hi", "tom_mid", "tom_floor"]:
            g, r = per_vel(lambda m, on: at500(m, on, 1000, 4000), G, R, p)
            check(5, kit, f"{p} 1-4k @500ms dB", g, r, all(x <= max(-50, y + 3) for x, y in zip(g, r)), "<= -50 (or rec+3)")
            g, r = per_vel(lambda m, on: t60(m, on, 1000, 4000), G, R, p)
            lim = 0.5 if S else 1.0
            check(5, kit, f"{p} T60 1-4k s", g, r,
                  all(np.isnan(x) or x <= max(lim, y + 0.1) for x, y in zip(g, r)), f"<= {lim} (or rec)")
        g, r = per_vel(lambda m, on: at500(m, on, 250, 1000), G, R, "tom_floor")
        check(5, kit, "tom_floor 250-1k @500ms dB", g, r, all(x <= max(-35, y + 3) for x, y in zip(g, r)), "<= -35 (or rec+3)")
    if 6 in only:
        g, r = per_vel(lambda m, on: e_db(m, on, 2000, 8000, 0, 8) - e_db(m, on, 40, 400, 0, 8), G, R, "kick")
        check(6, kit, "kick click 2-8k re body 0-8ms dB", g, r, all(abs(x - y) <= 3 for x, y in zip(g, r)), "rec +-3")
    if 7 in only:
        g, r = per_vel(lambda m, on: tonal(m, on, 0.01, 0.12)[0], G, R, "hat_closed")
        check(7, kit, "hat closed 4-12k flatness", g, r, all(x >= min(0.40, y - 0.02) for x, y in zip(g, r)), ">= min(.40, rec-.02)")
        g, r = per_vel(lambda m, on: stability(m, on, 0.03, 0.25), G, R, "hat_closed")
        check(7, kit, "hat closed stability", g, r, all(x <= max(0.20, y + 0.03) for x, y in zip(g, r)), "<= max(.20, rec+.03)")
    if 8 in only:
        if S:
            g, r = per_vel(lambda m, on: t60(m, on, 8000, 16000, -5, -25), G, R, "ride", [110, 127])
            check(8, kit, "ride loud T60 8-16k s", g, r, all(x <= 2.5 for x in g), "<= 2.5")
            g, r = per_vel(lambda m, on: stability(m, on, 0.2, 1.2), G, R, "ride")
            check(8, kit, "ride stability", g, r, all(x >= min(0.5, y - 0.05) for x, y in zip(g, r)), ">= min(.5, rec-.05)")
        else:
            g, r = per_vel(lambda m, on: tonal(m, on, 0.15, 0.65)[1], G, R, "ride")
            check(8, kit, "ride peaks/kHz 4-12k", g, r, all(x >= min(15, y - 2) for x, y in zip(g, r)), ">= 15 (or rec-2)")
            g, r = per_vel(lambda m, on: tonal(m, on, 0.15, 0.65)[0], G, R, "ride")
            check(8, kit, "ride 4-12k flatness", g, r, all(x <= max(0.07, y + 0.02) for x, y in zip(g, r)), "<= .07 (or rec+.02)")
    if 9 in only:
        g, r = per_vel(c3, G, R, "crash", loud)
        check(9, kit, "crash first-3ms centroid kHz", g, r,
              all(x >= min(t, y - 0.5) for x, y, t in zip(g, r, (4.5, 4.5, 5.5))), ">= 4.5 (5.5 v127) or rec-.5")
        g, r = per_vel(lambda m, on: e_db(m, on, 6000, 20000, 0, 3) - e_db(m, on, 200, 1500, 0, 3), G, R, "crash", loud)
        check(9, kit, "crash 6-20k vs 0.2-1.5k first 3ms dB", g, r,
              all(x >= min(-2, y - 2) for x, y in zip(g, r)), ">= -2 (or rec-2)")
    if 12 in only:
        for p in ["kick", "snare", "tom_mid", "tom_floor", "hat_closed", "hat_open", "ride", "crash"]:
            g, r = per_vel(crest, G, R, p)
            check(12, kit, f"{p} crest 0-50ms dB", g, r, all(abs(x - y) <= 2 for x, y in zip(g, r)), "rec +-2")
    if 13 in only and S:
        g, r = per_vel(kick_pitch, G, R, "kick")
        check(13, kit, "kick dominant Hz", [x[0] for x in g], [x[0] for x in r],
              all(abs(x[0] - y[0]) <= 5 for x, y in zip(g[1:], r[1:])), "rec +-5 Hz (v80+)")
        check(13, kit, "kick 47-57Hz line re it dB", [x[1] for x in g], [x[1] for x in r],
              all(x[1] <= y[1] + 4 for x, y in zip(g, r)), "<= rec+4")
    if 14 in only:
        for p in ["kick", "tom_mid", "tom_floor"]:
            g, r = per_vel(lambda m, on: flat_band(m, on, 100, 400, 3000, 8000), G, R, p, [40, 80])
            check(14, kit, f"{p} soft 3-8k flatness", g, r, all(x >= min(0.40, y - 0.03) for x, y in zip(g, r)), ">= .40 (or rec-.03)")


def groove_cues(kit, g, r):
    def stereo(x, lo, hi):
        lft, rt = bandpass(x[:, 0], lo, hi), bandpass(x[:, 1], lo, hi)
        return float(np.sum(lft * rt) / np.sqrt(np.sum(lft * lft) * np.sum(rt * rt)))

    def band(x, lo, hi):
        return 10 * np.log10(np.mean(bandpass(x.mean(1), lo, hi) ** 2) + 1e-30)
    G, R = load(g), load(r)
    cg, cr = stereo(G, 250, 1000), stereo(R, 250, 1000)
    check(15, kit, "groove 250-1k L/R corr", cg, cr, cg <= cr + 0.1, "<= rec+0.1")
    tg = band(G, 30, 20000)
    tr = band(R, 30, 20000)
    for lo, hi, nm in ((30, 60, "sub 30-60"), (4000, 8000, "4-8k")):
        a, b = band(G, lo, hi) - tg, band(R, lo, hi) - tr
        check(15, kit, f"groove {nm} re all dB", a, b, abs(a - b) <= 3, "rec +-3")


def main():
    a = sys.argv[1:]
    only = set(range(1, 16))
    rec = dict(REC)
    grooves = None
    hits = {"studio": a[0], "jazz": a[1]}
    i = 2
    while i < len(a):
        if a[i] == "--rec":
            rec = {"studio": a[i + 1], "jazz": a[i + 2]}
            i += 3
        elif a[i] == "--groove":
            grooves = a[i + 1:i + 5]
            i += 5
        elif a[i] == "--cues":
            only = {int(c) for c in a[i + 1].split(",")}
            i += 2
        else:
            raise SystemExit(f"unknown {a[i]}")
    for kit in ("studio", "jazz"):
        if hits[kit] != "-":
            cues(kit, load(hits[kit]), load(rec[kit]), only)
    if grooves and 15 in only:
        groove_cues("studio", grooves[0], grooves[2])
        groove_cues("jazz", grooves[1], grooves[3])
    print(f"{'cue':>3} {'kit':6} {'measure':38} {'ours':>27} | {'recording':>27} | target")
    tally = {}
    for cue, kit, name, g, r, ok, tgt in sorted(ROWS, key=lambda x: (x[0], x[1])):
        print(f"{cue:3d} {kit:6} {name:38} {fmt(g):>27} | {fmt(r):>27} | {'PASS' if ok else 'FAIL'} {tgt}")
        t = tally.setdefault((cue, kit), [0, 0])
        t[0] += bool(ok)
        t[1] += 1
    print("\nper cue and kit (checks passed / checks):")
    print("  " + "  ".join(f"{c}{k[0]} {p}/{n}" for (c, k), (p, n) in sorted(tally.items())))


if __name__ == "__main__":
    main()
