"""Attack waveforms (first 80 ms, mid channel, each normalised to its own peak) and the
>2 kHz energy envelope of the attack, for a bass, a middle and a treble note at mf and ff."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana'))
import numpy as np, os
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from common import *
cases = [(33, 'ff'), (60, 'pp'), (60, 'mf'), (60, 'ff'), (96, 'mf'), (96, 'ff')]
fig, axs = plt.subplots(len(cases), 2, figsize=(16, 3 * len(cases)))
for r, (key, dyn) in enumerate(cases):
    for src, k in (('sal', 4), ('c2', 3)):
        x, sr = note(src, dyn, key); m = x.mean(1)
        i0 = onset(x, sr, -50); seg = m[max(0, i0 - int(0.002 * sr)):i0 + int(0.08 * sr)]
        t = (np.arange(len(seg)) / sr - 0.002) * 1000
        axs[r, 0].plot(t, seg / np.abs(seg).max() + (0 if src == 'sal' else -2.2), color=COL[k], lw=.7, label=SHORT[k])
        b, a = signal.butter(4, 2000 / (sr / 2), 'high'); h = signal.lfilter(b, a, m)[max(0, i0 - int(0.002 * sr)):i0 + int(0.08 * sr)]
        n = int(0.0005 * sr); e = 10 * np.log10((h[:len(h) // n * n] ** 2).reshape(-1, n).mean(1) + 1e-30)
        e0 = 10 * np.log10((seg[:len(seg) // n * n] ** 2).reshape(-1, n).mean(1).max())
        axs[r, 1].plot(np.arange(len(e)) * 0.5 - 2, e - e0, color=COL[k], label=SHORT[k] + ' >2 kHz (dB re broadband peak)')
    axs[r, 0].set_title(f'{kname(key)} {dyn}: attack waveform (mid, each normalised; cand2 offset −2.2)'); axs[r, 0].legend(fontsize=7)
    axs[r, 1].set_title(f'{kname(key)} {dyn}: energy above 2 kHz, 0.5-ms bins'); axs[r, 1].set_ylim(-70, 5); axs[r, 1].legend(fontsize=7)
    for a_ in axs[r]: a_.grid(alpha=.3); a_.set_xlabel('ms')
fig.tight_layout(); fig.savefig(os.path.join(PLOTS, 'note_attack_waveforms.png'), dpi=70); plt.close(fig)
print('ok')
