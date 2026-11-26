"""Round 42 (chorda-c17, v4): the four open items as numbers, each against both grands:

  top3-5   the top phrase's 300-500 Hz (dB re the Salamander's, 1/6-octave mean)
  ppknock  the top keys' knock at pp, 180-1400 Hz over 5-40 ms (dB re the note; knockfit)
  relbass  the bass's release, 4-8 kHz in its first 50 ms (dB re before)
  tail.5/1 the soft chords' and the ballad's tail after the pedal at 0.5 and 1.0 s (dB, mean)
  run60    the run's 50-70 Hz (dB re the Salamander's)

    python -I quick18.py <phrases dir> <notes dir>
"""
import os, sys
import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
from labpaths import LAB  # noqa: E402
import ana17 as A  # noqa: E402
import knockfit  # noqa: E402
from report17 import END  # noqa: E402


def main():
    ph, nt = sys.argv[1], sys.argv[2]
    E, N = os.path.join(LAB, 'eng40', 'wav'), os.path.join(LAB, 'notes5', 'wav')
    S = {'sal': (os.path.join(E, 'salamander'), os.path.join(N, 'salamander')),
         'bk': (os.path.join(E, 'bitklavier'), os.path.join(N, 'bitklavier')), 'cand': (ph, nt)}
    i35 = [i for i, c in enumerate(A.SIXTHS) if 300 <= c <= 500]
    i60 = [i for i, c in enumerate(A.SIXTHS) if 50 <= c <= 70]
    Ls = {p: A.ltas(A.load_phrase(S['sal'][0], p)[0]) for p in ('top', 'fast_run')}
    print(f'{"":>6}{"top3-5":>8}{"ppknock":>9}{"relbass":>9}{"tail.5":>8}{"tail1":>8}{"run60":>8}')
    for k, (pd, nd) in S.items():
        top = float(np.mean((A.ltas(A.load_phrase(pd, 'top')[0]) - Ls['top'])[i35]))
        run = float(np.mean((A.ltas(A.load_phrase(pd, 'fast_run')[0]) - Ls['fast_run'])[i60]))
        knockfit.CACHE.clear()
        env = knockfit.env(nd, 'pp', [99, 102, 105, 108])
        pp = float(10 * np.log10(np.nanmean(10 ** (env[1:3, 1:4] / 10))))
        _, rel = A.release(nd, 'mf', [x for x in A.KEYS if 21 <= x <= 45])
        tails = []
        for p in ('soft_chords', 'ballad'):
            x, on = A.load_phrase(pd, p)
            t = A.tail_decay(x, on / A.SR + END[p]).mean(0)
            tails.append((t[10], t[20]))
        t5, t1 = np.mean([a for a, _ in tails]), np.mean([b for _, b in tails])
        print(f'{k:>6}{top:8.1f}{pp:9.1f}{float(np.mean(rel[0][5:7])):9.1f}{t5:8.1f}{t1:8.1f}{run:8.1f}')


if __name__ == '__main__':
    main()
