"""Round 40: the bass phrase's own score — what "bass notes alone" sounds like against the
Salamander, note by note and moment by moment: each note's octave levels (63 Hz … 4 kHz) in
three windows (the attack 0-80 ms, the bloom 80-500 ms, the ring 0.5-2.4 s), the phrase
brought to -20 LUFS as the ABX kit does; plus each note's stereo correlation.

    python bassobj.py <ref dir> <cand dir>
"""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '.'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import os, sys
import numpy as np
from heard import lufs, SR
import soundfile as sf

OCT = [63, 125, 250, 500, 1000, 2000, 4000]
WINS = [(0.0, 0.08), (0.08, 0.5), (0.5, 2.4)]
# The bloom and ring are most of what a held bass note is; the attack a third of it.
WIN_W = [0.6, 1.0, 1.0]
# Bands where most of a bass note's loudness and colour lives count fully; 4 kHz half.
OCT_W = [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.5]


def load(d):
    for ext in ('wav', 'flac'):
        p = os.path.join(d, f'bass.{ext}')
        if os.path.exists(p):
            x, _ = sf.read(p, dtype='float64', always_2d=True)
            return x * 10 ** ((-20 - lufs(x)) / 20)
    raise FileNotFoundError(d)


def octs(m):
    w = np.hanning(len(m))
    s = np.abs(np.fft.rfft(m * w)) ** 2 / (w ** 2).sum() / len(m)
    f = np.fft.rfftfreq(len(m), 1 / SR)
    return np.array([10 * np.log10(s[(f >= c / 2 ** .5) & (f < c * 2 ** .5)].sum() + 1e-30) for c in OCT])


def feats(x):
    out = []
    for i in range(5):
        a = int(3.0 * i * SR)
        seg = x[a:a + int(2.9 * SR)]
        m = seg.mean(1)
        lv = [octs(m[int(p * SR):int(q * SR)]) for p, q in WINS]
        corr = np.corrcoef(seg[:int(1.5 * SR), 0], seg[:int(1.5 * SR), 1])[0, 1]
        out.append((lv, corr))
    return out


_REF = {}


def score(ref_dir, cand_dir, verbose=False):
    if ref_dir not in _REF:
        _REF[ref_dir] = feats(load(ref_dir))
    R, C = _REF[ref_dir], feats(load(cand_dir))
    err, cor = [], []
    rows = []
    for (rl, rc), (cl, cc) in zip(R, C):
        e = 0.0
        wsum = 0.0
        for w, (a, b) in enumerate(zip(rl, cl)):
            # A band more than 30 dB under the note's loudest counts little: inaudible.
            top = a.max()
            ww = np.array(OCT_W) * np.where(a > top - 30, 1.0, 0.25) * WIN_W[w]
            d = np.clip(b - a, -15, 15)
            e += (ww * d * d).sum()
            wsum += ww.sum()
            rows.append(b - a)
        err.append(np.sqrt(e / wsum))
        cor.append(abs(cc - rc))
    total = float(np.mean(err)) + 4 * float(np.mean(cor))
    if verbose:
        names = [21, 24, 28, 33, 36]
        print('note window  ' + ' '.join(f'{c:>6}' for c in OCT))
        for i, n in enumerate(names):
            for w in range(3):
                print(f'{n:4} {WINS[w][0]:.2f}-{WINS[w][1]:.2f}' + ' '.join(f'{v:6.1f}' for v in rows[3 * i + w]))
            print(f'      err {err[i]:.2f}  corr {C[i][1]:.2f} / {R[i][1]:.2f}')
        print(f'BASS {total:.3f}')
    return total


if __name__ == '__main__':
    score(sys.argv[1], sys.argv[2], verbose=True)
