"""The phrase metric for Chorda's voicing pass three (round 38): how far a render of the ABX
kit's seven phrases is from the Salamander's, in the terms the owner's words named.

    python metric.py <salamander wav dir> <candidate wav dir>   # prints the breakdown
"""
import os, sys
import numpy as np
from scipy.io import wavfile
from scipy import signal

PHRASES = ['middle_c', 'soft_chords', 'fast_run', 'ballad', 'ff_chord', 'bass', 'top']
CENTRES = 1000 * 2 ** (np.arange(-15, 13) / 3)


def a_weight(f):
    f2 = f * f
    ra = (12194**2 * f2**2) / ((f2 + 20.6**2) * np.sqrt((f2 + 107.7**2) * (f2 + 737.9**2)) * (f2 + 12194**2))
    return 20 * np.log10(ra) + 2.0


A = a_weight(CENTRES)


def load(path):
    sr, x = wavfile.read(path)
    if x.dtype == np.int32:
        x = x / 2147483648.0
    elif x.dtype == np.int16:
        x = x / 32768.0
    x = np.asarray(x, dtype=np.float64)
    if x.ndim == 1:
        x = np.stack([x, x], 1)
    return sr, x


def thirds(x, sr):
    f, p = signal.welch(x.mean(1), sr, nperseg=16384, noverlap=12288)
    tot = p.sum()
    out = np.empty(len(CENTRES))
    for i, c in enumerate(CENTRES):
        sel = (f >= c / 2 ** (1 / 6)) & (f < c * 2 ** (1 / 6))
        out[i] = 10 * np.log10(p[sel].sum() / tot + 1e-30)
    return out


def smooth(x):
    p = 10 ** (x / 10)
    return 10 * np.log10(np.convolve(p, np.ones(3) / 3, mode='same'))


def env_db(x, sr, ms=10):
    n = int(sr * ms / 1000)
    m = x.mean(1)
    k = len(m) // n
    return 20 * np.log10(np.sqrt((m[:k * n].reshape(k, n) ** 2).mean(1) + 1e-20))


def feats(sr, x):
    mid = x.mean(1)
    rms = np.sqrt((mid ** 2).mean())
    e = env_db(x, sr)
    loud = e[e > e.max() - 40]
    return dict(
        thirds=thirds(x, sr),
        crest=20 * np.log10(np.abs(x).max() / rms),
        dyn=np.percentile(loud, 95) - np.percentile(loud, 10),
        corr=np.corrcoef(x[:, 0], x[:, 1])[0, 1],
    )


_REF = {}


def ref(dir_):
    if dir_ not in _REF:
        _REF[dir_] = {p: feats(*load(os.path.join(dir_, p + '.wav'))) for p in PHRASES}
    return _REF[dir_]


AUDIBLE = (CENTRES >= 40) & (CENTRES <= 16000)
FLOOR = 0.02


def band_error(r, c):
    """Energy-weighted (A-weighted reference) RMS of the octave-smoothed differences, every
    band 40 Hz-16 kHz; each band also carries a floor weight, so a quiet band made 30 dB too
    loud (a hiss, a metallic edge) costs even where the reference has little energy."""
    rs, cs = smooth(r)[AUDIBLE], smooth(c)[AUDIBLE]
    w = 10 ** ((rs + A[AUDIBLE]) / 10)
    w = w / w.sum() + FLOOR
    d = cs - rs
    return float(np.sqrt((w * d * d).sum() / w.sum()))


def score(ref_dir, cand_dir, verbose=False):
    R = ref(ref_dir)
    parts = {'bands': [], 'crest': [], 'dyn': [], 'corr': []}
    rows = []
    for p in PHRASES:
        c = feats(*load(os.path.join(cand_dir, p + '.wav')))
        r = R[p]
        be = band_error(r['thirds'], c['thirds'])
        parts['bands'].append(be)
        parts['crest'].append(abs(c['crest'] - r['crest']))
        parts['dyn'].append(abs(c['dyn'] - r['dyn']))
        parts['corr'].append(abs(c['corr'] - r['corr']))
        rows.append((p, be, c['crest'], r['crest'], c['dyn'], r['dyn'], c['corr'], r['corr']))
    # Weights: a band error's dB counts fully; crest and envelope spread at a quarter per dB
    # (a peak's level is heard less exactly than a band's); correlation at 4 per unit.
    s = dict(bands=np.mean(parts['bands']), crest=0.25 * np.mean(parts['crest']),
             dyn=0.25 * np.mean(parts['dyn']), corr=4 * np.mean(parts['corr']))
    total = sum(s.values())
    if verbose:
        print(f"{'phrase':12} {'bands':>6} {'crest c/s':>12} {'dyn c/s':>12} {'corr c/s':>12}")
        for p, be, cc, rc, cd, rd, co, ro in rows:
            print(f"{p:12} {be:6.2f} {cc:5.1f}/{rc:5.1f} {cd:5.1f}/{rd:5.1f} {co:5.2f}/{ro:5.2f}")
        print('TOTAL %.3f  ' % total + '  '.join(f'{k}={v:.3f}' for k, v in s.items()))
    return total, s


def detail(ref_dir, cand_dir):
    """Per phrase, the octave-smoothed difference at each centre (dB)."""
    R = ref(ref_dir)
    out = {}
    for p in PHRASES:
        c = feats(*load(os.path.join(cand_dir, p + '.wav')))
        out[p] = smooth(c['thirds']) - smooth(R[p]['thirds'])
    return out


if __name__ == '__main__':
    score(sys.argv[1], sys.argv[2], verbose=True)
    if len(sys.argv) > 3:
        d = detail(sys.argv[1], sys.argv[2])
        print('centres', ' '.join(f'{c:6.0f}' for c in CENTRES))
        for p, v in d.items():
            print(f'{p:12}', ' '.join(f'{x:6.1f}' for x in v))
