"""Candidate 7 probe: the treble's dynamics and fundamentals against the reference.
Levels as s09_dynamics.py reads them (K-weighted first second, each piano aligned on its mf
mean over A0..C6), the steps between dynamics per register, and each top key's fundamental
at 1 s and 3 s re its peak.
    python -I tprobe2.py <cand notes dir> <ref notes dir>"""
import sys, os
import numpy as np, soundfile as sf
from scipy import signal
SR = 48000
KEYS = list(range(21, 109, 3))


def kweight(x):
    b = [1.53512485958697, -2.69169618940638, 1.19839281085285]
    a = [1.0, -1.69065929318241, 0.73248077421585]
    y = signal.lfilter(b, a, x, axis=0)
    return signal.lfilter([1.0, -2.0, 1.0], [1.0, -1.99004745483398, 0.99007225036621], y, axis=0)


def seg(x, key):
    i = KEYS.index(key)
    y = x[int(i * 6 * SR):int((i + 1) * 6 * SR)]
    e = np.abs(y.mean(1))
    return y[int(np.argmax(e > 0.02 * e.max())):]


def level(y):
    kw = kweight(y[:SR])
    return -0.691 + 10 * np.log10(np.mean(kw ** 2, 0).sum() + 1e-30)


def fund(y, key):
    f0 = 440 * 2 ** ((key - 69) / 12)
    f, t, Z = signal.stft(y[:int(3.4 * SR)].mean(1), SR, nperseg=2400, noverlap=1800)
    b = (f > f0 * 0.97) & (f < f0 * 1.05)
    e = 10 * np.log10((np.abs(Z[b]) ** 2).sum(0) + 1e-30)
    pk = e[t < 0.15].max()
    return e[(t > 0.9) & (t < 1.1)].mean() - pk, e[(t > 2.9) & (t < 3.1)].mean() - pk


def read(d):
    L, Fd = {}, {}
    for dyn in ('pp', 'mf', 'ff'):
        p = os.path.join(d, f'notes_{dyn}.wav')
        x, _ = sf.read(p, dtype='float64', always_2d=True)
        L[dyn] = np.array([level(seg(x, k)) for k in KEYS])
        if dyn == 'mf':
            Fd = {k: fund(seg(x, k), k) for k in KEYS if k >= 84}
    off = L['mf'][:22].mean()
    return {d_: v - off for d_, v in L.items()}, Fd


cache = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'ref_cache.npz')
c, cf = read(sys.argv[1])
if os.path.exists(cache) and len(sys.argv) < 4:
    z = np.load(cache, allow_pickle=True)
    r, rf = z['r'].item(), z['rf'].item()
else:
    r, rf = read(sys.argv[2])
    np.savez(cache, r=r, rf=rf)
K = np.array(KEYS)
for nm, a, b in [('A0-B2', 21, 47), ('C3-B4', 48, 71), ('C5-B6', 72, 95), ('C7-C8', 96, 108)]:
    m = (K >= a) & (K <= b)
    s = lambda L: ((L['mf'] - L['pp'])[m].mean(), (L['ff'] - L['mf'])[m].mean(), (L['mf'])[m].mean(), L['ff'][m].mean(), L['pp'][m].mean())
    x, y = s(c), s(r)
    print(f'{nm}: mf-pp {x[0]:5.1f}/{y[0]:5.1f}  ff-mf {x[1]:5.1f}/{y[1]:5.1f}  '
          f'level-ref pp {x[4]-y[4]:+5.1f} mf {x[2]-y[2]:+5.1f} ff {x[3]-y[3]:+5.1f}')
print('key  dmf   dff   dpp  ff-mf c/r     f1@1s c/r      f1@3s c/r')
for k in [k for k in KEYS if k >= 84]:
    i = KEYS.index(k)
    print(f'{k:3d} {c["mf"][i]-r["mf"][i]:+5.1f} {c["ff"][i]-r["ff"][i]:+5.1f} {c["pp"][i]-r["pp"][i]:+5.1f}  '
          f'{c["ff"][i]-c["mf"][i]:5.1f}/{r["ff"][i]-r["mf"][i]:5.1f}  {cf[k][0]:6.1f}/{rf[k][0]:6.1f}  {cf[k][1]:6.1f}/{rf[k][1]:6.1f}')
