"""Round 42 (chorda-c17): the difference analysis's measures (report17.py draws them). Every
source is matched in loudness (BS.1770, -20 LUFS) and aligned on its first sound; the
Salamander and the bitKlavier are both references, their own difference the natural spread.
"""
import os
import numpy as np
import soundfile as sf
from scipy import signal

SR = 48000
KEYS = list(range(21, 109, 3))
SIXTHS = [50 * 2 ** (i / 6) for i in range(0, 50)]  # 50 Hz ... ~16 kHz
PHRASES = ['middle_c', 'soft_chords', 'fast_run', 'ballad', 'ff_chord', 'bass', 'top']


def lufs(x):
    from heard import lufs as l
    return l(x)


def load_phrase(d, ph):
    x, _ = sf.read(os.path.join(d, ph + '.wav'), dtype='float64', always_2d=True)
    x = x * 10 ** ((-20 - lufs(x)) / 20)
    e = np.abs(x.mean(1))
    pk = e[:int(0.5 * SR)].max()
    on = int(np.argmax(e > pk * 10 ** (-30 / 20)))
    return x, on


def band_edges(c, frac=6):
    return c / 2 ** (1 / (2 * frac)), c * 2 ** (1 / (2 * frac))


def ltas(x):
    """1/6-octave long-term spectrum (dB), mono."""
    f, P = signal.welch(x.mean(1), SR, nperseg=8192)
    out = []
    for c in SIXTHS:
        lo, hi = band_edges(c)
        out.append(10 * np.log10(P[(f >= lo) & (f < hi)].sum() + 1e-30))
    return np.array(out)


TSO = np.array([0, 0.01, 0.02, 0.035, 0.05, 0.075, 0.1, 0.15, 0.2, 0.3, 0.45, 0.65, 1.0, 1.5, 2.2, 3.2])


