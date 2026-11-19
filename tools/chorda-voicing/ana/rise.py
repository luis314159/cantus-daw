"""Lead-in (ms, -50 -> -20 dB of the 5-ms-envelope's peak) of a phrase's first note."""
import sys, numpy as np, soundfile as sf
for d in sys.argv[1:]:
    x, sr = sf.read(d + '/middle_c.wav', always_2d=True)
    m = np.abs(x).max(1)[: int(0.2 * sr)]
    n = int(0.0005 * sr)
    e = np.array([m[i:i + n].max() for i in range(0, len(m) - n, n)])
    pk = e.max()
    a = np.argmax(e > pk * 10 ** (-50 / 20)); b = np.argmax(e > pk * 10 ** (-20 / 20))
    print(f'{d.split("/")[-1]:14} lead-in {(b - a) * 0.5:5.1f} ms   first 4 ms re peak {20*np.log10(e[a:a+8].max()/pk):5.1f} dB')
