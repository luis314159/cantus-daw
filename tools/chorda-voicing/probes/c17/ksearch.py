"""Round 42 (chorda-c17): a coordinate search of the action's knock against both recorded grands
(knockfit's score on the fit keys), each trial the single notes through the lab over
`base=shipped` + the fixed spec + the trial's knobs; the held-out keys' score printed beside.

    python -I ksearch.py <tag> <fixed spec> <knob=lo:hi:start,...> [rounds 2]

A knob's steps are a quarter of its range, halved each round. The log and the best spec go to
<lab>/c17/search-<tag>.log. Every trial's renders are deleted after scoring.
"""
import os, sys, shutil, subprocess
import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
from labpaths import LAB, EXE  # noqa: E402
import knockfit  # noqa: E402

FIT = [99, 102, 105, 108]
HELD = [84, 87, 90, 93, 96]


def render(spec, out):
    env = dict(os.environ, CHORDA_PHRASES='notes_pp,notes_mf,notes_ff')
    subprocess.run([EXE, 'lab', out, 'n', spec], check=True, env=env,
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    return os.path.join(out, 'wav', 'n')


def trial(tag, i, spec):
    out = os.path.join(LAB, 'c17', f'ks-{tag}-{i}')
    d = render(spec, out)
    for k in [k for k in knockfit.CACHE if k[0] == d]:
        del knockfit.CACHE[k]
    mid = [k for k in knockfit.KEYS if 48 <= k <= 72]
    bass = [k for k in knockfit.KEYS if 24 <= k <= 45]
    s = knockfit.score(d, FIT) + knockfit.between_score(d, mid)
    h = knockfit.score(d, HELD) + knockfit.between_score(d, bass)
    shutil.rmtree(out, ignore_errors=True)
    return s, h


def main():
    tag, fixed, knobs = sys.argv[1], sys.argv[2], sys.argv[3]
    rounds = int(sys.argv[4]) if len(sys.argv) > 4 else 2
    P = {}
    for kv in knobs.split(','):
        k, r = kv.split('=')
        f = r.split(':')
        lo, hi, x = (float(v) for v in f[:3])
        P[k] = [lo, hi, x, len(f) > 3 and f[3] == 'log']
    log = open(os.path.join(LAB, 'c17', f'search-{tag}.log'), 'a')

    def spec_of(cur):
        return ','.join(['base=shipped'] + [s for s in fixed.split(',') if s] + [f'{k}={v:.5g}' for k, v in cur.items()])

    cur = {k: v[2] for k, v in P.items()}
    # A `log` knob steps by ratios: a quarter of its range in octaves of its own.
    def step_to(k, x, sgn, rnd):
        lo, hi, _, lg = P[k]
        if lg:
            r = (hi / lo) ** (1 / 4 / 2 ** rnd)
            return min(hi, max(lo, x * r ** sgn))
        return min(hi, max(lo, x + sgn * (hi - lo) / 4 / 2 ** rnd))
    n = 0
    best, held = trial(tag, n, spec_of(cur))
    print(f'start {best:.3f} (held {held:.3f}) {spec_of(cur)}', file=log, flush=True)
    for rnd in range(rounds):
        for k in P:
            for sgn in (1, -1):
                improved = True
                while improved:
                    improved = False
                    x = step_to(k, cur[k], sgn, rnd)
                    if x == cur[k]:
                        break
                    t = dict(cur, **{k: x})
                    n += 1
                    s, h = trial(tag, n, spec_of(t))
                    print(f'{n:3d} {k}={x:.5g} {s:.3f} (held {h:.3f})', file=log, flush=True)
                    if s < best - 1e-3:
                        best, held, cur, improved = s, h, t, True
        print(f'round {rnd}: {best:.3f} (held {held:.3f}) {spec_of(cur)}', file=log, flush=True)
    print(f'BEST {best:.3f} held {held:.3f}\n{spec_of(cur)}')


if __name__ == '__main__':
    main()
