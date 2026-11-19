"""The fast run, note by note (a strike every 0.125 s): third-octave spectra over the first 30 ms
of each note and over 30-120 ms, averaged over the 29 notes; prints source minus reference (each
window normalised on its 200 Hz-1.2 kHz power) and the windows' levels.
   python -I runatk.py <cand dir> <ref dir>"""
import sys, os, numpy as np, soundfile as sf
C = 1000 * 2 ** (np.arange(-6, 13) / 3)
def spec(d):
    x, sr = sf.read(os.path.join(d, 'fast_run.wav'), always_2d=True); m = x.mean(1)
    a = np.abs(m); t0 = int(np.argmax(a > 0.05 * a.max()))
    out = []
    for w0, w1 in ((0.0, 0.03), (0.03, 0.12)):
        P = 0; L = []
        for i in range(29):
            s = t0 + int(i * 0.125 * sr)
            y = m[s + int(w0 * sr):s + int(w1 * sr)]
            n = 8192; Y = np.abs(np.fft.rfft(y * np.hanning(len(y)), n)) ** 2; P = P + Y; L.append(np.mean(y ** 2))
        F = np.fft.rfftfreq(8192, 1 / sr)
        ref = P[(F > 200) & (F < 1200)].sum()
        out.append((np.array([10 * np.log10(P[(F >= c / 2 ** (1 / 6)) & (F < c * 2 ** (1 / 6))].sum() / ref + 1e-30) for c in C]), 10 * np.log10(np.mean(L))))
    return out
A, R = spec(sys.argv[1]), spec(sys.argv[2])
print('Hz        ' + ' '.join(f'{c:6.0f}' for c in C))
for k, nm in enumerate(('0-30 ms', '30-120 ms')):
    print(f'{nm:9} ' + ' '.join(f'{v:+6.1f}' for v in A[k][0] - R[k][0]) + f'   level {A[k][1] - R[k][1]:+.1f} dB re ref')
print(f'attack minus body, cand {A[0][1]-A[1][1]:+.1f} dB, ref {R[0][1]-R[1][1]:+.1f} dB')
