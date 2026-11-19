"""Round 40: how much room follows a release — each phrase's level after its last release
against its level while held (dB), the reference's beside it. Too much reads blurred
("borroso"), too little dry.

    python tails.py <ref dir> <cand dir> ...
"""
import os, sys
import numpy as np
import soundfile as sf

SR = 48000
# phrase: (held window, after-release window) in s
WIN = {'middle_c': ((0.5, 2.9), (3.25, 4.5)), 'ff_chord': ((1.0, 3.9), (5.25, 7.0)),
       'top': ((1.4, 4.0), (6.2, 7.2)), 'ballad': ((3.0, 13.0), (14.6, 16.5)),
       'soft_chords': ((0.5, 8.5), (9.45, 11.5))}


def lv(x, a, b):
    s = x[int(a * SR):int(b * SR)]
    return 10 * np.log10((s ** 2).mean() + 1e-30)


def score(ref, cand):
    tot = 0.0
    for p, (h, t) in WIN.items():
        v = []
        for d in (ref, cand):
            x, _ = sf.read(os.path.join(d, p + '.wav'), dtype='float64', always_2d=True)
            v.append(lv(x, *t) - lv(x, *h))
        tot += abs(v[1] - v[0])
    return tot / len(WIN)


def main():
    dirs = sys.argv[1:]
    print('phrase      ' + ' '.join(f'{os.path.basename(os.path.normpath(d))[:12]:>12}' for d in dirs))
    for p, (h, t) in WIN.items():
        row = []
        for d in dirs:
            x, _ = sf.read(os.path.join(d, p + '.wav'), dtype='float64', always_2d=True)
            row.append(lv(x, *t) - lv(x, *h))
        print(f'{p:12}' + ' '.join(f'{v:12.1f}' for v in row))


if __name__ == '__main__':
    main()
