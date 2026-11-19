"""Round 40: each octave band's level over time for one note of a phrase (loudness-matched
at -20 LUFS like the ABX), sampled at a few instants — attack, early decay, sustain.

    python bandenv.py <dir> <phrase> <onset s> <ref> <src> ...
"""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '.'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys
import numpy as np
from scipy import signal
from heard import matched, SR

BANDS = [63, 125, 250, 500, 1000, 2000, 4000]
TIMES = [0.005, 0.015, 0.03, 0.06, 0.1, 0.2, 0.4, 0.8, 1.6, 2.4]


def band_env(m, c):
    sos = signal.butter(4, [c / 2 ** .5, c * 2 ** .5], 'bandpass', fs=SR, output='sos')
    y = signal.sosfilt(sos, m)
    n = int(0.005 * SR)
    k = len(y) // n
    return 10 * np.log10((y[:k * n].reshape(k, n) ** 2).mean(1) + 1e-20)


def main():
    d, phrase, onset, *srcs = sys.argv[1:]
    a = int(float(onset) * SR)
    X = {s: matched(d, s, phrase)[a:a + int(2.6 * SR)].mean(1) for s in srcs}
    print('time (s) ' + ' '.join(f'{t:6.3f}' for t in TIMES))
    for c in BANDS:
        print(f'--- {c} Hz')
        for s in srcs:
            e = band_env(X[s], c)
            print(f'{s[:9]:9}' + ' '.join(f'{e[int(t / 0.005)]:6.1f}' for t in TIMES))


if __name__ == '__main__':
    main()
