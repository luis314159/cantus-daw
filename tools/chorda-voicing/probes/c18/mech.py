"""Round 42 (chorda-c18): the top keys' mechanics, measured under the fundamental (where no string
of the key sounds) at a millisecond's resolution — what the owner calls "the hammers' sound and the
mechanism" in a recorded grand's top octave, against Chorda's "knock with reverb".

    python -I mech.py <label>=<notes dir> ... [--keys 96,99,102,105,108] [--dyn mf] [--peaks]

Per source (the mean over the keys, each key's level re its note's first 60 ms):
  env     the sub-fundamental band (60 Hz ... 0.7 f0, 8th-order Butterworth, causal) as a 1 ms
          RMS at chosen times from the string's onset (negative: before the strings sound);
  bands   third octaves 63 Hz ... 0.7 f0 over 0-8, 8-30, 30-100, 100-300 ms;
  decay   each third octave's slope (dB/s) fitted over 20-200 ms, and the share of the band's
          energy 30-300 ms against 0-30 ms (dB: the "dryness" — a knock that rings low);
  shape   the envelope's roughness 2-60 ms (the dB envelope's deviation from its own 10 ms
          smoothing: discrete transients read high, a smooth noise decay low) and, with --peaks,
          each key's transients (local maxima at least 3 dB over the 6 ms around them).
"""
import os, sys
import numpy as np
import soundfile as sf
from scipy import signal

SR = 48000
KEYS = list(range(21, 109, 3))
TIMES = [-20, -10, -5, -2, 0, 1, 2, 3, 5, 7, 10, 15, 20, 30, 40, 60, 80, 120, 160, 240, 320, 480]
WINS = [(0, 8), (8, 30), (30, 100), (100, 300)]
CEN = [63 * 2 ** (i / 3) for i in range(0, 18)]  # 63 Hz ... 3.2 kHz


def load(d, dyn):
    x, _ = sf.read(os.path.join(d, f'notes_{dyn}.wav'), dtype='float64', always_2d=True)
    return x.mean(1)


def onset(m, k):
    a = int(KEYS.index(k) * 6 * SR)
    seg = np.abs(m[a:a + int(0.3 * SR)])
    return a + int(np.argmax(seg > seg.max() * 0.05))


def sub_band(y, f0, lo=60.0):
    sos = signal.butter(8, [lo, 0.7 * f0], 'band', fs=SR, output='sos')
    return signal.sosfilt(sos, y)


def rms_env(z, win_ms=1.0, hop=12):
    n = int(win_ms * SR / 1000)
    c = np.concatenate([[0], np.cumsum(z * z)])
    i = np.arange(0, len(z) - n, hop)
    return i + n // 2, (c[i + n] - c[i]) / n


