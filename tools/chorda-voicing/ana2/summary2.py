"""Round 40: the second analysis's ranked items, from one try's logs (`try2.sh`)."""
import os, re, sys
import numpy as np

d = sys.argv[1]


def log(name):
    p = os.path.join(d, name + '.log')
    return open(p, encoding='utf-8', errors='replace').read().splitlines() if os.path.exists(p) else []


# 1. Beats: depth (A1 ... A4), coherence, sweep.
dep = {}
for line in log('s24_beat_depth'):
    m = re.match(r"\['(\S+)', ([\d.na]+), ([\d.na]+)\]", line)
    if m:
        dep[m.group(1)] = (float(m.group(2)), float(m.group(3)))
keys = ['A1', 'D#2', 'F#2', 'D#3', 'F#3', 'A3', 'C4', 'D#4', 'F#4', 'A4']
print('1 beat depth   ' + '  '.join(f'{k} {dep[k][0]:.1f}/{dep[k][1]:.1f}' for k in keys if k in dep))
coh = {}
for line in log('s06_coherence'):
    m = re.match(r'(\S+)\s+coherence c5 ([-\d.na]+) sal ([-\d.na]+)', line)
    if m:
        coh[m.group(1)] = (float(m.group(2)), float(m.group(3)))
print('1 dip together ' + '  '.join(f'{k} {coh[k][0]:.2f}/{coh[k][1]:.2f}' for k in ['A1', 'D#2', 'D#3', 'F#3', 'A3', 'C4', 'A4', 'C5'] if k in coh))
sw = {}
for line in log('s07_sweep'):
    m = re.match(r'(\S+)\s+sweep-coherence c5 ([-\d.na]+)\s+sal ([-\d.na]+)', line)
    if m:
        sw[m.group(1)] = (float(m.group(2)), float(m.group(3)))
print('1 flanger      ' + '  '.join(f'{k} {sw[k][0]:.2f}/{sw[k][1]:.2f}' for k in ['A1', 'D#2', 'D#3', 'F#3', 'A3', 'C4', 'F#4'] if k in sw))
# 2. C5-A5 colour.
for line in log('s21_upper_mid_colour'):
    if line.startswith('==') or line.startswith('mean') or re.match(r'(C5|D#5|F#5|A5|A4)\s', line):
        print('2 ' + line)
# 3. Attack rise.
on = False
for line in log('s18_rise_summary'):
    if line.startswith('=='):
        on = 'rise' in line
        continue
    if on and line.startswith('mf'):
        print('3 rise ' + line)
# 4/8. Sustain.
for name in ('s17_sustain', 's19_tonal_sustain', 's20_top_gaps', 's09_dynamics'):
    lines = [l for l in log(name) if l.strip()]
    if lines:
        print(f'-- {name}: ' + ' | '.join(lines[-6:]))
