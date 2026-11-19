"""The release in detail: what is heard in the 300 ms after key-up, per band, relative to the
note's total level just before key-up; spectrograms of three releases."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana2'))
import numpy as np, csv, os
import matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt
from common import *

BANDS = [(20, 200), (200, 1000), (1000, 4000), (4000, 16000)]
rows = []
print('key | burst over the pre-release total, per band, max of 0–120 ms after key-up minus the band\'s own level before (dB)  c5 / sal')
for key in KEYS[:24]:
    out = [kname(key)]
    for src in ['c5', 'sal']:
        x = note(src, 'mf', key)
        on = onset(x, -30)
        ku = on + int(HELD * SR)
        seg = x[ku - int(0.3 * SR):ku + int(0.4 * SR)]
        tot_pre = np.mean(seg[int(0.15 * SR):int(0.29 * SR)] ** 2) * 2
        v = []
        for lo, hi in BANDS:
            y = bandpass(seg, lo, hi)
            t, e = short_term_rms(y, 0.01, 0.002)
            pre = e[(t > 0.15) & (t < 0.29)].mean()
            post = e[(t > 0.3) & (t < 0.42)].max()
            v.append(round(post - pre, 1))
        out.append(v)
    rows.append(out)
    print(out)

fig, axs = plt.subplots(2, 3, figsize=(15, 7), sharey=True)
for j, key in enumerate([48, 60, 72]):
    for i, src in enumerate(['c5', 'sal']):
        x = note(src, 'mf', key)
        on = onset(x, -30)
        ku = on + int(HELD * SR)
        seg = x[ku - int(0.3 * SR):ku + int(1.2 * SR)].mean(1)
        f, t, Z = signal.spectrogram(seg, SR, nperseg=1024, noverlap=1024 - 128)
        Zd = db(Z); ref = Zd.max()
        axs[i, j].pcolormesh(t - 0.3, f, Zd, vmin=ref - 110, vmax=ref, shading='auto', cmap='magma')
        axs[i, j].set_yscale('symlog', linthresh=200); axs[i, j].set_ylim(20, 20000)
        axs[i, j].axvline(0, color='c', lw=.6)
        axs[i, j].set_title(f'{kname(key)} mf release, {NAME[src]} (0 = key up)', fontsize=9)
axs[1, 1].set_xlabel('s')
fig.tight_layout(); fig.savefig(os.path.join(FIG, 'release_spectrograms.png'), dpi=95); plt.close(fig)
