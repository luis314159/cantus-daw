"""Candidate 7 probe: the top keys at pp/mf/ff - K-weighted level of the first second, the
string tone (around each partial) vs the knock (< 0.6 f0), and the fundamental at 3 s.
    python -I tprobe.py <notes dir> [<ref notes dir>]"""
import sys, os
import numpy as np, soundfile as sf
from scipy import signal
SR = 48000
KEYS = list(range(21, 109, 3))

def kweight(x):
    # ITU-R BS.1770 K-weighting (48 kHz coefficients)
    b = [1.53512485958697, -2.69169618940638, 1.19839281085285]
    a = [1.0, -1.69065929318241, 0.73248077421585]
    y = signal.lfilter(b, a, x, axis=0)
    b = [1.0, -2.0, 1.0]; a = [1.0, -1.99004745483398, 0.99007225036621]
    return signal.lfilter(b, a, y, axis=0)

def load(d, dyn):
    x, sr = sf.read(os.path.join(d, f'notes_{dyn}.wav'), dtype='float64', always_2d=True)
    return x

def onset(m):
    e = np.abs(m)
    return int(np.argmax(e > 0.02 * e.max()))

def stats(x, key):
    i = KEYS.index(key)
    y = x[int(i * 6 * SR):int((i + 1) * 6 * SR)]
    on = onset(y.mean(1)); y = y[on:]
    kw = kweight(y[:SR])
    lev = -0.691 + 10 * np.log10(np.mean(kw ** 2, 0).sum() + 1e-30)
    f0 = 440 * 2 ** ((key - 69) / 12)
    m = y[:SR].mean(1) * np.hanning(SR)
    X = np.abs(np.fft.rfft(m)) ** 2; F = np.fft.rfftfreq(SR, 1 / SR)
    knock = X[(F > 20) & (F < 0.6 * f0)].sum()
    tone = sum(X[(F > n * f0 * 0.97) & (F < n * f0 * 1.06)].sum() for n in range(1, 8))
    # fundamental envelope at 3 s re its peak (STFT, 50 ms frames)
    f, t, Z = signal.stft(y[:int(3.4 * SR)].mean(1), SR, nperseg=2400, noverlap=1800)
    b = (f > f0 * 0.97) & (f < f0 * 1.05)
    e = 10 * np.log10((np.abs(Z[b]) ** 2).sum(0) + 1e-30)
    pk = e[t < 0.15].max()
    at3 = e[(t > 2.9) & (t < 3.1)].mean() - pk
    at1 = e[(t > 0.9) & (t < 1.1)].mean() - pk
    return lev, 10 * np.log10(tone / (knock + 1e-30)), at1, at3

srcs = sys.argv[1:]
keys = [int(k) for k in os.environ.get('TKEYS', '84,87,90,93,96,99,102,105,108').split(',')]
for d in srcs:
    print('==', os.path.basename(os.path.normpath(d)))
    L = {dyn: load(d, dyn) for dyn in ('pp', 'mf', 'ff') if os.path.exists(os.path.join(d, f'notes_{dyn}.wav'))}
    for k in keys:
        row = []
        for dyn in L:
            lev, tk, a1, a3 = stats(L[dyn], k)
            row.append(f'{dyn} {lev:6.1f} t/k {tk:5.1f} f1@1s {a1:6.1f} @3s {a3:6.1f}')
        print(k, ' | '.join(row))
