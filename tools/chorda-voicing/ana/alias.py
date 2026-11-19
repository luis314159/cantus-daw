"""Top keys at ff: every spectral peak above -70 dB re max between 5 and 24 kHz, labelled as on the
partial series, a possible alias (48 kHz - partial), or other. Also DC per source over all notes."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana'))
import numpy as np
from common import *
from notes_analysis import partials
for key in (99, 102, 105, 108):
    for src in ('sal', 'c2'):
        x, sr = note(src, 'ff', key); m = x.mean(1)
        f0, B, found, _ = partials(m, key)
        seg = m[int(0.01 * sr):int(0.51 * sr)]
        S = 20 * np.log10(np.abs(np.fft.rfft(seg * np.hanning(len(seg)))) + 1e-20); S -= S.max()
        f = np.fft.rfftfreq(len(seg), 1 / sr)
        pk, _ = signal.find_peaks(S, height=-70, prominence=15)
        ser = np.array([n * f0 * np.sqrt(1 + B * n * n) for n in range(1, 30)])
        out = []
        for i in pk:
            if f[i] < 5000: continue
            lab = 'P' if np.min(np.abs(ser - f[i])) < 0.03 * f0 else ('A' if np.min(np.abs((48000 - ser) - f[i])) < 0.03 * f0 else '?')
            out.append(f'{f[i]/1000:.1f}k{lab}{S[i]:.0f}')
        print(kname(key), src, f'f0={f0:.0f} B={B:.4f}', ' '.join(out[:14]))
for src in ('sal', 'c2'):
    for dyn in DYN:
        x, sr = load(f"{NOTES[src]}/notes_{dyn}.wav")
        print(src, dyn, 'DC (mean) dBFS', round(20 * np.log10(abs(x.mean()) + 1e-15), 1), 'peak dBFS', round(20 * np.log10(np.abs(x).max()), 1))
