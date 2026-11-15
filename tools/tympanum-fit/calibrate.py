"""Each measured table's level and tonal weight through the engine, against the recording
(round 40).

    python -I calibrate.py <engine hits.flac> <reference hits.flac> <gains.json>

The tables are fitted to the recorded kit's mono mix; the engine plays them through its
virtual microphones, whose colour and level are its own. This measures the difference
(500-ms RMS, every velocity) and adds it to the table's `gain_db` in gains.json.
"""
import json
import os
import sys
import numpy as np
sys.path.insert(0, __file__.rsplit("/", 1)[0].rsplit("\\", 1)[0])
from metrics import load, hit, SR, VELS, partials

PARTS = {"CRASH": ["crash"], "HAT": ["hat_closed", "hat_open", "hat_pedal"],
         "RIDE": ["ride", "ride_bell"], "KICK": ["kick"],
         "SNARE": ["snare", "rimshot", "sidestick"], "TOM_HI": ["tom_hi"], "TOM_MID": ["tom_mid"],
         "TOM_FLOOR": ["tom_floor"]}
DRUMS = {"KICK", "SNARE", "TOM_HI", "TOM_MID", "TOM_FLOOR"}


def rms_db(m, on):
    return 10 * np.log10(np.mean(m[on:on + SR // 2] ** 2) + 1e-30)


def main():
    M, R = load(sys.argv[1]), load(sys.argv[2])
    path = sys.argv[3]
    gains = json.load(open(path)) if os.path.exists(path) else {}
    for name, pieces in PARTS.items():
        d = []
        for p in pieces:
            for v in VELS:
                a, on_a = hit(M, p, v)
                b, on_b = hit(R, p, v)
                d.append(rms_db(b, on_b) - rms_db(a, on_a))
        g = float(np.median(d))
        print(f"{name}: {np.round(d, 1)} -> {g:+.2f} dB")
        gains[name] = round(gains.get(name, 0.0) + g, 2)
        # each state's each layer's own remainder, half a step (a layer's level as its
        # recording's, not only the table's median)
        from tables import TABLES
        states = next(parts for _, nm, parts in TABLES if nm == name)
        i = 0
        for p in pieces:
            si = [x[0] for x in states].index(p)
            for v in VELS:
                key = f"{name}_LEVEL_{si}_{v}"
                gains[key] = round(gains.get(key, 0.0) + 0.5 * (d[i] - g), 2)
                i += 1
        # The engine's microphones (their sum's comb, the room) make pure partials stand
        # out more than in the recording's: the modes' weight against the bands is
        # trimmed until the strongest three's share above 1 kHz is the recording's.
        r = []
        # (a drum's partials are its body, not a wash: its weight stays as measured)
        for p in [] if name in DRUMS else pieces:
            if p == "hat_pedal":
                continue
            for v in VELS:
                a, on_a = hit(M, p, v)
                b, on_b = hit(R, p, v)
                r.append(partials(b, on_b, p)[1] / max(partials(a, on_a, p)[1], 1e-6))
        k = float(np.clip(np.sqrt(np.median(r)), 0.5, 1.3)) if r else 1.0
        print(f"{name}: top-three ratio {np.round(r, 2)} -> modes x{k:.2f}")
        gains[f"{name}_MODES"] = round(gains.get(f"{name}_MODES", 1.0) * k, 3)
        # The ride's zones apart: its bow toward the recording's (a full step, the partials'
        # share goes as their power), its bell's tones kept as they are matched.
        if name == "RIDE" and r:
            bow = [x for x, p in zip(r, [p for p in pieces for _ in VELS]) if p == "ride"]
            kb = float(np.clip(np.median(bow), 0.4, 1.3)) if bow else 1.0
            gains["RIDE_MODES_0"] = round(gains.get("RIDE_MODES_0", 1.0) * kb / k, 3)
            print(f"RIDE bow: x{kb:.2f} (beyond the table's x{k:.2f})")
            # and each velocity's own (a soft ride's partials stand out more than a hard one's)
            for v, x in zip(VELS, bow):
                kv = float(np.clip(np.sqrt(x / max(kb, 1e-6)), 0.6, 1.4))
                key = f"RIDE_MODES_0_{v}"
                gains[key] = round(gains.get(key, 1.0) * kv, 3)
    json.dump(gains, open(path, "w"), indent=1)


if __name__ == "__main__":
    main()
