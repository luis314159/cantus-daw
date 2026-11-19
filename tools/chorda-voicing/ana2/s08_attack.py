"""Attack shape: waveforms of the first 60 ms, the attack's spectrum against the sustain's,
and the knock/thump under the treble at each dynamic."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana2'))
import numpy as np, csv, os
import matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt
from common import *

sel = [21, 36, 60, 72, 84, 96]
fig, axs = plt.subplots(len(sel), 2, figsize=(14, 13))
for i, key in enumerate(sel):
    for j, src in enumerate(['c5', 'sal']):
        x = note(src, 'mf', key)
        on = onset(x, -50)
        xp = np.vstack([np.zeros((int(0.005 * SR), 2)), x]); y = xp[on:on + int(0.065 * SR)]
        t = (np.arange(len(y)) / SR - 0.005) * 1000
        y = y / np.abs(y).max()
        axs[i, j].plot(t, y[:, 0], lw=.6, color=COL[src])
        axs[i, j].plot(t, y[:, 1] - 2.2, lw=.6, color=COL[src], alpha=.6)
        axs[i, j].set_title(f'{kname(key)} mf, {NAME[src]} (top L, bottom R; normalised)', fontsize=9)
        axs[i, j].set_yticks([])
axs[-1, 0].set_xlabel('ms'); axs[-1, 1].set_xlabel('ms')
fig.tight_layout(); fig.savefig(os.path.join(FIG, 'attack_waveforms_mf.png'), dpi=90); plt.close(fig)

# attack spectrum (0-30 ms) minus sustain spectrum (0.3-0.6 s), 1/3 octave, per register
fc3 = frac_oct_bands(3, 40, 20000)
fig, axs = plt.subplots(2, 3, figsize=(15, 7.5)); axs = axs.ravel()
for i, key in enumerate(sel):
    for src in ['c5', 'sal']:
        x = note(src, 'mf', key)
        on = onset(x, -50)
        a = x[on:on + int(0.03 * SR)]
        s = x[on + int(0.3 * SR):on + int(0.6 * SR)]
        def spec(seg):
            f, P = signal.periodogram(seg * signal.windows.hann(len(seg))[:, None], SR, axis=0)
            return band_sum(f, P.sum(1), fc3, 3) / len(seg)
        Sa, Ss = spec(a), spec(s)
        tot = Ss.sum()
        axs[i].semilogx(fc3, db(Sa / tot), color=COL[src], lw=2, label=f'{NAME[src]} attack 0–30 ms')
        axs[i].semilogx(fc3, db(Ss / tot), color=COL[src], lw=1, ls='--', label=f'{NAME[src]} 0.3–0.6 s')
    axs[i].set_title(f'{kname(key)} mf: attack vs sustain spectrum (dB re sustain total)', fontsize=9)
    axs[i].set_ylim(-80, 20); axs[i].grid(alpha=.3, which='both')
axs[0].legend(fontsize=7)
fig.tight_layout(); fig.savefig(os.path.join(FIG, 'attack_spectra_mf.png'), dpi=95); plt.close(fig)

# treble knock vs tone at each dynamic: energy below 0.6*f0 vs at/above in the first 80 ms
print('key dyn | knock-to-tone ratio dB first 80 ms (c5 / sal) | tone level dB re mf C4 tone')
rows = []
for dyn in ['pp', 'mf', 'ff']:
    for key in [78, 84, 87, 90, 93, 96, 99, 102, 105, 108]:
        out = [kname(key), dyn]
        for src in ['c5', 'sal']:
            x = note(src, dyn, key)
            on = onset(x, -40)
            seg = x[on:on + int(0.08 * SR)]
            f, P = signal.periodogram(seg * signal.windows.hann(len(seg))[:, None], SR, axis=0)
            P = P.sum(1)
            f0 = kfreq(key)
            knock = P[(f > 20) & (f < 0.6 * f0)].sum()
            tone = P[(f >= 0.9 * f0) & (f < 20000)].sum()
            out.append(round(db(knock / tone), 1))
        rows.append(out)
        print(out)
with open(os.path.join(CSV, 'treble_knock.csv'), 'w', newline='') as fh:
    w = csv.writer(fh); w.writerow(['key', 'dyn', 'c5_knock_to_tone_db', 'sal_knock_to_tone_db']); w.writerows(rows)
