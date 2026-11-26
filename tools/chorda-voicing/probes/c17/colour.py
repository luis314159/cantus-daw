"""Round 42 (chorda-c17): a register's colour as partials by frequency — every key's partials
(found on its own inharmonicity), each key normalised to its own partials' total in the window,
then binned by frequency (half octaves) and averaged over the register's keys. A partial table
keyed by number cannot show a fixed band (a board's or an EQ's): this can.

    python -I colour.py <ref notes dir> <label>=<notes dir> ... [--dyn mf] [--keys 48-72]
           [--win 0.0,0.1]

The notes renders (notes_pp/mf/ff: A0 ... C8 every minor third, one key every 6 s).
"""
import os, sys
import numpy as np
import soundfile as sf

SR = 48000
KEYS = list(range(21, 109, 3))
EDGES = [100 * 2 ** (i / 2) for i in range(0, 16)]  # 100 Hz ... 18 kHz, half octaves


def partials(y, f0):
    n_fft = 1 << 17
    P = np.abs(np.fft.rfft(y * np.hanning(len(y)), n_fft)) ** 2 / len(y)
    F = np.fft.rfftfreq(n_fft, 1 / SR)
    out, B = [], 0.0
    for n in range(1, 60):
        g = n * f0 * np.sqrt(1 + B * n * n)
        if g > 16000:
            break
        w = (F > g * (1 - 0.3 / n - 0.008)) & (F < g * (1 + 0.3 / n + 0.008)) if n < 3 else (F > g * 0.985) & (F < g * 1.015)
        i = np.flatnonzero(w)[np.argmax(P[w])]
        out.append((F[i], P[max(0, i - 2):i + 3].sum()))
        if n >= 4:
            fr = np.array([o[0] for o in out])
            nn = np.arange(1, len(fr) + 1)
            B = max(0.0, float(np.polyfit(nn ** 2, (fr / (nn * fr[0])) ** 2 - 1, 1)[0]))
    return out


def profile(d, dyn, keys, win):
    x, sr = sf.read(os.path.join(d, f'notes_{dyn}.wav'), dtype='float64', always_2d=True)
    m = x.mean(1)
    rows = []
    for k in keys:
        a = int(KEYS.index(k) * 6.0 * SR)
        seg = np.abs(m[a:a + int(0.3 * SR)])
        on = a + int(np.argmax(seg > seg.max() * 0.05))
        y = m[on + int(win[0] * SR):on + int(win[1] * SR)]
        ps = partials(y, 440 * 2 ** ((k - 69) / 12))
        tot = sum(p for _, p in ps)
        b = np.zeros(len(EDGES) - 1)
        for f, p in ps:
            j = np.searchsorted(EDGES, f) - 1
            if 0 <= j < len(b):
                b[j] += p / tot
        rows.append(10 * np.log10(b + 1e-12))
    R = np.array(rows)
    R[R < -110] = np.nan
    return np.nanmean(R, 0)


def main():
    a = sys.argv[1:]
    opt = {'--dyn': 'mf', '--keys': '48-72', '--win': '0.0,0.1'}
    for k in list(opt):
        if k in a:
            i = a.index(k)
            opt[k] = a[i + 1]
            del a[i:i + 2]
    lo, hi = (int(v) for v in opt['--keys'].split('-'))
    keys = [k for k in KEYS if lo <= k <= hi]
    win = tuple(float(v) for v in opt['--win'].split(','))
    srcs = [('ref', a[0])] + [tuple(s.split('=', 1)) for s in a[1:]]
    print(f'keys {keys}, {opt["--dyn"]}, window {win} s; dB of the partials in each half octave re the note')
    print(f'{"":>12}' + ''.join(f'{e:7.0f}' for e in EDGES[:-1]))
    ref = profile(srcs[0][1], opt['--dyn'], keys, win)
    print(f'{"ref":>12}' + ''.join(f'{v:7.1f}' for v in ref))
    for lab, d in srcs[1:]:
        v = profile(d, opt['--dyn'], keys, win) - ref
        print(f'{lab[:12]:>12}' + ''.join(f'{x:+7.1f}' for x in v))


if __name__ == '__main__':
    main()
