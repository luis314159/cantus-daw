"""Pedal-down resonance and chord blending: the energy that does not belong to any played
note's partials, with the sustain pedal down (soft_chords chord 1, ff_chord, ballad bar 1),
and a zoomed spectrum with the played partials marked. Also intermodulation probe: energy at
the difference tones of the ff chord's strongest partial pairs."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana2'))
import numpy as np, csv, os
import matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt
from common import *

R = list(csv.DictReader(open(os.path.join(CSV, 'notes.csv'))))
Bk = {}
for src in ['c5', 'sal']:
    ks = [int(r['key']) for r in R if r['src'] == src and r['dyn'] == 'mf']
    bs = [float(r['B']) for r in R if r['src'] == src and r['dyn'] == 'mf']
    kk = [k for k, b in zip(ks, bs) if b > 0]; bb = [np.log(b) for b in bs if b > 0]
    Bk[src] = lambda k, kk=kk, bb=bb: float(np.exp(np.interp(k, kk, bb)))

CASES = [('soft_chords', 0.5, 2.0, [48, 55, 59, 64, 67]),
         ('ff_chord', 0.8, 3.5, [24, 36, 43, 48, 52, 55, 60, 64, 67, 72]),
         ('ballad', 1.0, 3.2, [36, 43, 52, 55, 76, 74])]
fig, axs = plt.subplots(len(CASES), 1, figsize=(14, 11))
for ci, (ph, a, b, keys) in enumerate(CASES):
    for src in ['c5', 'sal']:
        x = phrase(ph, src)
        seg = x[int(a * SR):int(b * SR)].mean(1)
        n = len(seg); N = 1 << int(np.ceil(np.log2(n * 2)))
        F = np.abs(np.fft.rfft(seg * np.hanning(n), N)) ** 2
        f = np.fft.rfftfreq(N, 1 / SR)
        mask = np.zeros_like(f, bool)
        bw = 2.5 * SR / n + 1.0
        for k in keys:
            f0 = kfreq(k); B = Bk[src](k)
            for p in range(1, int(16000 / f0) + 1):
                fp = p * f0 * np.sqrt(1 + B * p * p)
                mask |= np.abs(f - fp) < max(bw, fp * 0.004)  # allow tuning slack 7 cents
        band = (f > 40) & (f < 8000)
        nt = F[band & ~mask].sum() / F[band].sum()
        # by octave
        octs = []
        for lo in [62.5, 125, 250, 500, 1000, 2000, 4000]:
            bb = (f >= lo) & (f < 2 * lo)
            octs.append(round(db(F[bb & ~mask].sum() / F[bb].sum()), 1))
        print(f'{ph:12s} {NAME[src]:10s} non-played energy share 40 Hz–8 kHz: {db(nt):.1f} dB; per octave from 62.5 Hz: {octs}')
        sm = (f > 60) & (f < 2500)
        axs[ci].plot(f[sm], db(F[sm] / F[band].max()), lw=.6, color=COL[src], label=NAME[src], alpha=.85)
    for k in keys:
        f0 = kfreq(k)
        for p in range(1, 40):
            if p * f0 < 2500:
                axs[ci].axvline(p * f0, color='k', lw=.3, alpha=.25)
    axs[ci].set_xscale('log'); axs[ci].set_ylim(-110, 5); axs[ci].grid(alpha=.3)
    axs[ci].set_title(f'{ph} {a}–{b} s, pedal down: spectrum (grey lines = played notes\' harmonics)')
axs[0].legend()
fig.tight_layout(); fig.savefig(os.path.join(FIG, 'pedal_spectra.png'), dpi=95); plt.close(fig)
