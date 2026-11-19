"""Phrase-level measures: stereo per band, tails after the last release (per band), noise
floor and hum, clicks, energy between partials with the pedal down. Loudness-matched
previews. Writes csv/phr_*.csv and figures."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana2'))
import numpy as np, csv, os
import matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt
from common import *

OCT = [63, 125, 250, 500, 1000, 2000, 4000, 8000, 16000]
# last key-up / pedal-up per phrase (s), from the score
END = {'middle_c': 3.0, 'soft_chords': 9.2, 'fast_run': 3.74, 'ballad': 4 * 10 / 3 + 1.0,
       'ff_chord': 5.0, 'bass': 14.5, 'top': 5.88}

# ---- stereo per octave band ----
out = []
fig, axs = plt.subplots(2, 4, figsize=(15, 6.5)); axs = axs.ravel()
for pi, ph in enumerate(PHRASES):
    for s in ['c5', 'sal']:
        x = phrase(ph, s)[:int(END[ph] * SR)]
        cs, sd = [], []
        for fc in OCT:
            y = bandpass(x, fc / np.sqrt(2), fc * np.sqrt(2))
            L, R = y[:, 0], y[:, 1]
            c = np.sum(L * R) / np.sqrt(np.sum(L ** 2) * np.sum(R ** 2))
            M, S = (L + R) / 2, (L - R) / 2
            side = db(np.mean(S ** 2) / np.mean(M ** 2))
            ild = db(np.mean(L ** 2) / np.mean(R ** 2))
            cs.append(c); sd.append(side)
            out.append([ph, s, fc, round(c, 3), round(side, 1), round(ild, 1)])
        axs[pi].semilogx(OCT, cs, 'o-', color=COL[s], label=NAME[s])
    axs[pi].set_title(ph); axs[pi].set_ylim(-1, 1); axs[pi].grid(alpha=.3); axs[pi].axhline(0, color='k', lw=.5)
axs[0].legend(); axs[0].set_ylabel('L/R correlation per octave')
axs[7].axis('off')
fig.suptitle('Inter-channel correlation per octave band (sounding part of each phrase)')
fig.tight_layout(); fig.savefig(os.path.join(FIG, 'stereo_corr_bands.png'), dpi=100); plt.close(fig)
with open(os.path.join(CSV, 'phr_stereo.csv'), 'w', newline='') as fh:
    w = csv.writer(fh); w.writerow(['phrase', 'src', 'oct', 'corr', 'side_db', 'ild_db']); w.writerows(out)
print('stereo done')

# ---- tails after the last release, per band ----
out = []
fig, axs = plt.subplots(2, 4, figsize=(15, 6.5)); axs = axs.ravel()
for pi, ph in enumerate(PHRASES):
    for s in ['c5', 'sal']:
        x = phrase(ph, s)
        t, e = short_term_rms(x, 0.02, 0.005)
        k0 = np.searchsorted(t, END[ph] - 0.1)
        ref = e[np.searchsorted(t, END[ph] - 0.15):np.searchsorted(t, END[ph] - 0.02)].mean()
        ee = e - ref
        axs[pi].plot(t[k0:] - END[ph], ee[k0:], color=COL[s], label=NAME[s])
        tt = t[k0:] - END[ph]
        def tdrop(v):
            idx = np.where(ee[k0:] < v)[0]
            return round(tt[idx[0]], 3) if len(idx) else np.nan
        floor = e[-int(0.3 / 0.005):].mean()
        row = [ph, s, tdrop(-10), tdrop(-20), tdrop(-40), tdrop(-60), round(floor, 1)]
        for fc in [125, 500, 2000, 8000]:
            y = bandpass(x, fc / 2, fc * 2)
            tb, eb = short_term_rms(y, 0.02, 0.005)
            rb = eb[np.searchsorted(tb, END[ph] - 0.15):np.searchsorted(tb, END[ph] - 0.02)].mean()
            ib = np.where((tb > END[ph]) & (eb - rb < -30))[0]
            row.append(round(tb[ib[0]] - END[ph], 3) if len(ib) else np.nan)
        out.append(row)
    axs[pi].set_title(f'{ph} (0 = last key/pedal up)'); axs[pi].set_ylim(-100, 10); axs[pi].grid(alpha=.3)
axs[0].legend(); axs[0].set_ylabel('dB re level before release')
axs[7].axis('off')
fig.tight_layout(); fig.savefig(os.path.join(FIG, 'tails.png'), dpi=100); plt.close(fig)
with open(os.path.join(CSV, 'phr_tails.csv'), 'w', newline='') as fh:
    w = csv.writer(fh); w.writerow(['phrase', 'src', 't_-10', 't_-20', 't_-40', 't_-60', 'floor_dbfs', 't30_125', 't30_500', 't30_2k', 't30_8k']); w.writerows(out)
for r in out:
    print(r)

# ---- noise floor / hum: the Salamander's quietest 0.5 s stretches ----
for s in ['c5', 'sal']:
    x = raw('bass', s) if s == 'c5' else raw('bass', 'sal')
for s in ['sal', 'c5']:
    segs = []
    for ph in PHRASES:
        x = raw(ph, s)
        tail = x[-int(0.6 * SR):]
        segs.append((ph, db(np.mean(tail ** 2) + 1e-30)))
    print('raw tail level (last 0.6 s) dBFS', s, [(p, round(v, 1)) for p, v in segs])

# spectrum of the Salamander's raw note-file gaps (5.3..5.95 s after each pp onset)
x = load(os.path.join(NOTES['sal'], 'notes_pp.wav'))
acc = None
for i in range(len(KEYS)):
    seg = x[int((i * 6 + 5.3) * SR):int((i * 6 + 5.95) * SR)].mean(1)
    f, P = signal.welch(seg, SR, nperseg=16384)
    acc = P if acc is None else acc + P
acc /= len(KEYS)
xc = load(os.path.join(NOTES['c5'], 'notes_pp.wav'))
acc2 = None
for i in range(len(KEYS)):
    seg = xc[int((i * 6 + 5.3) * SR):int((i * 6 + 5.95) * SR)].mean(1)
    f, P = signal.welch(seg, SR, nperseg=16384)
    acc2 = P if acc2 is None else acc2 + P
acc2 /= len(KEYS)
fig, ax = plt.subplots(figsize=(10, 4))
ax.semilogx(f, db(acc), color=COL['sal'], label='Salamander, pp note files 5.3–5.95 s after each onset')
ax.semilogx(f, db(acc2), color=COL['c5'], label='Chorda c5, same windows')
for h in [50, 60]:
    for k in range(1, 6):
        ax.axvline(h * k, color='g' if h == 50 else 'm', lw=.4, alpha=.5)
ax.set_xlim(20, 24000); ax.set_ylim(-220, -80); ax.legend(); ax.grid(alpha=.3)
ax.set_title('What is left between notes (raw renders): recording noise floor; green=50 Hz series, magenta=60 Hz')
ax.set_xlabel('Hz'); ax.set_ylabel('PSD dB')
fig.tight_layout(); fig.savefig(os.path.join(FIG, 'noise_floor.png'), dpi=110); plt.close(fig)
# hum line strength
for h in [50, 60, 100, 120, 150, 180]:
    i = np.argmin(np.abs(f - h)); nb = (np.abs(f - h) > 5) & (np.abs(f - h) < 25)
    print(f'line {h} Hz: {db(acc[i]) - db(np.median(acc[nb])):.1f} dB over neighbours')
print('noise floor total dBFS (sal, c5):', round(db(acc.sum() * (f[1] - f[0])), 1), round(db(acc2.sum() * (f[1] - f[0]) + 1e-30), 1))
