"""Zooms on the suspicious spots s14 found: bass C1 3.0-3.6 s envelope (periodic drops), the
>8 kHz spikes in middle_c (3.637 s) and ff_chord (5.3-5.75 s) with their absolute level."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana2'))
import numpy as np, os
import matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt
from common import *
fig, ax = plt.subplots(3, 1, figsize=(12, 10))
for s in ['c5', 'sal']:
    x = phrase('bass', s)
    t, e = short_term_rms(x, 0.005, 0.001)
    ax[0].plot(t, e, color=COL[s], label=NAME[s])
ax[0].set_xlim(2.95, 3.8); ax[0].set_ylim(-60, 0); ax[0].legend(); ax[0].grid(alpha=.3)
ax[0].set_title('bass phrase, C1 onset (3.0 s): 5-ms level, dBFS')
sos = signal.butter(4, 8000, 'high', fs=SR, output='sos')
for k, (ph, a, b) in enumerate([('middle_c', 3.55, 3.75), ('ff_chord', 5.2, 5.9)]):
    for s in ['c5', 'sal']:
        x = phrase(ph, s).mean(1)
        h = signal.sosfiltfilt(sos, x)
        t = np.arange(len(x)) / SR
        m = (t > a) & (t < b)
        ax[k + 1].plot(t[m], db20(h[m]), color=COL[s], lw=.5, label=NAME[s] + ' >8 kHz |x| dBFS')
        print(ph, s, 'max >8k level in window %.1f dBFS; whole-band 20ms rms there %.1f dBFS' % (db20(np.abs(h[m]).max()), db(np.mean(x[m]**2))))
    ax[k + 1].set_ylim(-160, -40); ax[k + 1].grid(alpha=.3); ax[k + 1].legend(); ax[k + 1].set_title(f'{ph}: >8 kHz content, {a}-{b} s')
fig.tight_layout(); fig.savefig(os.path.join(FIG, 'artefact_zooms.png'), dpi=95); plt.close(fig)
x = phrase('bass', 'c5'); t, e = short_term_rms(x, 0.005, 0.001)
m = (t > 3.05) & (t < 4.5); r = e[m] - np.polyval(np.polyfit(t[m], e[m], 2), t[m])
F = np.abs(np.fft.rfft(r * np.hanning(len(r)), 1 << 14)); fr = np.fft.rfftfreq(1 << 14, 0.001)
sel = (fr > 2) & (fr < 40); print('C1 envelope modulation peak at %.2f Hz, depth p3-p97 %.1f dB' % (fr[sel][np.argmax(F[sel])], np.percentile(r, 97) - np.percentile(r, 3)))
x = phrase('bass', 'sal'); t, e = short_term_rms(x, 0.005, 0.001)
m = (t > 3.05) & (t < 4.5); r = e[m] - np.polyval(np.polyfit(t[m], e[m], 2), t[m])
F = np.abs(np.fft.rfft(r * np.hanning(len(r)), 1 << 14))
print('Salamander C1 envelope modulation peak at %.2f Hz, depth p3-p97 %.1f dB' % (fr[sel][np.argmax(F[sel])], np.percentile(r, 97) - np.percentile(r, 3)))
