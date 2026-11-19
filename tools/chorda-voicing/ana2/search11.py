"""Round 40, candidate 6 (second builder): the one search on the combined measure (search11: the integrator's bounds - hf_decay 0.9-1.4, hf_treble near 3) — each trial
a full run6.sh (both analysts' measures, no every-key render), scored by accept3 (analyst 1's
accept.py + the second analysis's ranked items) + 0.5 x round 38's phrase metric + 0.7 x the
phrases' balance against the staging triad.

    python search11.py <tag> <params, comma list> <start spec file> [rounds] [scale] [parallel]
Log: <lab>/search/search11-<tag>.log; the best: <lab>/search/best11-<tag>.txt (the spec, then the
score). Env: CHORDA_EXE, CHORDA_PARTIALS, CHORDA_LEVELS (and CHORDA_LAB, see labpaths.py).
"""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import os, re, sys, shutil, subprocess, itertools, concurrent.futures as cf
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
V = LAB  # data: the lab; code: HERE and ../ana
OUTD = os.path.join(V, 'search')
os.makedirs(os.path.join(V, 'c6'), exist_ok=True)
os.makedirs(OUTD, exist_ok=True)
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.join(TOOLS, 'ana'))
import accept  # noqa: E402
import accept3  # noqa: E402

P = {
    'body.eq4k': (2.0, 'add', -12.0, 9.0), 'body.eq8k': (2.0, 'add', -14.0, 6.0),
    'reg.mid.6': (2.0, 'add', -12.0, 9.0), 'reg.treble.6': (2.0, 'add', -12.0, 9.0),
    'partials_amount': (0.2, 'add', 0.0, 1.2), 'reg.bass.level': (1.0, 'add', -9.0, 6.0),
    'reg.mid.level': (1.0, 'add', -9.0, 6.0), 'reg.treble.level': (1.0, 'add', -9.0, 6.0),
    'coupling': (1.1, 'mul', 0.5, 1.8), 'decay_mid': (1.15, 'mul', 0.8, 4.0),
    'uni_b': (1.3, 'mul', 0.005, 0.3), 'uni_weight_low': (0.12, 'add', 0.0, 1.0),
    'top_damper': (1.4, 'mul', 0.01, 0.5), 'landing': (1.6, 'mul', 0.05, 8.0),
    'felt_upper': (1.15, 'mul', 0.2, 1.5), 'room.early': (1.25, 'mul', 0.1, 2.0),
    'itd_jitter': (0.15, 'add', 0.0, 1.2), 'body.coh_hz': (1.4, 'mul', 50.0, 1500.0),
    'spread': (1.2, 'mul', 0.05, 1.0), 'hf_decay': (1.12, 'mul', 0.9, 1.4),
    'hf_treble': (1.1, 'mul', 2.6, 3.4), 'output': (1.06, 'mul', 0.003, 0.05),
    'hf_bass': (1.4, 'mul', 0.5, 20.0), 'body.direct': (1.25, 'mul', 0.1, 0.9),
    'uni_skew': (0.25, 'add', 0.0, 1.5), 'detune_scale': (1.25, 'mul', 0.1, 3.0),
    'itd_ms': (1.25, 'mul', 0.1, 1.5), 'top_hz': (1.2, 'mul', 4000.0, 16000.0),
    'itd_shape': (0.5, 'add', 0.0, 3.0), 'uni_weight_low': (0.08, 'add', 0.0, 1.0),
    'decay_treble': (1.25, 'mul', 0.3, 6.0),
}
_n = itertools.count()


def spec(vals):
    return ','.join(f'{k}={v:.6g}' for k, v in vals.items())


