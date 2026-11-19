"""Round 40 (candidate 9): tuning in the acceptance suite. Every key's partials 1-6 (every key at
mf, keys_mf: key k at (k-21)*4 s; 0.1-1.1 s, both channels' power) in cents, candidate against
the Salamander, after taking out the median offset of the first partial (a whole-instrument
pitch offset is not a mistuned key); the worst key; the beats of the consonant intervals
(fifths, fourths, octaves, twelfths) in the test chords (the soft chords' four and the ff
chord). Writes the log accept3 reads (P20) and exits 1 when any key's first partial is more
than 10 cents from the reference's stretch (the gate).
    python -I tuning.py <cand keys_mf.wav> [<log>]"""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys, itertools
import numpy as np, soundfile as sf
SR = 48000; N = 1 << 18
REF = _os.path.join(LAB, 'keys/wav/salamander/keys_mf.wav')
CHORDS = [[48, 55, 59, 64, 67], [45, 52, 59, 60, 64, 71], [45, 53, 60, 64, 69], [43, 50, 57, 60, 64, 65],
          [24, 36, 43, 48, 52, 55, 60, 64, 67, 72]]
RAT = {5: (4, 3), 7: (3, 2), 12: (2, 1), 19: (3, 1), 24: (4, 1)}


def partials(x, k):
    a = int(((k - 21) * 4 + 0.1) * SR); y = x[a:a + SR]
    P = sum(np.abs(np.fft.rfft(y[:, c] * np.hanning(len(y)), N)) ** 2 for c in range(2))
    F = np.fft.rfftfreq(N, 1 / SR); f0 = 440 * 2 ** ((k - 69) / 12); out, prev = [], 0.0
    for n in range(1, 7):
        g = n * f0 * 2 ** (prev / 1200)
        m = (F > g * 2 ** (-60 / 1200)) & (F < g * 2 ** (60 / 1200))
        if not m.any() or n * f0 > 16000:
            out.append(np.nan); continue
        i = np.flatnonzero(m)[np.argmax(P[m])]
        a_, b_, c_ = (np.log(P[j] + 1e-30) for j in (i - 1, i, i + 1))
        f = (i + 0.5 * (a_ - c_) / (a_ - 2 * b_ + c_)) * SR / N
        out.append(1200 * np.log2(f / (n * f0))); prev = out[-1]
    return np.array(out)


c = sf.read(sys.argv[1], always_2d=True)[0]; r = sf.read(REF, always_2d=True)[0]
C = {k: partials(c, k) for k in range(21, 109)}; R = {k: partials(r, k) for k in range(21, 109)}
d1 = np.array([C[k][0] - R[k][0] for k in range(21, 109)]); off = np.nanmedian(d1); d1 -= off
d13 = np.array([C[k][:3] - R[k][:3] for k in range(21, 109)]) - off
wi = int(np.nanargmax(np.abs(d1)))
beats = []
for ch in CHORDS:
    for a, b in itertools.combinations(sorted(ch), 2):
        if b - a not in RAT: continue
        q, p = RAT[b - a]
        bt = [q * 440 * 2 ** ((a - 69) / 12 + X[a][q - 1] / 1200) - p * 440 * 2 ** ((b - 69) / 12 + X[b][p - 1] / 1200) for X in (C, R)]
        if np.all(np.isfinite(bt)):
            beats.append(abs(abs(bt[0]) - abs(bt[1])))
lines = [f'offset {off:+.1f} cents (median, taken out)',
         f'first partial rms {np.sqrt(np.nanmean(d1 ** 2)):.2f} cents; partials 1-3 rms {np.sqrt(np.nanmean(d13 ** 2)):.2f}',
         f'worst key {wi + 21} {d1[wi]:+.1f} cents',
         f'chord beats mean |diff| {np.mean(beats):.2f} Hz over {len(beats)} consonant intervals',
         'GATE ' + ('FAIL' if np.nanmax(np.abs(d1)) > 10 else 'ok') + ' (every key within 10 cents of the reference)']
open(sys.argv[2] if len(sys.argv) > 2 else 'tuning.log', 'w').write('\n'.join(lines) + '\n')
print('\n'.join(lines))

