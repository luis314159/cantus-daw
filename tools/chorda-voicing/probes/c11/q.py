"""Candidate 11's quick probe: round 38's metric (per phrase), each phrase's presence band
against the reference, middle C's partials 8-9 (1/6 octave, as ltas.py), and the single notes'
presence-minus-body per register and dynamic (as vbright.py, early window).
   python -I q.py <lab wav dir> <notes wav dir or -> """
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '../..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys, os, io, contextlib
import numpy as np, soundfile as sf
V = LAB
sys.path.insert(0, TOOLS)
import metric

cand, notes = sys.argv[1], sys.argv[2]
SAL = V + '/eng40/wav/salamander'
buf = io.StringIO()
with contextlib.redirect_stdout(buf):
    tot, s = metric.score(SAL, cand, verbose=True)
rows = buf.getvalue().splitlines()
bands = {r.split()[0]: float(r.split()[1]) for r in rows[1:8]}
d = metric.detail(SAL, cand)
C = metric.CENTRES
pres = (C >= 1200) & (C <= 3300)
hi = (C >= 3900) & (C <= 6500)
print(f'metric {tot:.3f}  ' + ' '.join(f'{k}={v:.2f}' for k, v in s.items()))
print('phrase       bands  pres(1.26-3.2k)  hi(4-6.3k)')
for p in metric.PHRASES:
    print(f'{p:12} {bands[p]:5.2f}  {d[p][pres].mean():+6.1f}  {d[p][hi].mean():+6.1f}')


def ltas(path):
    x, sr = sf.read(path, always_2d=True)
    n = 8192
    P = 0
    for c in range(2):
        for i in range(0, len(x) - n, n // 2):
            P = P + np.abs(np.fft.rfft(x[i:i + n, c] * np.hanning(n))) ** 2
    F = np.fft.rfftfreq(n, 1 / sr)
    tot = P[(F > 200) & (F < 5000)].sum()
    out = []
    for f in (2016, 2263, 4032, 5080):
        out.append(10 * np.log10(P[(F >= f * 2 ** (-1 / 12)) & (F < f * 2 ** (1 / 12))].sum() / tot))
    return np.array(out)


m = ltas(cand + '/middle_c.wav') - ltas(SAL + '/middle_c.wav')
print('middle C 1/6 oct re ref: 2.0k %+.1f  2.26k %+.1f  4.0k %+.1f  5.1k %+.1f' % tuple(m))
if notes != '-':
    import importlib.util
    KEYS = list(range(21, 109, 3))
    REG = [('bass', 21, 45), ('tenor', 48, 57), ('mid', 60, 69), ('upper', 72, 84)]

    def meas(path):
        x, sr = sf.read(path, always_2d=True)
        mm = x.sum(1)
        out = {}
        for i, k in enumerate(KEYS):
            if k > 84:
                continue
            seg = mm[int(i * 6 * sr):int(i * 6 * sr) + 3 * sr]
            a = np.abs(seg)
            on = int(np.argmax(a > 0.05 * a.max()))
            f0 = 440 * 2 ** ((k - 69) / 12)
            r = []
            for t0, t1 in ((0.03, 0.6), (0.6, 2.0)):
                y = seg[on + int(t0 * sr):on + int(t1 * sr)]
                P = np.abs(np.fft.rfft(y * np.hanning(len(y)))) ** 2
                F = np.fft.rfftfreq(len(y), 1 / sr)
                body = P[(F >= f0 * 0.8) & (F < max(1200, f0 * 2.5))].sum()
                pr = P[(F >= 1600) & (F < 3200)].sum()
                r.append(10 * np.log10(pr / body + 1e-30))
            out[k] = r
        return out
    print('notes: presence-body re ref (early / late)   pp | mf | ff')
    R = {dy: meas(f'{V}/notes5/wav/salamander/notes_{dy}.wav') for dy in ('pp', 'mf', 'ff')}
    Cn = {dy: meas(f'{notes}/notes_{dy}.wav') for dy in ('pp', 'mf', 'ff')}
    for rn, a, b in REG:
        ks = [k for k in KEYS if a <= k <= b]
        cells = []
        for dy in ('pp', 'mf', 'ff'):
            e = np.mean([Cn[dy][k][0] - R[dy][k][0] for k in ks])
            l = np.mean([Cn[dy][k][1] - R[dy][k][1] for k in ks])
            cells.append(f'{e:+5.1f}/{l:+5.1f}')
        print(f'  {rn:6} ' + ' | '.join(cells))
