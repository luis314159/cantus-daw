"""The hi-hat's states, each layer's partials against its noise in the first 170 ms
(round 40, drums-gen).

    python -I calibrate_hat.py <gains.json> <state>=<model hits>,<reference hits> ...

`state` is the hat table's state index (0 closed, 1-3 a quarter, half and three quarters
open, 4 open; tables.py's order); the model hits the engine's render of that state
(`tympanum_measure hits`, `TYMP_HAT_OPEN` for 1-3), the reference the recorded kit's at
the same opening. Closed is read from the `hat_closed` slot, the others from `hat_open`.

Why: `calibrate.py` matches a hat's strongest partials 0.1-0.6 s after the strike — where
a hit alone is judged — and never saw the half-open states. In a groove the hat is struck
every few hundred ms, so what the ear (and the groove's measure) hears is each stroke's
first 170 ms; there a quarter-open stroke's partials stood out of its noise 3-7 times more
than the recording's (6-16 kHz effective components 5-11 per kHz against 28-37), while
from 200 ms on they stood out less: the partials too strong and dying too soon. So two
trims per state and layer, from the effective components of 8192-sample frames (the groove
measure's) against the recording's: the partials' level from the frames in the first 50 ms
(`HAT_MODES_<state>_<velocity>`) and their T60s from the frames 200 ms on
(`HAT_T60_<state>_<velocity>`). tables_blob.py applies both. (Letting a partial grow in
instead — a smaller share struck at once — does not reach a 170-ms frame: tried, no change.)
"""
import json
import sys
import numpy as np
sys.path.insert(0, __file__.rsplit("/", 1)[0].rsplit("\\", 1)[0])
from metrics import load, hit, SR, VELS


def comps(m, on, t0=0.0, n=8192):
    a = on + int(t0 * SR)
    w = m[a:a + n]
    P = np.abs(np.fft.rfft(w * np.hanning(len(w)))) ** 2
    f = np.fft.rfftfreq(len(w), 1 / SR)
    q = P[(f >= 6000) & (f < 16000)]
    return float(q.sum() ** 2 / (q ** 2).sum() / 10)


def early(m, on):
    """The mean of the effective components in frames from the strike to 100 ms."""
    return float(np.mean([comps(m, on, t) for t in (0.0, 0.02, 0.05, 0.1)]))


def main():
    path = sys.argv[1]
    gains = json.load(open(path))
    name = "HAT"
    for arg in sys.argv[2:]:
        si, files = arg.split("=")
        fm, fr = files.split(",")
        xm, xr = load(fm), load(fr)
        piece = "hat_closed" if si == "0" else "hat_open"
        for v in VELS:
            a, on_a = hit(xm, piece, v)
            b, on_b = hit(xr, piece, v)
            # the stroke's first 50 ms: how much of its partials ring at once
            em = np.mean([comps(a, on_a, t) for t in (0.0, 0.02, 0.05)])
            er = np.mean([comps(b, on_b, t) for t in (0.0, 0.02, 0.05)])
            # from 200 ms on: the partials' level against the noise
            lm = np.mean([comps(a, on_a, t) for t in (0.2, 0.4)])
            lr = np.mean([comps(b, on_b, t) for t in (0.2, 0.4)])
            # Too tonal early and too noisy late: the partials too strong and dying too
            # soon — weaker (from the early frames) and longer (from the late ones).
            ka = float(np.clip((em / er) ** 0.6, 0.4, 1.5))
            kt = float(np.clip((lm / lr) ** 0.6, 0.6, 2.5))
            ka_key, kt_key = f"{name}_MODES_{si}_{v}", f"{name}_T60_{si}_{v}"
            gains[ka_key] = round(gains.get(ka_key, 1.0) * ka, 3)
            gains[kt_key] = round(float(np.clip(gains.get(kt_key, 1.0) * kt, 0.5, 4.0)), 3)
            gains.pop(f"{name}_DIRECT_{si}_{v}", None)
            print(f"state {si} v{v}: early {em:5.1f} (rec {er:5.1f}) modes x{ka:.2f} = {gains[ka_key]};"
                  f"  late {lm:5.1f} (rec {lr:5.1f}) T60 x{kt:.2f} = {gains[kt_key]}")
    json.dump(gains, open(path, "w"), indent=1)


if __name__ == "__main__":
    main()
