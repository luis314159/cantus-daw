"""The reference's sub-125 Hz while a sample plays: a floor (the same at pp and ff, the same
under a top key that has nothing there) or the note's (growing with the strike)?
Third-octave levels (dBFS, mid channel) over 0.3-1.3 s after each single note's onset (past the
key's thump) for keys >= C4, pp / mf / ff.   python -I hum2.py <notes dir>"""
import sys, os
import numpy as np, soundfile as sf
from scipy import signal

C = np.array([31.5, 40, 50, 63, 80, 100, 125])
KEYS = list(range(21, 109, 3))
d = sys.argv[1]
print('key dyn  ' + ' '.join(f'{c:6.0f}' for c in C) + '   note level (dBFS, 0.3-1.3 s)')
for dy in ('pp', 'mf', 'ff'):
    x, sr = sf.read(os.path.join(d, f'notes_{dy}.wav'), always_2d=True)
    m = x.mean(1)
    for i, k in enumerate(KEYS):
        if k < 60 or k % 12 not in (0, 6):
            continue
        s0 = int(i * 6 * sr)
        seg = m[s0:s0 + 3 * sr]
        on = int(np.argmax(np.abs(seg) > 0.05 * np.abs(seg).max()))
        y = seg[on + int(0.3 * sr):on + int(1.3 * sr)]
        f, p = signal.welch(y, sr, nperseg=16384)
        df = f[1] - f[0]
        v = [10 * np.log10(p[(f >= c / 2 ** (1 / 6)) & (f < c * 2 ** (1 / 6))].sum() * df + 1e-30) for c in C]
        print(f'{k:3} {dy}   ' + ' '.join(f'{a:6.1f}' for a in v) + f'   {10 * np.log10(np.mean(y ** 2)):6.1f}')
