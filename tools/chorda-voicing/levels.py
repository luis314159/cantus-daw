"""Round 40: each listening phrase's loudness against the staging triad's (C-major at
velocity 100, `triad`), against the same for the reference — the balance between the
registers and the dynamics, which a level-free comparison cannot see. Mean |difference|, LU.

    python levels.py <ref dir> <cand dir>
"""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '.'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import os, sys
import soundfile as sf
from heard import lufs
import metric

_REF = {}


def rel(d):
    x = {}
    for p in metric.PHRASES + ['triad']:
        y, _ = sf.read(os.path.join(d, p + '.wav'), dtype='float64', always_2d=True)
        x[p] = lufs(y)
    return {p: x[p] - x['triad'] for p in metric.PHRASES}


def score(ref_dir, cand_dir, verbose=False):
    if ref_dir not in _REF:
        _REF[ref_dir] = rel(ref_dir)
    R, C = _REF[ref_dir], rel(cand_dir)
    d = {p: C[p] - R[p] for p in R}
    if verbose:
        print(' '.join(f'{p}={v:+.1f}' for p, v in d.items()))
    return sum(abs(v) for v in d.values()) / len(d)


if __name__ == '__main__':
    print('LEVELS %.2f' % score(sys.argv[1], sys.argv[2], True))
