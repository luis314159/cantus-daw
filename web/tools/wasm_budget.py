#!/usr/bin/env python3
"""The site's WebAssembly stays small (round 39, P3): every .wasm in a trunk build, gzipped
at level 9, must weigh at most --max bytes (250 000 by default — the plan's "250 KB gzip").
Lighthouse holds the rest of the budget (web/lighthouserc.json); this is the one line it
cannot measure, so site-budget.yml and tools/web-budget.ps1 both run it.

  python3 web/tools/wasm_budget.py web/dist
"""

import argparse
import gzip
import sys
from pathlib import Path

MAX_GZIP = 250_000


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("dist", type=Path)
    ap.add_argument("--max", type=int, default=MAX_GZIP, help="bytes, gzipped")
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
    return 1 if over else 0


if __name__ == "__main__":
    sys.exit(main())