def trial(vals, tag, label, log, tries=3):
    """One full run6 evaluation; a trial whose analysis failed (the machine's memory runs
    short while other builders compile: MemoryError in a script) is run again."""
    name = f'z{tag}{next(_n) % 8}'
    sf = os.path.join(V, 'c6', f'{name}.txt')
    open(sf, 'w').write(spec(vals) + '\n')
    env = dict(os.environ, NO_KEYS='1')
    for attempt in range(tries):
        subprocess.run(['bash', os.path.join(HERE, 'run6.sh'), name, sf], env=env, stdout=subprocess.DEVNULL,
                       stderr=subprocess.DEVNULL)
        o1, o2 = os.path.join(V, 'ana', 'out_' + name), os.path.join(V, 'ana2', 'out_' + name)
        try:
            dep = [l for l in open(os.path.join(o2, 's24_beat_depth.log'), encoding='utf-8', errors='replace')
                   if l.startswith("['")]
            if len(dep) < 20:
                raise ValueError('a short beat list')
            a, _ = accept.score(o1)
            if a >= 1e8:
                raise ValueError('an analyst-1 measure missing')
            parts = accept3.score3(o2)
            gone = [k for k, v in parts.items() if not k.startswith('P18') and not np.isfinite(v)]
            if gone:  # a missing measure would count as a perfect one
                raise ValueError('missing ' + ','.join(gone))
            total3 = accept3.total(a, parts)
            ml = open(os.path.join(o1, 'metric.log'), encoding='utf-8', errors='replace').read()
            metric = float(re.search(r'TOTAL ([\d.]+)', ml).group(1))
            levels = float(re.search(r'LEVELS ([\d.]+)', ml).group(1))
            break
        except Exception as e:  # a failed trial is never a best
            log.write(f'99 {label} failed (try {attempt + 1}) {e}\n'); log.flush()
    else:
        return 99.0
    total = total3 + 0.5 * metric + 0.7 * levels
    log.write(f'{total:.4f} {label} | accept3={total3:.3f} accept={a:.3f} metric={metric:.3f} levels={levels:.2f} '
              + ' '.join(f'{k.split()[0]}={v:.2f}' for k, v in parts.items()) + f' | {spec(vals)}\n')
    log.flush()
    return total


def step(k, v, sign, scale):
    st, kind, lo, hi = P[k]
    v = v * (st ** (sign * scale)) if kind == 'mul' else v + sign * st * scale
    return min(max(v, lo), hi)


def main():
    tag, params, start = sys.argv[1], sys.argv[2].split(','), sys.argv[3]
    rounds = int(sys.argv[4]) if len(sys.argv) > 4 else 1
    scale0 = float(sys.argv[5]) if len(sys.argv) > 5 else 1.0
    par = int(sys.argv[6]) if len(sys.argv) > 6 else 4
    vals = {k: float(v) for k, v in (kv.split('=') for kv in open(start).readline().strip().split(','))}
    for k in params:
        vals.setdefault(k, 0.0)
    log = open(os.path.join(OUTD, f'search11-{tag}.log'), 'a')
    best = float(os.environ['START_SCORE']) if 'START_SCORE' in os.environ else trial(vals, tag, 'start', log)
    print('start', round(best, 3), flush=True)
    open(os.path.join(OUTD, f'best11-{tag}.txt'), 'w').write(spec(vals) + '\n%.4f\n' % best)
    for rnd in range(rounds):
        scale = scale0 / (2 ** rnd)
        for j in range(0, len(params), par // 2):
            jobs = []
            for k in params[j:j + par // 2]:
                for sg in (1, -1):
                    t = dict(vals, **{k: step(k, vals[k], sg, scale)})
                    if t[k] != vals[k]:
                        jobs.append((k, t))
            with cf.ThreadPoolExecutor(par) as ex:
                res = list(ex.map(lambda kt: trial(kt[1], tag, f'r{rnd} {kt[0]}={kt[1][kt[0]]:.4g}', log), jobs))
            moved = {}
            for (k, t), s in zip(jobs, res):
                if s < best and (k not in moved or s < moved[k][0]):
                    moved[k] = (s, t[k])
            if moved:
                k1 = min(moved, key=lambda k: moved[k][0])
                cand = [(moved[k1][0], dict(vals, **{k1: moved[k1][1]}))]
                if len(moved) > 1:
                    both = dict(vals, **{k: v for k, (s, v) in moved.items()})
                    cand.append((trial(both, tag, f'r{rnd} together {",".join(moved)}', log), both))
                s, v = min(cand, key=lambda c: c[0])
                if s < best:
                    best, vals = s, v
                    open(os.path.join(OUTD, f'best11-{tag}.txt'), 'w').write(spec(vals) + '\n%.4f\n' % best)
                    log.write(f'BEST {best:.4f} | {spec(vals)}\n'); log.flush()
            print(rnd, j, round(best, 3), flush=True)
            # Disk: each trial's renders go once scored.
            for d in os.listdir(os.path.join(V, 'lab4', 'wav')):
                if d.startswith(f'z{tag}'):
                    for root in ('lab4/wav', 'notes5/wav', 'ana', 'ana2'):
                        for pre in ('', 'out_', 'prev_'):
                            shutil.rmtree(os.path.join(V, root, pre + d), ignore_errors=True)


if __name__ == '__main__':
    main()
