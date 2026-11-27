"""Each character preset against the measured spread of its reference pianos (round 42,
chorda-presets): every feature of `features.py`, the references' span widened by the
feature's tolerance, and whether the preset lands inside.

    python -I -X utf8 fit.py <wav root> <grands root…> -- <preset[=render dir]> …

The references are read from <wav root> (kw, fb, bk_bar) or, for the two grands, from the lab's
`eng40/wav` and `notes5/wav` (the second argument on: their roots, joined with `+`). A preset
`v_upright=v_upright_t` measures the trial render `v_upright_t` against v_upright's references.
Writes nothing; prints one table a preset.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import features  # noqa: E402
import presets  # noqa: E402

# What a feature may stray from the references' span and still be that kind of piano: about
# the measurement's own scatter between neighbouring keys and phrases, and under the two
# recorded grands' own difference (tilt 0.4 dB/oct, bright 1.5 dB, beat 1.2 dB, bass 7 dB).
TOL = {'tilt': 0.6, 'bright': 2.5, 'bright_vel': 3.0, 'decay_bass': 0.25, 'decay_mid': 0.25,
       'decay_treble': 0.25, 'beat': 1.0, 'bass': 2.0}
# The decay's tolerance is a share of the span's value, the rest are absolute.
RELATIVE = {'decay_bass', 'decay_mid', 'decay_treble'}
REF_DIR = {'salamander': 'salamander', 'bk_stage': 'bitklavier'}
# A reference's feature that is no measure of its kind: the Old Piano FB was sampled at one
# velocity, so its attack cannot brighten with it (0 dB), which no struck string does.
SKIP = {('fb', 'bright_vel')}


def main(argv):
    i = argv.index('--')
    root, grands, which = argv[0], argv[1], argv[i + 1:]
    cache = {}

    def m(src):
        if src not in cache:
            if src in REF_DIR:
                cache[src] = features.measure(root, f'{REF_DIR[src]}@{grands}')
            else:
                cache[src] = features.measure(root, src)
        return cache[src]

    total_in = total = 0
    for w in which:
        name, _, render = w.partition('=')
        render = render or name
        refs = presets.KIND.get(name, [])
        got = m(render)
        print(f'\n## {name} ({render}) against {", ".join(refs) or "no recorded piano of its kind"}')
        print('| feature | preset | references | span ± tolerance | |')
        print('|---|---|---|---|---|')
        for c in features.COLS:
            if c not in got:
                continue
            vals = [m(r)[c] for r in refs if c in m(r) and (r, c) not in SKIP]
            if not vals:
                print(f'| {c} | {got[c]:.2f} | — | — | |')
                continue
            t = TOL[c] * (max(abs(v) for v in vals) if c in RELATIVE else 1.0)
            lo, hi = min(vals) - t, max(vals) + t
            ok = lo <= got[c] <= hi
            total += 1
            total_in += ok
            print(f'| {c} | {got[c]:.2f} | {" / ".join("%.2f" % v for v in vals)} | {lo:.2f} … {hi:.2f} | {"in" if ok else "OUT"} |')
    print(f'\n{total_in} of {total} inside')


if __name__ == '__main__':
    main(sys.argv[1:])
