"""The hi-hat's partials against its noise over a stroke, fitted in the synthesis (round 40,
drums-gen).

    python -I fit_hat_tonal.py <fit.json> <gains.json> <state>=<reference hits> ...

In a groove the hat is struck every few hundred ms, and what the ear (and the groove's
measure, analysis-drums s06) hears is each stroke's frames of 8192 samples: how many
components stand in its 6-16 kHz. A quarter-open stroke's partials, as fitted, swamped its
first 170 ms (5-11 effective components per kHz against DRSKit's 28-35) and were gone by
200 ms (56 against 30): too strong and too short. Trimming them through the engine one
step at a time did not converge (a longer ring adds to the first frame too), so here each
state's layer is fitted in Python's synthesis (synth.py, the engine's algorithm without
the microphones) on a grid: the partials' level `ka` and their T60s' factor `kt` whose
frames at 0, 50, 200 and 400 ms carry the recording's effective components (least squares
in log). Written as `HAT_MODES_<state>_<velocity>` (net of the table's own trims, so the
product is `ka`) and `HAT_T60_<state>_<velocity>`; the engine's own colour is left to
`calibrate.py`'s level and checked by the groove.

`state`: 0 closed (the `hat_closed` slot), 1-3 a quarter, half, three quarters open, 4 open
(the `hat_open` slot of a render at that opening).
"""
import copy
import json
import sys
import numpy as np
sys.path.insert(0, __file__.rsplit("/", 1)[0].rsplit("\\", 1)[0])
import synth
from metrics import load, hit, SR, VELS

STATES = ["hat_closed", "hat_o25", "hat_o50", "hat_o75", "hat_open"]
TIMES = (0.0, 0.05, 0.2, 0.4)
KA = np.geomspace(0.03, 2.0, 14)
KT = [0.5, 0.7, 1.0, 1.4, 2.0, 2.8]


def comps(m, a):
    w = m[a:a + 8192]
    P = np.abs(np.fft.rfft(w * np.hanning(len(w)))) ** 2
    f = np.fft.rfftfreq(len(w), 1 / SR)
    q = P[(f >= 6000) & (f < 16000)]
    return float(q.sum() ** 2 / (q ** 2).sum() / 10)


def profile(m, on):
    return np.array([comps(m, on + int(t * SR)) for t in TIMES])


def synth_profile(fit, piece, vel, ka, kt):
    t = copy.deepcopy(fit)
    for lay in t["pieces"][piece]:
        lay["modes"] = [[md[0], md[1] * ka, md[2] * kt] + list(md[3:]) for md in lay["modes"]]
    return np.mean([profile(synth.render(t, piece, vel, SR, np.random.default_rng(s)), 0)
                    for s in (1, 2)], axis=0)


def main():
    fit = json.load(open(sys.argv[1]))
    path = sys.argv[2]
    gains = json.load(open(path))
    for arg in sys.argv[3:]:
        si, ref = arg.split("=")
        si = int(si)
        piece = STATES[si]
        x = load(ref)
        slot = "hat_closed" if si == 0 else "hat_open"
        for v in VELS:
            m, on = hit(x, slot, v)
            want = profile(m, on)
            best = None
            for ka in KA:
                for kt in KT:
                    got = synth_profile(fit, piece, v, ka, kt)
                    e = float(np.sum(np.log(got / want) ** 2))
                    if best is None or e < best[0]:
                        best = (e, ka, kt, got)
            e, ka, kt, got = best
            net = ka / (gains.get("HAT_MODES", 1.0) * gains.get(f"HAT_MODES_{si}", 1.0))
            gains[f"HAT_MODES_{si}_{v}"] = round(float(net), 4)
            gains[f"HAT_T60_{si}_{v}"] = round(float(kt), 3)
            gains.pop(f"HAT_DIRECT_{si}_{v}", None)
            print(f"{piece} v{v}: recording {np.round(want, 1)}  synthesis {np.round(got, 1)}"
                  f"  -> partials x{ka:.3f}, T60 x{kt}")
    json.dump(gains, open(path, "w"), indent=1)


if __name__ == "__main__":
    main()
