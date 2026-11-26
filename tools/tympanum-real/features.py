"""Per-hit features of a drum stroke (round 42, tympanum-real) — the drums library's measures.

`features(x, fs, kind)` takes one stroke (stereo, float, the onset near the start) and
returns a flat dict. Every measure is on the kit's own mix (what a user hears on loading
it), so a recorded kit's room and overheads and a modelled kit's virtual mics are part of it.

What each key means (the report's chart names in brackets):

- `onset_s`, `peak_db`, `rms80_db` (level: the first 80 ms after the onset), `energy_db` (the whole hit) —
  loudness and the dynamics across velocities [dinámica];
- `rise_ms` (10 → 90 % of the envelope's first peak, 0.25-ms frames), `crest10_db` (peak over
  RMS of the first 10 ms) [ataque];
- `cent_<w>` the spectral centroid (Hz) in windows `a` 0–3 ms, `b` 3–20, `c` 20–80, `d`
  80–300, `e` 300–1000 ms [brillo]; `stick_hf` the first 5 ms's share above 5 kHz (dB)
  [palo];
- `band_<w>_<f>` octave-band level (dB re the hit's peak band) per window, `t30_<f>` each
  octave band's decay (Schroeder, −5 → −35 dB, ms; T20 doubled where the band ends sooner)
  [caída];
- drums only: `f0` the lowest strong mode (Hz) at 20–200 ms, `glide_c` its pitch at 10–60 ms
  over 150–450 ms (cents), `r2`, `r3` the next two modes' ratios to f0 and `upper_db` the
  energy at 1.3–3 × f0 over that at f0 ± 12 % (edge strokes excite the asymmetric modes)
  [modos, golpe];
- snare: `wires_db` the 3–10 kHz noise at 40–250 ms over the whole hit's energy there,
  `wires_t_ms` its decay to −20 dB, `wires_flat` its spectral flatness [bordonera];
- cymbals and hats: `flat_4_12` spectral flatness at 4–12 kHz (50–500 ms), `bloom_ms` when the
  4–16 kHz band peaks, `cent_traj` the centroid at 0.1, 0.3, 0.7, 1.5 s [lavado];
- `late_hf_db` 4–10 kHz energy at 60–400 ms over 0–60 ms (toms and kick: the snare's
  sympathetic buzz and the room) [resonancia];
- `side_early_db`, `side_late_db` the side over mid energy at 0–20 ms and 100–600 ms,
  `corr_late` the channels' correlation then, `pan_db` left over right (+ = left),
  `c50_db` the energy before 50 ms over after it [imagen, sala];
- `spec3` the third-octave spectrum of the first 200 ms (dB), for take-to-take variation.
"""
import numpy as np
from scipy import signal

OCT = [63, 125, 250, 500, 1000, 2000, 4000, 8000, 16000]
WINS = {"a": (0.0, 0.003), "b": (0.003, 0.020), "c": (0.020, 0.080), "d": (0.080, 0.300),
        "e": (0.300, 1.000)}
DRUMS = {"kick", "snare", "racktom", "floortom"}
CYMBALS = {"crash", "ride", "splash", "china", "stack", "hihat"}
THIRDS = [25 * 2 ** (k / 3) for k in range(31)]  # 25 Hz … 20 kHz


def db(x, floor=1e-12):
    return 10.0 * np.log10(np.maximum(x, floor))


def onset(m, fs):
    """The first sample over −40 dB of the peak, walked back to the envelope's last
    minimum (a stroke's very first sound, not where it is loud)."""
    a = np.abs(m)
    pk = a.max()
    if pk <= 0:
        return 0
    i = int(np.argmax(a > pk * 0.01))
    j = max(0, i - int(0.002 * fs))
    return j + int(np.argmin(a[j:i + 1])) if i > j else i


def seg(x, fs, t0, t1):
    return x[int(t0 * fs):max(int(t0 * fs) + 1, int(t1 * fs))]


def centroid(x, fs):
    if len(x) < 16:
        return float("nan")
    n = max(256, 1 << int(np.ceil(np.log2(len(x)))))
    X = np.abs(np.fft.rfft(x * np.hanning(len(x)), n)) ** 2
    f = np.fft.rfftfreq(n, 1 / fs)
    s = X[f > 20].sum()
    return float((f * X).sum() / s) if s > 0 else float("nan")


_SOS = {}


def bandpass(fc, fs, width=2 ** 0.5):
    key = (fc, fs, width)
    if key not in _SOS:
        lo, hi = fc / width, min(fc * width, 0.95 * fs / 2)
        _SOS[key] = signal.butter(4, [lo, hi], btype="band", fs=fs, output="sos")
    return _SOS[key]


def band_energy(x, fs, lo, hi):
    n = max(1024, 1 << int(np.ceil(np.log2(max(len(x), 2)))))
    X = np.abs(np.fft.rfft(x, n)) ** 2
    f = np.fft.rfftfreq(n, 1 / fs)
    return float(X[(f >= lo) & (f < hi)].sum())


