"""Plots and register summaries from csv/notes_features.csv and csv/notes_partials.csv."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana'))
import numpy as np, csv, os
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from common import *

S = {'sal': (4, 'Salamander'), 'c2': (3, 'cand2'), 'b14': (1, 'beta.14')}


def read(name):
    with open(os.path.join(CSV, name)) as fh:
        rows = list(csv.DictReader(fh))
    for r in rows:
        for k, v in r.items():
            try:
                r[k] = float(v)
            except ValueError:
                pass
    return rows


F = read('notes_features.csv')
P = read('notes_partials.csv')


def series(src, dyn, col):
    d = [r for r in F if r['src'] == src and r['dyn'] == dyn]
    return np.array([r['key'] for r in d]), np.array([r[col] for r in d])


def kticks(ax):
    ks = [21, 33, 45, 57, 69, 81, 93, 105]
    ax.set_xticks(ks); ax.set_xticklabels([kname(k) for k in ks])


# 1. inharmonicity and tuning
fig, ax = plt.subplots(1, 2, figsize=(15, 5))
for src in ('sal', 'c2', 'b14'):
    k_, nm = S[src]
    for dyn, mk in (('mf', 'o-'),):
        x, B = series(src, dyn, 'B'); _, c = series(src, dyn, 'cents_vs_et'); _, n = series(src, dyn, 'npart')
        ok = n >= 4
        ax[0].semilogy(x[ok], B[ok], mk, color=COL[k_], label=nm, lw=2 if src != 'b14' else .8)
        ax[1].plot(x, c, mk, color=COL[k_], label=nm, lw=2 if src != 'b14' else .8)
ax[0].set_title('Inharmonicity B per key (mf; fitted on partials, ≥4 partials)'); ax[1].set_title('Tuning of the fitted f0 vs equal temperament A440 (cents)')
for a in ax:
    a.grid(alpha=.3, which='both'); a.legend(); kticks(a)
fig.tight_layout(); fig.savefig(os.path.join(PLOTS, 'note_inharmonicity_tuning.png'), dpi=80); plt.close(fig)

# 2. velocity → loudness and brightness
fig, ax = plt.subplots(1, 3, figsize=(18, 5))
off = {}
for src in ('sal', 'c2'):
    _, m = series(src, 'mf', 'lufs1s'); off[src] = m.mean()
for src in ('sal', 'c2'):
    k_, nm = S[src]
    for dyn, ls in (('pp', ':'), ('mf', '-'), ('ff', '--')):
        x, l = series(src, dyn, 'lufs1s')
        ax[0].plot(x, l - off[src], ls, color=COL[k_], label=f'{nm} {dyn}')
        x, c = series(src, dyn, 'centroid_early')
        ax[2].semilogy(x, c, ls, color=COL[k_], label=f'{nm} {dyn}')
    x, lp = series(src, 'pp', 'lufs1s'); _, lf = series(src, 'ff', 'lufs1s'); _, lm = series(src, 'mf', 'lufs1s')
    ax[1].plot(x, lf - lp, 'o-', color=COL[k_], label=f'{nm} ff−pp')
    ax[1].plot(x, lm - lp, 's:', color=COL[k_], label=f'{nm} mf−pp')
ax[0].set_title('Loudness of the first second (LUFS, each source aligned on its mean mf)')
ax[1].set_title('Dynamic range per key (LU): ff−pp and mf−pp')
ax[2].set_title('Spectral centroid 0.06–0.31 s (Hz) per dynamic')
for a in ax:
    a.grid(alpha=.3, which='both'); a.legend(fontsize=7); kticks(a)
fig.tight_layout(); fig.savefig(os.path.join(PLOTS, 'note_velocity.png'), dpi=80); plt.close(fig)

# 3. tone colour: partial levels relative to the strongest, selected keys, mf and ff
sel = [21, 33, 45, 60, 72, 84, 96]
for dyn in ('mf', 'ff'):
    fig, axs = plt.subplots(2, 4, figsize=(18, 8))
    for ax_, key in zip(axs.flat, sel):
        for src in ('sal', 'c2'):
            k_, nm = S[src]
            d = [p for p in P if p['src'] == src and p['dyn'] == dyn and p['key'] == key]
            n = np.array([p['n'] for p in d]); l = np.array([p['lvl_db'] for p in d])
            if len(l):
                ax_.plot(n, l - l.max(), 'o-', ms=3, color=COL[k_], label=nm)
        ax_.set_title(f'{kname(key)} {dyn}: partial level (dB re strongest), 30–130 ms')
        ax_.set_ylim(-70, 3); ax_.grid(alpha=.3); ax_.legend(fontsize=8); ax_.set_xlabel('partial n')
    axs.flat[-1].axis('off')
    fig.tight_layout(); fig.savefig(os.path.join(PLOTS, f'note_partials_{dyn}.png'), dpi=80); plt.close(fig)

# 4. partial decay: prompt and late (dB/s) and beating roughness vs partial frequency
fig, axs = plt.subplots(3, 7, figsize=(24, 11))
for j, key in enumerate(sel):
    for src in ('sal', 'c2'):
        k_, nm = S[src]
        d = [p for p in P if p['src'] == src and p['dyn'] == 'mf' and p['key'] == key and p['lvl_db'] > -90]
        f = np.array([p['f'] for p in d])
        for i, col in enumerate(('prompt_dbs', 'late_dbs', 'beat_rms_db')):
            axs[i, j].plot(f, [p[col] for p in d], 'o', ms=3, color=COL[k_], label=nm)
    axs[0, j].set_title(f'{kname(key)} mf'); axs[0, j].set_ylim(-80, 5); axs[1, j].set_ylim(-40, 5); axs[2, j].set_ylim(0, 12)
    for i in range(3):
        axs[i, j].grid(alpha=.3); axs[i, j].set_xscale('log')
    axs[2, j].set_xlabel('partial Hz')
axs[0, 0].set_ylabel('prompt decay 0.05–0.5 s (dB/s)'); axs[1, 0].set_ylabel('late decay 1.5–3.4 s (dB/s)')
axs[2, 0].set_ylabel('envelope roughness (beats) dB rms'); axs[0, 0].legend()
fig.tight_layout(); fig.savefig(os.path.join(PLOTS, 'note_partial_decays.png'), dpi=70); plt.close(fig)

# 5. band decays (early/late) per key, octave bands
fig, axs = plt.subplots(2, 4, figsize=(20, 8))
for i, c in enumerate((125, 250, 500, 1000, 2000, 4000, 8000)):
    a = axs.flat[i]
    for src in ('sal', 'c2'):
        k_, nm = S[src]
        x, e = series(src, 'mf', f'dec_early_{c}'); _, l = series(src, 'mf', f'dec_late_{c}')
        a.plot(x, e, 'o-', ms=3, color=COL[k_], label=f'{nm} 0.1–0.6 s')
        a.plot(x, l, 's--', ms=3, color=COL[k_], alpha=.7, label=f'{nm} 1.5–3.4 s')
    a.set_title(f'{c} Hz octave: decay rate (dB/s), mf'); a.grid(alpha=.3); kticks(a); a.set_ylim(-80, 5)
axs.flat[0].legend(fontsize=7); axs.flat[-1].axis('off')
fig.tight_layout(); fig.savefig(os.path.join(PLOTS, 'note_band_decays.png'), dpi=80); plt.close(fig)

# 6. release, attack, stereo per key
fig, axs = plt.subplots(2, 3, figsize=(19, 9))
for src in ('sal', 'c2'):
    k_, nm = S[src]
    for d_, ls in ((100, '-'), (250, '--'), (500, ':')):
        x, v = series(src, 'mf', f'rel_{d_}ms_db'); axs[0, 0].plot(x, v, ls, color=COL[k_], label=f'{nm} +{d_} ms')
    x, v = series(src, 'mf', 'rel_noise_hf_db'); axs[0, 1].plot(x, v, 'o-', color=COL[k_], label=f'{nm} 1–8 kHz')
    x, v = series(src, 'mf', 'rel_noise_lf_db'); axs[0, 1].plot(x, v, 's--', color=COL[k_], label=f'{nm} 20–200 Hz')
    x, v = series(src, 'mf', 'rise_ms'); axs[0, 2].semilogy(x, np.maximum(v, 0.5), 'o-', color=COL[k_], label=nm)
    for dyn, ls in (('pp', ':'), ('ff', '-')):
        x, a_ = series(src, dyn, 'centroid_att'); _, e_ = series(src, dyn, 'centroid_early')
        axs[1, 0].plot(x, a_ / e_, ls, color=COL[k_], label=f'{nm} {dyn}')
    x, v = series(src, 'mf', 'balance_db'); axs[1, 1].plot(x, v, 'o-', color=COL[k_], label=f'{nm} L/R balance dB')
    x, v = series(src, 'mf', 'corr'); axs[1, 2].plot(x, v, 'o-', color=COL[k_], label=f'{nm} L/R correlation')
axs[0, 0].set_title('After key-up: level vs just before (dB), mf'); axs[0, 1].set_title('Release noise: band energy 0–150 ms after key-up vs 200 ms before (dB)')
axs[0, 2].set_title('Rise time 10→90 % (ms), mf'); axs[1, 0].set_title('Attack brightness: centroid 0–50 ms / centroid 60–310 ms')
axs[1, 1].set_title('L/R level balance of each held note (dB, + = left), mf'); axs[1, 2].set_title('L/R correlation of each held note, mf')
for a in axs.flat:
    a.grid(alpha=.3, which='both'); a.legend(fontsize=7); kticks(a)
fig.tight_layout(); fig.savefig(os.path.join(PLOTS, 'note_release_attack_stereo.png'), dpi=80); plt.close(fig)

# 7. register summary table (csv): mean differences c2 - sal by register and dynamic
REG = {'bass A0-B2': (21, 47), 'middle C3-B5': (48, 83), 'treble C6-C8': (84, 108)}
cols = ['lufs1s', 'centroid_att', 'centroid_early', 'centroid_late', 'thump_db', 'rise_ms', 'B', 'cents_vs_et',
        'dec_early_250', 'dec_early_1000', 'dec_early_4000', 'dec_late_250', 'dec_late_1000', 'dec_late_4000',
        'rel_100ms_db', 'rel_250ms_db', 'rel_500ms_db', 'rel_1000ms_db', 'rel_noise_hf_db', 'rel_noise_lf_db',
        'corr', 'balance_db', 'side_db'] + [f'{w}_{c}' for w in ('att', 'early', 'late') for c in (63, 125, 250, 500, 1000, 2000, 4000, 8000, 16000)]
out = []
for dyn in DYN:
    for rn, (a, b) in REG.items():
        row = dict(dyn=dyn, register=rn)
        for c in cols:
            for src in ('sal', 'c2'):
                v = np.array([r[c] for r in F if r['src'] == src and r['dyn'] == dyn and a <= r['key'] <= b])
                v = v[np.isfinite(v)]
                row[f'{src}_{c}'] = round(np.median(v), 3) if len(v) else np.nan
        out.append(row)
with open(os.path.join(CSV, 'notes_register_summary.csv'), 'w', newline='') as fh:
    w = csv.DictWriter(fh, fieldnames=list(out[0].keys())); w.writeheader(); w.writerows(out)
for r in out:
    print(r['dyn'], r['register'])
    for c in cols:
        print(f'   {c:18s} sal {r["sal_" + c]:>9}  c2 {r["c2_" + c]:>9}')
