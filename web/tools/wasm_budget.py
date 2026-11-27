#!/usr/bin/env python3
"""The site's WebAssembly stays small (round 39, P3): every .wasm in a trunk build, gzipped
at level 9, must weigh at most --max bytes (250 000 by default — the plan's "250 KB gzip").
Lighthouse holds the rest of the budget (web/lighthouserc.json); this is the one line it
cannot measure, so site-budget.yml and tools/web-budget.ps1 both run it.

And the home's page, in both languages, gzipped at level 6, must weigh at most --doc-max
bytes (42 000): Lighthouse's simulated slow 4G sends it with TCP's slow start, 14 600 bytes
in the first round trip and 29 200 in the second, and nothing else on the page can start
before it is whole. Past 43 800 bytes on the line (this measure plus about 1 KB of headers
and the server's own gzip) it takes a third round trip and the LCP 150 ms more: beta.16's
home went out at 43 737 bytes and 1.88 s, the next at 43 822 bytes and 2.11 s, over the
budget, for 85 bytes (round 41, the site budget). A clear line here instead of a puzzle there.

  python3 web/tools/wasm_budget.py web/dist
"""

import argparse
import gzip
import sys
from pathlib import Path

MAX_GZIP = 250_000
MAX_DOC_GZIP = 42_000
HOMES = ["index.html", "es/index.html"]


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("dist", type=Path)
    ap.add_argument("--max", type=int, default=MAX_GZIP, help="bytes, gzipped")
    ap.add_argument("--doc-max", type=int, default=MAX_DOC_GZIP, help="bytes, gzipped")
    a = ap.parse_args()
    files = sorted(a.dist.glob("*.wasm"))
    if not files:
        print(f"wasm_budget: no .wasm in {a.dist} (build the site first)", file=sys.stderr)
        return 2
    over = False
    for f in files:
        raw = f.read_bytes()
        z = len(gzip.compress(raw, compresslevel=9, mtime=0))
        ok = z <= a.max
        over |= not ok
        print(f"wasm_budget: {f.name}: {len(raw):,} B raw, {z:,} B gzip, budget {a.max:,} B - {'ok' if ok else 'OVER'}")
    for name in HOMES:
        f = a.dist / name
        if not f.is_file():
            print(f"wasm_budget: no {name} in {a.dist} (pre-render the site first)", file=sys.stderr)
            return 2
        raw = f.read_bytes()
        z = len(gzip.compress(raw, compresslevel=6, mtime=0))
        ok = z <= a.doc_max
        over |= not ok
        print(f"wasm_budget: {name}: {len(raw):,} B raw, {z:,} B gzip, two round trips {a.doc_max:,} B - {'ok' if ok else 'OVER'}")
    return 1 if over else 0


if __name__ == "__main__":
    sys.exit(main())
