"""Artefact hunt: abrupt level steps (voice stops/gates), clicks (isolated HF spikes), digital
silence, energy above 16 kHz and aliasing-like inharmonic lines in the top phrase."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana2'))
import numpy as np, os
import matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt
from common import *

print('--- abrupt drops: 5-ms level falling >= 10 dB within 20 ms while above -100 dBFS ---')
for ph in PHRASES:
    for s in ['c5', 'sal']:
        x = phrase(ph, s)
        t, e = short_term_rms(x, 0.005, 0.001)
        ev = []
        for i in range(20, len(e)):
            if e[i - 20] > -100 and e[i - 20] - e[i] >= 10 and (not ev or t[i] - ev[-1][0] > 0.1):
                ev.append((round(t[i], 3), round(e[i - 20], 1), round(e[i], 1)))
        ds = np.where(np.abs(x).max(1) == 0)[0]
        first_zero = round(ds[0] / SR, 3) if len(ds) and ds[-1] == len(x) - 1 else None
        print(f'{ph:12s} {NAME[s]:10s} drops: {ev[:6]}  digital silence from: {first_zero}')

print('--- clicks: samples of the >8 kHz band exceeding 12x its 20-ms rms (outside note onsets) ---')
for ph in PHRASES:
    for s in ['c5', 'sal']:
        x = phrase(ph, s).mean(1)
        sos = signal.butter(4, 8000, 'high', fs=SR, output='sos')
        h = signal.sosfiltfilt(sos, x)
        w = int(0.02 * SR)
        rms = np.sqrt(np.convolve(h ** 2, np.ones(w) / w, 'same')) + 1e-9
        idx = np.where(np.abs(h) > 12 * rms)[0]
        groups = []
        for i in idx:
            if not groups or i - groups[-1] > int(0.05 * SR):
                groups.append(i)
        print(f'{ph:12s} {NAME[s]:10s} {len(groups)} spike(s) at', [round(g / SR, 3) for g in groups[:8]])

print('--- energy above 16 kHz re total, and top phrase lines not on any played harmonic ---')
for ph in PHRASES:
    o = []
    for s in ['c5', 'sal']:
        f, P = ltas(phrase(ph, s))
        o.append(round(db(P[f > 16000].sum() / P.sum()), 1))
    print(f'{ph:12s} >16 kHz share: c5 {o[0]} dB, sal {o[1]} dB')

# middle_c tail zoom
fig, ax = plt.subplots(figsize=(10, 4))
for s in ['c5', 'sal']:
    t, e = short_term_rms(phrase('middle_c', s), 0.005, 0.001)
    ax.plot(t, e, color=COL[s], label=NAME[s])
ax.set_xlim(2.8, 5); ax.set_ylim(-125, -10); ax.axvline(3.0, color='k', lw=.5)
ax.set_title('middle_c after key-up (3.0 s): 5-ms level, dBFS (−120 = digital silence)')
ax.grid(alpha=.3); ax.legend(); ax.set_xlabel('s')
fig.tight_layout(); fig.savefig(os.path.join(FIG, 'middle_c_tail.png'), dpi=100); plt.close(fig)
