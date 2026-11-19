"""The top phrase's staccato: energy > 2 kHz in 5-80 ms and 80-195 ms against the first 5 ms."""
import sys, numpy as np, soundfile as sf
from scipy import signal
ons = [0.0, 0.3, 0.6, 0.9]
for d in sys.argv[1:]:
    x, sr = sf.read(d + '/top.wav', always_2d=True)
    b, a = signal.butter(4, 2000 / (sr / 2), 'high'); h = signal.filtfilt(b, a, x, axis=0)
    m = np.abs(x).max(1); lat = np.argmax(m > m.max() * 10 ** (-40 / 20)) / sr
    n = int(0.0005 * sr); nb = 400; acc = np.zeros(nb)
    for o in ons:
        i = int((o + lat) * sr) - 10 * n
        if i < 0: i = 0
        seg = h[i:i + nb * n]
        acc += (seg ** 2).sum(1).reshape(nb, n).mean(1)
    tt = (np.arange(nb) - 10) * 0.5
    d0 = acc[(tt >= 0) & (tt < 5)].sum(); e = acc[(tt >= 5) & (tt < 80)].sum(); l = acc[(tt >= 80) & (tt < 195)].sum()
    print(f'{d.split("/")[-1]:12} early {10*np.log10(e/d0):6.1f}  late {10*np.log10(l/d0):6.1f}')
