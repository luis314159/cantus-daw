"""Is the reference's 31-125 Hz in middle C the note or the recording's floor? (candidate 11)
For each source's middle_c.wav: third-octave levels (dBFS, mid channel) over the note's first
second, over its last second (key up at 3 s, the phrase ends at 5 s) and, for the reference,
over the quietest second of its pp single notes (the samples' own floor between notes), and the
same bands' levels re the phrase's whole power (the metric's normalisation).
   python -I hum.py <dir> ... """
import sys, os
import numpy as np, soundfile as sf
from scipy import signal

C = 1000 * 2 ** (np.arange(-15, 13) / 3)
SEL = C <= 160


def thirds(x, sr, ref_tot=None):
    f, p = signal.welch(x, sr, nperseg=16384 if len(x) >= 16384 else len(x), noverlap=None)
    df = f[1] - f[0]
    out = []
    for c in C:
        s = (f >= c / 2 ** (1 / 6)) & (f < c * 2 ** (1 / 6))
        out.append(10 * np.log10(p[s].sum() * df + 1e-30))
    return np.array(out)


for d in sys.argv[1:]:
    x, sr = sf.read(os.path.join(d, 'middle_c.wav'), always_2d=True)
    m = x.mean(1)
    whole = thirds(m, sr)
    tot = 10 * np.log10(np.mean(m ** 2) + 1e-30)
    a = thirds(m[:sr], sr)
    b = thirds(m[int(4.0 * sr):int(5.0 * sr)], sr)
    print(f'== {d}  (phrase power {tot:.1f} dBFS)')
    print('Hz      ' + ' '.join(f'{c:6.0f}' for c in C[SEL]))
    print('0-1 s   ' + ' '.join(f'{v:6.1f}' for v in a[SEL]))
    print('4-5 s   ' + ' '.join(f'{v:6.1f}' for v in b[SEL]))
    print('re tot  ' + ' '.join(f'{v - tot:6.1f}' for v in whole[SEL]))
    pp = os.path.join(d, 'notes_pp.wav')
    if os.path.exists(pp):
        y, _ = sf.read(pp, always_2d=True)
        y = y.mean(1)
        # the quietest second: the 5-6 s gap after each pp note (key up at 3.5 s)
        q = min((np.mean(y[int((i * 6 + 5) * sr):int((i * 6 + 6) * sr)] ** 2), i) for i in range(10, 25))
        i = q[1]
        f = thirds(y[int((i * 6 + 5) * sr):int((i * 6 + 6) * sr)], sr)
        print(f'pp gap  ' + ' '.join(f'{v:6.1f}' for v in f[SEL]) + f'   (after key {21 + 3 * i})')
