"""Each drum layer's colour through the engine, band by band (round 40, drums-gen).

    python -I calibrate_bands.py <engine hits> <reference hits> <gains.json> [piece ...]

The tables are fitted to the recording's mono mix; the engine plays them through its
twelve virtual microphones, whose sum has a colour of its own — each mic's pattern off
axis, the proximity of the close ones, a head's highs beamed along its normal, the paths'
combs — and that colour changes over the hit (the close mic's attack first, the room
after). `calibrate.py` corrects the level; this corrects the colour: per quarter-octave
band (the tables' own) and per time region of the hit — the attack (0-12 ms), the body
(12-135 ms), the tail (200 ms on) — the recording's band energy against the engine's, a
damped step of the difference, smoothed over three bands, a band too narrow for its
window left alone (clipped to ±4 dB a step, ±10 in all), into
`<TABLE>_BAND_<state>_<velocity>` (a list, band-major: NB × 3), which tables_blob.py adds
to the layer's envelopes at the breakpoints of each region.
"""
import json
import sys
import numpy as np
from scipy import signal
sys.path.insert(0, __file__.rsplit("/", 1)[0].rsplit("\\", 1)[0])
from metrics import load, hit, SR, VELS
from fit import CENTRES, edges

# Breakpoints (ms) each region covers, and the window (s) it is measured in.
REGIONS = [((0, 12), (0.0, 0.012)), ((18, 135), (0.012, 0.135)), ((200, 1e9), (0.15, 0.8))]
PARTS = {"KICK": ["kick"], "SNARE": ["snare", "rimshot", "sidestick"], "TOM_HI": ["tom_hi"],
         "TOM_MID": ["tom_mid"], "TOM_FLOOR": ["tom_floor"]}
STEP, MAX_STEP, MAX_ALL = 0.6, 4.0, 10.0


def band_energies(m, on):
    out = np.full((len(CENTRES), len(REGIONS)), np.nan)
    for k, c in enumerate(CENTRES):
        lo, hi = edges(c)
        if hi > 0.47 * SR:
            continue
        sos = signal.butter(3, [lo, hi], btype="band", fs=SR, output="sos")
        y = signal.sosfiltfilt(sos, m)
        for r, (_, (a, b)) in enumerate(REGIONS):
            seg = y[on + int(a * SR):on + int(b * SR)]
            out[k, r] = 10 * np.log10(np.mean(seg * seg) + 1e-30)
    return out


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
            for v in VELS:
                a, on_a = hit(M, p, v)
                b, on_b = hit(R, p, v)
                em, er = band_energies(a, on_a), band_energies(b, on_b)
                d = er - em
                # bands far under the hit's loudest in that region are noise floor: left be
                for r in range(len(REGIONS)):
                    floor = np.nanmax(er[:, r]) - 60.0
                    d[er[:, r] < floor, r] = 0.0
                # a band too narrow for the window (fewer than 4 of its periods in it)
                # reads noise, not colour: left be
                for r, (_, (a0, b0)) in enumerate(REGIONS):
                    for k, c in enumerate(CENTRES):
                        if c * (b0 - a0) < 4.0:
                            d[k, r] = 0.0
                d = np.nan_to_num(d)
                # one hit's quarter-octave readings scatter: the colour is their trend
                # across three bands
                d = np.array([np.convolve(np.pad(d[:, r], 1, mode="edge"), np.ones(3) / 3,
                                          mode="valid") for r in range(len(REGIONS))]).T
                # the level is calibrate.py's: only the colour (the mean over bands, energy-
                # weighted, taken out per region)
                for r in range(len(REGIONS)):
                    wgt = 10 ** (er[:, r] / 10)
                    wgt = np.nan_to_num(wgt) / max(np.nansum(wgt), 1e-30)
                    d[:, r] -= np.sum(wgt * d[:, r])
                step = np.clip(STEP * d, -MAX_STEP, MAX_STEP)
                key = f"{name}_BAND_{si}_{v}"
                old = np.array(gains.get(key, [0.0] * (len(CENTRES) * len(REGIONS)))).reshape(len(CENTRES), len(REGIONS))
                new = np.clip(old + step, -MAX_ALL, MAX_ALL)
                gains[key] = [round(float(x), 2) for x in new.reshape(-1)]
                hf = [i for i, c in enumerate(CENTRES) if c >= 4000]
                print(f"{name} {p} v{v}: step dB (attack/body/tail) above 4 kHz "
                      f"{np.mean(step[hf, 0]):+.1f} {np.mean(step[hf, 1]):+.1f} {np.mean(step[hf, 2]):+.1f}; "
                      f"below 200 Hz {np.mean(step[:11, 0]):+.1f} {np.mean(step[:11, 1]):+.1f} {np.mean(step[:11, 2]):+.1f}")
    json.dump(gains, open(path, "w"), indent=1)


if __name__ == "__main__":
    main()
