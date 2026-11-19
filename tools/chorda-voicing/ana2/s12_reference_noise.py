"""What in the reference is the recording rather than the piano: low-frequency rumble/hum
under notes whose fundamental is far above it, the broadband floor between partials, and
the sample lead-in before each onset."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana2'))
import numpy as np, csv, os
import matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt
from common import *

fig, ax = plt.subplots(1, 2, figsize=(14, 4.5))
print('Energy under 0.7*f0 (no string partial there) during 0.5–3.0 s, re the whole note, mf:')
acc = {}
for src in ['c5', 'sal']:
    out = []
    for key in [60, 66, 72, 78, 84, 90, 96]:
        x = note(src, 'mf', key)
        on = onset(x, -30)
        seg = x[on + int(0.5 * SR):on + int(3.0 * SR)]
        f, P = signal.welch(seg, SR, nperseg=1 << 15, axis=0)
        P = P.sum(1)
        lf = P[(f > 15) & (f < 0.7 * kfreq(key))].sum() / P[f > 15].sum()
        out.append((kname(key), round(db(lf), 1)))
        if key >= 72:
            acc[src] = acc.get(src, 0) + P
    print(f'  {NAME[src]:10s}', out)
for src in ['c5', 'sal']:
    P = acc[src] / 5
    sm = (f > 10) & (f < 1000)
    ax[0].semilogx(f[sm], db(P[sm]), color=COL[src], label=NAME[src])
for h in [50, 100, 150, 60, 120, 180]:
    ax[0].axvline(h, color='g' if h % 50 == 0 else 'm', lw=.5, alpha=.6)
ax[0].set_title('Below the notes: mean spectrum of C5–C7 at mf, 0.5–3 s\n(green 50-Hz series, magenta 60-Hz series)')
ax[0].set_xlabel('Hz'); ax[0].set_ylabel('PSD dB'); ax[0].grid(alpha=.3, which='both'); ax[0].legend()
# hum line check: compare line bins with neighbours in the Salamander
P = acc['sal'] / 5
for h in [50, 60, 100, 120, 150, 180]:
    i = np.argmin(np.abs(f - h)); nb = (np.abs(f - h) > 3) & (np.abs(f - h) < 15)
    print(f'  Salamander line at {h} Hz: {db(P[i]) - db(np.median(P[nb])):+.1f} dB over its neighbours')

# lead-in before the onset: Salamander samples start with a quiet stretch
leads = {'c5': [], 'sal': []}
for src in ['c5', 'sal']:
    for key in KEYS:
        x = note(src, 'mf', key)
        a = np.abs(x).max(1)
        pk = a[:SR].max()
        i30 = np.argmax(a > pk * 10 ** (-30 / 20))
        leads[src].append(i30 / SR * 1000)
ax[1].plot(KEYS, leads['c5'], 'o-', color=COL['c5'], label=NAME['c5'])
ax[1].plot(KEYS, leads['sal'], 'o-', color=COL['sal'], label=NAME['sal'])
ax[1].set_title('Delay from note-on to the sound reaching −30 dB of its peak (ms), mf')
ax[1].set_xlabel('MIDI key'); ax[1].grid(alpha=.3); ax[1].legend()
fig.tight_layout(); fig.savefig(os.path.join(FIG, 'reference_artefacts.png'), dpi=100); plt.close(fig)
print('lead ms c5 median %.1f, sal median %.1f, sal range %.1f..%.1f' % (np.median(leads['c5']), np.median(leads['sal']), min(leads['sal']), max(leads['sal'])))
