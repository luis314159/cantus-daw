"""Round 42 (chorda-c17): what lies between a note's partials, and when — the sound lab's
`noise` feature (tools/soundlab: bins farther than 0.2 f0 from every partial) in time windows,
as a level per third octave against the note's whole level in its first 100 ms; and the
recording's own floor (the last 0.3 s before the next key, scaled to the same window), so the
references' hiss is not read as the piano's.

    python -I between.py <ref notes dir> <label>=<notes dir> ... [--dyn mf] [--keys 48-72]
"""
import os, sys
import numpy as np
import soundfile as sf

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..', '..', 'soundlab'))
from soundlab.core import band_edges  # noqa: E402

SR = 48000
KEYS = list(range(21, 109, 3))
WINS = [(0.02, 0.12), (0.12, 0.32), (0.32, 0.82), (0.82, 1.82)]
FC, LO, HI = band_edges(100.0, 16000.0, 1)  # octaves


def fit_partials(y, f0):
    n_fft = 1 << 17
    P = np.abs(np.fft.rfft(y * np.hanning(len(y)), n_fft)) ** 2
    F = np.fft.rfftfreq(n_fft, 1 / SR)
    fr, B = [], 0.0
    for n in range(1, 40):
        g = n * f0 * np.sqrt(1 + B * n * n)
        if g > 15000:
            break
        w = (F > g - 0.08 * f0) & (F < g + 0.08 * f0)
        fr.append(F[np.flatnonzero(w)[np.argmax(P[w])]])
        if n >= 4:
            nn = np.arange(1, len(fr) + 1)
            B = max(0.0, float(np.polyfit(nn ** 2, (np.array(fr) / (nn * fr[0])) ** 2 - 1, 1)[0]))
    return fr[0], B


def noise_bands(y, f0, B):
    w = np.hanning(len(y))
    P = np.abs(np.fft.rfft(y * w)) ** 2 / (w ** 2).sum()
    fr = np.fft.rfftfreq(len(y), 1 / SR)
    near = np.zeros(len(fr), bool)
    k = 1
    while True:
        fk = k * f0 * np.sqrt(1 + B * k * k)
        if fk > 0.47 * SR:
            break
        near |= np.abs(fr - fk) <= 0.2 * f0
        k += 1
    out = []
    for a, e in zip(LO, HI):
        sel = (fr >= a) & (fr < e)
        nb = sel & ~near
        out.append(10 * np.log10(P[nb].mean() * sel.sum() + 1e-30) if nb.sum() >= 3 else np.nan)
    return np.array(out)


def measure(d, dyn, keys):
    x, _ = sf.read(os.path.join(d, f'notes_{dyn}.wav'), dtype='float64', always_2d=True)
    m = x.mean(1)
    rows, floors = [], []
    for k in keys:
        a = int(KEYS.index(k) * 6.0 * SR)
        seg = np.abs(m[a:a + int(0.3 * SR)])
        on = a + int(np.argmax(seg > seg.max() * 0.05))
        f0, B = fit_partials(m[on:on + SR], 440 * 2 ** ((k - 69) / 12))
        y0 = m[on:on + int(0.1 * SR)]
        tot = 10 * np.log10((y0 ** 2).mean() + 1e-30)
        r = [noise_bands(m[on + int(w0 * SR):on + int(w1 * SR)], f0, B) - tot for w0, w1 in WINS]
        rows.append(r)
        fl = m[a + int(5.65 * SR):a + int(5.95 * SR)]
        floors.append(noise_bands(fl, f0, B) - tot)
    return np.nanmean(np.array(rows), 0), np.nanmean(np.array(floors), 0)


def main():
    a = sys.argv[1:]
    opt = {'--dyn': 'mf', '--keys': '48-72'}
    for k in list(opt):
        if k in a:
            i = a.index(k)
            opt[k] = a[i + 1]
            del a[i:i + 2]
    lo, hi = (int(v) for v in opt['--keys'].split('-'))
    keys = [k for k in KEYS if lo <= k <= hi]
    srcs = [('ref', a[0])] + [tuple(s.split('=', 1)) for s in a[1:]]
    print(f'keys {lo}-{hi} {opt["--dyn"]}: noise between partials per octave, dB re the note (first 100 ms)')
    print(f'{"":>22}' + ''.join(f'{c:7.0f}' for c in FC))
    for lab, d in srcs:
        R, Fl = measure(d, opt['--dyn'], keys)
        for (w0, w1), r in zip(WINS, R):
            print(f'{lab[:9] + f" {w0:.2f}-{w1:.2f}":>22}' + ''.join(f'{v:7.1f}' for v in r))
        print(f'{lab[:9] + " floor":>22}' + ''.join(f'{v:7.1f}' for v in Fl))


if __name__ == '__main__':
    main()
