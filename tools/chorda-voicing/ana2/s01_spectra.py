"""Long-term spectra (1/3 and 1/12 octave) and spectrograms per phrase, loudness-matched
previews. Writes csv/ltas_third.csv, figures/ltas_<phrase>.png, figures/spec_<phrase>.png."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana2'))
import numpy as np, csv, os
import matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt
from common import *

fc3 = frac_oct_bands(3, 25, 20000)
fc12 = frac_oct_bands(12, 25, 20000)
rows = []
summary = {}
for ph in PHRASES:
    S = {}
    for s in ['b14', 'c5', 'sal']:
        x = phrase(ph, s)
        f, P = ltas(x)
        S[s] = (band_sum(f, P, fc3, 3), band_sum(f, P, fc12, 12))
    fig, ax = plt.subplots(2, 1, figsize=(10, 7), sharex=True)
    for s in ['b14', 'c5', 'sal']:
        ax[0].semilogx(fc12, db(S[s][1]), color=COL[s], label=NAME[s], lw=1.2)
    ax[0].set_ylabel('1/12-oct power (dB)')
    ax[0].legend(); ax[0].grid(alpha=.3, which='both')
    ax[0].set_title(f'{ph}: long-term spectrum (both at -20 LUFS)')
    d3 = db(S['c5'][0]) - db(S['sal'][0])
    d12 = db(S['c5'][1]) - db(S['sal'][1])
    ax[1].semilogx(fc12, d12, color='#888', lw=.8, label='1/12 oct')
    ax[1].semilogx(fc3, d3, color=COL['c5'], lw=2, marker='o', ms=3, label='1/3 oct')
    ax[1].axhline(0, color='k', lw=.6)
    ax[1].set_ylim(-25, 25); ax[1].set_ylabel('Chorda c5 − Salamander (dB)')
    ax[1].set_xlabel('Hz'); ax[1].legend(); ax[1].grid(alpha=.3, which='both')
    ax[1].set_xlim(25, 20000)
    fig.tight_layout(); fig.savefig(os.path.join(FIG, f'ltas_{ph}.png'), dpi=110); plt.close(fig)
    for f0, a, b in zip(fc3, db(S['c5'][0]), db(S['sal'][0])):
        rows.append([ph, round(f0, 1), round(a, 2), round(b, 2), round(a - b, 2)])
    summary[ph] = d3

with open(os.path.join(CSV, 'ltas_third.csv'), 'w', newline='') as fh:
    w = csv.writer(fh); w.writerow(['phrase', 'fc', 'c5_db', 'sal_db', 'diff_db']); w.writerows(rows)

# octave-ish summary printout
groups = [(25, 80), (80, 160), (160, 315), (315, 630), (630, 1250), (1250, 2500), (2500, 5000), (5000, 10000), (10000, 20000)]
print('phrase      ' + ' '.join(f'{a}-{b}'.rjust(10) for a, b in groups))
for ph in PHRASES:
    out = []
    for a, b in groups:
        m = (fc3 >= a) & (fc3 < b)
        # power-average difference over the group
        x = phrase(ph, 'c5'); y = phrase(ph, 'sal')
        out.append(np.mean(summary[ph][m]))
    print(f'{ph:12s}' + ' '.join(f'{v:10.1f}' for v in out))

# heat map of differences
fig, ax = plt.subplots(figsize=(11, 3.6))
M = np.array([summary[p] for p in PHRASES])
im = ax.pcolormesh(np.arange(len(fc3) + 1), np.arange(len(PHRASES) + 1), M, cmap='RdBu_r', vmin=-15, vmax=15)
ax.set_yticks(np.arange(len(PHRASES)) + .5); ax.set_yticklabels(PHRASES)
tk = [i for i, f in enumerate(fc3) if any(abs(f / v - 1) < .06 for v in [31.5, 63, 125, 250, 500, 1000, 2000, 4000, 8000, 16000])]
ax.set_xticks(np.array(tk) + .5); ax.set_xticklabels([f'{fc3[i]:.0f}' for i in tk])
plt.colorbar(im, label='Chorda c5 − Salamander, dB (1/3 oct, at equal LUFS)')
ax.set_title('Long-term spectrum difference per phrase (red = Chorda has more)')
fig.tight_layout(); fig.savefig(os.path.join(FIG, 'ltas_heatmap.png'), dpi=110); plt.close(fig)

# spectrograms
for ph in PHRASES:
    fig, ax = plt.subplots(2, 1, figsize=(11, 7), sharex=True, sharey=True)
    for i, s in enumerate(['c5', 'sal']):
        x = phrase(ph, s).mean(1)
        f, t, Z = signal.spectrogram(x, SR, nperseg=2048, noverlap=2048 - 256, window='hann')
        Zd = db(Z)
        ax[i].pcolormesh(t, f, Zd, vmin=Zd.max() - 100, vmax=Zd.max(), shading='auto', cmap='magma')
        ax[i].set_yscale('symlog', linthresh=200); ax[i].set_ylim(20, 24000)
        ax[i].set_title(f'{ph}: {NAME[s]}'); ax[i].set_ylabel('Hz')
    ax[1].set_xlabel('s')
    fig.tight_layout(); fig.savefig(os.path.join(FIG, f'spec_{ph}.png'), dpi=100); plt.close(fig)
print('done')
