"""Short-term loudness contour of each phrase (K-weighted 50-ms level), both pianos at
-20 LUFS: the dynamic shape a listener follows. Also middle_c's decay in detail."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana2'))
import numpy as np, os
import matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt
from common import *

fig, axs = plt.subplots(4, 2, figsize=(15, 13)); axs = axs.ravel()
for i, ph in enumerate(PHRASES):
    for s in ['c5', 'sal']:
        x = kweight(phrase(ph, s))
        t, e = short_term_rms(x, 0.05, 0.01)
        axs[i].plot(t, e, color=COL[s], label=NAME[s])
    axs[i].set_title(f'{ph}: K-weighted 50-ms level (dBFS), both at −20 LUFS'); axs[i].grid(alpha=.3)
    axs[i].set_ylim(-80, -5)
axs[0].legend()
# middle_c, partial-free broadband envelope stats
for s in ['c5', 'sal']:
    x = kweight(phrase('middle_c', s))
    t, e = short_term_rms(x, 0.05, 0.01)
    pk = e.max()
    v = {tt: round(e[np.searchsorted(t, tt)] - pk, 1) for tt in [0.1, 0.25, 0.5, 1.0, 2.0, 2.9]}
    print('middle_c', NAME[s], 'level re peak at', v)
for ph in ['ballad', 'soft_chords', 'ff_chord']:
    for s in ['c5', 'sal']:
        x = kweight(phrase(ph, s))
        t, e = short_term_rms(x, 0.05, 0.01)
        act = e > e.max() - 40
        print(ph, NAME[s], 'loudness range p10-p95 of sounding part: %.1f dB' % (np.percentile(e[act], 95) - np.percentile(e[act], 10)))
axs[7].axis('off')
fig.tight_layout(); fig.savefig(os.path.join(FIG, 'phrase_contours.png'), dpi=90); plt.close(fig)
