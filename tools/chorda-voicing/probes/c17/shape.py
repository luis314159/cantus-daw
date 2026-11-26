"""Round 42 (chorda-c17): the colour control's measured shape — from sig.py's JSON, the
reference's long-term spectrum minus a candidate's on the middle phrases, per octave around the
colour stage's centres (250 Hz ... 4 kHz: the three thirds of each octave summed), the median
over the phrases; printed as the Rust constant `chorda::colour::SHAPE`.

    python -I shape.py <sig.json> <ref label> <cand label> [phrases middle_c,soft_chords,fast_run,ballad]
"""
import json, sys
import numpy as np

CENTRES = [250, 500, 1000, 2000, 4000]
THIRDS = [63 * 2 ** (i / 3) for i in range(0, 25)]


def octave(b, c):
    idx = [i for i, f in enumerate(THIRDS) if c / 2 ** 0.5 <= f < c * 2 ** 0.5]
    return 10 * np.log10(sum(10 ** (b[i] / 10) for i in idx))


def main():
    res = json.load(open(sys.argv[1]))
    ref, cand = sys.argv[2], sys.argv[3]
    phrases = (sys.argv[4] if len(sys.argv) > 4 else 'middle_c,soft_chords,fast_run,ballad').split(',')
    D = []
    for ph in phrases:
        r, c = res[ph][ref]['bands']['all'], res[ph][cand]['bands']['all']
        D.append([octave(r, f) - octave(c, f) for f in CENTRES])
        print(f'{ph:>12}' + ''.join(f'{v:+7.1f}' for v in D[-1]))
    med = np.clip(np.median(np.array(D), 0), -9, 9)
    print(f'{"median":>12}' + ''.join(f'{v:+7.1f}' for v in med))
    print('pub const SHAPE: [f64; BANDS] = [' + ', '.join(f'{v:.1f}' for v in med) + '];')


if __name__ == '__main__':
    main()
