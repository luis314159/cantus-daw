"""Feature battery for one hit (mono segment + onset)."""
from common import *

OCT = [31.5, 63, 125, 250, 500, 1000, 2000, 4000, 8000, 16000]
WINS = [(0, 3), (3, 10), (10, 30), (30, 100), (100, 300), (300, 1000)]


def cut(m, on, pre_ms=2, length=None):
    a = max(0, on - int(pre_ms * SR / 1000))
    y = m[a:] if length is None else m[a:a + int(length * SR)]
    return y, on - a


def oct_levels(y):
    """Energy per octave band (dB) by FFT."""
    n = max(len(y), 256)
    X = np.abs(np.fft.rfft(np.pad(y, (0, n - len(y))) * 1.0)) ** 2
    f = np.fft.rfftfreq(n, 1 / SR)
    out = []
    for fc in OCT:
        k = (f >= fc / np.sqrt(2)) & (f < min(fc * np.sqrt(2), 23900))
        out.append(10 * np.log10(X[k].sum() + 1e-30))
    return np.array(out)


def window_table(m, on):
    """Octave levels per time window, re the loudest cell (dB)."""
    t = np.array([oct_levels(m[on + int(a * SR / 1000):on + int(b * SR / 1000)]) for a, b in WINS])
    # per-ms normalisation so windows of different length compare as power
    dur = np.array([(b - a) for a, b in WINS])[:, None]
    t = t - 10 * np.log10(dur)
    return t - np.nanmax(t)


def env_db(m, win_ms=1.0):
    n = max(1, int(win_ms * SR / 1000))
    e = np.sqrt(np.convolve(m * m, np.ones(n) / n, mode="same") + 1e-30)
    return 20 * np.log10(e)


def attack(m, on):
    e = env_db(m[on:on + int(0.2 * SR)], 0.5)
    pk = int(np.argmax(e))
    # 10 % -> 90 % amplitude rise
    lin = 10 ** (e / 20)
    mx = lin[pk]
    i10 = int(np.argmax(lin > 0.1 * mx)); i90 = int(np.argmax(lin > 0.9 * mx))
    return pk / SR * 1000, (i90 - i10) / SR * 1000


def level_at(m, on, ms_list, win_ms=20):
    e = env_db(m, win_ms)
    pk = e[on:on + int(0.1 * SR)].max()
    return [e[min(len(e) - 1, on + int(t * SR / 1000))] - pk for t in ms_list]


def t60_bands(m, on, bands=((40, 100), (100, 250), (250, 1000), (1000, 4000), (4000, 16000))):
    out = []
    for lo, hi in bands:
        y = bandpass(m, lo, hi)[on:]
        out.append((schroeder_t60(y, lo_db=-1, hi_db=-11), schroeder_t60(y, lo_db=-15, hi_db=-35)))
    return out  # (early-ish EDT-like, late)


def pitch_track(m, on, lo=30, hi=400, wins=((5, 25), (25, 60), (60, 150), (150, 400), (400, 1000))):
    """Strongest spectral peak within [lo,hi] per window (Hz), parabolic interpolation."""
    out = []
    for a, b in wins:
        y = m[on + int(a * SR / 1000):on + int(b * SR / 1000)]
        if len(y) < 64:
            out.append(np.nan); continue
        n = 1 << 18
        X = np.abs(np.fft.rfft(y * np.hanning(len(y)), n))
        f = np.fft.rfftfreq(n, 1 / SR)
        k = np.nonzero((f >= lo) & (f <= hi))[0]
        i = k[np.argmax(X[k])]
        a_, b_, c_ = np.log(X[i - 1:i + 2] + 1e-30)
        d = 0.5 * (a_ - c_) / (a_ - 2 * b_ + c_ + 1e-30)
        out.append(f[i] + d * (f[1] - f[0]))
    return out


def low_partials(m, on, a=60, b=800, lo=30, hi=1200, n_peaks=6):
    y = m[on + int(a * SR / 1000):on + int(b * SR / 1000)]
    n = 1 << 18
    X = 20 * np.log10(np.abs(np.fft.rfft(y * np.hanning(len(y)), n)) + 1e-12)
    f = np.fft.rfftfreq(n, 1 / SR)
    k = (f >= lo) & (f <= hi)
    from scipy.signal import find_peaks
    pk, _ = find_peaks(X[k], prominence=6, distance=int(5 / (f[1] - f[0])))
    ff = f[k][pk]; aa = X[k][pk]
    o = np.argsort(aa)[::-1][:n_peaks]
    o = o[np.argsort(ff[o])]
    return [(round(float(ff[i]), 1), round(float(aa[i] - aa.max()), 1)) for i in o]


def flat_band(m, on, a, b, lo, hi, nfft=2048):
    y = m[on + int(a * SR / 1000):on + int(b * SR / 1000)]
    f, t, S = signal.stft(y, SR, nperseg=min(nfft, len(y)), noverlap=min(nfft, len(y)) // 2)
    P = np.abs(S) ** 2
    k = (f >= lo) & (f <= hi)
    return float(np.mean(flatness(P[k], axis=0)))


def first_ms_centroid(m, on, ms=3):
    return centroid(m[on:on + int(ms * SR / 1000)])


def noise_floor(m, on):
    """Pre-onset level and last 50 ms level (dB re peak)."""
    pk = np.abs(m).max()
    pre = m[:max(on - int(0.0005 * SR), 1)]
    pre_db = db(np.sqrt(np.mean(pre ** 2))) - db(pk) if len(pre) > 20 else np.nan
    tail = m[-int(0.05 * SR):]
    return pre_db, db(np.sqrt(np.mean(tail ** 2))) - db(pk)


def proxy_diff(mg, ong, mr, onr, wins=((0, 10), (10, 50), (50, 200), (200, 800))):
    """Loudness-weighted 1/3-oct spectral difference per window: gen - rec (dB).
    Returns per window (weighted mean |diff|, signed tilt summary, worst bands)."""
    res = []
    for a, b in wins:
        c, G = thirdoct_spec(mg[ong + int(a * SR / 1000):ong + int(b * SR / 1000)])
        _, R = thirdoct_spec(mr[onr + int(a * SR / 1000):onr + int(b * SR / 1000)])
        wl = loudness_weights(c)
        audR = R + wl; audG = G + wl
        top = max(np.nanmax(audR), np.nanmax(audG))
        w = np.clip((np.maximum(audR, audG) - (top - 45)) / 45, 0, 1)  # bands within 45 dB of the loudest matter
        d = G - R
        wm = float(np.nansum(w * np.abs(d)) / (np.nansum(w) + 1e-9))
        worst = np.argsort(-(w * np.abs(np.nan_to_num(d))))[:3]
        res.append((wm, [(int(c[i]), round(float(d[i]), 1)) for i in worst]))
    return res
