"""Each top key's fundamental (and second partial) decay from the every-key render (keys_mf:
key k at (k-21)*4 s, held 3.5 s): slopes 0.05-0.5 / 0.5-1.5 / 1.5-3.3 s (dB/s), level at 3 s
re peak.   python -I fslope.py <keys_mf.wav> [<ref keys_mf.wav>] [lo hi]"""
import sys, os
import numpy as np, soundfile as sf
from scipy import signal
SR = 48000


def run(path, lo, hi, n=1, mode=os.environ.get("FMODE", "mono")):
    x, _ = sf.read(path, dtype='float64', always_2d=True)
    out = {}
    for k in range(lo, hi + 1):
        y2 = x[int((k - 21) * 4 * SR):int(((k - 21) * 4 + 3.45) * SR)]
        e = np.abs(y2.mean(1)); on = int(np.argmax(e > 0.02 * e.max())); y2 = y2[on:]
        f0 = 440 * 2 ** ((k - 69) / 12) * n
        chans = [y2.mean(1)] if mode == 'mono' else [y2[:, 0], y2[:, 1]]
        P = 0
        for y in chans:
            f, t, Z = signal.stft(y, SR, nperseg=2400, noverlap=1800)
            b = (f > f0 * 0.97) & (f < f0 * 1.05)
            P = P + (np.abs(Z[b]) ** 2).sum(0)
        env = 10 * np.log10(P + 1e-30)
        pk = env[t < 0.15].max()
        sl = lambda a, c: np.polyfit(t[(t > a) & (t < c)], env[(t > a) & (t < c)], 1)[0]
        out[k] = (sl(0.05, 0.5), sl(0.5, 1.5), sl(1.5, 3.0), env[(t > 2.9) & (t < 3.1)].mean() - pk)
    return out


lo, hi = (int(sys.argv[3]), int(sys.argv[4])) if len(sys.argv) > 4 else (84, 108)
n = int(sys.argv[5]) if len(sys.argv) > 5 else 1
a = run(sys.argv[1], lo, hi, n)
b = run(sys.argv[2], lo, hi, n) if len(sys.argv) > 2 else None
print('key  prompt  mid  late  @3s   (cand / ref)')
for k in a:
    r = a[k]; s = b[k] if b else (np.nan,) * 4
    print(f'{k:3d}  ' + '  '.join(f'{r[i]:6.1f}/{s[i]:6.1f}' for i in range(4)))
