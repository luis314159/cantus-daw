"""Long-term spectra of a phrase (L+R power, 1/6 octave, 0.1-1.5 s after each note-on is not
needed: the whole phrase), each source normalised to its 0.2-5 kHz total; prints dB against the
last source.   python -I ltas.py <phrase> <dir1> <dir2> ... <ref dir>"""
import sys, os
import numpy as np, soundfile as sf
ph = sys.argv[1]; dirs = sys.argv[2:]
cent = 100 * 2 ** (np.arange(0, 6 * 5.7) / 6)
def spec(d):
    x, sr = sf.read(os.path.join(d, ph + '.wav'), always_2d=True)
    n = 8192; P = 0
    for c in range(2):
        for i in range(0, len(x) - n, n // 2):
            P = P + np.abs(np.fft.rfft(x[i:i + n, c] * np.hanning(n))) ** 2
    F = np.fft.rfftfreq(n, 1 / sr)
    b = np.array([P[(F >= f * 2 ** (-1 / 12)) & (F < f * 2 ** (1 / 12))].sum() for f in cent])
    tot = P[(F > 200) & (F < 5000)].sum()
    return 10 * np.log10(b / tot + 1e-30)
S = [spec(d) for d in dirs]
print('Hz     ' + ' '.join(f'{os.path.basename(os.path.normpath(d))[:8]:>8}' for d in dirs[:-1]) + '   (dB re ref, each normalised 0.2-5 kHz)')
for i, f in enumerate(cent):
    if 150 < f < 6000:
        print(f'{f:6.0f} ' + ' '.join(f'{s[i] - S[-1][i]:+8.1f}' for s in S[:-1]))
