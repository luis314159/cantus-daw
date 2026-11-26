"""Round 42 (chorda-c18): the top keys' action noise over the whole spectrum — the partials masked
out (Blackman-Harris frames of 10.7 ms, every bin within 0.2 f0 + 400 Hz of a partial dropped,
each third octave's level from the bins left, scaled to the band's width), dB re the note's first
60 ms, averaged over the keys (power).

    python -I mech2.py <label>=<notes dir> ... [--keys 96,99,102,105,108] [--dyn mf] [--json out]

Prints per third octave (125 Hz ... 10 kHz) the level at chosen times and the early decay (dB/s,
fitted 5-60 ms) and the late one (60-250 ms); with two or more sources the first two are taken
as the grands and every later source is printed again as its distance outside their span.
"""
import os, sys, json
import numpy as np
import soundfile as sf
from scipy import signal

SR = 48000
KEYS = list(range(21, 109, 3))
CEN = [125 * 2 ** (i / 3) for i in range(0, 20)]  # 125 Hz ... 10 kHz
TS = [0, 5, 10, 20, 30, 45, 60, 90, 120, 180, 250, 350]
N, HOP = 512, 120


def measure(d, dyn, keys):
    """Cached for the references (a folder named salamander or bitklavier)."""
    ref_dir = os.path.basename(os.path.normpath(d)) in ('salamander', 'bitklavier')
    cache = os.path.join(d, f'.mech2_{dyn}_{"_".join(map(str, keys))}.npz')
    if ref_dir and os.path.exists(cache):
        z = np.load(cache)
        return z['t'], z['L']
    t, L = _measure(d, dyn, keys)
    if ref_dir:
        np.savez(cache, t=t, L=L)
    return t, L


def _measure(d, dyn, keys):
    x, _ = sf.read(os.path.join(d, f'notes_{dyn}.wav'), dtype='float64', always_2d=True)
    m = x.mean(1)
    w = signal.windows.blackmanharris(N)
    F = np.fft.rfftfreq(N, 1 / SR)
    acc = np.zeros((len(CEN), 200))
    cnt = np.zeros((len(CEN), 200))
    times = None
    for k in keys:
        a0 = int(KEYS.index(k) * 6 * SR)
        seg = np.abs(m[a0:a0 + int(0.3 * SR)])
        on = a0 + int(np.argmax(seg > seg.max() * 0.05))
        ref = (m[on:on + int(0.06 * SR)] ** 2).mean()
        f0 = 440 * 2 ** ((k - 69) / 12)
        near = np.zeros(len(F), bool)
        for n in range(1, 40):
            fk = n * f0 * np.sqrt(1 + 4e-4 * n * n) if k > 90 else n * f0
            near |= np.abs(F - fk) <= 0.2 * f0 + 400
        starts = on - N // 2 + np.arange(200) * HOP
        times = (np.arange(200) * HOP) / SR * 1000
        for j, s in enumerate(starts):
            P = np.abs(np.fft.rfft(m[s:s + N] * w)) ** 2 / (w ** 2).sum() / N
            for i, c in enumerate(CEN):
                sel = (F >= c / 2 ** (1 / 6)) & (F < c * 2 ** (1 / 6))
                keep = sel & ~near
                if keep.sum() >= 1 and sel.sum() >= 1:
                    acc[i, j] += P[keep].mean() * sel.sum() / ref
                    cnt[i, j] += 1
    with np.errstate(divide='ignore', invalid='ignore'):
        L = 10 * np.log10(acc / cnt)
    return times, L


def slope(t, L, a, b):
    s = (t >= a) & (t < b) & np.isfinite(L)
    if s.sum() < 3:
        return np.nan
    return np.polyfit(t[s] / 1000, L[s], 1)[0]


def main():
    a = sys.argv[1:]
    keys, dyn, js = [96, 99, 102, 105, 108], 'mf', None
    for opt in ('--keys', '--dyn', '--json'):
        if opt in a:
            i = a.index(opt)
            v = a[i + 1]
            del a[i:i + 2]
            if opt == '--keys':
                keys = [int(k) for k in v.split(',')]
            elif opt == '--dyn':
                dyn = v
            else:
                js = v
    srcs = [tuple(s.split('=', 1)) for s in a]
    R = {lab: measure(d, dyn, keys) for lab, d in srcs}
    t = R[srcs[0][0]][0]
    tab = {}
    for lab, _ in srcs:
        L = R[lab][1]
        rows = {}
        for i, c in enumerate(CEN):
            lv = [float(np.interp(x, t, L[i])) if np.isfinite(L[i]).any() else np.nan for x in TS]
            rows[round(c)] = dict(levels=lv, early=float(slope(t, L[i], 5, 60)), late=float(slope(t, L[i], 60, 250)))
        tab[lab] = rows
    print(f'== {dyn}, keys {keys}: action noise between the partials, dB re the note; ms {TS}; slopes dB/s 5-60 | 60-250')
    for c in tab[srcs[0][0]]:
        print(f'-- {c} Hz')
        for lab, _ in srcs:
            r = tab[lab][c]
            print(f'{lab:>8}' + ''.join(f'{v:6.1f}' for v in r['levels']) + f'  |{r["early"]:6.0f}{r["late"]:6.0f}')
    if len(srcs) >= 3:
        g1, g2 = srcs[0][0], srcs[1][0]
        print('\n== distance outside the grands\' span (0 inside; mean +- half the difference, >= 2 dB)')
        print(f'{"Hz":>8}' + ''.join(f'{x:6d}' for x in TS))
        for lab, _ in srcs[2:]:
            print(f'-- {lab}')
            tot = []
            for c in tab[lab]:
                A, B, C = (np.array(tab[s][c]['levels']) for s in (g1, g2, lab))
                mid, half = (A + B) / 2, np.maximum(np.abs(A - B) / 2, 2)
                dd = np.maximum(np.abs(C - mid) - half, 0) * np.sign(C - mid)
                tot.append(dd)
                print(f'{c:8d}' + ''.join(f'{v:6.1f}' for v in dd))
            T = np.array(tot)
            print(f'   RMS {np.sqrt(np.nanmean(T ** 2)):.2f}')
    if js:
        with open(js, 'w') as f:
            json.dump(tab, f)


if __name__ == '__main__':
    main()
