"""List a crate's non-ignored #[test] fns that time something: they call Instant::now, or a
function of the same file that does. Prints file:line fn-name. Rough (brace counting)."""
import re, sys, pathlib

root = pathlib.Path(sys.argv[1])
for f in sorted(root.rglob('*.rs')):
    s = f.read_text(encoding='utf-8', errors='replace')
    if 'Instant' not in s:
        continue
    # functions: name -> (start line, body text, attrs)
    fns = []
    for m in re.finditer(r'((?:\s*#\[[^\]]*\]\s*)*)\s*(?:pub(?:\([^)]*\))?\s+)?fn\s+(\w+)[^{;]*\{', s):
        attrs, name = m.group(1), m.group(2)
        i, depth = m.end(), 1
        while depth and i < len(s):
            c = s[i]
            depth += (c == '{') - (c == '}')
            i += 1
        fns.append((name, s.count('\n', 0, m.start(2)) + 1, s[m.end():i], attrs))
    timers = {n for n, _, b, _ in fns if 'Instant::now' in b}
    # helpers calling timers, two levels
    for _ in range(2):
        timers |= {n for n, _, b, _ in fns if any(re.search(rf'\b{t}\s*\(', b) for t in timers)}
    for n, line, b, attrs in fns:
        if '#[test]' in attrs and '#[ignore' not in attrs and n in timers:
            print(f'{f.relative_to(root)}:{line} {n}')
