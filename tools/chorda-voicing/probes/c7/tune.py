"""Every key's partials (n = 1..6) in cents against n x the equal-tempered A440 fundamental,
from keys_mf (key k at (k-21)*4 s): a 0.1-1.1 s window, both channels' power, the peak within
+-60 cents of the stretched estimate, parabolic interpolation.
    python -I tune.py <cand keys_mf.wav> <ref keys_mf.wav> [lo hi]"""
import sys
import numpy as np, soundfile as sf
SR = 48000
N = 1 << 18


def partials(x, k):
    a = int(((k - 21) * 4 + 0.1) * SR)
    y = x[a:a + SR]
    P = sum(np.abs(np.fft.rfft(y[:, c] * np.hanning(len(y)), N)) ** 2 for c in range(2))
    F = np.fft.rfftfreq(N, 1 / SR)
    f0 = 440 * 2 ** ((k - 69) / 12)
    out, prev = [], 0.0
    for n in range(1, 7):
        guess = n * f0 * 2 ** (prev / 1200)
        m = (F > guess * 2 ** (-60 / 1200)) & (F < guess * 2 ** (60 / 1200))
        if not m.any() or n * f0 > 16000:
            out.append(np.nan); continue
        i = np.flatnonzero(m)[np.argmax(P[m])]
        a_, b_, c_ = np.log(P[i - 1] + 1e-30), np.log(P[i] + 1e-30), np.log(P[i + 1] + 1e-30)
        d = 0.5 * (a_ - c_) / (a_ - 2 * b_ + c_)
        f = (i + d) * SR / N
        out.append(1200 * np.log2(f / (n * f0)))
        prev = out[-1]
    return out


lo, hi = (int(sys.argv[3]), int(sys.argv[4])) if len(sys.argv) > 4 else (21, 108)
c, _ = sf.read(sys.argv[1], always_2d=True); r, _ = sf.read(sys.argv[2], always_2d=True)
NM = ['C', 'C#', 'D', 'D#', 'E', 'F', 'F#', 'G', 'G#', 'A', 'A#', 'B']
print('key   cand p1..p6 (cents re ET n*f0)                 | ref p1..p6')
D = []
for k in range(lo, hi + 1):
    a, b = partials(c, k), partials(r, k)
    D.append(np.array(a) - np.array(b))
    print(f'{NM[k%12]}{k//12-1:<2} {k:3d} ' + ' '.join(f'{v:6.1f}' for v in a) + ' | ' + ' '.join(f'{v:6.1f}' for v in b))
D = np.array(D)
print('rms cand-ref per partial:', ' '.join(f'{np.sqrt(np.nanmean(D[:, i] ** 2)):.1f}' for i in range(6)))
