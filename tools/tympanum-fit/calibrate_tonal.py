"""A cymbal's partials against its wash, region by region, through the engine (round 40,
drums-gen).

    python -I calibrate_tonal.py <engine hits> <reference hits> <gains.json> [piece ...]

`fit.py` sets each region's partials against the noise in its own synthesis (Python, no
microphones); `calibrate.py` trims a table's partials as a whole by its strongest three.
Neither sees that through the engine a hard ride's 8-12 kHz turned to wash (flatness 0.26
against DRSKit's 0.11-0.14, fine-structure stability 0.24 against 0.50) while its strongest
partials were right. Here, per state and layer, the share of power standing 10 dB over the
local median (fit.tonal_frac's measure, in the fit's own window) in each of fit.REGIONS,
engine against recording; the partials of each region scaled by the square root of the
ratio, damped and clipped, into `<TABLE>_TONAL_<state>_<velocity>` (one factor per
region), which tables_blob.py applies to each mode by its frequency.
"""
import json
import sys
import numpy as np
sys.path.insert(0, __file__.rsplit("/", 1)[0].rsplit("\\", 1)[0])
from metrics import load, hit, VELS
from fit import tonal_frac, SPEC, REGIONS

# (drums-gen 2: the hat's closed and open strokes too — cue 7, the closed hat too tonal)
PARTS = {"CRASH": ["crash"], "RIDE": ["ride", "ride_bell"], "HAT": ["hat_closed", "hat_open"]}


def main():
    M, R = load(sys.argv[1]), load(sys.argv[2])
    path = sys.argv[3]
    want = sys.argv[4:]
    gains = json.load(open(path))
    from tables import TABLES
    for name, pieces in PARTS.items():
        states = next(parts for _, nm, parts in TABLES if nm == name)
        for p in pieces:
            if want and p not in want:
                continue
            si = [x[0] for x in states].index(p)
            a0, b0 = SPEC[p]["win"]
            for v in VELS:
                a, on_a = hit(M, p, v)
                b, on_b = hit(R, p, v)
                got = tonal_frac(a, on_a, a0, b0)
                ref = tonal_frac(b, on_b, a0, b0)
                k = np.clip(np.sqrt(ref / np.maximum(got, 1e-3)) ** 0.8, 0.6, 1.8)
                if p == "hat_closed":
                    # A closed stroke's 110 ms is too short for the share over the median to
                    # read its partials: its 4-12 kHz flatness (the analysis's cue 7) judges
                    # them — flatter than the recording, more partials; less, fewer.
                    from accept_b10 import tonal as flat_peaks
                    fg = flat_peaks(a, on_a, a0, b0)[0]
                    fr = flat_peaks(b, on_b, a0, b0)[0]
                    k = np.full(len(REGIONS), float(np.clip((fg / max(fr, 1e-3)) ** 1.5, 0.5, 1.5)))
                    got, ref = np.array([fg]), np.array([fr])
                # the regions below 1 kHz are the body, which calibrate.py's level holds
                k[:2] = 1.0
                key = f"{name}_TONAL_{si}_{v}"
                old = np.array(gains.get(key, [1.0] * len(REGIONS)))
                gains[key] = [round(float(x), 3) for x in np.clip(old * k, 0.3, 4.0)]
                print(f"{name} {p} v{v}: tonal share {np.round(got, 2)} (rec {np.round(ref, 2)}) "
                      f"-> x{np.round(k, 2)}")
    json.dump(gains, open(path, "w"), indent=1)


if __name__ == "__main__":
    main()
