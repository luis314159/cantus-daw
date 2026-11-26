"""Round 42 (chorda-c17): one number for the action's knock against both recorded grands — the
top keys' knock envelope under their fundamentals (knockenv's bands and windows) at pp / mf / ff,
each cell's distance outside the two grands' span (0 inside it; their mean ± half their
difference, at least 2 dB each side), RMS over the cells. The fit's own keys are `--keys`
(default the top four, A7 ... C8); `--held` scores others the fit never saw.

    python -I knockfit.py <cand notes dir> [--keys 99,102,105,108] [--verbose]
"""
import os, sys
import numpy as np
import soundfile as sf
from scipy import signal

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from labpaths import LAB  # noqa: E402

SR = 48000
KEYS = list(range(21, 109, 3))
BANDS = [(60, 180), (180, 500), (500, 1400), (1400, 2800)]
TS = [0, 5, 10, 20, 40, 80, 160, 320]
CACHE = {}


def env(d, dyn, keys):
    """Per band and window, the knock's level under the fundamental (dB re the note's first
    60 ms): short Blackman-Harris frames (10 ms, sidelobes under -90 dB), each band only up to
    0.7 f0 less the window's main lobe, so a strong fundamental cannot leak into it."""
    key = (d, dyn, tuple(keys))
    if key in CACHE:
        return CACHE[key]
    x, _ = sf.read(os.path.join(d, f'notes_{dyn}.wav'), always_2d=True)
    m = x.mean(1)
    n, hop = 480, 120
    w = signal.windows.blackmanharris(n)
    F = np.fft.rfftfreq(n, 1 / SR)
    acc = np.zeros((len(BANDS), len(TS) - 1))
    cnt = np.zeros_like(acc)
    for k in keys:
        a = int(KEYS.index(k) * 6 * SR)
        seg = np.abs(m[a:a + int(0.3 * SR)])
        on = a + int(np.argmax(seg > seg.max() * 0.05))
        ref = 10 * np.log10((m[on:on + int(0.06 * SR)] ** 2).mean())
        f0 = 440 * 2 ** ((k - 69) / 12)
        top = 0.7 * f0 - 4 * SR / n
        frames = []
        for c in range(on - n // 2, on + int(0.32 * SR), hop):
            y = m[c:c + n] * w
            frames.append((c + n // 2 - on, np.abs(np.fft.rfft(y)) ** 2 / (w ** 2).sum()))
        for i, (lo, hi) in enumerate(BANDS):
            hi2 = min(hi, top)
            if hi2 <= lo * 1.3:
                continue
            sel = (F >= lo) & (F < hi2)
            for j in range(len(TS) - 1):
                p = [P[sel].sum() for t, P in frames if TS[j] * SR / 1000 <= t < max(TS[j + 1], TS[j] + 3) * SR / 1000]
                if p:
                    acc[i, j] += 10 * np.log10(np.mean(p) / n + 1e-30) - ref
                    cnt[i, j] += 1
    r = np.where(cnt > 0, acc / np.maximum(cnt, 1), np.nan)
    CACHE[key] = r
    return r


def score(cand, keys, verbose=False):
    N = os.path.join(LAB, 'notes5', 'wav')
    out = []
    for dyn in ('pp', 'mf', 'ff'):
        sal, bk = (env(os.path.join(N, s), dyn, keys) for s in ('salamander', 'bitklavier'))
        ch = env(cand, dyn, keys)
        mid, half = (sal + bk) / 2, np.maximum(np.abs(sal - bk) / 2, 2.0)
        d = np.maximum(np.abs(ch - mid) - half, 0) * np.sign(ch - mid)
        out.append(d)
        if verbose:
            print(f'{dyn}: cand minus the grands\' span (0 inside), bands x windows {TS} ms')
            for i, bnd in enumerate(BANDS):
                print(f'  {bnd[0]:5d}-{bnd[1]:<5d}' + ''.join(f'{v:6.1f}' for v in d[i]))
    D = np.array(out)
    return float(np.sqrt(np.nanmean(D ** 2)))


def between_score(cand, keys, dyns=('mf', 'ff'), verbose=False):
    """The noise between the partials (between.py) in its first two windows (0.02-0.32 s:
    the action and the case; later the references' own recording noise counts), each octave
    125 Hz - 4 kHz, against both grands' span as `score` does; 8 kHz one-sided (never over them)."""
    import between
    N = os.path.join(LAB, 'notes5', 'wav')
    out = []
    for dyn in dyns:
        k = ('B', dyn, tuple(keys))
        for s in ('salamander', 'bitklavier'):
            if (s,) + k not in CACHE:
                CACHE[(s,) + k] = between.measure(os.path.join(N, s), dyn, keys)[0][:2, 1:8]
        sal, bk = CACHE[('salamander',) + k], CACHE[('bitklavier',) + k]
        ch = between.measure(cand, dyn, keys)[0][:2, 1:8]
        mid, half = (sal + bk) / 2, np.maximum(np.abs(sal - bk) / 2, 2.0)
        d = np.maximum(np.abs(ch - mid) - half, 0) * np.sign(ch - mid)
        # The 8 kHz octave counts only above the grands: their own recordings' floor lies
        # under it (a knock may not add hiss; it need not copy theirs).
        d[:, -1] = np.maximum(d[:, -1], 0)
        out.append(d)
        if verbose:
            print(f'between {dyn} keys {keys[0]}-{keys[-1]}: cand minus the span, windows x octaves 125 Hz - 8 kHz')
            for r in d:
                print('  ' + ''.join(f'{v:6.1f}' for v in r))
    return float(np.sqrt(np.nanmean(np.array(out) ** 2)))


def main():
    a = sys.argv[1:]
    keys = [99, 102, 105, 108]
    if '--keys' in a:
        i = a.index('--keys')
        keys = [int(k) for k in a[i + 1].split(',')]
        del a[i:i + 2]
    v = '--verbose' in a
    a = [x for x in a if x not in ('--verbose', '--between')]
    print(f'KNOCK {score(a[0], keys, v):.3f}')
    if '--between' in sys.argv:
        mid = [k for k in KEYS if 48 <= k <= 72]
        print(f'BETWEEN {between_score(a[0], mid, verbose=v):.3f}')


if __name__ == '__main__':
    main()
