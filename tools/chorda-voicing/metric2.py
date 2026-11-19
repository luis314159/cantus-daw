"""Round 38's phrase metric with an audibility floor (round 40, candidate 11) — printed beside
the old number, never instead of it.

Why: the old metric's floor weight makes every third-octave band 40 Hz-16 kHz count, and the
reference's middle C carries 31-125 Hz that is not its piano: the Salamander's recording floor
(hum and rumble) while a sample plays — the same level under a C8 as under a C4 (whose notes
have nothing there), only the sampler's velocity gain moving it, and gone the moment the sample
is released (`probes/c11/hum.py`, `hum2.py`, `hum3.py`). Chorda, which has no hum, was charged
10-27 dB for it in middle C's bands.

The change: each phrase's bands are placed at a listening level — the reference's ff chord at
90 dB SPL (its Leq), every other reference phrase at its recorded level against it, and the
candidate at its reference phrase's loudness (as the metric's normalisation already assumes) —
and each band, reference and candidate alike, is raised to ISO 226:2003's threshold of hearing
at its centre before the difference is taken: a difference no one can hear costs nothing; a
candidate band louder than the threshold where the reference's is not is charged from the
threshold. Nothing else in the metric moves (weights, smoothing, crest, dynamics, correlation).

    python -I metric2.py <salamander wav dir> <candidate wav dir>
"""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '.'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import os, sys
import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import metric
from metric import PHRASES, CENTRES, AUDIBLE, FLOOR, A, smooth, feats, load

# ISO 226:2003, the threshold of hearing T_f (dB SPL) at the third-octave centres; 16 kHz
# from ISO 389-7 (free field).
ISO_HZ = [20, 25, 31.5, 40, 50, 63, 80, 100, 125, 160, 200, 250, 315, 400, 500, 630, 800,
          1000, 1250, 1600, 2000, 2500, 3150, 4000, 5000, 6300, 8000, 10000, 12500, 16000]
ISO_TF = [78.5, 68.7, 59.5, 51.1, 44.0, 37.5, 31.5, 26.5, 22.1, 17.9, 14.4, 11.4, 8.6, 6.2,
          4.4, 3.0, 2.2, 2.4, 3.5, 1.7, -1.3, -4.2, -6.0, -5.4, -1.5, 6.0, 12.6, 13.9, 12.3,
          44.0]
THRESH = np.interp(np.log(CENTRES), np.log(ISO_HZ), ISO_TF)
FF_SPL = 90.0


def power_db(x):
    return 10 * np.log10(np.mean(x.mean(1) ** 2) + 1e-30)


def band_error_audible(r, c, spl):
    """The metric's band error with both spectra floored at the threshold of hearing: `r`, `c`
    the third-octave levels re the phrase's power, `spl` that power's level (dB SPL)."""
    rs, cs = smooth(r), smooth(c)
    t = THRESH - spl  # the threshold re the phrase's power
    rs_a, cs_a = np.maximum(rs, t)[AUDIBLE], np.maximum(cs, t)[AUDIBLE]
    w = 10 ** ((rs[AUDIBLE] + A[AUDIBLE]) / 10)
    w = w / w.sum() + FLOOR
    d = cs_a - rs_a
    return float(np.sqrt((w * d * d).sum() / w.sum()))


def score2(ref_dir, cand_dir, verbose=False):
    Rx = {p: load(os.path.join(ref_dir, p + '.wav')) for p in PHRASES}
    off = FF_SPL - power_db(Rx['ff_chord'][1])
    parts = {'bands': [], 'crest': [], 'dyn': [], 'corr': []}
    rows = []
    for p in PHRASES:
        r = feats(*Rx[p])
        c = feats(*load(os.path.join(cand_dir, p + '.wav')))
        spl = power_db(Rx[p][1]) + off
        be = band_error_audible(r['thirds'], c['thirds'], spl)
        parts['bands'].append(be)
        parts['crest'].append(abs(c['crest'] - r['crest']))
        parts['dyn'].append(abs(c['dyn'] - r['dyn']))
        parts['corr'].append(abs(c['corr'] - r['corr']))
        rows.append((p, be, metric.band_error(r['thirds'], c['thirds']), spl))
    s = dict(bands=np.mean(parts['bands']), crest=0.25 * np.mean(parts['crest']),
             dyn=0.25 * np.mean(parts['dyn']), corr=4 * np.mean(parts['corr']))
    total = sum(s.values())
    if verbose:
        print(f"{'phrase':12} {'bands':>6} {'(old)':>6} {'SPL':>5}")
        for p, be, old, spl in rows:
            print(f'{p:12} {be:6.2f} {old:6.2f} {spl:5.1f}')
        print('TOTAL(audible) %.3f  ' % total + '  '.join(f'{k}={v:.3f}' for k, v in s.items()))
    return total, s


if __name__ == '__main__':
    old, _ = metric.score(sys.argv[1], sys.argv[2])
    new, _ = score2(sys.argv[1], sys.argv[2], verbose=True)
    print(f'round 38 metric {old:.3f}  audible {new:.3f}')
