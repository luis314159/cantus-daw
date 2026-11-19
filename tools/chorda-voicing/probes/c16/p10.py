import sys, re, numpy as np
p = []
for line in open(sys.argv[1]):
    m = re.match(r'(A4|C5|D#5|F#5|A5)\s+(.*)', line)
    if m:
        v = [tuple(float(x) for x in t.split('/')) for t in m.group(2).split()]
        p += [abs(c - s) / 5 for c, s in v[:3]]
print(f'P10 {np.mean(p):.2f}')
