"""Round 40: the owner's preview — every listening phrase from each source, brought to one
loudness per phrase (BS.1770 -20 LUFS, lowered together if a peak would pass -1 dBFS: what
the ABX kit does), written as 24-bit FLAC, named plainly (this is not a blind test).

    python preview.py <out dir> <label>=<wav dir> ...
"""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '.'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import os, sys
import numpy as np
import soundfile as sf
from heard import lufs

PHRASES = ['bass', 'middle_c', 'soft_chords', 'fast_run', 'ballad', 'ff_chord', 'top']


def main():
    out, *pairs = sys.argv[1:]
    srcs = [p.split('=', 1) for p in pairs]
    os.makedirs(out, exist_ok=True)
    lines = ['phrase,source,lufs_before,gain_db,peak_after_dbfs']
    for ph in PHRASES:
        X = []
        for _, d in srcs:
            x, sr = sf.read(os.path.join(d, ph + '.wav'), dtype='float64', always_2d=True)
            X.append(x)
        n = min(len(x) for x in X)
        X = [x[:n].copy() for x in X]
        f = int(0.03 * sr)
        for x in X:
            x[-f:] *= np.linspace(1, 0, f)[:, None]
        L = [lufs(x) for x in X]
        P = [20 * np.log10(np.abs(x).max()) for x in X]
        target = min([-20.0] + [l + (-1.0 - p) for l, p in zip(L, P)])
        for (label, _), x, l in zip(srcs, X, L):
            g = target - l
            y = x * 10 ** (g / 20)
            g2 = target - lufs(y)
            y *= 10 ** (g2 / 20)
            sf.write(os.path.join(out, f'{ph} - {label}.flac'), y, sr, subtype='PCM_24')
            lines.append(f'{ph},{label},{l:.2f},{g + g2:.2f},{20 * np.log10(np.abs(y).max()):.2f}')
    open(os.path.join(out, 'levels.csv'), 'w').write('\n'.join(lines) + '\n')


if __name__ == '__main__':
    main()
