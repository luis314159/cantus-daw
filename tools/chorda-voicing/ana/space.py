"""Space: early reflections (high-passed energy vs time after the top phrase's staccato onsets),
inter-channel time difference of the direct sound per single note, the room tail after the
last release (decay rate, colour, correlation) and the noise floor of the reference."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana'))
import numpy as np, csv, os
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from common import *

# ---- 1. early reflections: top phrase staccato (C7 E7 G7 C8, 0.12 s) and fast_run ----
fig, axs = plt.subplots(2, 2, figsize=(15, 9))
rows = []
for col, (ph, ons) in enumerate((('top', [0.0, 0.3, 0.6, 0.9]), ('fast_run', [0.125 * i for i in range(29)]))):
    for k in (3, 4):
        x, sr = phrase(ph, k)
        b, a = signal.butter(4, 2000 / (sr / 2), 'high')
        h = signal.filtfilt(b, a, x, axis=0)
        lat = onset(x, sr, -40) / sr - ons[0]
        n = int(0.0005 * sr)
        nb = 400
        accM = np.zeros(nb); accS = np.zeros(nb)
        for o in ons:
            i = int((o + lat) * sr) - 10 * n
            seg = h[i:i + nb * n]
            if len(seg) < nb * n:
                continue
            M = (seg[:, 0] + seg[:, 1]) / 2; S = (seg[:, 0] - seg[:, 1]) / 2
            accM += (M ** 2).reshape(nb, n).mean(1); accS += (S ** 2).reshape(nb, n).mean(1)
        tt = (np.arange(nb) - 10) * 0.5
        ref = 10 * np.log10(accM.max())
        axs[0, col].plot(tt, 10 * np.log10(accM) - ref, color=COL[k], lw=1, label=f'{SHORT[k]} mid')
        axs[1, col].plot(tt, 10 * np.log10(accS) - ref, color=COL[k], lw=1, label=f'{SHORT[k]} side')
        e = accM + accS
        d = e[(tt >= 0) & (tt < 5)].sum(); er = e[(tt >= 5) & (tt < 80)].sum(); lt = e[(tt >= 80) & (tt < 195)].sum()
        rows.append(dict(phrase=ph, source=SHORT[k], early_5_80ms_vs_direct_db=round(10 * np.log10(er / d), 2),
                         late_80_195ms_vs_direct_db=round(10 * np.log10(lt / d), 2),
                         side_vs_mid_5_80ms_db=round(10 * np.log10(accS[(tt >= 5) & (tt < 80)].sum() / accM[(tt >= 5) & (tt < 80)].sum()), 2)))
    for r_ in (0, 1):
        axs[r_, col].set_xlim(-5, 195); axs[r_, col].set_ylim(-70, 3); axs[r_, col].grid(alpha=.3); axs[r_, col].legend()
        axs[r_, col].set_xlabel('ms after onset'); axs[r_, col].set_ylabel('dB re direct peak (>2 kHz)')
    axs[0, col].set_title(f'{ph}: energy >2 kHz after onsets, mid (avg of {len(ons)} onsets)')
    axs[1, col].set_title(f'{ph}: energy >2 kHz after onsets, side')
fig.tight_layout(); fig.savefig(os.path.join(PLOTS, 'space_early.png'), dpi=80); plt.close(fig)
with open(os.path.join(CSV, 'space_early.csv'), 'w', newline='') as fh:
    w = csv.DictWriter(fh, fieldnames=list(rows[0].keys())); w.writeheader(); w.writerows(rows)
for r in rows:
    print(r)

# ---- 2. inter-channel time/level difference of each single note's direct sound (mf) ----
itd = []
for src in ('sal', 'c2'):
    for key in KEYS:
        x, sr = note(src, 'mf', key)
        i0 = onset(x, sr, -50)
        seg = x[i0:i0 + int(0.03 * sr)]
        b, a = signal.butter(2, [300 / (sr / 2), 6000 / (sr / 2)], 'band')
        s = signal.filtfilt(b, a, seg, axis=0)
        c = signal.correlate(s[:, 0], s[:, 1], 'full')
        lags = np.arange(-len(s) + 1, len(s))
        m = np.abs(lags) <= int(0.0015 * sr)
        j = np.argmax(c[m])
        norm = np.sqrt((s[:, 0] ** 2).sum() * (s[:, 1] ** 2).sum())
        itd.append(dict(src=src, key=key, name=kname(key), lag_ms=round(lags[m][j] / sr * 1000, 3),
                        max_xcorr=round(c[m][j] / norm, 3),
                        ild_db=round(10 * np.log10((seg[:, 0] ** 2).sum() / (seg[:, 1] ** 2).sum()), 2)))
with open(os.path.join(CSV, 'space_itd.csv'), 'w', newline='') as fh:
    w = csv.DictWriter(fh, fieldnames=list(itd[0].keys())); w.writeheader(); w.writerows(itd)
fig, ax = plt.subplots(1, 3, figsize=(16, 4.5))
for src, k in (('sal', 4), ('c2', 3)):
    d = [r for r in itd if r['src'] == src]
    ax[0].plot([r['key'] for r in d], [r['lag_ms'] for r in d], 'o-', color=COL[k], label=SHORT[k])
    ax[1].plot([r['key'] for r in d], [r['ild_db'] for r in d], 'o-', color=COL[k], label=SHORT[k])
    ax[2].plot([r['key'] for r in d], [r['max_xcorr'] for r in d], 'o-', color=COL[k], label=SHORT[k])
for a_, t_ in zip(ax, ('L-R time difference of the attack (ms, + = R later)', 'L/R level difference, first 30 ms (dB, + = left)', 'peak L/R cross-correlation of the attack')):
    a_.set_title(t_); a_.grid(alpha=.3); a_.legend(); a_.set_xlabel('MIDI key (mf)')
fig.tight_layout(); fig.savefig(os.path.join(PLOTS, 'space_itd_ild.png'), dpi=80); plt.close(fig)

# ---- 3. noise floor of the reference: slot ends (5.6–5.95 s), spectrum ----
fig, ax = plt.subplots(figsize=(9, 5))
for src, k in (('sal', 4), ('c2', 3)):
    P = 0
    for key in KEYS:
        x, sr = note(src, 'pp', key)
        tail = x[int(5.6 * sr):int(5.95 * sr)]
        f, p = psd(tail.mean(1), sr, 8192)
        P = P + p / len(KEYS)
    lvl = 10 * np.log10(P.sum() * (f[1] - f[0]) + 1e-30)
    ax.semilogx(f[1:], smooth_oct(f[1:], db(P[1:]), 6), color=COL[k], label=f'{SHORT[k]} (total {lvl:.1f} dBFS raw)')
    print(src, 'pp slot-end floor total dBFS', round(lvl, 1))
ax.set_title('Residual at 2.1–2.45 s after key-up (pp single notes, mean PSD, raw level)')
ax.set_xlim(20, 20000); ax.grid(alpha=.3, which='both'); ax.legend(); ax.set_xlabel('Hz'); ax.set_ylabel('dB/Hz')
fig.tight_layout(); fig.savefig(os.path.join(PLOTS, 'noise_floor.png'), dpi=80); plt.close(fig)
