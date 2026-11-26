#!/usr/bin/env python3
"""Read model 5's tables blob (`cantus-dsp/src/pulsus_measured.bin`, format CMT1 as
`tools/tympanum-fit/tables_blob.py` writes it) — each table's states and velocity layers with
their level, so the level law across velocity can be checked (round 42, tympanum-real).

    python -I tools/tympanum-real/blob_read.py <pulsus_measured.bin> [--table N]

Per layer: its velocity, the recording's 500-ms level the fit stored (`level`), and the
layer's own energy: the bands' power summed over the first breakpoints (`bands0`, dB) and the
modes' amplitude sum (`modes`, dB) — what the engine plays at that velocity.
"""
import struct
import sys

import numpy as np


def read(path):
    b = open(path, "rb").read()
    assert b[:4] == b"CMT1", "not a CMT1 blob"
    ver, nb, nbp, nt = struct.unpack_from("<IIII", b, 4)
    o = 20
    tabs = []
    for _ in range(nt):
        tid, hat, drum, _z, gain, n = struct.unpack_from("<BBBBfI", b, o)
        o += 12
        hz = np.frombuffer(b, "<f4", n, o)
        o += 4 * n
        (ns,) = struct.unpack_from("<I", b, o)
        o += 4
        states = []
        for _s in range(ns):
            op, foot, _a, _b, _c, nl = struct.unpack_from("<fBBBBI", b, o)
            o += 12
            layers = []
            for _l in range(nl):
                vel, level, s0, s1, g0, g1 = struct.unpack_from("<6f", b, o)
                o += 24
                env = np.frombuffer(b, "<i2", nb * nbp, o).astype(float).reshape(nb, nbp) / 100.0
                o += 2 * nb * nbp
                amp = np.frombuffer(b, "<f4", n, o)
                o += 4 * n
                o += 2 * n + 2 * n + n  # t60, rise, direct
                (ets,) = struct.unpack_from("<f", b, o)
                o += 4
                if ver >= 2 and ets > 0:
                    o += 2 * n + 4 * n + 2 * n
                layers.append({"vel": vel, "level": level, "stick": (s0, s1), "glide": (g0, g1),
                               "env": env, "amp": np.array(amp)})
            states.append({"open": op, "foot": foot, "layers": layers})
        tabs.append({"id": tid, "hat": hat, "drum": drum, "gain_db": gain, "hz": np.array(hz), "states": states})
    return {"version": ver, "nb": nb, "nbp": nbp, "tables": tabs}


def main():
    blob = read(sys.argv[1])
    only = int(sys.argv[sys.argv.index("--table") + 1]) if "--table" in sys.argv else None
    for t in blob["tables"]:
        if only is not None and t["id"] != only:
            continue
        print(f"table {t['id']} hat={t['hat']} drum={t['drum']} gain {t['gain_db']:+.2f} dB, {len(t['hz'])} modes")
        for s in t["states"]:
            print(f"  state open {s['open']:.2f} foot {s['foot']}")
            for l in s["layers"]:
                e0 = 10 * np.log10(np.sum(10 ** (l["env"][:, :6] / 10)) + 1e-30)
                ma = 20 * np.log10(np.sum(np.abs(l["amp"])) + 1e-30)
                print(f"    vel {l['vel']:6.1f} level {l['level']:7.2f}  bands0 {e0:7.2f}  modes {ma:7.2f}  stick {l['stick'][0]:.3g},{l['stick'][1]:.3g}  glide {l['glide'][0]:.4f}")


if __name__ == "__main__":
    main()