def schroeder_t(y, fs, lo=-5.0, hi=-35.0):
    e = y ** 2
    if e.sum() <= 0:
        return float("nan")
    edc = np.cumsum(e[::-1])[::-1]
    edc = db(edc / edc[0])
    t = np.arange(len(edc)) / fs
    i0 = np.argmax(edc <= lo)
    i1 = np.argmax(edc <= hi)
    if edc[-1] > hi or i1 <= i0:
        i1 = np.argmax(edc <= -25.0)
        if edc[-1] > -25.0 or i1 <= i0:
            return float("nan")
        return float((t[i1] - t[i0]) * 60.0 / 20.0 * 1000.0 / 3.0)  # T20 → T60/2 ≈ T30
    return float((t[i1] - t[i0]) * 60.0 / 30.0 * 1000.0 / 2.0)


def flatness(x, fs, lo, hi):
    f, P = signal.welch(x, fs, nperseg=min(2048, len(x)))
    P = P[(f >= lo) & (f < hi)]
    if len(P) == 0 or P.mean() <= 0:
        return float("nan")
    return float(np.exp(np.mean(np.log(P + 1e-20))) / P.mean())


def modes(x, fs, n=3):
    """The strongest low peaks of a drum's 20–200 ms spectrum: f0 and the next ones."""
    y = seg(x, fs, 0.020, 0.200)
    N = 1 << 15
    X = np.abs(np.fft.rfft(y * np.hanning(len(y)), N))
    f = np.fft.rfftfreq(N, 1 / fs)
    keep = (f > 30) & (f < 1500)
    Xd = db(X ** 2)
    pk, _ = signal.find_peaks(np.where(keep, Xd, -300), prominence=6, distance=int(8 / (fs / N)))
    if len(pk) == 0:
        return []
    top = Xd[pk].max()
    strong = [p for p in pk if Xd[p] > top - 18]
    return [float(f[p]) for p in sorted(strong)[:n]], f, X


def peak_freq(x, fs, lo, hi):
    N = 1 << 15
    X = np.abs(np.fft.rfft(x * np.hanning(len(x)), N))
    f = np.fft.rfftfreq(N, 1 / fs)
    m = (f >= lo) & (f <= hi)
    if not m.any() or X[m].max() <= 0:
        return float("nan")
    i = np.argmax(np.where(m, X, 0))
    if 0 < i < len(X) - 1:  # parabolic interpolation on the log magnitude
        a, b, c = np.log(X[i - 1:i + 2] + 1e-20)
        d = 0.5 * (a - c) / (a - 2 * b + c) if (a - 2 * b + c) != 0 else 0.0
        return float((i + d) * fs / N)
    return float(f[i])


def thirds(x, fs):
    n = 1 << 14
    X = np.abs(np.fft.rfft(x * np.hanning(len(x)), n)) ** 2
    f = np.fft.rfftfreq(n, 1 / fs)
    out = []
    for c in THIRDS:
        m = (f >= c / 2 ** (1 / 6)) & (f < c * 2 ** (1 / 6))
        out.append(float(db(X[m].sum())) if m.any() else -120.0)
    return out


