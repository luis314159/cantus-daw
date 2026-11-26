"""Round 42 (chorda-c17): the analysis's open items as numbers, for quick trials — each against
the two grands (Salamander / bitKlavier):

  top45   the top phrase's 4.5-11 kHz level (dB re the Salamander's, 1/6-octave mean)
  late    the noise between partials 0.32-0.82 s, C3-C5 and D#5-C8 at mf, 0.25-4 kHz mean (dB)
  pp      the treble's pp attack, 250-500 Hz, 0-30 ms re 30-300 ms (dB)
  rel     the bass's release, 4-8 kHz in its first 50 ms (dB re before)
  <90     the 50-90 Hz level of middle C, the run and the top phrase (dB re the Salamander's)
  knock   knockfit's score on the top keys (0 inside the grands' span)

    python -I quick17.py <phrases dir> <notes dir>
"""
import os, sys
import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
from labpaths import LAB  # noqa: E402
import ana17 as A  # noqa: E402
import between  # noqa: E402
import knockfit  # noqa: E402


def main():
    ph, nt = sys.argv[1], sys.argv[2]
    E, N = os.path.join(LAB, 'eng40', 'wav'), os.path.join(LAB, 'notes5', 'wav')
    S = {'sal': (os.path.join(E, 'salamander'), os.path.join(N, 'salamander')),
         'bk': (os.path.join(E, 'bitklavier'), os.path.join(N, 'bitklavier')), 'cand': (ph, nt)}
    sel = [i for i, c in enumerate(A.SIXTHS) if 4500 <= c <= 11000]
    L = {k: A.ltas(A.load_phrase(v[0], 'top')[0]) for k, v in S.items()}
    out = {}
    for k in S:
        top = float(np.mean(L[k][sel] - L['sal'][sel]))
        late = []
        for lo, hi in ((48, 72), (75, 108)):
            R, _ = between.measure(S[k][1], 'mf', [x for x in A.KEYS if lo <= x <= hi])
            late.append(float(np.mean(R[2][2:7])))
        _, _, cs, spec = A.attack(S[k][1], 'pp', [x for x in A.KEYS if 75 <= x <= 108])
        cs2, rel = A.release(S[k][1], 'mf', [x for x in A.KEYS if 21 <= x <= 45])
        low = []
        for p in ('middle_c', 'fast_run', 'top'):
            Lp = A.ltas(A.load_phrase(S[k][0], p)[0])
            Ls = A.ltas(A.load_phrase(S['sal'][0], p)[0])
            low.append(float(np.mean((Lp - Ls)[:6])))
        out[k] = (top, late[0], late[1], float(np.mean(spec[1:3])), float(np.mean(rel[0][5:7]))) + tuple(low)
    print(f'{"":>6}{"top45":>8}{"late-mid":>10}{"late-tre":>10}{"pp-att":>8}{"rel-bass":>10}{"<90 mc":>8}{"<90 run":>8}{"<90 top":>8}')
    for k, v in out.items():
        print(f'{k:>6}' + ''.join(f'{x:{w}.1f}' for x, w in zip(v, (8, 10, 10, 8, 10, 8, 8, 8))))
    print(f'KNOCK {knockfit.score(nt, [99, 102, 105, 108]):.2f}  held {knockfit.score(nt, [84, 87, 90, 93, 96]):.2f}')


if __name__ == '__main__':
    main()
