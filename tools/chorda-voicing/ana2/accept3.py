"""Round 40, candidate 6 (second builder): accept2's score plus the second analysis's other
ranked items, each 1 at the edge of the analyst's target (lower closer):
  P12 sustain re attack (s17: A0 ... B6 at its four times; the treble's top is the
      reference's rumble, left out) — the middle's after-sound, the upper middle's tail;
  P13 the upper middle's first half-second and the treble's tail (s19: D#5-A5 at 0.4/0.65 s,
      C6-A6 at 3.25 s, mf);
  P14 the soft treble's knock against its tone (s08: pp C7-C8);
  P15 the steps pp -> mf by register (s09);
  P16 the release: its noise against the attack and its peak against the note (s13c/s13b);
  P17 the sub-bass: A0-F#1's second partial against the fundamental (s21 on the bass keys);
  P18 the keyboard's evenness key by key (keylev.py on keys_mf, when there);
  P19 the soft trill's top notes after it (s20 at 6.6/7.0/7.4 s).

    python accept3.py <ana out dir> <ana2 out dir> [keylev log]
"""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import os, re, sys
import numpy as np
sys.path.insert(0, tool_path('ana'))
sys.path.insert(0, tool_path('ana2'))
import accept2
from accept2 import log

WEIGHTS = {'P9 beats': 1.0, 'P10 upper colour': 0.6, 'P11 rise': 0.4, 'P12 sustain': 0.8,
           'P13 upper tone': 0.5, 'P14 treble knock': 0.4, 'P15 pp steps': 0.5, 'P16 release': 0.4,
           'P17 sub-bass': 0.4, 'P18 evenness': 0.6, 'P19 trill tail': 0.3, 'P20 tuning': 1.0}


def nums(s):
    return [float(v) for v in re.findall(r'-?\d+\.\d+|-?\d+', s)]


def score3(d2, keylev=None):
    parts = accept2.score2(d2)
    p = []
    for line in log(d2, 's17_sustain'):
        m = re.match(r'(\S+): c5 (.*)\| sal (.*)\| late', line)
        if m and not m.group(1).startswith('C7'):
            c, s = nums(m.group(2)), nums(m.group(3))
            p += [abs(a - b) / 5 for a, b in zip(c, s)]
    parts['P12 sustain'] = float(np.mean(p)) if p else float('nan')
    p, dyn = [], None
    for line in log(d2, 's19_tonal_sustain'):
        if line.startswith('=='):
            dyn = line.split()[1]
            continue
        m = re.match(r'(\S+)\s+c5 (.*)\| sal (.*)', line)
        if dyn == 'mf' and m:
            c, s = nums(m.group(2)), nums(m.group(3))
            if m.group(1) in ('D#5', 'F#5', 'A5'):
                p += [abs(c[i] - s[i]) / 5 for i in (1, 2)]
            elif m.group(1) in ('C6', 'D#6', 'F#6', 'A6'):
                p.append(abs(c[7] - s[7]) / 5)
    parts['P13 upper tone'] = float(np.mean(p)) if p else float('nan')
    p = []
    for line in log(d2, 's08_attack'):
        m = re.match(r"\['(C7|D#7|F#7|A7|C8)', 'pp', np.float64\(([-\d.]+)\), np.float64\(([-\d.]+)\)\]", line)
        if m:
            p.append(max(0.0, float(m.group(2)) - float(m.group(3))) / 5)
    parts['P14 treble knock'] = float(np.mean(p)) if p else float('nan')
    st = {}
    for line in log(d2, 's09_dynamics'):
        m = re.match(r'\s+(Chorda c5|Salamander)\s+(\S+): mf-pp ([-\d.]+) dB', line)
        if m:
            st[(m.group(1), m.group(2))] = float(m.group(3))
    p = [abs(st[('Chorda c5', r)] - st[('Salamander', r)]) / 2 for r in ('A0-B2', 'C3-B4', 'C5-B6', 'C7-C8')
         if ('Chorda c5', r) in st and ('Salamander', r) in st]
    parts['P15 pp steps'] = float(np.mean(p)) if p else float('nan')
    p = []
    for line in log(d2, 's13c_release_vs_attack'):
        m = re.search(r'c5 ([-\d.]+) dB, sal ([-\d.]+) dB', line)
        if m:
            p.append(abs(float(m.group(1)) - float(m.group(2))) / 8)
    for line in log(d2, 's13b_release_abs'):
        m = re.match(r'\S+ \[\(np.float64\(([-\d.]+)\), np.float64\(([-\d.]+)\)\), \(np.float64\(([-\d.]+)\)', line)
        if m:
            p.append(abs(float(m.group(1)) - float(m.group(3))) / 8)
    parts['P16 release'] = float(np.mean(p)) if p else float('nan')
    p = []
    for line in log(d2, 's21_bass'):
        m = re.match(r'(A0|C1|D#1|F#1)\s+(\S+)/(\S+)', line)
        if m:
            p.append(abs(float(m.group(2)) - float(m.group(3))) / 5)
        if line.startswith('== ff'):
            break
    parts['P17 sub-bass'] = float(np.mean(p)) if p else float('nan')
    p = float('nan')
    if keylev and os.path.exists(keylev):
        for line in open(keylev, encoding='utf-8', errors='replace'):
            m = re.search(r'rms of the difference ([\d.]+)', line)
            if m:
                p = float(m.group(1)) / 2
    parts['P18 evenness'] = p
    p = []
    for line in log(d2, 's20_top_gaps'):
        if line.startswith('Chorda') or line.startswith('Salamander'):
            v = dict(re.findall(r'([\d.]+)s:\s*(-?[\d.]+)', line))
            p.append([float(v[t]) for t in ('6.6', '7.0', '7.4')])
    parts['P19 trill tail'] = float(np.mean([abs(a - b) / 10 for a, b in zip(*p)])) if len(p) == 2 else float('nan')
    # P20 (candidate 9): tuning against the reference (tuning.py's log) - the first partial's
    # rms over every key / 4 cents, partials 1-3 / 15, the test chords' consonant beats / 1 Hz,
    # and 3 more when any key is past the 10-cent gate (a mistuned key never passes quietly).
    p = float('nan')
    t = os.path.join(d2, 'tuning.log')
    if os.path.exists(t):
        s = open(t, encoding='utf-8').read()
        r1, r13 = (float(v) for v in re.search(r'first partial rms ([\d.]+) cents; partials 1-3 rms ([\d.]+)', s).groups())
        b = float(re.search(r'mean \|diff\| ([\d.]+) Hz', s).group(1))
        p = (r1 / 4 + r13 / 15 + b / 1.0) / 3 + (3.0 if 'GATE FAIL' in s else 0.0)
    parts['P20 tuning'] = p
    return parts


def total(a, parts):
    return a + sum(WEIGHTS[k] * v for k, v in parts.items() if k in WEIGHTS and np.isfinite(v))


if __name__ == '__main__':
    import accept
    a, _ = accept.score(sys.argv[1])
    parts = score3(sys.argv[2], sys.argv[3] if len(sys.argv) > 3 else None)
    for k, v in parts.items():
        print(f'{k:20} {v:6.2f}')
    print(f'accept {a:.3f}  ACCEPT3 {total(a, parts):.3f}')