def features(st, fs, kind):
    st = np.asarray(st, dtype=np.float64)
    L, R = st[:, 0], st[:, 1]
    m = 0.5 * (L + R)
    s = 0.5 * (L - R)
    o = onset(m, fs)
    m, s, L, R = m[o:], s[o:], L[o:], R[o:]
    out = {"onset_s": o / fs}
    pk = np.abs(m).max()
    if pk <= 1e-9:
        return None
    out["peak_db"] = float(20 * np.log10(pk))
    out["rms80_db"] = float(db(np.mean(seg(m, fs, 0, 0.08) ** 2)))
    out["energy_db"] = float(db(np.sum(m ** 2) / fs))
    # attack
    fr = int(0.00025 * fs)
    env = np.sqrt(np.convolve(m[: int(0.05 * fs)] ** 2, np.ones(fr) / fr, mode="same"))
    ip = int(np.argmax(env))
    e10, e90 = 0.1 * env[ip], 0.9 * env[ip]
    i10 = int(np.argmax(env[: ip + 1] >= e10))
    i90 = int(np.argmax(env[: ip + 1] >= e90))
    out["rise_ms"] = (i90 - i10) / fs * 1000.0
    out["peak_ms"] = ip / fs * 1000.0
    a10 = seg(m, fs, 0, 0.010)
    out["crest10_db"] = float(20 * np.log10(np.abs(a10).max() / (np.sqrt(np.mean(a10 ** 2)) + 1e-20)))
    for w, (t0, t1) in WINS.items():
        out[f"cent_{w}"] = centroid(seg(m, fs, t0, t1), fs)
    a5 = seg(m, fs, 0, 0.005)
    out["stick_hf"] = float(db(band_energy(a5, fs, 5000, 20000)) - db(band_energy(a5, fs, 20, 20000)))
    # octave bands per window, and decays
    bands = {}
    for fc in OCT:
        if fc * 2 ** 0.5 >= fs / 2:
            continue
        y = signal.sosfilt(bandpass(fc, fs), m)
        bands[fc] = y
        out[f"t30_{fc}"] = schroeder_t(y, fs)
        for w, (t0, t1) in WINS.items():
            out[f"band_{w}_{fc}"] = float(db(np.mean(seg(y, fs, t0, t1) ** 2)))
    ref = max(out[f"band_{w}_{fc}"] for w in WINS for fc in bands)
    for w in WINS:
        for fc in bands:
            out[f"band_{w}_{fc}"] -= ref
    # the drums' modes
    if kind in DRUMS:
        lo, hi = (30, 200) if kind == "kick" else (60, 600)
        f0 = peak_freq(seg(m, fs, 0.020, 0.200), fs, lo, hi)
        out["f0"] = f0
        if f0 == f0:
            # round 40's windows (the strongest low partial at 10-60 ms against 150-450 ms):
            # 50 ms is the shortest that still resolves a kick's 50-60 Hz
            fe = peak_freq(seg(m, fs, 0.010, 0.060), fs, f0 * 0.85, f0 * 1.3)
            fl = peak_freq(seg(m, fs, 0.150, 0.450), fs, f0 * 0.8, f0 * 1.2)
            out["glide_c"] = float(1200 * np.log2(fe / fl)) if fe == fe and fl == fl and fl > 0 else float("nan")
            y = seg(m, fs, 0.020, 0.200)
            e0 = band_energy(y, fs, f0 * 0.88, f0 * 1.12)
            eu = band_energy(y, fs, f0 * 1.3, f0 * 3.0)
            out["upper_db"] = float(db(eu) - db(e0))
            r = modes(m, fs)
            if r:
                fs_, _, _ = r
                hi_modes = [x for x in fs_ if x > f0 * 1.2]
                out["r2"] = hi_modes[0] / f0 if len(hi_modes) > 0 else float("nan")
                out["r3"] = hi_modes[1] / f0 if len(hi_modes) > 1 else float("nan")
    if kind == "snare":
        y = signal.sosfilt(signal.butter(4, [3000, 10000], btype="band", fs=fs, output="sos"), m)
        e = y ** 2
        out["wires_db"] = float(db(np.sum(seg(e, fs, 0.04, 0.25))) - db(np.sum(e)))
        fr = int(0.005 * fs)
        env = db(np.convolve(e, np.ones(fr) / fr, mode="same"))
        i40 = int(0.04 * fs)
        if len(env) > i40:
            tail = env[i40:]
            k = np.argmax(tail < env[i40] - 20.0)
            out["wires_t_ms"] = float(k / fs * 1000.0 + 40.0) if k > 0 else float("nan")
        out["wires_flat"] = flatness(seg(m, fs, 0.04, 0.25), fs, 3000, 10000)
    if kind in CYMBALS:
        out["flat_4_12"] = flatness(seg(m, fs, 0.05, 0.5), fs, 4000, 12000)
        y = signal.sosfilt(signal.butter(4, [4000, 16000], btype="band", fs=fs, output="sos"), m)
        fr = int(0.010 * fs)
        env = np.convolve(y ** 2, np.ones(fr) / fr, mode="same")
        out["bloom_ms"] = float(np.argmax(env[: int(1.5 * fs)]) / fs * 1000.0)
        for t in (0.1, 0.3, 0.7, 1.5):
            out[f"cent_t{t}"] = centroid(seg(m, fs, t - 0.05, t + 0.05), fs)
    if kind in DRUMS:
        e0 = band_energy(seg(m, fs, 0, 0.06), fs, 4000, 10000)
        e1 = band_energy(seg(m, fs, 0.06, 0.4), fs, 4000, 10000)
        out["late_hf_db"] = float(db(e1) - db(e0))
    # image
    def ratio(a, b, t0, t1):
        return float(db(np.sum(seg(a, fs, t0, t1) ** 2)) - db(np.sum(seg(b, fs, t0, t1) ** 2)))
    out["side_early_db"] = ratio(s, m, 0, 0.02)
    out["side_late_db"] = ratio(s, m, 0.1, 0.6)
    l, r = seg(L, fs, 0.1, 0.6), seg(R, fs, 0.1, 0.6)
    den = np.sqrt(np.sum(l ** 2) * np.sum(r ** 2))
    out["corr_late"] = float(np.sum(l * r) / den) if den > 0 else float("nan")
    out["pan_db"] = ratio(L, R, 0, 0.3)
    e_early = np.sum(seg(m, fs, 0, 0.05) ** 2)
    e_late = np.sum(seg(m, fs, 0.05, 3.0) ** 2)
    out["c50_db"] = float(db(e_early) - db(e_late))
    out["spec3"] = thirds(seg(m, fs, 0, 0.2), fs)
    return out