def since_onset_map(x, on, onsets):
    """Energy per 1/6-octave band (rows) per bin of time since the latest onset (TSO edges),
    dB, averaged over the frames that fall in each bin: the attack / early / late decay."""
    m = x.mean(1)
    n, hop = 1024, 256
    w = signal.windows.hann(n, sym=False)
    F = np.fft.rfftfreq(n, 1 / SR)
    idx = [((F >= band_edges(c)[0]) & (F < band_edges(c)[1])) for c in SIXTHS]
    acc = np.zeros((len(SIXTHS), len(TSO) - 1))
    cnt = np.zeros(len(TSO) - 1)
    t0 = [on + int(t * SR) for t in onsets]
    mm = np.concatenate([np.zeros(n // 2), m, np.zeros(n)])
    for s in range(0, len(m), hop):
        prev = [o for o in t0 if o <= s + int(0.004 * SR)]
        if not prev:
            continue
        dt = (s - max(prev)) / SR
        j = np.searchsorted(TSO, dt, side='right') - 1
        if j < 0 or j >= len(TSO) - 1:
            continue
        X = np.abs(np.fft.rfft(mm[s:s + n] * w)) ** 2
        acc[:, j] += [X[i].sum() for i in idx]
        cnt[j] += 1
    with np.errstate(divide='ignore'):
        return 10 * np.log10(acc / np.maximum(cnt, 1) + 1e-30), cnt


def stereo_bands(x):
    """Per 1/3 octave 63 Hz ... 16 kHz: correlation of L and R, side/mid (dB)."""
    cs = [63 * 2 ** (i / 3) for i in range(25)]
    out = []
    for c in cs:
        lo, hi = band_edges(c, 3)
        sos = signal.butter(4, [lo, min(hi, 0.45 * SR)], 'band', fs=SR, output='sos')
        L, R = signal.sosfilt(sos, x[:, 0]), signal.sosfilt(sos, x[:, 1])
        corr = (L * R).sum() / np.sqrt((L * L).sum() * (R * R).sum() + 1e-30)
        side = 10 * np.log10(((L - R) ** 2).sum() / ((L + R) ** 2).sum() + 1e-30)
        out.append((corr, side))
    return cs, np.array(out)


def tail_decay(x, end_t):
    """The tail after the phrase's last release: level (dB re the 50 ms before it) every
    50 ms for 1.5 s, in octave bands 125 Hz ... 8 kHz."""
    m = x.mean(1)
    a = int(end_t * SR)
    res = []
    for c in [125, 250, 500, 1000, 2000, 4000, 8000]:
        sos = signal.butter(4, [c / 1.414, c * 1.414], 'band', fs=SR, output='sos')
        z = signal.sosfilt(sos, m) ** 2
        ref = z[a - int(0.05 * SR):a].mean() + 1e-30
        res.append([10 * np.log10(z[a + int(t * SR):a + int((t + 0.05) * SR)].mean() / ref + 1e-30)
                    for t in np.arange(0, 1.5, 0.05)])
    return np.array(res)


def note_of(d, dyn, k):
    x, _ = sf.read(os.path.join(d, f'notes_{dyn}.wav'), dtype='float64', always_2d=True)
    a = int(KEYS.index(k) * 6.0 * SR)
    seg = x[a:a + int(6.0 * SR)]
    e = np.abs(seg.mean(1))
    on = int(np.argmax(e[:int(0.3 * SR)] > e[:int(0.3 * SR)].max() * 0.05))
    return seg[on:], on


def partial_table(m, f0, nmax=12):
    """Per partial: frequency (Hz), cents re n f0 sqrt(1+Bn^2)... (from the fit), B, level at
    the attack (dB re the strongest), the early decay (dB/s, 0.05-0.5 s) and the late (1-3 s)."""
    n_fft = 1 << 17
    y = m[:SR]
    P = np.abs(np.fft.rfft(y * np.hanning(len(y)), n_fft)) ** 2
    F = np.fft.rfftfreq(n_fft, 1 / SR)
    fr, B = [], 0.0
    for n in range(1, nmax + 1):
        g = n * f0 * np.sqrt(1 + B * n * n)
        if g > 18000:
            break
        w = (F > g - 0.1 * f0) & (F < g + 0.1 * f0)
        fr.append(F[np.flatnonzero(w)[np.argmax(P[w])]])
        if n >= 4:
            nn = np.arange(1, len(fr) + 1)
            B = max(0.0, float(np.polyfit(nn ** 2, (np.array(fr) / (nn * fr[0])) ** 2 - 1, 1)[0]))
    rows = []
    hop = int(0.025 * SR)
    for n, f in enumerate(fr, 1):
        # Heterodyne to the partial, low-pass, envelope in dB every 25 ms.
        t = np.arange(len(m)) / SR
        z = m * np.exp(-2j * np.pi * f * t)
        bw = min(0.3 * f0, 40.0)
        sos = signal.butter(2, bw, fs=SR, output='sos')
        e = np.abs(signal.sosfilt(sos, z))
        env = 20 * np.log10(np.array([e[i:i + hop].mean() for i in range(0, len(e) - hop, hop)]) + 1e-12)
        tt = np.arange(len(env)) * 0.025
        def slope(a, b):
            s = (tt >= a) & (tt < b)
            return float(np.polyfit(tt[s], env[s], 1)[0]) if s.sum() > 3 else np.nan
        rows.append(dict(n=n, f=f, cents=1200 * np.log2(f / (n * fr[0])), peak=float(env[:8].max()),
                         early=slope(0.05, 0.5), late=slope(1.0, 3.0)))
    top = max(r['peak'] for r in rows)
    for r in rows:
        r['peak'] -= top
    return rows, B


def release(d, dyn, keys):
    """After each key's release (3.5 s): octave levels in 0-50, 50-200, 200-800 ms against the
    100 ms before it, averaged over `keys`."""
    x, _ = sf.read(os.path.join(d, f'notes_{dyn}.wav'), dtype='float64', always_2d=True)
    m = x.mean(1)
    cs = [125, 250, 500, 1000, 2000, 4000, 8000]
    acc = np.zeros((3, len(cs)))
    for k in keys:
        a = int(KEYS.index(k) * 6.0 * SR)
        seg = np.abs(m[a:a + int(0.3 * SR)])
        on = a + int(np.argmax(seg > seg.max() * 0.05))
        r = on + int(3.5 * SR)
        for j, c in enumerate(cs):
            sos = signal.butter(4, [c / 1.414, c * 1.414], 'band', fs=SR, output='sos')
            z = signal.sosfilt(sos, m[r - int(0.2 * SR):r + int(0.9 * SR)]) ** 2
            base = z[int(0.1 * SR):int(0.2 * SR)].mean() + 1e-30
            for i, (w0, w1) in enumerate(((0, 0.05), (0.05, 0.2), (0.2, 0.8))):
                acc[i, j] += 10 * np.log10(z[int((0.2 + w0) * SR):int((0.2 + w1) * SR)].mean() / base + 1e-30)
    return cs, acc / len(keys)


def attack(d, dyn, keys):
    """Per key group: the rise (ms, -20 to -3 dB of the peak), the time to peak, and the
    attack's octave spectrum (0-30 ms) against its 30-300 ms."""
    x, _ = sf.read(os.path.join(d, f'notes_{dyn}.wav'), dtype='float64', always_2d=True)
    m = x.mean(1)
    cs = [125, 250, 500, 1000, 2000, 4000, 8000]
    rise, peak, spec = [], [], np.zeros(len(cs))
    for k in keys:
        a = int(KEYS.index(k) * 6.0 * SR)
        seg = m[a:a + int(0.5 * SR)]
        e = np.sqrt(signal.lfilter([0.04], [1, -0.96], seg ** 2))
        on = int(np.argmax(e > e.max() * 0.01))
        pk = int(np.argmax(e[:int(0.3 * SR)]))
        i3 = int(np.argmax(e > e[pk] * 10 ** (-3 / 20)))
        i20 = int(np.argmax(e > e[pk] * 10 ** (-20 / 20)))
        rise.append((i3 - i20) / SR * 1000)
        peak.append((pk - on) / SR * 1000)
        for j, c in enumerate(cs):
            sos = signal.butter(4, [c / 1.414, min(c * 1.414, 23000)], 'band', fs=SR, output='sos')
            z = signal.sosfilt(sos, seg[on:]) ** 2
            spec[j] += 10 * np.log10(z[:int(0.03 * SR)].mean() / (z[int(0.03 * SR):int(0.3 * SR)].mean() + 1e-30) + 1e-30)
    return float(np.median(rise)), float(np.median(peak)), cs, spec / len(keys)
