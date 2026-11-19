"""Per phrase: long-term spectra (1/3 and 1/12 octave), difference curves, tilt, crest factor,
peak-to-loudness, loudness range, stereo correlation/width per band, mid/side spectra."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana'))
import numpy as np, csv, os
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from common import *

rows, bandrows, strows = [], [], []
fig, axs = plt.subplots(4, 2, figsize=(14, 16))
fig2, axs2 = plt.subplots(4, 2, figsize=(14, 16))
fig3, axs3 = plt.subplots(4, 2, figsize=(14, 16))
for pi, ph in enumerate(PHRASES):
    ax, ax2, ax3 = axs.flat[pi], axs2.flat[pi], axs3.flat[pi]
    ref = None
    spec = {}
    for k in (1, 2, 3, 4):
        x, sr = phrase(ph, k)
        m = x.mean(1)
        s = (x[:, 0] - x[:, 1]) / 2
        f, P = psd(np.stack([x[:, 0], x[:, 1], m, s], 1), sr)
        _, Pxy = signal.csd(x[:, 0], x[:, 1], sr, nperseg=16384, noverlap=12288)
        b = bands(f, P)  # thirds x 4
        bxy = []
        for c in THIRDS:
            lo, hi = c * 2 ** (-1 / 6), c * 2 ** (1 / 6)
            mm = (f >= lo) & (f < hi)
            bxy.append(Pxy[mm].real.sum() / np.sqrt(P[mm, 0].sum() * P[mm, 1].sum() + 1e-30))
        bxy = np.array(bxy)
        spec[k] = (f, P, b, bxy)
        pk = 20 * np.log10(np.abs(x).max())
        rms = 10 * np.log10((x ** 2).mean())
        L = lufs(x, sr)
        t, st = short_term(x, sr, win=3.0, hop=0.1)
        stv = st[st > st.max() - 40]
        lra = np.percentile(stv, 95) - np.percentile(stv, 10) if len(stv) > 5 else np.nan
        tl = tilt(THIRDS, b[:, 2])
        corr = np.sum(x[:, 0] * x[:, 1]) / np.sqrt(np.sum(x[:, 0] ** 2) * np.sum(x[:, 1] ** 2))
        sm = 10 * np.log10(np.sum(s ** 2) / np.sum(m ** 2))
        bal = 10 * np.log10(np.sum(x[:, 0] ** 2) / np.sum(x[:, 1] ** 2))
        dc = 20 * np.log10(np.abs(x.mean(0)).max() + 1e-12)
        # spectral centroid of the whole phrase (mid)
        pm = P[:, 2]
        cen = np.sum(f * pm) / np.sum(pm)
        rows.append(dict(phrase=ph, source=SHORT[k], lufs=round(L, 2), peak_dbfs=round(pk, 2),
                         crest_db=round(pk - rms, 2), plr_db=round(pk - L, 2), lra_lu=round(lra, 2),
                         tilt_db_oct=round(tl, 2), centroid_hz=round(cen), corr=round(corr, 3),
                         side_minus_mid_db=round(sm, 2), lr_balance_db=round(bal, 2), dc_dbfs=round(dc, 1)))
        for i, c in enumerate(THIRDS):
            bandrows.append(dict(phrase=ph, source=SHORT[k], band_hz=round(c, 1), mid_db=round(b[i, 2], 2),
                                 side_db=round(b[i, 3], 2), corr=round(bxy[i], 3)))
        sdb = smooth_oct(f[1:], db(pm[1:]), 12)
        if k in (1, 3, 4):
            ax.semilogx(f[1:], sdb, color=COL[k], lw=1.2 if k != 1 else 0.8, label=SHORT[k])
            ax3.semilogx(THIRDS, bxy, 'o-', ms=3, color=COL[k], label=SHORT[k] + ' corr')
    for k in (1, 2, 3):
        d = spec[k][2][:, 2] - spec[4][2][:, 2]
        ax2.semilogx(THIRDS, d, 'o-', ms=3, color=COL[k], lw=2 if k == 3 else 1, label=f'{SHORT[k]} - Salamander')
    ax.set_xlim(25, 20000); ax.set_ylim(spec[4][2][:, 2].max() - 90 - 20, None)
    ax.set_title(f'{ph}: long-term spectrum (mid, 1/12-oct, all at equal LUFS)')
    ax.set_xlabel('Hz'); ax.set_ylabel('dB'); ax.grid(True, which='both', alpha=.3); ax.legend()
    ax2.axhline(0, color='k', lw=.8); ax2.set_xlim(25, 20000); ax2.set_ylim(-25, 25)
    ax2.set_title(f'{ph}: 1/3-octave difference vs Salamander (dB)'); ax2.grid(True, which='both', alpha=.3); ax2.legend(fontsize=8)
    ax3.set_xlim(25, 20000); ax3.set_ylim(-0.3, 1.05); ax3.grid(True, which='both', alpha=.3)
    ax3.set_title(f'{ph}: L/R correlation per 1/3 octave (1 = mono, 0 = wide)'); ax3.legend(fontsize=8)
for a in (axs.flat[7], axs2.flat[7], axs3.flat[7]):
    a.axis('off')
for fg, n in ((fig, 'ltas'), (fig2, 'ltas_diff'), (fig3, 'stereo_corr')):
    fg.tight_layout(); fg.savefig(os.path.join(PLOTS, f'phr_{n}.png'), dpi=80)


def wcsv(name, rows):
    with open(os.path.join(CSV, name), 'w', newline='') as fh:
        w = csv.DictWriter(fh, fieldnames=list(rows[0].keys()))
        w.writeheader(); w.writerows(rows)


wcsv('phrase_summary.csv', rows)
wcsv('phrase_bands.csv', bandrows)
for r in rows:
    print(r)
