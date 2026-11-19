"""Round 40: what the listener hears — each phrase brought to -20 LUFS (BS.1770, as the ABX
kit does), then octave-band levels (dB, absolute) per window of each note (or of the phrase).

    python heard.py <dir> <phrase> <ref> <src> ...
"""
import os, sys
import numpy as np
import soundfile as sf
from scipy import signal

SR = 48000
OCT = [31.5, 63, 125, 250, 500, 1000, 2000, 4000, 8000, 16000]

# BS.1770 K-weighting at 48 kHz.
KB1, KA1 = [1.53512485958697, -2.69169618940638, 1.19839281085285], [1.0, -1.69065929318241, 0.73248077421585]
KB2, KA2 = [1.0, -2.0, 1.0], [1.0, -1.99004745483398, 0.99007225036621]


def lufs(x):
    y = signal.lfilter(KB2, KA2, signal.lfilter(KB1, KA1, x, axis=0), axis=0)
    n, h = int(0.4 * SR), int(0.1 * SR)
    z = np.array([(y[i:i + n] ** 2).mean(0).sum() for i in range(0, len(y) - n, h)])
    l = -0.691 + 10 * np.log10(z + 1e-30)
    g = z[l > -70]
    rel = -0.691 + 10 * np.log10(g.mean()) - 10
    g = z[(l > -70) & (l > rel)]
    return -0.691 + 10 * np.log10(g.mean())


def load(d, src, phrase):
    for ext in ('wav', 'flac'):
        p = os.path.join(d, src, f'{phrase}.{ext}')
        if os.path.exists(p):
            x, sr = sf.read(p, dtype='float64', always_2d=True)
            return x
    raise FileNotFoundError(f'{d}/{src}/{phrase}')


def matched(d, src, phrase):
    x = load(d, src, phrase)
    return x * 10 ** ((-20 - lufs(x)) / 20)


def octs(m):
    w = np.hanning(len(m))
    s = np.abs(np.fft.rfft(m * w)) ** 2 / (w ** 2).sum() * 2 / SR * (SR / len(m))
    f = np.fft.rfftfreq(len(m), 1 / SR)
    return np.array([10 * np.log10(s[(f >= c / 2 ** .5) & (f < c * 2 ** .5)].sum() + 1e-30) for c in OCT])


def main():
    d, phrase, *srcs = sys.argv[1:]
    X = {s: matched(d, s, phrase) for s in srcs}
    ref = srcs[0]
    if phrase == 'bass':
        segs = [(3.0 * i + a, 3.0 * i + b, f'key {k} {a:.2f}-{b:.2f}s')
                for i, k in enumerate([21, 24, 28, 33, 36]) for a, b in ((0, .08), (.08, .5), (.5, 2.4))]
    else:
        L = min(len(x) for x in X.values()) / SR
        segs = [(0, L, 'whole')]
    print(f'{phrase}: octave levels at -20 LUFS; ref absolute, others minus ref')
    print(' ' * 34 + ' '.join(f'{c:>6.0f}' for c in OCT))
    for a, b, lab in segs:
        R = octs(X[ref][int(a * SR):int(b * SR)].mean(1))
        print(f'{lab:22} {ref[:10]:10}' + ' '.join(f'{v:6.1f}' for v in R))
        for s in srcs[1:]:
            v = octs(X[s][int(a * SR):int(b * SR)].mean(1)) - R
            print(f'{"":22} {s[:10]:10}' + ' '.join(f'{v:6.1f}' for v in v))


if __name__ == '__main__':
    main()
