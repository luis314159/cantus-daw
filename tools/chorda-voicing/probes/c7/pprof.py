"""Partial profile of keys (n = 1..16, L+R power, 0.05-1.05 s) from keys_mf renders, each key
normalised to its partials' total; prints source - reference per partial, mean over the keys.
    python -I pprof.py k1,k2,.. <dir1> ... <ref dir>"""
import sys, os
import numpy as np, soundfile as sf
SR = 48000; N = 1 << 17
keys = [int(k) for k in sys.argv[1].split(',')]; dirs = sys.argv[2:]
X = {d: sf.read(os.path.join(d, 'keys_mf.wav'), always_2d=True)[0] for d in dirs}
def prof(x, k):
    a = int(((k - 21) * 4 + 0.05) * SR); y = x[a:a + SR]
    P = sum(np.abs(np.fft.rfft(y[:, c] * np.hanning(len(y)), N)) ** 2 for c in range(2)); F = np.fft.rfftfreq(N, 1 / SR)
    f0 = 440 * 2 ** ((k - 69) / 12) * 2 ** (1 / 1200)
    v = []
    for n in range(1, 17):
        g = n * f0 * (1 + 0.0004 * n * n * (k / 60) ** 2) ** 0.5
        m = (F > g * 0.985) & (F < g * 1.03)
        v.append(P[m].max() if m.any() else 1e-30)
    v = np.array(v); return 10 * np.log10(v / v.sum())
R = np.array([prof(X[dirs[-1]], k) for k in keys])
print('n:      ' + ' '.join(f'{n:5d}' for n in range(1, 17)))
for d in dirs[:-1]:
    D = np.array([prof(X[d], k) for k in keys]) - R
    print(f'{os.path.basename(os.path.normpath(d))[:7]:7s} ' + ' '.join(f'{v:+5.1f}' for v in D.mean(0)))
    if os.environ.get('PERKEY'):
        for i, k in enumerate(keys):
            print(f'   {k:3d}   ' + ' '.join(f'{v:+5.1f}' for v in D[i]))
