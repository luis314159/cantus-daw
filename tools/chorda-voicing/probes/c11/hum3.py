import sys, os, numpy as np, soundfile as sf
from scipy import signal
C = 1000 * 2 ** (np.arange(-15, 13) / 3); SEL = C <= 200
def th(path, t0=0, t1=None):
    x, sr = sf.read(path, always_2d=True); m = x.mean(1)[int(t0*sr):(int(t1*sr) if t1 else None)]
    f, p = signal.welch(m, sr, nperseg=16384, noverlap=12288); df = f[1]-f[0]
    return np.array([10*np.log10(p[(f >= c/2**(1/6)) & (f < c*2**(1/6))].sum()*df + 1e-30) for c in C])
d = sys.argv[1]
print('Hz          ' + ' '.join(f'{c:6.0f}' for c in C[SEL]))
for ph, t0, t1 in (('middle_c', 0, 3), ('top', 0, 4), ('top', 4.4, 6.0), ('soft_chords', 0, 9), ('fast_run', 0, 4)):
    print(f'{ph:9}{t0:>3}  ' + ' '.join(f'{v:6.1f}' for v in th(os.path.join(d, ph + '.wav'), t0, t1)[SEL]))
