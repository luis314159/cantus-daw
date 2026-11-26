"""Round 42 (chorda-c17): the fast run note by note (a strike every 125 ms, held 115 ms) — octave
levels in each note's first 25 ms, its 25-110 ms and the 10 ms after the next strike's start
are left out; each window's spectrum against the note's own 125 Hz-1 kHz power (so the run's
loudness does not count), averaged over the 29 notes, against both grands.

    python -I run.py <ref dir> <label>=<dir> ... (each holding fast_run.wav)
"""
import os, sys
import numpy as np
import soundfile as sf
from scipy import signal

SR = 48000
OCT = [125, 250, 500, 1000, 2000, 4000, 8000]


def spec(d):
    x, _ = sf.read(os.path.join(d, 'fast_run.wav'), always_2d=True)
    m = x.mean(1)
    a = np.abs(m)
    t0 = int(np.argmax(a > 0.05 * a.max()))
    out = []
    for w0, w1 in ((0.0, 0.025), (0.025, 0.11)):
        acc = np.zeros(len(OCT))
        for i in range(29):
            s = t0 + int((0.125 * i + w0) * SR)
            y = m[s:s + int((w1 - w0) * SR)]
            n = 1 << 14
            P = np.abs(np.fft.rfft(y * signal.windows.hann(len(y)), n)) ** 2
            F = np.fft.rfftfreq(n, 1 / SR)
            ref = P[(F >= 125) & (F < 1000)].sum()
            acc += np.array([10 * np.log10(P[(F >= c / 2 ** 0.5) & (F < c * 2 ** 0.5)].sum() / ref + 1e-30) for c in OCT])
        out.append(acc / 29)
    return out


def main():
    srcs = [('ref', sys.argv[1])] + [tuple(s.split('=', 1)) for s in sys.argv[2:]]
    R = spec(srcs[0][1])
    for j, w in enumerate(('0-25 ms', '25-110 ms')):
        print(f'== {w}: octave dB re 125 Hz-1 kHz (ref), others minus ref')
        print(f'{"":>12}' + ''.join(f'{c:7d}' for c in OCT))
        print(f'{"ref":>12}' + ''.join(f'{v:7.1f}' for v in R[j]))
        for lab, d in srcs[1:]:
            print(f'{lab[:12]:>12}' + ''.join(f'{v:+7.1f}' for v in spec(d)[j] - R[j]))


if __name__ == '__main__':
    main()
