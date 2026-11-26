"""Round 42 (chorda-c17): one note's partials against references — each partial's level in
time windows (dB re the note's total in the first window) and its decay (dB/s), the partials
found on the note's own inharmonicity (B fitted on its first partials).

    python -I parts.py <wav> <onset s> <MIDI key> <label>=<wav>[@onset] ... [--n 24]

The first file is the reference. A file is a phrase or a notes render; `@onset` gives each
its own onset (default the first's). Each source's onset is refined to its first sound.
"""
import sys
import numpy as np
import soundfile as sf

SR = 48000
WINS = [(0.0, 0.08), (0.08, 0.3), (0.3, 0.8), (0.8, 1.6), (1.6, 2.8)]


def load(spec, t0):
    path, at = (spec.split('@') + [None])[:2]
    x, sr = sf.read(path, dtype='float64', always_2d=True)
    m = x.mean(1)
    a = int((float(at) if at else t0) * SR)
    seg = np.abs(m[max(0, a - int(0.05 * SR)):a + int(0.3 * SR)])
    on = max(0, a - int(0.05 * SR)) + int(np.argmax(seg > seg.max() * 0.03))
    return m[on:]


def peaks(m, f0, nmax):
    n_fft = 1 << 16
    y = m[:int(1.0 * SR)]
    P = np.abs(np.fft.rfft(y * np.hanning(len(y)), n_fft)) ** 2
    F = np.fft.rfftfreq(n_fft, 1 / SR)
    B, found = 0.0, []
    for n in range(1, nmax + 1):
        g = n * f0 * np.sqrt(1 + B * n * n)
        w = (F > g * (1 - 0.6 / n - 0.01)) & (F < g * (1 + 0.6 / n + 0.01)) if n < 3 else (F > g * 0.985) & (F < g * 1.015)
        if not w.any() or g > 20000:
            break
        i = np.flatnonzero(w)[np.argmax(P[w])]
        found.append(F[i])
        if n >= 3:
            nn = np.arange(1, len(found) + 1)
            r = (np.array(found) / (nn * found[0])) ** 2 - 1
            B = max(0.0, float(np.polyfit(nn ** 2, r, 1)[0])) if len(found) > 3 else 0.0
    return np.array(found), B


def levels(m, freqs):
    out = []
    for a, b in WINS:
        y = m[int(a * SR):int(b * SR)]
        if len(y) < 256:
            out.append(np.full(len(freqs), np.nan))
            continue
        n_fft = 1 << max(14, int(np.ceil(np.log2(len(y)))) + 1)
        P = np.abs(np.fft.rfft(y * np.hanning(len(y)), n_fft)) ** 2 / len(y)
        F = np.fft.rfftfreq(n_fft, 1 / SR)
        row = []
        for f in freqs:
            w = (F > f * 0.992) & (F < f * 1.008)
            row.append(P[w].max() if w.any() else 1e-30)
        out.append(10 * np.log10(np.array(row) + 1e-30))
    return np.array(out)


def main():
    a = sys.argv[1:]
    nmax = 24
    if '--n' in a:
        i = a.index('--n')
        nmax = int(a[i + 1])
        del a[i:i + 2]
    ref, t0, key = a[0], float(a[1]), int(a[2])
    srcs = [('ref', ref)] + [tuple(s.split('=', 1)) for s in a[3:]]
    f0 = 440 * 2 ** ((key - 69) / 12)
    rows = {}
    for lab, path in srcs:
        m = load(path, t0)
        fr, B = peaks(m, f0, nmax)
        L = levels(m, fr)
        tot = 10 * np.log10((10 ** (L[0] / 10)).sum())
        rows[lab] = (fr, B, L - tot)
    n = min(len(r[0]) for r in rows.values())
    print(f'key {key}: n ' + ' '.join(f'{i:5d}' for i in range(1, n + 1)))
    for lab, (fr, B, L) in rows.items():
        print(f'{lab:>10} B={B:.2e} f: ' + ' '.join(f'{f:5.0f}' for f in fr[:n]))
    for w, (wa, wb) in enumerate(WINS):
        print(f'-- {wa:.2f}-{wb:.2f} s, dB re the attack window total (ref absolute, others minus ref)')
        r = rows['ref'][2][w, :n]
        print(f'{"ref":>10}  ' + ' '.join(f'{v:5.1f}' for v in r))
        for lab, (fr, B, L) in rows.items():
            if lab != 'ref':
                print(f'{lab:>10}  ' + ' '.join(f'{v:+5.1f}' for v in L[w, :n] - r))
    print('-- decay 0.08-0.3 -> 0.8-1.6 s (dB/s): ref absolute, others minus ref')
    span = (1.2 - 0.19)
    rd = (rows['ref'][2][3, :n] - rows['ref'][2][1, :n]) / span
    print(f'{"ref":>10}  ' + ' '.join(f'{v:5.1f}' for v in rd))
    for lab, (fr, B, L) in rows.items():
        if lab != 'ref':
            d = (L[3, :n] - L[1, :n]) / span
            print(f'{lab:>10}  ' + ' '.join(f'{v:+5.1f}' for v in d - rd))


if __name__ == '__main__':
    main()