# Round 40, the technician's pass (2026-11-18, docs/round40/chorda-tune.md): the candidate on
# its own, every key A0-C8, measured as Cantus's suite measures it (chorda::tunekit): each
# partial heterodyned to where the FFT found it and averaged over frames (a triangle 4 periods
# long, every 5 ms); its frequency the power-weighted mean phase advance over 0.1-1.1 s (a
# beating unison's mean, as the ear hears it, not its strongest string's peak). The key's pitch
# is partials 1-8 below 5 kHz weighed by power and the dominance region (a log-normal bump at
# 700 Hz, 1.5 octaves wide); its raggedness is that pitch against its neighbours' mean (the
# curve's own bend over two semitones is under a hundredth of a cent); every octave (2:1, 4:2)
# and fifth (3:2) is checked against the same interval on a smooth fit (a quartic in key) of the
# candidate's own partials. A second view of the suite's gate (chorda::tests_tune_gate, which
# measures against the designed curve itself and holds twice as tight): this one knows only
# the render, so its reference is a fit and its tolerances are doubled. Gate 2 fails when a key
# A0-E6 is more than 2 cents off its neighbours (4 above), or an interval beats more than
# 0.5 Hz and 4 cents off the fit (5 above E6; a 4:2 1 Hz and 7 cents). Candidate 16 failed it
# (B0 +16, A#0, C1, E1; 33 intervals); the technician's pass passes it.
def mean_freq(x, k, near):
    a = int(((k - 21) * 4 + 0.1) * SR); b = a + SR
    f0 = 440 * 2 ** ((k - 69) / 12)
    half = max(16, int(2 * SR / f0))
    t = np.arange(a - half, b + half)
    seg = x[a - half:b + half]
    y = (seg[:, 0] + seg[:, 1]) * np.exp(-2j * np.pi * near * t / SR)
    c = np.cumsum(np.concatenate([[0], y]))
    box = (c[half:] - c[:-half]) / half
    c2 = np.cumsum(np.concatenate([[0], box]))
    tri = (c2[half:] - c2[:-half]) / half
    z = tri[::240]
    if len(z) < 3:
        return np.nan, 0.0
    r = np.sum(z[1:] * np.conj(z[:-1]))
    return near + np.angle(r) * SR / (2 * np.pi * 240), float(np.mean(np.abs(z) ** 2))


def key_partials(x, X, k):
    f0 = 440 * 2 ** ((k - 69) / 12)
    out = {}
    for n in range(1, 9):
        if n * f0 > 5000 or not np.isfinite(X[k][min(n, 6) - 1]):
            break
        near = n * f0 * 2 ** ((X[k][n - 1] if n <= 6 else X[k][5] + 0.0) / 1200)
        f, p = mean_freq(x, k, near)
        out[n] = (1200 * np.log2(f / (n * f0)), p)
    return out


M = {k: key_partials(c, C, k) for k in range(21, 109)}


def pitch_of(k):
    f0 = 440 * 2 ** ((k - 69) / 12)
    ws = [(np.sqrt(p) * (0.1 + 0.9 * np.exp(-0.5 * (np.log2(n * f0 / 700) / 1.5) ** 2)), ct)
          for n, (ct, p) in M[k].items()]
    return sum(w * ct for w, ct in ws) / sum(w for w, _ in ws)


P = np.array([pitch_of(k) for k in range(21, 109)])
rag = P[1:-1] - 0.5 * (P[:-2] + P[2:])
tol = np.array([2.0 if k <= 88 else 4.0 for k in range(22, 108)])
# Above E6 informative only: a 1 s FFT's start point there can land on the board's tail.
bad = [(k, r) for k, r, t in zip(range(22, 108), rag, tol) if abs(r) > t and k <= 88]
top = [(k, r) for k, r, t in zip(range(22, 108), rag, tol) if abs(r) > t and k > 88]
fits = {}
for n in range(1, 5):
    ks = [k for k in range(21, 109) if n in M[k]]
    if len(ks) > 8:
        fits[n] = np.poly1d(np.polyfit(ks, [M[k][n][0] for k in ks], 4))
ivs = []
for lo in range(21, 102):
    for hi, p, q in ((lo + 12, 2, 1), (lo + 12, 4, 2), (lo + 7, 3, 2)):
        if hi > 108 or p not in M[lo] or q not in M[hi] or p not in fits or q not in fits:
            continue
        err = (M[lo][p][0] - M[hi][q][0]) - (fits[p](lo) - fits[q](hi))
        hz = p * 440 * 2 ** ((lo - 69) / 12) * err / 1731.234
        ct, slow = (7.0, 1.0) if p == 4 else ((4.0 if hi <= 88 else 5.0), 0.5)
        ivs.append((lo, hi, p, q, err, hz, abs(hz) > slow and abs(err) > ct))
worst_iv = max(ivs, key=lambda x: abs(x[5]))
bad_iv = [x for x in ivs if x[6]]
mr = int(np.argmax(np.abs(rag)))
lines2 = [f'pitch (ear, mean frequency) rms {np.sqrt(np.mean((P - np.median(P)) ** 2)):.2f} cents about its median; '
          f'most ragged key {22 + mr} {rag[mr]:+.2f}',
          f'intervals: fastest {worst_iv[0]}-{worst_iv[1]} {worst_iv[2]}:{worst_iv[3]} {worst_iv[4]:+.2f} cents '
          f'({worst_iv[5]:+.2f} Hz) off the fit; {len(bad_iv)} of {len(ivs)} over tolerance',
          'GATE2 ' + ('FAIL ' + ' '.join(f'{k}:{r:+.1f}' for k, r in bad[:8])
                      + ' ' + ' '.join(f'{a}-{b}:{e:+.1f}' for a, b, _, _, e, _, _ in bad_iv[:8])
                      if bad or bad_iv else 'ok')
          + ' (every key A0-E6 on a smooth curve, its octaves and fifths with it)'
          + (' [above E6, informative: ' + ' '.join(f'{k}:{r:+.1f}' for k, r in top) + ']' if top else '')]
open(sys.argv[2] if len(sys.argv) > 2 else 'tuning.log', 'a').write('\n'.join(lines2) + '\n')
print('\n'.join(lines2))
sys.exit(1 if (np.nanmax(np.abs(d1)) > 10 or bad or bad_iv) else 0)
