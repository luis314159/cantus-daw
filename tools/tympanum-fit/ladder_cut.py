"""The ladder's cost probe (round 40, ladder; drum-strategies.md §3.1f): R3 with a cymbal's
modes above a cut frequency dropped, the fine noise table then carrying what they did.

    python -I ladder_cut.py <out dir> [kit:art:vel ...]

Writes `<kit>-<art>-v<vel>-R3c<khz>.wav` for cuts at 4, 6 and 8 kHz and prints, per cut,
how many modes remain and the cymbal measures against R0 and the full R3.
"""
import os
import sys
import numpy as np
import soundfile as sf

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import ladder as L  # noqa: E402
import ladder_measure as M  # noqa: E402
from early import onset1  # noqa: E402

CUTS = [4000, 6000, 8000]
KEYS = ["flat", "peaks", "eff", "stab", "fl816", "T8_16", "tail_flat", "tail_eff", "xc"]


def r3(kit, art, vel, cut=None):
    m, on = L.recorded(kit, art, vel)
    n = len(m)
    tab, lay = L.layer(kit, art, vel)
    if cut:
        lay = dict(lay, modes=[md for md in lay["modes"] if md[0] < cut])
    a0 = onset1(m, on) - int(0.0005 * L.SR)
    s0 = L.seed(kit, art, vel)
    rng = lambda k: np.random.default_rng(s0 + k)
    bands = L.place(L.render_bands(tab, lay, n - on, rng(1)), n, on)
    modes = L.place(L.render_modes(lay, n - on, rng(2)), n, on)
    att = L.attack(m, a0, modes + bands * L.late_gate(n, a0))
    P, Pres = L.residual(m, [modes, att], a0)
    P3 = L.table_power(L.table_of(Pres, a0), len(P), a0)
    y = modes + att + L.noise_resynth(P3, n, rng(4)) * L.onset_gate(n, a0)
    g, s = L.clip_window(n, a0)
    return (y * g)[s:], (m * g)[s:], on - s, a0 - s, len(lay["modes"])


def main():
    out = sys.argv[1]
    items = [a.split(":") for a in sys.argv[2:]] or [["drs", "crash", "127"], ["drs", "ride", "127"],
                                                     ["virt", "crash", "127"], ["virt", "ride", "80"],
                                                     ["drs", "hat_open", "127"]]
    for kit, art, vel in items:
        vel = int(vel)
        full, m, on, first, nm = r3(kit, art, vel)
        ref = M.measures(m, on, first, art)
        rows = [("R0", ref, None), ("R3", M.measures(full, on, first, art), nm)]
        for c in CUTS:
            y, _, _, _, k = r3(kit, art, vel, c)
            sf.write(os.path.join(out, f"{kit}-{art}-v{vel}-R3c{c // 1000}.wav"), y, L.SR, subtype="PCM_24")
            rows.append((f"R3c{c // 1000}", M.measures(y, on, first, art), k))
        print(f"\n{kit} {art} v{vel}   " + " ".join(f"{k:>9s}" for k in KEYS) + "   modes  met")
        for name, r, k in rows:
            met = sum(bool(M.meets(q, r.get(q, np.nan), ref.get(q, np.nan))) for q in KEYS if q in ref)
            print(f"  {name:6s}" + " ".join(f"{r.get(q, np.nan):9.3f}" for q in KEYS) + f"   {k if k is not None else '':>5}  {met}/{sum(q in ref for q in KEYS)}")


if __name__ == "__main__":
    main()
