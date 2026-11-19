"""Side-by-side spectrograms per phrase (beta.14, candidate 2, Salamander), equal loudness, and
attack zooms (first 400 ms) for the phrases with a clear first onset."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana'))
import numpy as np, os
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from common import *


def spec(m, sr, nper, hop):
    f, t, S = signal.stft(m, sr, window='hann', nperseg=nper, noverlap=nper - hop, boundary=None, padded=False)
    return f, t, 20 * np.log10(np.abs(S) + 1e-12)


for ph in PHRASES:
    fig, axs = plt.subplots(3, 1, figsize=(13, 12), sharex=True)
    Ss = {}
    for k in (1, 3, 4):
        x, sr = phrase(ph, k)
        Ss[k] = spec(x.mean(1), sr, 4096, 256)
    vmax = max(S.max() for _, _, S in Ss.values())
    for ax, k in zip(axs, (1, 3, 4)):
        f, t, S = Ss[k]
        ax.pcolormesh(t, f[1:], S[1:], vmin=vmax - 100, vmax=vmax, cmap='magma', shading='auto')
        ax.set_yscale('log'); ax.set_ylim(25, 22000)
        ax.set_title(f'{ph} — {SHORT[k]}  (mid, equal LUFS, 100 dB range)'); ax.set_ylabel('Hz')
    axs[-1].set_xlabel('s')
    fig.tight_layout(); fig.savefig(os.path.join(PLOTS, f'spg_{ph}.png'), dpi=75); plt.close(fig)

# attack zooms
for ph, t1 in (('middle_c', 0.5), ('ff_chord', 0.5), ('bass', 0.5), ('top', 1.2)):
    fig, axs = plt.subplots(1, 3, figsize=(16, 6), sharey=True)
    Ss = {}
    for k in (1, 3, 4):
        x, sr = phrase(ph, k)
        i0 = max(0, onset(x, sr, -40) - int(0.01 * sr))
        Ss[k] = spec(x[i0:i0 + int(t1 * sr)].mean(1), sr, 512, 32)
    vmax = max(S.max() for _, _, S in Ss.values())
    for ax, k in zip(axs, (1, 3, 4)):
        f, t, S = Ss[k]
        ax.pcolormesh(t * 1000, f[1:], S[1:], vmin=vmax - 90, vmax=vmax, cmap='magma', shading='auto')
        ax.set_yscale('log'); ax.set_ylim(40, 22000); ax.set_xlabel('ms from onset-10 ms')
        ax.set_title(f'{ph} attack — {SHORT[k]}')
    axs[0].set_ylabel('Hz')
    fig.tight_layout(); fig.savefig(os.path.join(PLOTS, f'spg_attack_{ph}.png'), dpi=75); plt.close(fig)
print('ok')
