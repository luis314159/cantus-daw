"""Attack thump and release thump in absolute terms: band energy relative to the note's own
broadband energy in the same window (attack) or just before key-up (release)."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana'))
import numpy as np, csv, os
from common import *
rows = []
def bandE(seg, sr, lo, hi):
    f, p = signal.welch(seg, sr, nperseg=min(4096, len(seg)))
    return 10 * np.log10(p[(f >= lo) & (f < hi)].sum() + 1e-30), 10 * np.log10(p.sum() + 1e-30)
for src in ('sal', 'c2'):
    for dyn in DYN:
        for key in KEYS:
            x, sr = note(src, dyn, key); m = x.mean(1)
            i0 = onset(x, sr, -50)
            f0 = kfreq(key)
            lo_hi = (20, min(150, 0.7 * f0)) if f0 > 60 else (None, None)
            r = dict(src=src, dyn=dyn, key=key, name=kname(key))
            a = m[i0:i0 + int(0.05 * sr)]
            if lo_hi[0]:
                b, t = bandE(a, sr, *lo_hi); r['attack_lf_re_total_db'] = round(b - t, 1)
                # how long the sub-fundamental energy lasts: level 20-150 Hz at 0.3-0.5 s re attack
                b2, _ = bandE(m[i0 + int(0.3 * sr):i0 + int(0.5 * sr)], sr, *lo_hi)
                r['lf_0.3s_re_attack_lf_db'] = round(b2 - b, 1)
            else:
                r['attack_lf_re_total_db'] = np.nan; r['lf_0.3s_re_attack_lf_db'] = np.nan
            bh, t = bandE(a, sr, 2000, 12000); r['attack_hf_re_total_db'] = round(bh - t, 1)
            ku = int(3.5 * sr)
            _, pre = bandE(m[ku - int(0.2 * sr):ku], sr, 20, 24000)
            post = m[ku:ku + int(0.15 * sr)]
            bl, _ = bandE(post, sr, 20, 200); bh2, _ = bandE(post, sr, 1000, 8000)
            r['release_lf_re_note_db'] = round(bl - pre, 1); r['release_hf_re_note_db'] = round(bh2 - pre, 1)
            # peak of the 5-ms envelope in 0-60 ms after key-up re the 5-ms level just before key-up
            n = int(0.005 * sr)
            e = 10 * np.log10((m[ku - 8 * n:ku + 12 * n] ** 2).reshape(20, n).mean(1) + 1e-30)
            r['release_spike_db'] = round(e[8:].max() - e[:8].mean(), 1)
            rows.append(r)
with open(os.path.join(CSV, 'notes_bursts.csv'), 'w', newline='') as fh:
    w = csv.DictWriter(fh, fieldnames=list(rows[0].keys())); w.writeheader(); w.writerows(rows)
REG = {'bass': (21, 47), 'middle': (48, 83), 'treble': (84, 108)}
for dyn in DYN:
    for rn, (a, b) in REG.items():
        s = []
        for src in ('sal', 'c2'):
            d = [q for q in rows if q['src'] == src and q['dyn'] == dyn and a <= q['key'] <= b]
            md = lambda c: np.nanmedian([q[c] for q in d])
            s.append(f"{src}: attLF {md('attack_lf_re_total_db'):.1f} LF@0.3s {md('lf_0.3s_re_attack_lf_db'):.1f} attHF {md('attack_hf_re_total_db'):.1f} relLF {md('release_lf_re_note_db'):.1f} relHF {md('release_hf_re_note_db'):.1f} spike {md('release_spike_db'):.1f}")
        print(dyn, rn, ' | '.join(s))
# levels per register and dynamic (raw), c2 - sal
F = list(csv.DictReader(open(os.path.join(CSV, 'notes_features.csv'))))
print('raw level differences c2 - sal (median dB): rms of first 60 ms / LUFS of first second')
for dyn in DYN:
    for rn, (a, b) in REG.items():
        d = {}
        for c in ('rms_50ms_db', 'lufs1s'):
            v = {s: np.array([float(q[c]) for q in F if q['src'] == s and q['dyn'] == dyn and a <= int(q['key']) <= b]) for s in ('sal', 'c2')}
            d[c] = np.median(v['c2'] - v['sal'])
        print(f'  {dyn} {rn:7s} 60ms {d["rms_50ms_db"]:+.1f}  1s {d["lufs1s"]:+.1f}')
