"""Write Chorda's unison pass (round 41, `DSP_VERSION` 11) as the shipped Rust module.

    python -I tools/chorda-voicing/ship_unison.py <spec file> <table file> [<out .rs>]

`<spec file>`: one line `key=value,…` — what `chorda::tests_unison::unison_optimise` prints
after `SPEC ` (each key's unison width, stiffness scatter, pattern and bridge share), with the
pass's global keys first (e.g. `bridge.pull_hz=0.1,`). `<table file>`: what
`unison_retable` prints after `SPEC ` (`tune.<key>=<cents>` for every key). Written (default):
`crates/cantus-dsp/src/chorda_voicing_unison.rs`. Run `cargo fmt -p cantus-dsp` after.
"""

import os
import re
import sys

HEAD = '''//! Chorda's unison pass (`DSP_VERSION` 11, round 41, ADR 0098's fifth amendment,
//! `docs/round41/chorda-tune.md`): what ships over the technician's pass of `DSP_VERSION` 10
//! so no unison beats — the owner heard E4, C#3, D3, C3, B2 and E5 "as if one of the strings
//! inside the key were out of tune". [`SPEC`] (the voicing lab's format, applied after the
//! technician's) holds every bridge resonance's pull on partials 1–3 under 0.1 Hz and lays
//! each key's unison as `chorda::tests_unison::unison_optimise` found it: as wide as it can
//! be without a beat the ear counts in five seconds, its stiffness scatter, its pattern (a
//! fan, or two strings alike and a third apart) and its bridge's share of the strings' loss
//! chosen to keep the key's level as `DSP_VERSION` 10 had it. [`TUNE`] is each key's last
//! correction (cents), re-measured on the tuned unisons with
//! `chorda::tests_unison::unison_retable` A0–E6; above E6 it stays nought (a string there
//! sits on the curve, the board's tail is the rest of the whole key's measure) but for B6,
//! D#7 and A#7, half a cent each toward the curve the whole key is measured on — no more, so a
//! string alone stays within half a cent of it. D6 is laid as a pair (two strings alike, one
//! apart): as a fan its slow beat swung its level past a recorded grand's ceiling. Generated
//! by `tools/chorda-voicing/ship_unison.py`.

use super::KEYS;

'''


def main():
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    spec = open(sys.argv[1], encoding="utf-8").read().strip()
    table = open(sys.argv[2], encoding="utf-8").read().strip()
    out = sys.argv[3] if len(sys.argv) > 3 else os.path.join(
        os.path.dirname(os.path.abspath(__file__)), "..", "..", "crates", "cantus-dsp", "src",
        "chorda_voicing_unison.rs")
    items = [x for x in spec.split(",") if x]
    for x in items:
        if not re.fullmatch(r"[a-z_.0-9]+=-?[0-9.e-]+", x):
            sys.exit(f"not a spec item: {x}")
    tune = {}
    for x in table.split(","):
        m = re.fullmatch(r"tune\.(\d+)=(-?[0-9.]+)", x.strip())
        if m:
            tune[int(m.group(1))] = float(m.group(2))
    if sorted(tune) != list(range(21, 109)):
        sys.exit("the table must give every key 21..108")
    # The spec, a few items a line, under 100 columns.
    lines, cur = [], ""
    for x in items:
        piece = x + ","
        if len(cur) + len(piece) > 88:
            lines.append(cur)
            cur = ""
        cur += piece
    if cur:
        lines.append(cur.rstrip(",") if cur.endswith(",") else cur)
    lines[-1] = lines[-1].rstrip(",")
    body = HEAD
    body += "/// The pass's spec, over the technician's (`DSP_VERSION` 10's).\n"
    body += "pub const SPEC: &str = concat!(\n"
    body += "".join(f'    "{l}",\n' for l in lines)
    body += ");\n\n"
    body += "/// Each key's last correction (cents, its whole ladder), A0 first.\n"
    body += "#[rustfmt::skip]\npub const TUNE: [f32; KEYS] = [\n"
    for k0 in range(21, 109, 8):
        vals = ", ".join(f"{tune[k]:.2f}" for k in range(k0, k0 + 8))
        body += f"    {vals}, // {k0}–{k0 + 7}\n"
    body += "];\n"
    with open(out, "w", encoding="utf-8", newline="\n") as f:
        f.write(body)
    print(f"wrote {out}: {len(items)} spec items, table of 88")


if __name__ == "__main__":
    main()
