"""Onset vibration of a phrase: band envelopes (L+R power, 10 ms), the slow decay taken out
(a 150 ms median), the modulation's rms (dB) in 0.08-0.6 s per band, and the strongest
modulation rate.   python -I vib.py <phrase> <dir> ..."""
import sys, os
import numpy as np, soundfile as sf
from scipy import signal
ph = sys.argv[1]
for d in sys.argv[2:]:
    x, sr = sf.read(os.path.join(d, ph + '.wav'), always_2d=True)
    on = int(np.argmax(np.abs(x).sum(1) > 0.05 * np.abs(x).sum(1).max()))
    x = x[on:on + sr]
    out = []
    for lo, hi in ((40, 150), (150, 500), (500, 1500), (1500, 5000)):
        b, a = signal.butter(4, [lo / (sr / 2), hi / (sr / 2)], 'band')
        y = signal.filtfilt(b, a, x, axis=0)
        e = (y ** 2).sum(1)
        h = int(0.01 * sr); env = 10 * np.log10(np.array([e[i:i + h].mean() for i in range(0, len(e) - h, h)]) + 1e-20)
        sm = signal.medfilt(env, 15)
        r = (env - sm)[8:60]
        f = np.fft.rfftfreq(len(r), 0.01); R = np.abs(np.fft.rfft(r * np.hanning(len(r))))
        m = (f > 2) & (f < 30)
        out.append(f'{lo}-{hi}: {np.sqrt(np.mean(r ** 2)):.2f} dB @{f[m][np.argmax(R[m])]:.0f} Hz')
    print(f'{os.path.basename(os.path.normpath(d)):10s} ' + ' | '.join(out))
