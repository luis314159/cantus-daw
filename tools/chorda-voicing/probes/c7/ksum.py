import sys, re, numpy as np
for f in sys.argv[1:]:
    L = [list(map(float, re.findall(r'-?\d+\.?\d*', l))) for l in open(f) if re.match(r'^\s*\d+ ', l)]
    A = np.array(L); print(open(f).readline().strip())
    out = []
    for nm, i in (('prompt', 1), ('mid', 3), ('late', 5), ('at3', 7)):
        for lo, hi, tag in ((84, 95, 'C6'), (96, 108, 'C7')):
            m = (A[:, 0] >= lo) & (A[:, 0] <= hi)
            out.append(f'{nm} {tag} {A[m, i].mean():6.1f}/{A[m, i + 1].mean():6.1f} |d| {np.abs(A[m, i] - A[m, i + 1]).mean():4.1f}')
    print('  ' + '\n  '.join(out))
