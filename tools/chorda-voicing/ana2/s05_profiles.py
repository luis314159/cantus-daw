"""Partial-level profiles (timbre) per key at mf, and per-partial envelope images (beating
pattern) for selected keys."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana2'))
import numpy as np, csv, os
import matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt
from common import *
from partials import find_partials, hetero_env

dyn = 'mf'
sel = [21, 27, 33, 36, 42, 48, 54, 60, 66, 72, 78, 84]
fig, axs = plt.subplots(3, 4, figsize=(16, 10)); axs = axs.ravel()
prof = {}
for ai, key in enumerate(sel):
    for src in ['c5', 'sal']:
        x = note(src, dyn, key)
        on = onset(x, -30)
        m = x[on:].mean(1)
        f0, B, pl = find_partials(m, key, 0.25, 1.25)
        n = [p[0] for p in pl]; lv = [p[2] for p in pl]
        prof[(src, key)] = dict(zip(n, lv))
        axs[ai].plot(n, lv, 'o-', ms=3, color=COL[src], label=NAME[src])
    axs[ai].set_title(f'{kname(key)} {dyn}: partial levels 0.25–1.25 s'); axs[ai].set_xlim(0, 40); axs[ai].set_ylim(-80, 2)
    axs[ai].grid(alpha=.3)
axs[0].legend(); axs[8].set_xlabel('partial number'); axs[0].set_ylabel('dB re strongest')
fig.tight_layout(); fig.savefig(os.path.join(FIG, 'partial_profiles_mf.png'), dpi=100); plt.close(fig)

# summary: energy share of partial groups
print('key   share(%) p1 | p2-4 | p5-10 | p11+   (c5 / sal)')
for key in sel:
    line = f'{kname(key):4s}'
    for grp in [(1, 1), (2, 4), (5, 10), (11, 99)]:
        v = []
        for src in ['c5', 'sal']:
            d = prof[(src, key)]
            tot = sum(10 ** (l / 10) for l in d.values())
            g = sum(10 ** (l / 10) for n, l in d.items() if grp[0] <= n <= grp[1])
            v.append(100 * g / tot)
        line += f' | {v[0]:5.1f}/{v[1]:<5.1f}'
    print(line)

# envelope images: partial x time, detrended dB
for key in [33, 45, 60, 72]:
    fig, axs = plt.subplots(1, 2, figsize=(14, 5.5), sharey=True)
    for i, src in enumerate(['c5', 'sal']):
        x = note(src, dyn, key)
        on = onset(x, -30)
        m = x[on:on + int(3.4 * SR)].mean(1)
        f0, B, pl = find_partials(m, key, 0.25, 1.25)
        img = np.full((24, 340), np.nan)
        for n, fn, lv in pl:
            if n > 24:
                continue
            t, ev = hetero_env(m, fn, min(0.3 * f0, 25), hop=0.01)
            ev = ev[:340]
            tt = t[:340]
            mk = tt > 0.1
            tr = np.polyval(np.polyfit(tt[mk], ev[mk], 2), tt)
            img[n - 1, :len(ev)] = ev - tr
        im = axs[i].imshow(img, aspect='auto', origin='lower', extent=[0, 3.4, 0.5, 24.5], cmap='RdBu_r', vmin=-15, vmax=15)
        axs[i].set_title(f'{kname(key)} mf, {NAME[src]}: each partial\'s envelope minus its trend')
        axs[i].set_xlabel('s')
    axs[0].set_ylabel('partial number')
    plt.colorbar(im, ax=axs, label='dB')
    fig.savefig(os.path.join(FIG, f'beats_{kname(key).replace("#", "s")}.png'), dpi=100); plt.close(fig)
print('done')
