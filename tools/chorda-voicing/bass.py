"""Round 40: the bass notes alone, measured per note — model 1, model 2 (and any candidate)
against the Salamander. Each note's window starts at its onset (the phrase: A0 C1 E1 A1 C2 at
velocity 92, every 3 s, held 2.5 s).

    python bass.py <dir with source subfolders> salamander chorda1 chorda2 [cand ...]
"""
import os, sys
import numpy as np
import soundfile as sf

SR = 48000
KEYS = [21, 24, 28, 33, 36]
ONSETS = [3.0 * i for i in range(5)]
OCT = [31.5, 63, 125, 250, 500, 1000, 2000, 4000, 8000]
WINS = [(0.0, 0.06), (0.06, 0.4), (0.4, 1.2), (1.2, 2.4)]


def load(d, src, phrase='bass'):
    for ext in ('wav', 'flac'):
        p = os.path.join(d, src, f'{phrase}.{ext}')
        if os.path.exists(p):
            x, sr = sf.read(p, dtype='float64', always_2d=True)
            assert sr == SR
            return x
    raise FileNotFoundError(src)


def octs(m):
    """Energy per octave band (dB), of a mono window (Hann)."""
    w = np.hanning(len(m))
    s = np.abs(np.fft.rfft(m * w)) ** 2
    f = np.fft.rfftfreq(len(m), 1 / SR)
    return np.array([10 * np.log10(s[(f >= c / 2 ** .5) & (f < c * 2 ** .5)].sum() + 1e-30) for c in OCT])


def harmonics(m, f0, n=24):
    """Each partial's level (dB) over a window (peak within ±3 % of n f0 — the bass's
    inharmonicity moves the upper partials up)."""
    w = np.hanning(len(m))
    s = np.abs(np.fft.rfft(m * w, 8 * len(m)))
    f = np.fft.rfftfreq(8 * len(m), 1 / SR)
    out = []
    for k in range(1, n + 1):
        lo, hi = k * f0 * 0.99, k * f0 * (1.0 + 0.0004 * k * k) * 1.03
        sel = (f >= lo) & (f <= hi)
        out.append(20 * np.log10(s[sel].max() + 1e-30) if sel.any() else -300)
    return np.array(out)


def env(m, ms=10):
    n = SR * ms // 1000
    k = len(m) // n
    return 20 * np.log10(np.sqrt((m[:k * n].reshape(k, n) ** 2).mean(1)) + 1e-12)


def note(x, i):
    a = int(ONSETS[i] * SR)
    return x[a:a + int(2.9 * SR)]


def describe(x, i):
    seg = note(x, i)
    m = seg.mean(1)
    e = env(m)
    pk = e.max()
    ipk = int(e.argmax())
    f0 = 440 * 2 ** ((KEYS[i] - 69) / 12)
    r = {}
    r['peak_db'] = pk
    r['rise_ms'] = 10 * ipk
    # decay: 0.1-0.6 s and 0.6-2.0 s after the peak (dB/s)
    def slope(t0, t1):
        a, b = int(t0 * 100), int(t1 * 100)
        t = np.arange(a, b) / 100
        return -np.polyfit(t, e[a:b], 1)[0]
    r['dec_early'] = slope(0.1, 0.6)
    r['dec_late'] = slope(0.6, 2.2)
    tot = 10 * np.log10((m[:int(1.2 * SR)] ** 2).sum())
    r['oct'] = [octs(m[int(a * SR):int(b * SR)]) - tot for a, b in WINS]
    r['harm'] = harmonics(m[int(0.1 * SR):int(1.1 * SR)], f0)
    r['corr'] = np.corrcoef(seg[:int(1.5 * SR), 0], seg[:int(1.5 * SR), 1])[0, 1]
    # the share of the first 1.2 s's energy between 300 and 700 Hz, and below 150 Hz
    s = np.abs(np.fft.rfft(m[:int(1.2 * SR)])) ** 2
    f = np.fft.rfftfreq(int(1.2 * SR), 1 / SR)
    r['lowmid'] = 10 * np.log10(s[(f > 300) & (f < 700)].sum() / s.sum())
    r['low'] = 10 * np.log10(s[f < 150].sum() / s.sum())
    r['centroid'] = (f * s).sum() / s.sum()
    return r


def main():
    d = sys.argv[1]
    srcs = sys.argv[2:]
    X = {s: load(d, s) for s in srcs}
    ref = srcs[0]
    for i, k in enumerate(KEYS):
        print(f'=== key {k}')
        R = {s: describe(X[s], i) for s in srcs}
        print(f"{'src':10} {'peak':>6} {'rise':>5} {'dEarly':>6} {'dLate':>6} {'corr':>5} {'lowmid':>7} {'<150':>6} {'centr':>6}")
        for s in srcs:
            r = R[s]
            print(f"{s:10} {r['peak_db']:6.1f} {r['rise_ms']:5d} {r['dec_early']:6.1f} {r['dec_late']:6.1f} {r['corr']:5.2f} {r['lowmid']:7.1f} {r['low']:6.1f} {r['centroid']:6.0f}")
        for w, (a, b) in enumerate(WINS):
            print(f'  oct {a:.2f}-{b:.2f}s (minus ref)  ' + ' '.join(f'{c:>6.0f}' for c in OCT))
            for s in srcs[1:]:
                print(f'    {s:10}' + ' '.join(f'{v:6.1f}' for v in R[s]['oct'][w] - R[ref]['oct'][w]))
        hr = R[ref]['harm'] - R[ref]['harm'].max()
        print('  harmonics 0.1-1.1 s (ref rel. to its max; others minus ref, each rel. to own max)')
        print(f'    {"ref":10}' + ' '.join(f'{v:5.0f}' for v in hr))
        for s in srcs[1:]:
            h = R[s]['harm'] - R[s]['harm'].max()
            print(f'    {s:10}' + ' '.join(f'{v:5.0f}' for v in h - hr))


if __name__ == '__main__':
    main()
