"""Round 40, candidate 6: every key at mf (`keys_mf`: key k at (k-21)*4 s, held 3.5 s), the
second analysis's level (K-weighted energy over the first second from the onset, both
channels' power), candidate against the Salamander. Prints each key's deviation from a
quadratic trend through its three neighbours either side (s09's evenness, but on every key),
and with `fit <old levels csv> <new csv> [gain] [lo] [hi]` refits the per-key level table.

    python -I keylev.py <candidate keys_mf.wav> [fit old.csv new.csv gain lo hi]
"""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana2'))
import numpy as np
from common import load, kweight, onset, SR

SAL = _os.path.join(LAB, 'keys/wav/salamander/keys_mf.wav')
NAMES = ['C', 'C#', 'D', 'D#', 'E', 'F', 'F#', 'G', 'G#', 'A', 'A#', 'B']


def kname(k):
    return f'{NAMES[k % 12]}{k // 12 - 1}'


def levels(path):
    x = load(path)
    out = []
    for k in range(21, 109):
        s = x[int((k - 21) * 4 * SR): int((k - 20) * 4 * SR)]
        on = onset(s, -30)
        y = kweight(s[on:on + SR])
        out.append(-0.691 + 10 * np.log10(np.mean(y ** 2, 0).sum() + 1e-30))
    return np.array(out)


def dev(L):
    K = np.arange(21, 109)
    d = []
    for i in range(len(L)):
        idx = [j for j in range(max(0, i - 3), min(len(L), i + 4)) if j != i]
        p = np.polyfit(K[idx], L[idx], 2)
        d.append(L[i] - np.polyval(p, K[i]))
    return np.array(d)


c, s = levels(sys.argv[1]), levels(SAL)
dc, ds = dev(c), dev(s)
off = np.mean(c[:66]) - np.mean(s[:66])
print(f'evenness rms (A0..F#6 / G6..C8): cand {np.sqrt(np.mean(dc[:66]**2)):.2f} / {np.sqrt(np.mean(dc[66:]**2)):.2f}'
      f'   sal {np.sqrt(np.mean(ds[:66]**2)):.2f} / {np.sqrt(np.mean(ds[66:]**2)):.2f} dB;'
      f' rms of the difference {np.sqrt(np.mean((dc-ds)**2)):.2f}')
diff = dc - ds
worst = np.argsort(-np.abs(diff))[:12]
print('worst (cand dev / sal dev): ' + ', '.join(f'{kname(21+i)} {dc[i]:+.1f}/{ds[i]:+.1f}' for i in sorted(worst)))
reg = [(21, 47), (48, 71), (72, 83), (84, 95), (96, 108)]
print('register level, cand - sal (aligned on A0..F#6): ' + ', '.join(
    f'{kname(a)}-{kname(b)} {np.mean(c[a-21:b-20]) - np.mean(s[a-21:b-20]) - off:+.1f}' for a, b in reg))
for k in (84, 86, 87, 88, 89, 101, 102, 103, 105):
    i = k - 21
    print(f'  {kname(k)} cand {dc[i]:+.1f} sal {ds[i]:+.1f}', end='')
print()
if len(sys.argv) > 2 and sys.argv[2] == 'fit':
    old = {int(a): float(b) for a, b in (l.split(',') for l in open(sys.argv[3]) if l.strip())}
    gain = float(sys.argv[5]) if len(sys.argv) > 5 else 0.7
    lo = int(sys.argv[6]) if len(sys.argv) > 6 else 72
    hi = int(sys.argv[7]) if len(sys.argv) > 7 else 108
    with open(sys.argv[4], 'w') as fh:
        for k in range(21, 109):
            v = old.get(k, 0.0)
            if lo <= k <= hi:
                v = float(np.clip(v - gain * diff[k - 21], -12, 12))
            fh.write(f'{k},{v:.2f}\n')
    print('wrote', sys.argv[4])
