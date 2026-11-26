"""Round 42 (chorda-c17): partials by number, key by key — each key's partials 1..N (found on
its own inharmonicity) in a window, dB re the key's partials' total, against both grands.

    python -I pnum.py <ref notes dir> <label>=<notes dir> ... [--dyn mf] [--keys 48-72]
           [--win 0.0,0.1] [--n 12] [--per-key]
"""
import os, sys
import numpy as np
import soundfile as sf

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from colour import partials, KEYS, SR  # noqa: E402


def prof(d, dyn, keys, win, nmax):
    x, _ = sf.read(os.path.join(d, f'notes_{dyn}.wav'), dtype='float64', always_2d=True)
    m = x.mean(1)
    rows = []
    for k in keys:
        a = int(KEYS.index(k) * 6.0 * SR)
        seg = np.abs(m[a:a + int(0.3 * SR)])
        on = a + int(np.argmax(seg > seg.max() * 0.05))
        ps = partials(m[on + int(win[0] * SR):on + int(win[1] * SR)], 440 * 2 ** ((k - 69) / 12))
        p = np.array([q for _, q in ps[:nmax]] + [1e-30] * (nmax - len(ps)))
        rows.append(10 * np.log10(p / p.sum()))
    return np.array(rows)


def main():
    a = sys.argv[1:]
    opt = {'--dyn': 'mf', '--keys': '48-72', '--win': '0.0,0.1', '--n': '12'}
    for k in list(opt):
        if k in a:
            i = a.index(k)
            opt[k] = a[i + 1]
            del a[i:i + 2]
    per = '--per-key' in a
    a = [x for x in a if x != '--per-key']
    lo, hi = (int(v) for v in opt['--keys'].split('-'))
    keys = [k for k in KEYS if lo <= k <= hi]
    win = tuple(float(v) for v in opt['--win'].split(','))
    n = int(opt['--n'])
    srcs = [('ref', a[0])] + [tuple(s.split('=', 1)) for s in a[1:]]
    R = prof(srcs[0][1], opt['--dyn'], keys, win, n)
    print(f'keys {keys[0]}-{keys[-1]} {opt["--dyn"]} {win} s: partial n, dB re the key total (ref), others minus ref')
    print(f'{"n":>12}' + ''.join(f'{i:6d}' for i in range(1, n + 1)))
    print(f'{"ref":>12}' + ''.join(f'{v:6.1f}' for v in R.mean(0)))
    for lab, d in srcs[1:]:
        D = prof(d, opt['--dyn'], keys, win, n) - R
        print(f'{lab[:12]:>12}' + ''.join(f'{v:+6.1f}' for v in D.mean(0)))
        if per:
            for i, k in enumerate(keys):
                print(f'{k:>12}' + ''.join(f'{v:+6.1f}' for v in D[i]))


if __name__ == '__main__':
    main()
