#!/usr/bin/env python3
"""Model 6's level law (round 42, tympanum-real): writes `LAW6` in
`crates/cantus-dsp/src/pulsus_measured_law.rs`.

    python -I tools/tympanum-real/law6.py <level_law_dump output> <law.rs> [--check]

`level_law_dump` (cantus-dsp, ignored test) prints each table's first-80-ms level at the law's
velocities through Pulsus: model 5's (`table NN:`, with `LAW`) and model 6's drums (`six NN:`,
with `LAW6` as it is). Model 6 changes how a hit comes in, not how loud a drummer's stroke
is: each drum's `LAW6` brings its level to model 5's at every velocity — the recorded kit's
law (`law.py`) and the calibrated level at 127 alike. Run it again after any change to model
6's onset (`pulsus_measured_onset.rs`) or to the tables.
"""
import re
import sys
from pathlib import Path

import numpy as np

VELS = [1, 8, 16, 24, 32, 40, 48, 56, 64, 72, 80, 88, 96, 104, 112, 120, 127]


def dump(path):
    five, six = {}, {}
    for line in open(path, encoding="utf-8", errors="replace"):
        m = re.match(r"(table|six)\s+(\d+):(.*)", line.strip())
        if m:
            pts = dict((int(a), float(b)) for a, b in re.findall(r"(\d+):(-?[\d.]+)", m.group(3)))
            (five if m.group(1) == "table" else six)[int(m.group(2))] = np.array([pts[v] for v in VELS])
    return five, six


def current(text):
    m = re.search(r"pub\(crate\) const LAW6: &\[\(u8, \[i16; 17\]\)\] = &\[\n(.*?)\];", text, flags=re.S)
    out = {}
    for t, cs in re.findall(r"\((\d+), \[([-\d, ]+)\]\)", m.group(1) if m else ""):
        out[int(t)] = 0.01 * np.array([int(c) for c in cs.split(",")])
    return out


def main():
    five, six = dump(sys.argv[1])
    rs = Path(sys.argv[2])
    raw = rs.read_bytes()
    crlf = b"\r\n" in raw
    text = raw.decode("utf-8").replace("\r\n", "\n")
    now = current(text)
    rows = []
    for tid, L6 in sorted(six.items()):
        if tid not in five:
            continue
        bare = L6 - now.get(tid, np.zeros(len(VELS)))
        C = five[tid] - bare
        print(f"six {tid:2}: model 5 " + " ".join(f"{x:6.1f}" for x in five[tid]))
        print(f"        model 6 " + " ".join(f"{x:6.1f}" for x in bare))
        print(f"        add     " + " ".join(f"{x:6.1f}" for x in C))
        rows.append((tid, [int(round(100 * c)) for c in C]))
    if "--check" in sys.argv:
        return
    body = "".join(f"    ({t}, [{', '.join(str(c) for c in cs)}]),\n" for t, cs in rows)
    new = re.sub(r"(pub\(crate\) const LAW6: &\[\(u8, \[i16; 17\]\)\] = &\[\n)(.*?)(\];)",
                 lambda m: m.group(1) + body + m.group(3), text, flags=re.S)
    rs.write_text(new, encoding="utf-8", newline="\r\n" if crlf else "\n")
    print(f"{len(rows)} tables -> {rs}")


if __name__ == "__main__":
    main()
