"""Write a candidate's files as the Rust module Chorda ships (round 40: candidate 16 became
`crates/cantus-dsp/src/chorda_voicing_c16.rs`, DSP_VERSION 7). The spec goes in verbatim
(applied over beta.14's voicing, a later key overriding an earlier one), the two tables row by
row from A0. Shipping a new candidate is still a DSP_VERSION bump with its golden hashes, an
ADR amendment and the tests that pin `Voicing::default` (`docs/round40/chorda.md`).

    python -I ship_rust.py <spec.txt> <partials.csv> <levels.csv> <out.rs> [name]

Run on candidates/c16 it writes chorda_voicing_c16.rs byte for byte (the check in README.md).
"""
import sys

KEYS = range(21, 109)
WIDTH = 95  # the module's widest line


def rows(path, n):
    out = {}
    for line in open(path, encoding='utf-8'):
        f = [x.strip() for x in line.strip().split(',')]
        if len(f) == n + 1 and f[0].isdigit():
            out[int(f[0])] = f[1:]
    return out


def spec_lines(spec):
    items = [kv + ',' for kv in spec.split(',') if kv]
    items[-1] = items[-1][:-1]
    lines, cur = [], ''
    for it in items:
        if cur and len('    "' + cur + it + '",') > WIDTH:
            lines.append(cur)
            cur = ''
        cur += it
    lines.append(cur)
    return [f'    "{s}",' for s in lines]


def main():
    spec_p, part_p, lev_p, out_p = sys.argv[1:5]
    name = sys.argv[5] if len(sys.argv) > 5 else 'candidate 16'
    spec = open(spec_p, encoding='utf-8').readline().strip()
    part = rows(part_p, 6)
    lev = rows(lev_p, 1)
    zero = ['-0.00'] * 6
    o = []
    o.append("//! Chorda's shipped voicing since `DSP_VERSION` 7 (round 40): candidate 16, as the voicing")
    o.append("//! lab played it (`docs/round40/chorda.md`, *Candidate 16*). [`SPEC`] is the lab's spec,")
    o.append("//! verbatim (`voicing\\candidate16.txt`'s first line: `key=value`, applied in order over")
    o.append("//! beta.14's voicing, a later key overriding an earlier one); [`PARTIALS`] and [`LEVELS`] are")
    o.append("//! its two tables (`candidate16-partials.csv`, `candidate16-levels.csv`), each key's row from")
    o.append("//! A0. Generated from those files; a test holds [`super::Voicing::default`] to them.")
    o.append('')
    o.append('use super::{KEYS, PARTIALS as P};')
    o.append('')
    o.append("/// The lab's spec, verbatim.")
    o.append('pub const SPEC: &str = concat!(')
    o += spec_lines(spec)
    o.append(');')
    o.append('')
    o.append("/// Each key's partial voicing (dB, partials 2 … 7), A0 first. (B2's −3.14 dB is a measured")
    o.append('/// level, not π.)')
    o.append('#[rustfmt::skip]')
    o.append('#[allow(clippy::approx_constant)]')
    o.append('pub const PARTIALS: [[f32; P]; KEYS] = [')
    for k in KEYS:
        o.append(f"    [{', '.join(part.get(k, zero))}], // {k}")
    o.append('];')
    o.append('')
    o.append("/// Each key's own level (dB), A0 first.")
    o.append('#[rustfmt::skip]')
    o.append('pub const LEVELS: [f32; KEYS] = [')
    vals = [lev.get(k, ['0.00'])[0] for k in KEYS]
    for i in range(0, len(vals), 8):
        o.append('    ' + ', '.join(vals[i:i + 8]) + ',')
    o.append('];')
    text = '\n'.join(o) + '\n'
    if name != 'candidate 16':
        text = text.replace('candidate 16', name).replace('Candidate 16', name[:1].upper() + name[1:])
    open(out_p, 'w', encoding='utf-8', newline='\n').write(text)
    print(f'{out_p}: {len(spec.split(","))} spec keys, {len(part)} partial rows, {len(lev)} levels ({name})')


if __name__ == '__main__':
    main()
