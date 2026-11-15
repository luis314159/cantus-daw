"""The fourth listen's files: each cymbal and hat alone at four velocities, the old model,
the new one and the recording, the three at the same loudness (round 40, tymp-engine).

    python -I make_b4.py <old hits> <new hits> <recorded hits> <out dir>
"""
import os
import sys
import numpy as np
import soundfile as sf
sys.path.insert(0, __file__.rsplit("/", 1)[0].rsplit("\\", 1)[0])
from metrics import load, PIECES, VELS, SR

NAMES = {"crash": "crash", "hat_closed": "hihat-cerrado", "hat_open": "hihat-abierto",
         "hat_pedal": "hihat-pedal", "ride": "ride", "ride_bell": "ride-campana",
         "kick": "bombo", "snare": "caja", "rimshot": "caja-aro", "sidestick": "caja-baqueta-cruzada",
         "tom_hi": "tom-alto", "tom_mid": "tom-medio", "tom_floor": "tom-de-piso"}
SOURCES = [("1-antes", 0), ("2-nuevo", 1), ("3-grabado", 2)]
# Each hit gets this long (s) in the listening files.
KEEP = {"crash": 3.5, "hat_closed": 1.0, "hat_open": 2.5, "hat_pedal": 1.0, "ride": 3.5,
        "ride_bell": 3.5, "kick": 2.0, "snare": 1.5, "rimshot": 1.5, "sidestick": 1.0,
        "tom_hi": 2.5, "tom_mid": 2.5, "tom_floor": 3.0}
ONLY = None


def cut(x, piece):
    out = []
    for v in VELS:
        i = PIECES.index(piece) * 4 + VELS.index(v)
        seg = x[int(i * 4 * SR):int((i + 1) * 4 * SR)].mean(1)
        out.append(seg[:int(KEEP[piece] * SR)])
    return np.concatenate(out)


def main():
    xs = [load(p) for p in sys.argv[1:4]]
    out = sys.argv[4]
    global ONLY
    ONLY = sys.argv[5].split(",") if len(sys.argv) > 5 else None
    os.makedirs(out, exist_ok=True)
    for p, name in NAMES.items():
        if ONLY and p not in ONLY:
            continue
        segs = [cut(x, p) for x in xs]
        # The same RMS for the three (the recording's), the loudest peak at -3 dBFS.
        ref = np.sqrt(np.mean(segs[2] ** 2))
        segs = [s * ref / max(np.sqrt(np.mean(s ** 2)), 1e-12) for s in segs]
        peak = max(np.abs(s).max() for s in segs)
        g = 10 ** (-3 / 20) / peak
        for (tag, k) in SOURCES:
            sf.write(os.path.join(out, f"{name}-{tag}.flac"), segs[k] * g, SR, subtype="PCM_24")
        print(name, "rms", round(20 * np.log10(ref * g), 1), "dBFS")


if __name__ == "__main__":
    main()
