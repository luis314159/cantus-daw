"""The treble's velocity sweep (phrase `vsweep`: C5 ... C8 every minor third, 14 velocities
each, a strike every 2 s): each strike's K-weighted level over its first second, candidate
against the reference, both aligned on C5 at velocity 75. Prints per key the difference at
each velocity, and per register the steps 30->75 and 75->115.
    python -I vsweep.py <cand vsweep.wav> <ref vsweep.wav>"""
import sys
import numpy as np, soundfile as sf
from scipy import signal
SR = 48000
KEYS = [72, 75, 78, 81, 84, 87, 90, 93, 96, 99, 102, 105, 108]
VELS = [20, 30, 40, 50, 60, 70, 75, 80, 90, 100, 110, 115, 120, 127]


def kweight(x):
    y = signal.lfilter([1.53512485958697, -2.69169618940638, 1.19839281085285],
                       [1.0, -1.69065929318241, 0.73248077421585], x, axis=0)
    return signal.lfilter([1.0, -2.0, 1.0], [1.0, -1.99004745483398, 0.99007225036621], y, axis=0)


def levels(path):
    x, _ = sf.read(path, dtype='float64', always_2d=True)
    kw = kweight(x)
    L = np.zeros((len(KEYS), len(VELS)))
    for i in range(len(KEYS)):
        for j in range(len(VELS)):
            n = i * len(VELS) + j
            a = int(n * 2.0 * SR)
            seg = x[a:a + 2 * SR].mean(1)
            on = a + int(np.argmax(np.abs(seg) > 0.02 * np.abs(seg).max()))
            y = kw[on:on + SR]
            L[i, j] = -0.691 + 10 * np.log10(np.mean(y ** 2, 0).sum() + 1e-30)
    return L - L[0, VELS.index(75)]


c, r = levels(sys.argv[1]), levels(sys.argv[2])
np.save(sys.argv[1] + '.npy', c)
print('key   ' + ' '.join(f'{v:5d}' for v in VELS) + '   (cand - ref, dB; aligned on C5 v75)')
for i, k in enumerate(KEYS):
    print(f'{k:3d}   ' + ' '.join(f'{d:+5.1f}' for d in c[i] - r[i]))
j30, j75, j115 = VELS.index(30), VELS.index(75), VELS.index(115)
print('steps   v30->75 c/r   v75->115 c/r')
for nm, a, b in (('C5-B6', 72, 95), ('C7-C8', 96, 108)):
    m = [i for i, k in enumerate(KEYS) if a <= k <= b]
    print(f'{nm}  {np.mean(c[m, j75] - c[m, j30]):5.1f}/{np.mean(r[m, j75] - r[m, j30]):5.1f}   '
          f'{np.mean(c[m, j115] - c[m, j75]):5.1f}/{np.mean(r[m, j115] - r[m, j75]):5.1f}')
d = (c - r)[:, VELS.index(40):]
print(f'rms of the difference v40..v127: {np.sqrt(np.mean(d ** 2)):.2f} dB; C7-C8 rows: '
      f'{np.sqrt(np.mean(d[8:] ** 2)):.2f}')
