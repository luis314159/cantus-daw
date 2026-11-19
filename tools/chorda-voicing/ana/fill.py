"""Spectral fill between partials (what lies between the lines: soundboard/room/sympathetic
resonance/noise) and the bass's 500-1500 Hz partial prominence (longitudinal-mode region)."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana'))
import numpy as np, csv, os
from common import *
def floor_db(seg, sr, lo=100, hi=3000):
    w = np.hanning(len(seg)); S = np.abs(np.fft.rfft(seg * w)) ** 2
    f = np.fft.rfftfreq(len(seg), 1 / sr); m = (f >= lo) & (f < hi)
    d = 10 * np.log10(S[m] + 1e-30)
    return np.percentile(d, 20) - np.percentile(d, 99.9), np.percentile(d, 50) - np.percentile(d, 99.9)
rows = []
WIN = {'ff_chord': [(0.5, 1.5), (2.5, 3.5)], 'soft_chords': [(0.6, 1.6), (5.0, 6.0)], 'ballad': [(4.0, 5.0), (11.0, 12.0)],
       'middle_c': [(0.5, 1.5), (1.8, 2.8)], 'bass': [(12.5, 13.5)]}
for ph, ws in WIN.items():
    for k in (1, 3, 4):
        x, sr = phrase(ph, k)
        for a, b in ws:
            p20, p50 = floor_db(x[int(a * sr):int(b * sr)].mean(1), sr)
            rows.append(dict(what=ph, window=f'{a}-{b}s', source=SHORT[k], floor20_re_peak_db=round(p20, 1), median_re_peak_db=round(p50, 1)))
for src, k in (('sal', 4), ('c2', 3)):
    for reg, keys in (('single notes A1-C3 mf', range(33, 49, 3)), ('single notes C4-C5 mf', range(60, 73, 3))):
        v = []
        for key in keys:
            x, sr = note(src, 'mf', key)
            v.append(floor_db(x[int(0.5 * sr):int(1.5 * sr)].mean(1), sr))
        v = np.array(v)
        rows.append(dict(what=reg, window='0.5-1.5s', source=SHORT[k], floor20_re_peak_db=round(v[:, 0].mean(), 1), median_re_peak_db=round(v[:, 1].mean(), 1)))
with open(os.path.join(CSV, 'spectral_fill.csv'), 'w', newline='') as fh:
    w = csv.DictWriter(fh, fieldnames=list(rows[0].keys())); w.writeheader(); w.writerows(rows)
for r in rows: print(r)
# bass prominence 450-1500 Hz: max partial level there minus median partial level 150-3000 Hz (30-130 ms)
P = list(csv.DictReader(open(os.path.join(CSV, 'notes_partials.csv'))))
print('bass ff: strongest partial in 450-1500 Hz re median partial 150-3000 Hz (dB), and its frequency; early 30-130 ms')
for key in (21, 24, 27, 30, 33, 36, 39):
    s = []
    for src in ('sal', 'c2'):
        d = [p for p in P if p['src'] == src and p['dyn'] == 'ff' and int(p['key']) == key]
        f = np.array([float(p['f']) for p in d]); l = np.array([float(p['lvl_db']) for p in d])
        band = (f >= 450) & (f <= 1500); ref = (f >= 150) & (f <= 3000)
        i = np.argmax(np.where(band, l, -999))
        s.append(f'{src}: {l[i] - np.median(l[ref]):+.1f} dB @ {f[i]:.0f} Hz (n={f[i] / kfreq(key):.0f})')
    print(kname(key), ' | '.join(s))
