#!/usr/bin/env python3
"""Each measured table's level law (round 42, tympanum-real): writes `LAW` in
`crates/cantus-dsp/src/pulsus_measured_law.rs`.

    python -I tools/tympanum-real/law.py <level_law_dump output> <hits dir> <law.rs> [--check]

`level_law_dump` (cantus-dsp, ignored test) prints each table's first-80-ms level at the
law's velocities through Pulsus, with `LAW` as it is — run it with an empty `LAW`. The target:
- the studio kit's tables (1-8) take the level law of DRSKit, the kit they were measured
  from, as Tympanum's own recorded path plays it (`<hits dir>/ref/drskit.jsonl`, the piece on
  the table's key: crash 49, hat 42, ride 51, kick 36, snare 38, toms 45, 43, 41);
- the jazz kit's (17-24) keep their own law made monotone and smooth, rising at least
  0.1 dB a velocity unit, (Virtuosity's 60-dB
  span from velocity 16 to 127 is that library's curve, not a drummer's: the jazz kit already
  follows its shape, and only its steps that go down are taken out).
Either way the hardest hit (127) keeps the level the calibration gave it: only the law's shape
moves. `--check` prints the law the dump shows against the target, changing nothing.
"""
import json
import re
import sys
from collections import defaultdict
from pathlib import Path

import numpy as np

VELS = [1, 8, 16, 24, 32, 40, 48, 56, 64, 72, 80, 88, 96, 104, 112, 120, 127]
STUDIO_KEYS = {1: 49, 2: 42, 3: 51, 4: 36, 5: 38, 6: 45, 7: 43, 8: 41}


def dump(path):
    law = {}
    for line in open(path, encoding="utf-8", errors="replace"):
        m = re.match(r"table\s+(\d+):(.*)", line.strip())
        if m:
            pts = dict((int(a), float(b)) for a, b in re.findall(r"(\d+):(-?[\d.]+)", m.group(2)))
            law[int(m.group(1))] = np.array([pts[v] for v in VELS])
    return law


def ref_law(hits, key):
    by = defaultdict(list)
    for line in open(hits, encoding="utf-8"):
        r = json.loads(line)
        if r["key"] == key and r["kind"] not in ("percussion",) and (key != 42 or r["open"] <= 0.15):
            by[r["vel"]].append(r["rms80_db"])
    v = np.array(sorted(by))
    lv = np.array([np.mean(by[x]) for x in v])
    # below the softest measured velocity: its first step's slope continued
    slope = (lv[1] - lv[0]) / (v[1] - v[0])
    return np.array([np.interp(x, v, lv) if x >= v[0] else lv[0] + slope * (x - v[0]) for x in VELS])


def monotone(y):
    """Pool-adjacent-violators (non-decreasing), then a light 3-point smoothing that keeps it so."""
    y = list(map(float, y))
    blocks = [[y[0], 1]]
    for x in y[1:]:
        blocks.append([x, 1])
        while len(blocks) > 1 and blocks[-2][0] > blocks[-1][0]:
            a, b = blocks.pop(), blocks.pop()
            blocks.append([(a[0] * a[1] + b[0] * b[1]) / (a[1] + b[1]), a[1] + b[1]])
    out = np.concatenate([[m] * n for m, n in blocks])
    sm = out.copy()
    sm[1:-1] = 0.25 * out[:-2] + 0.5 * out[1:-1] + 0.25 * out[2:]
    return np.maximum.accumulate(sm)


def main():
    law = dump(sys.argv[1])
    hits = Path(sys.argv[2])
    rs = Path(sys.argv[3])
    check = "--check" in sys.argv
    rows = []
    for tid, L in sorted(law.items()):
        if tid in STUDIO_KEYS:
            T = ref_law(hits / "ref" / "drskit.jsonl", STUDIO_KEYS[tid])
        else:
            T = monotone(L)
            # a floor of 0.1 dB a velocity unit: a pooled flat top would fall by a hit's own
            # variation (± ~1 dB) as often as it rose; every recorded piece still rises there
            for i in range(1, len(T)):
                T[i] = max(T[i], T[i - 1] + 0.1 * (VELS[i] - VELS[i - 1]))
        C = (T - T[-1]) - (L - L[-1])
        print(f"table {tid:2}: now  " + " ".join(f"{x:6.1f}" for x in L - L[-1]))
        print(f"          want " + " ".join(f"{x:6.1f}" for x in T - T[-1]))
        print(f"          add  " + " ".join(f"{x:6.1f}" for x in C))
        rows.append((tid, [int(round(100 * c)) for c in C]))
    if check:
        return
    body = "\n".join(f"    ({t}, [{', '.join(str(c) for c in cs)}])," for t, cs in rows)
    raw = rs.read_bytes()
    crlf = b"\r\n" in raw
    text = raw.decode("utf-8").replace("\r\n", "\n")
    new = re.sub(r"(pub\(crate\) const LAW: &\[\(u8, \[i16; 17\]\)\] = &\[\n)(.*?)(\];)",
                 lambda m: m.group(1) + body + "\n" + m.group(3), text, flags=re.S)
    rs.write_text(new, encoding="utf-8", newline="\r\n" if crlf else "\n")
    print(f"{len(rows)} tables -> {rs}")


if __name__ == "__main__":
    main()
