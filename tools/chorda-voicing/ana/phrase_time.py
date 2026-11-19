"""Phrase time-domain measures: envelopes, tails after the last release (and abrupt cuts),
spatial build-up after onsets (side/mid vs time), quietest-window noise floor, HF clicks,
raw levels between phrases (the velocity curve as heard)."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana'))
import numpy as np, csv, os
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from common import *

BAR = 60 / 72 * 4
RELEASE = {'middle_c': 3.0, 'bass': 14.5, 'ff_chord': 5.0, 'soft_chords': 9.2,
           'ballad': 4 * BAR + 1.0, 'top': 4.4 + 15 * 0.09 + 0.085, 'fast_run': 28 * 0.125 + 0.115}


def env_db(x, sr, win=0.01):
    n = int(win * sr)
    e = (x ** 2).sum(1)
    k = len(e) // n
    return np.arange(k) * win + win / 2, 10 * np.log10(e[:k * n].reshape(k, n).mean(1) + 1e-30)


rows = []
fig, axs = plt.subplots(4, 2, figsize=(14, 15))
for pi, ph in enumerate(PHRASES):
    ax = axs.flat[pi]
    for k in (1, 3, 4):
        x, sr = phrase(ph, k)
        t, e = env_db(x, sr)
        ax.plot(t, e, color=COL[k], lw=1.0, label=SHORT[k])
        tr = RELEASE[ph]
        ref = e[(t > tr - 0.25) & (t < tr - 0.05)].mean()
        r = dict(phrase=ph, source=SHORT[k], release_s=round(tr, 3), level_before_db=round(ref, 2))
        for d in (0.1, 0.25, 0.5, 1.0, 1.5, 2.0):
            mm = (t > tr + d - 0.03) & (t < tr + d + 0.03)
            r[f'after_{int(d * 1000)}ms'] = round(e[mm].mean() - ref, 2) if mm.any() else np.nan
        # time for the tail to fall 20 / 40 dB below the pre-release level
        tt = t[t > tr]
        et = e[t > tr]
        for drop in (20, 40):
            i = np.argmax(et < ref - drop)
            r[f't_minus{drop}_s'] = round(tt[i] - tr, 3) if (et < ref - drop).any() else np.nan
        # abrupt cut: steepest 10-ms fall in the tail while above -100 dBFS
        de = np.diff(et)
        ok = et[1:] > -110
        r['steepest_fall_db_per_10ms'] = round(de[ok].min(), 1) if ok.any() else np.nan
        r['t_steepest_s'] = round(tt[1:][ok][np.argmin(de[ok])], 2) if ok.any() else np.nan
        # quietest 100-ms window: level and where
        t1, e1 = env_db(x, sr, 0.1)
        q = np.argmin(e1[:-1])
        r['quietest_dbfs'] = round(e1[q], 1)
        r['quietest_at_s'] = round(t1[q], 2)
        # HF clicks: samples whose >8 kHz high-pass exceeds 8x the local (50 ms) RMS
        b, a = signal.butter(4, 8000 / (sr / 2), 'high')
        h = signal.lfilter(b, a, x.mean(1))
        lr = np.sqrt(np.convolve(h ** 2, np.ones(2400) / 2400, 'same')) + 1e-9
        r['hf_spikes'] = int(np.sum((np.abs(h) > 8 * lr) & (np.abs(h) > 1e-4)))
        rows.append(r)
    ax.axvline(RELEASE[ph], color='k', ls=':', lw=1)
    ax.set_title(f'{ph}: level envelope (10 ms, dBFS at equal LUFS); dotted = last key/pedal up')
    ax.set_ylim(-130, 0); ax.grid(alpha=.3); ax.legend(); ax.set_xlabel('s')
axs.flat[7].axis('off')
fig.tight_layout(); fig.savefig(os.path.join(PLOTS, 'phr_envelopes.png'), dpi=80); plt.close(fig)

with open(os.path.join(CSV, 'phrase_tails.csv'), 'w', newline='') as fh:
    w = csv.DictWriter(fh, fieldnames=list(rows[0].keys())); w.writeheader(); w.writerows(rows)
for r in rows:
    print(r)

# ---- spatial build-up after onsets: side/mid energy vs time (fast_run 16ths, top staccato) ----
ONS = {'fast_run': [0.125 * i for i in range(29)], 'top': [0.0, 0.3, 0.6, 0.9],
       'bass': [0, 3, 6, 9, 12], 'soft_chords': [0, 2.2, 4.4, 6.6]}
fig, axs = plt.subplots(1, 4, figsize=(18, 4.8))
sp = []
for ax, (ph, ons) in zip(axs, ONS.items()):
    for k in (1, 3, 4):
        x, sr = phrase(ph, k)
        off = onset(x, sr, -40) / sr - ons[0]  # source latency
        M = (x[:, 0] + x[:, 1]) / 2
        S = (x[:, 0] - x[:, 1]) / 2
        n = int(0.002 * sr)
        acc_m = np.zeros(75); acc_s = np.zeros(75)
        for o in ons:
            i = int((o + off) * sr) - 5 * n
            if i < 0:
                i = 0
            mm = M[i:i + 75 * n]; ss = S[i:i + 75 * n]
            if len(mm) < 75 * n:
                continue
            acc_m += (mm ** 2).reshape(75, n).mean(1); acc_s += (ss ** 2).reshape(75, n).mean(1)
        tt = (np.arange(75) - 5) * 2
        ratio = 10 * np.log10(acc_s / acc_m)
        ax.plot(tt, ratio, color=COL[k], label=SHORT[k])
        sp.append(dict(phrase=ph, source=SHORT[k], side_mid_0_6ms=round(ratio[5:8].mean(), 2),
                       side_mid_10_40ms=round(ratio[10:25].mean(), 2), side_mid_60_140ms=round(ratio[35:75].mean(), 2)))
    ax.set_title(f'{ph}: side/mid (dB) vs ms after onset'); ax.grid(alpha=.3); ax.legend(); ax.set_xlabel('ms')
fig.tight_layout(); fig.savefig(os.path.join(PLOTS, 'phr_side_buildup.png'), dpi=80); plt.close(fig)
with open(os.path.join(CSV, 'phrase_side_buildup.csv'), 'w', newline='') as fh:
    w = csv.DictWriter(fh, fieldnames=list(sp[0].keys())); w.writeheader(); w.writerows(sp)
for r in sp:
    print(r)

# ---- raw levels: each phrase's LUFS relative to the source's own triad (C-E-G at velocity 100) ----
lv = []
for k in (1, 3, 4):
    tri = lufs(raw_phrase('triad', k)[0])
    for ph in PHRASES:
        x, sr = raw_phrase(ph, k)
        lv.append(dict(phrase=ph, source=SHORT[k], lufs_raw=round(lufs(x), 2), rel_triad=round(lufs(x) - tri, 2),
                       peak_rel_triad=round(20 * np.log10(np.abs(x).max()) - tri, 2)))
    lv.append(dict(phrase='triad', source=SHORT[k], lufs_raw=round(tri, 2), rel_triad=0.0, peak_rel_triad=np.nan))
with open(os.path.join(CSV, 'phrase_raw_levels.csv'), 'w', newline='') as fh:
    w = csv.DictWriter(fh, fieldnames=list(lv[0].keys())); w.writeheader(); w.writerows(lv)
for r in lv:
    print(r)
