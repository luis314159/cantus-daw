"""Round 40, candidate 6: the acceptance score with the second analysis's ranked items added
— accept.py's (the first analyst's measures) plus
  P9  beats: depth per key (A1 ... A4, both channels' power), the partials dipping together,
      the flanger (envelopes aligned on t*n) — 1 = at the edge of the analyst's target;
  P10 the upper middle's colour: partials 2-4 against the fundamental, A4 ... A5, per channel;
  P11 the attack's rise (-20 -> -3 dB of the 1-ms peak), C3-B5 and C6-C8.

    python accept2.py <ana out dir> <ana2 out dir>
"""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import os, re, sys
import numpy as np
sys.path.insert(0, tool_path('ana'))
import accept


def log(d, name):
    p = os.path.join(d, name + '.log')
    return open(p, encoding='utf-8', errors='replace').read().splitlines() if os.path.exists(p) else []


def pairs(lines, pat):
    out = {}
    for line in lines:
        m = re.match(pat, line)
        if m:
            out[m.group(1)] = (float(m.group(2)), float(m.group(3)))
    return out


def score2(d2, verbose=False):
    parts = {}
    dep = pairs(log(d2, 's24_beat_depth'), r"\['(\S+)', ([-\d.na]+), ([-\d.na]+)\]")
    coh = pairs(log(d2, 's06_coherence'), r'(\S+)\s+coherence c5 ([-\d.na]+) sal ([-\d.na]+)')
    sw = pairs(log(d2, 's07_sweep'), r'(\S+)\s+sweep-coherence c5 ([-\d.na]+)\s+sal ([-\d.na]+)')
    p9 = [abs(c - s) / 3 for k, (c, s) in dep.items() if np.isfinite(c)]
    p9 += [max(0.0, c - 0.1) / 0.1 for c, s in coh.values() if np.isfinite(c)]
    p9 += [max(0.0, c - 0.2) / 0.2 for c, s in sw.values() if np.isfinite(c)]
    parts['P9 beats'] = float(np.mean(p9)) if p9 else float('nan')
    p10 = []
    for line in log(d2, 's21_upper_mid_colour'):
        m = re.match(r'(A4|C5|D#5|F#5|A5)\s+(.*)', line)
        if m:
            v = [tuple(float(x) for x in t.split('/')) for t in m.group(2).split()]
            p10 += [abs(c - s) / 5 for c, s in v[:3]]
    parts['P10 upper colour'] = float(np.mean(p10)) if p10 else float('nan')
    p11 = []
    on = False
    for line in log(d2, 's18_rise_summary'):
        if line.startswith('=='):
            on = 'rise' in line
            continue
        if on and line.startswith('mf'):
            for nm, c, s in re.findall(r'(\S+) ([-\d.]+)/([-\d.]+)', line):
                c = float(c)
                if nm in ('C3-B3', 'C4-B4', 'C5-B5'):
                    p11.append(abs(c - 7.0) / 2)
                elif nm in ('C6-B6', 'C7-C8'):
                    p11.append(abs(c - 4.5) / 1.5)
    parts['P11 rise'] = float(np.mean(p11)) if p11 else float('nan')
    if verbose:
        for k, v in parts.items():
            print(f'{k:24} {v:6.2f}')
    return parts


if __name__ == '__main__':
    a, parts1 = accept.score(sys.argv[1], True)
    p2 = score2(sys.argv[2], True)
    total = a + p2['P9 beats'] + 0.6 * p2['P10 upper colour'] + 0.4 * p2['P11 rise']
    print(f'ACCEPT2 {total:.3f}')
