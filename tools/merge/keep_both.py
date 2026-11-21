"""Resolve additive merge conflicts by keeping both sides (ours, then theirs). Keeps CRLF.
Refuses diff3 bases (|||||||): those are not additive by construction."""
import sys

for p in sys.argv[1:]:
    raw = open(p, 'rb').read().decode('utf-8')
    nl = '\r\n' if '\r\n' in raw else '\n'
    out, state = [], None
    for line in raw.split(nl):
        if line.startswith('<<<<<<< '):
            state = 'ours'; continue
        if line.startswith('|||||||') and state:
            sys.exit(f'{p}: diff3 base, resolve by hand')
        if line == '=======' and state == 'ours':
            state = 'theirs'; continue
        if line.startswith('>>>>>>> ') and state == 'theirs':
            state = None; continue
        out.append(line)
    if state:
        sys.exit(f'{p}: unterminated conflict')
    open(p, 'wb').write(nl.join(out).encode('utf-8'))
    print('resolved', p)
