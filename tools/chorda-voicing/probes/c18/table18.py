"""Round 42 (chorda-c18): the pass's before -> after table against both grands, from the lab's
renders (`notes5/wav/<tag>`, `eng40/wav/<tag>`): the top keys' mechanics (mech2's distance outside
the grands' span at pp / mf / ff; mech's burst, late level, decay and dryness), knockfit's scores
(the c17 pass's own), the bass's rise, middle C's partials 5 and 2 in the phrase, and the
knock's share of four phrases (needs `<tag>nk` renders: the same spec with `knock=0`).

    python -I table18.py <tag> ...          (e.g. shipped c17 c18; grands always measured)
"""
import os, sys
import numpy as np

H = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, H)
sys.path.insert(0, os.path.join(H, '..', 'c17'))
sys.path.insert(0, os.path.join(H, '..', '..'))
from labpaths import LAB  # noqa: E402
import mech, mech2, knockfit  # noqa: E402
import ana17 as A  # noqa: E402

N = os.path.join(LAB, 'notes5', 'wav')
E = os.path.join(LAB, 'eng40', 'wav')
TOP = [96, 99, 102, 105, 108]


def mech2_rms(tag, dyn):
    t, g1 = mech2.measure(os.path.join(N, 'salamander'), dyn, TOP)
    _, g2 = mech2.measure(os.path.join(N, 'bitklavier'), dyn, TOP)
    _, c = mech2.measure(os.path.join(N, tag), dyn, TOP)
    rows = []
    for i in range(len(mech2.CEN)):
        a, b, x = (np.array([float(np.interp(s, t, L[i])) for s in mech2.TS]) for L in (g1, g2, c))
        mid, half = (a + b) / 2, np.maximum(np.abs(a - b) / 2, 2)
        rows.append(np.maximum(np.abs(x - mid) - half, 0))
    return float(np.sqrt(np.nanmean(np.array(rows) ** 2)))


def mech_summary(tag):
    r = mech.measure(os.path.join(N, tag), 'mf', TOP)
    env = np.mean([x['env'] for x in r], 0)
    T = mech.TIMES
    burst = float(np.mean([env[T.index(t)] for t in (5, 7, 10, 15, 20)]))
    late = float(np.mean([env[T.index(t)] for t in (120, 160, 240, 320)]))
    i = [j for j, c in enumerate(mech.CEN) if 500 <= c <= 2600]
    slope = float(np.nanmean(np.nanmean([x['slope'] for x in r], 0)[i]))
    dry = float(np.nanmean(np.nanmean([x['dry'] for x in r], 0)[i]))
    return burst, late, slope, dry


def rise(tag, keys):
    return [A.attack(os.path.join(N, tag), 'mf', [k])[0] for k in keys]


def share(tag, ph):
    import soundfile as sf
    p = os.path.join(E, tag, ph + '.wav')
    q = os.path.join(E, tag + 'nk', ph + '.wav')
    if not os.path.exists(q):
        return np.nan
    a = sf.read(p, always_2d=True)[0].mean(1)
    b = sf.read(q, always_2d=True)[0].mean(1)
    return float(10 * np.log10(((a - b) ** 2).sum() / (a ** 2).sum()))


def main():
    tags = sys.argv[1:]
    srcs = ['salamander', 'bitklavier'] + tags
    print('== the top keys (C7, D#7, F#7, A7, C8)')
    print(f'{"":>12}{"pp":>7}{"mf":>7}{"ff":>7}   (mech2: RMS dB outside the grands\' span; 0 inside)')
    for t in tags:
        print(f'{t:>12}' + ''.join(f'{mech2_rms(t, d):7.2f}' for d in ('pp', 'mf', 'ff')))
    print(f'{"":>12}{"burst":>8}{"late":>8}{"slope":>8}{"late/early":>11}   (mf, under 0.7 f0: dB re note 5-20 ms; 120-320 ms; dB/s 0.5-2.6 kHz; dB)')
    for s in srcs:
        b, l, sl, d = mech_summary(s)
        print(f'{s[:12]:>12}{b:8.1f}{l:8.1f}{sl:8.0f}{d:11.1f}')
    print('\n== knockfit (round 42 c17\'s scores; 0 = inside the grands\' span)')
    mid = [k for k in knockfit.KEYS if 48 <= k <= 72]
    top = [k for k in knockfit.KEYS if 84 <= k <= 105]
    bass = [k for k in knockfit.KEYS if 24 <= k <= 45]
    print(f'{"":>12}{"fit":>7}{"held":>7}{"b.mid":>7}{"b.top":>7}{"b.bass":>7}')
    for t in tags:
        d = os.path.join(N, t)
        knockfit.CACHE.clear()
        print(f'{t:>12}{knockfit.score(d, [99, 102, 105, 108]):7.2f}{knockfit.score(d, [84, 87, 90, 93, 96]):7.2f}'
              f'{knockfit.between_score(d, mid):7.2f}{knockfit.between_score(d, top):7.2f}{knockfit.between_score(d, bass):7.2f}')
    keys = [21, 24, 27, 30, 33, 36]
    print(f'\n== the bass\'s rise (ms, -20 to -3 dB of its peak), keys {keys}')
    for s in srcs:
        print(f'{s[:12]:>12}' + ''.join(f'{v:6.1f}' for v in rise(s, keys)))
    print('\n== the knock\'s share of each phrase (dB re the whole; needs <tag>nk)')
    phs = ['top', 'ff_chord', 'fast_run', 'soft_chords']
    print(f'{"":>12}' + ''.join(f'{p:>12}' for p in phs))
    for t in tags:
        print(f'{t:>12}' + ''.join(f'{share(t, p):12.1f}' for p in phs))


if __name__ == '__main__':
    main()
