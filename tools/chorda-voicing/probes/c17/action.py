"""Round 42 (chorda-c17): the action's noise, measured where the strings cannot hide it — under
a treble note's fundamental (nothing a string of that key sounds lies below it) and after a
key's release. From notes renders (notes_pp/mf/ff: A0 ... C8 every minor third, a key every
6 s, held 3.5 s).

    python -I action.py <ref notes dir> <label>=<notes dir> ... [--keys 84,87,...] [--dyn mf]

Per key: the sub-fundamental band (40 Hz ... 0.7 f0) in third octaves over 0-15 ms, 15-60 ms
and 60-250 ms after the strike, in dB against the note's whole level in its first 60 ms
(so a quieter or louder note does not move it); and the release (0-120 ms after the key comes
up, the same band) against that level too. The reference's samples begin with a recorded
pre-attack: each strike is found by its first sound.
"""
import os, sys
import numpy as np
import soundfile as sf

SR = 48000
EVERY, HELD = 6.0, 3.5
KEYS = list(range(21, 109, 3))
CEN = [50 * 2 ** (i / 3) for i in range(0, 22)]  # 50 Hz ... ~5 kHz


def bands(y, fmax):
    n = 1 << 15
    P = np.abs(np.fft.rfft(y * np.hanning(len(y)), n)) ** 2 / len(y)
    F = np.fft.rfftfreq(n, 1 / SR)
    out = []
    for c in CEN:
        lo, hi = c / 2 ** (1 / 6), c * 2 ** (1 / 6)
        if hi > fmax:
            out.append(np.nan)
            continue
        out.append(10 * np.log10(P[(F >= lo) & (F < hi)].sum() + 1e-30))
    return np.array(out), 10 * np.log10(P.sum() + 1e-30)


def measure(d, dyn, keys):
    x, sr = sf.read(os.path.join(d, f'notes_{dyn}.wav'), dtype='float64', always_2d=True)
    m = x.mean(1)
    res = {}
    for k in keys:
        i = KEYS.index(k)
        a = int(i * EVERY * SR)
        seg = np.abs(m[a:a + int(0.3 * SR)])
        on = a + int(np.argmax(seg > seg.max() * 0.05))
        f0 = 440 * 2 ** ((k - 69) / 12)
        fmax = 0.7 * f0
        _, tot = bands(m[on:on + int(0.06 * SR)], 1e9)
        rows = []
        for w0, w1 in ((0, 0.015), (0.015, 0.06), (0.06, 0.25)):
            b, _ = bands(m[on + int(w0 * SR):on + int(w1 * SR)], fmax)
            rows.append(b - tot + 10 * np.log10((w1 - w0) / 0.06))
        r = a + int(HELD * SR)
        b, _ = bands(m[r:r + int(0.12 * SR)], fmax)
        rows.append(b - tot + 10 * np.log10(0.12 / 0.06))
        res[k] = np.array(rows)
    return res


def total(b):
    b = b[~np.isnan(b)]
    return 10 * np.log10((10 ** (b / 10)).sum()) if len(b) else np.nan


def main():
    a = sys.argv[1:]
    keys, dyn = [84, 87, 90, 93, 96, 99, 102, 105, 108], 'mf'
    if '--keys' in a:
        i = a.index('--keys')
        keys = [int(k) for k in a[i + 1].split(',')]
        del a[i:i + 2]
    if '--dyn' in a:
        i = a.index('--dyn')
        dyn = a[i + 1]
        del a[i:i + 2]
    srcs = [('ref', a[0])] + [tuple(s.split('=', 1)) for s in a[1:]]
    R = {lab: measure(d, dyn, keys) for lab, d in srcs}
    names = ['0-15ms', '15-60ms', '60-250ms', 'release']
    print(f'== the sub-fundamental band, total dB re the note (first 60 ms), {dyn}')
    print(f'{"key":>4} ' + ''.join(f'{lab[:9] + " " + w:>22}' for w in names for lab, _ in srcs))
    for k in keys:
        print(f'{k:4d} ' + ''.join(f'{total(R[lab][k][j]):22.1f}' for j in range(4) for lab, _ in srcs))
    print('\n== mean over the keys, per third octave (dB re the note), ref absolute / others minus ref')
    print(f'{"":>18}' + ''.join(f'{c:6.0f}' for c in CEN))
    for j, w in enumerate(names):
        ref = np.nanmean(np.array([R['ref'][k][j] for k in keys]), 0)
        print(f'{"ref " + w:>18}' + ''.join(f'{v:6.1f}' for v in ref))
        for lab, _ in srcs[1:]:
            v = np.nanmean(np.array([R[lab][k][j] for k in keys]), 0) - ref
            print(f'{lab[:9] + " " + w:>18}' + ''.join(f'{x:+6.1f}' for x in v))


if __name__ == '__main__':
    main()