def measure(d, dyn, keys):
    m = load(d, dyn)
    out = []
    for k in keys:
        on = onset(m, k)
        f0 = 440 * 2 ** ((k - 69) / 12)
        pre = int(0.06 * SR)
        y = m[on - pre:on + int(0.6 * SR)]
        ref = (m[on:on + int(0.06 * SR)] ** 2).mean()
        z = sub_band(y, f0)
        t, e = rms_env(z)
        t = (t - pre) / SR * 1000
        edb = 10 * np.log10(e / ref + 1e-30)
        env = np.interp(TIMES, t, edb)
        # third octaves per window
        bands = np.full((len(WINS), len(CEN)), np.nan)
        slope = np.full(len(CEN), np.nan)
        dry = np.full(len(CEN), np.nan)
        for j, c in enumerate(CEN):
            lo, hi = c / 2 ** (1 / 6), c * 2 ** (1 / 6)
            if hi > 0.7 * f0:
                continue
            sos = signal.butter(4, [lo, hi], 'band', fs=SR, output='sos')
            b = signal.sosfilt(sos, y)
            tb, eb = rms_env(b, 2.0, 24)
            tb = (tb - pre) / SR * 1000
            for i, (w0, w1) in enumerate(WINS):
                s = (tb >= w0) & (tb < w1)
                bands[i, j] = 10 * np.log10(eb[s].mean() * (w1 - w0) / 60 / ref + 1e-30)
            s = (tb >= 20) & (tb < 200)
            slope[j] = np.polyfit(tb[s] / 1000, 10 * np.log10(eb[s] + 1e-30), 1)[0]
            early = eb[(tb >= 0) & (tb < 30)].sum()
            late = eb[(tb >= 30) & (tb < 300)].sum()
            dry[j] = 10 * np.log10(late / early + 1e-30)
        s = (t >= 2) & (t < 60)
        sm = np.convolve(edb, np.ones(40) / 40, 'same')
        rough = float(np.sqrt(np.mean((edb[s] - sm[s]) ** 2)))
        pk = []
        for i in np.flatnonzero(s | ((t >= -30) & (t < 2)) | ((t >= 60) & (t < 200))):
            w = edb[max(0, i - 12):i + 13]
            if edb[i] == w.max() and edb[i] - min(w[0], w[-1]) >= 3 and edb[i] > -45:
                pk.append((round(float(t[i]), 1), round(float(edb[i]), 1)))
        out.append(dict(env=env, bands=bands, slope=slope, dry=dry, rough=rough, peaks=pk))
    return out


def mean(rs, key):
    return np.nanmean(np.array([r[key] for r in rs]), 0)


def main():
    a = sys.argv[1:]
    keys, dyn = [96, 99, 102, 105, 108], 'mf'
    if '--keys' in a:
        i = a.index('--keys'); keys = [int(k) for k in a[i + 1].split(',')]; del a[i:i + 2]
    if '--dyn' in a:
        i = a.index('--dyn'); dyn = a[i + 1]; del a[i:i + 2]
    peaks = '--peaks' in a
    a = [x for x in a if x != '--peaks']
    srcs = [tuple(s.split('=', 1)) for s in a]
    R = {lab: measure(d, dyn, keys) for lab, d in srcs}
    print(f'== {dyn}, keys {keys}: the sub-fundamental band, 1 ms RMS, dB re the note (0-60 ms)')
    print(f'{"ms":>10}' + ''.join(f'{t:6d}' for t in TIMES))
    for lab, _ in srcs:
        print(f'{lab:>10}' + ''.join(f'{v:6.1f}' for v in mean(R[lab], 'env')))
    print(f'\n== third octaves per window (dB re note, per 60 ms), Hz:')
    print(f'{"":>16}' + ''.join(f'{c:6.0f}' for c in CEN))
    for i, w in enumerate(WINS):
        for lab, _ in srcs:
            v = np.nanmean(np.array([r['bands'][i] for r in R[lab]]), 0)
            print(f'{lab[:8] + " " + str(w[0]) + "-" + str(w[1]):>16}' + ''.join(f'{x:6.1f}' for x in v))
    print(f'\n== decay slope 20-200 ms (dB/s) and late/early (30-300 vs 0-30 ms, dB) per third octave')
    for lab, _ in srcs:
        print(f'{lab[:9] + " slope":>16}' + ''.join(f'{x:6.0f}' for x in mean(R[lab], 'slope')))
    for lab, _ in srcs:
        print(f'{lab[:9] + " late":>16}' + ''.join(f'{x:6.1f}' for x in mean(R[lab], 'dry')))
    print('\n== roughness of the envelope 2-60 ms (dB): ' + '  '.join(
        f'{lab} {np.mean([r["rough"] for r in R[lab]]):.2f}' for lab, _ in srcs))
    if peaks:
        for lab, _ in srcs:
            print(f'\n== {lab}: transients (ms, dB)')
            for k, r in zip(keys, R[lab]):
                print(f'  {k}: ' + ' '.join(f'{t:+.1f}/{v:.0f}' for t, v in r['peaks']))


if __name__ == '__main__':
    main()
