"""Partial envelopes over time for chosen notes (two-stage decay, beating), and the bass
spectra with the transverse series marked (phantom partials / longitudinal modes)."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana'))
import numpy as np, csv, os, pickle
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from common import *
from notes_analysis import partials, envelope, TE, NENV

CASES = [(33, 'ff'), (45, 'mf'), (60, 'mf'), (72, 'mf'), (84, 'mf'), (96, 'mf')]
fig, axs = plt.subplots(len(CASES), 2, figsize=(16, 4 * len(CASES)), sharey='row')
summary = []
for row, (key, dyn) in enumerate(CASES):
    for colx, src in enumerate(('c2', 'sal')):
        x, sr = note(src, dyn, key)
        m = x.mean(1)
        f0, B, found, _ = partials(m, key)
        X = np.fft.rfft(m); Fx = np.fft.rfftfreq(len(m), 1 / sr)
        f0n = kfreq(key)
        ax = axs[row, colx]
        cmap = plt.get_cmap('viridis')
        ns = [n for n in sorted(found)][:8]
        for i, n in enumerate(ns):
            half = min(0.4 * f0n, 12.0) if f0n < 200 else min(0.4 * f0n, 25.0)
            e = envelope(X, Fx, found[n][0], half)
            ax.plot(TE, e, color=cmap(i / max(1, len(ns) - 1)), lw=1, label=f'n={n} {found[n][0]:.0f} Hz')
            # two-stage: slope 0.05–0.6 s vs 1.5–3.4 s, and level drop until the knee
            mm1 = (TE > 0.05) & (TE < 0.6); mm2 = (TE > 1.5) & (TE < 3.4)
            s1 = np.polyfit(TE[mm1], e[mm1], 1)[0]; s2 = np.polyfit(TE[mm2], e[mm2], 1)[0]
            summary.append(dict(src=src, key=key, name=kname(key), dyn=dyn, n=n, f=round(found[n][0], 2),
                                prompt=round(s1, 1), after=round(s2, 1), ratio=round(s1 / s2, 2) if s2 < -0.1 else np.nan,
                                lvl0=round(e[(TE > 0.03) & (TE < 0.13)].mean(), 1), lvl3=round(e[(TE > 3.0) & (TE < 3.4)].mean(), 1)))
        ax.axvline(3.5, color='k', ls=':', lw=.8)
        ax.set_title(f'{kname(key)} {dyn} — {"cand2" if src == "c2" else "Salamander"}: partial envelopes (dB)')
        ax.set_xlim(0, 5); ax.grid(alpha=.3); ax.legend(fontsize=6, ncol=2)
        top = max(ax.get_ylim()[1], 0)
    axs[row, 0].set_ylim(-150, -20)
for a in axs[-1]:
    a.set_xlabel('s (key-up at 3.5 s)')
fig.tight_layout(); fig.savefig(os.path.join(PLOTS, 'note_partial_envelopes.png'), dpi=70); plt.close(fig)
with open(os.path.join(CSV, 'partial_two_stage.csv'), 'w', newline='') as fh:
    w = csv.DictWriter(fh, fieldnames=list(summary[0].keys())); w.writeheader(); w.writerows(summary)

# ---- bass: transverse series vs extra peaks (phantom partials, longitudinal modes) ----
cache = pickle.load(open(os.path.join(CSV, 'bass_spectra.pkl'), 'rb'))
keys = [21, 24, 27, 30, 33, 36]
fig, axs = plt.subplots(len(keys), 1, figsize=(17, 3.6 * len(keys)))
ext = []
for ax, key in zip(axs, keys):
    for src, k in (('sal', 4), ('c2', 3)):
        F, Mdb, f0, B, found = cache[(src, key)]
        ax.plot(F, Mdb + (0 if src == 'sal' else -60), color=COL[k], lw=.6, label=f'{SHORT[k]} (offset {0 if src == "sal" else -60} dB)')
        # peaks not on the transverse series
        pk, pr = signal.find_peaks(Mdb, height=-60, prominence=12)
        series_f = np.array([n * f0 * np.sqrt(1 + B * n * n) for n in range(1, 200)])
        tol = 0.06 * f0
        extra = [(F[i], Mdb[i]) for i in pk if F[i] > 0.8 * f0 and np.min(np.abs(series_f - F[i])) > tol]
        on = [(F[i], Mdb[i]) for i in pk if F[i] > 0.8 * f0 and np.min(np.abs(series_f - F[i])) <= tol]
        e_extra = 10 * np.log10(sum(10 ** (a / 10) for _, a in extra) + 1e-30)
        e_on = 10 * np.log10(sum(10 ** (a / 10) for _, a in on) + 1e-30)
        ext.append(dict(src=src, key=key, name=kname(key), n_series_peaks=len(on), n_extra_peaks=len(extra),
                        extra_vs_series_db=round(e_extra - e_on, 1),
                        extra_400_2000=len([1 for f, _ in extra if 400 <= f <= 2000]),
                        extra_list=' '.join(f'{f:.0f}' for f, a in sorted(extra, key=lambda t: -t[1])[:12])))
        for f, a in extra:
            ax.plot(f, a + (0 if src == 'sal' else -60), 'x', color=COL[k], ms=5)
    ax.set_xlim(0, 2500); ax.set_ylim(-140, 2); ax.grid(alpha=.3)
    ax.set_title(f'{kname(key)} ff, 0.05–1.55 s spectrum (dB re max); x = peak not on the fitted transverse series (phantom / longitudinal / other)')
    ax.legend(fontsize=8)
axs[-1].set_xlabel('Hz')
fig.tight_layout(); fig.savefig(os.path.join(PLOTS, 'bass_phantoms.png'), dpi=70); plt.close(fig)
with open(os.path.join(CSV, 'bass_extra_peaks.csv'), 'w', newline='') as fh:
    w = csv.DictWriter(fh, fieldnames=list(ext[0].keys())); w.writeheader(); w.writerows(ext)
for r in ext:
    print(r)
for r in summary:
    if r['n'] <= 4:
        print(r)
