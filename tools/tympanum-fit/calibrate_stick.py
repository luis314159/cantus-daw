"""Each layer's stick through the engine (round 40, drums-gen 2): the contact's burst and
knock, levelled against the recording's first milliseconds as the twelve virtual
microphones hear them.

    python -I calibrate_stick.py <engine hits> <reference hits> <gains.json> [piece ...]

`fit.py` fits the stick on the recording's mono mix alone; through the engine the close
mic, the overheads 3-4 ms later and the room add their own. The gap analysis (cues 6, 9,
12) found the studio kick's click 14 dB too strong at v127 (the jazz kick's, a felt
beater's, 16-23 dB), the crash's first 3 ms dull (its centroid 1-3 kHz against 4-6), the
cymbals' first millisecond spiking. Per piece and layer, a damped step on

- the **burst** by the highs of the first 3 ms (6-20 kHz; the kick's click, 2-8 kHz in its
  first 8 ms, against its body 40-400 Hz there),
- the **knock** by the first 3 ms at 0.2-1.5 kHz,

each against the hit's 0-50 ms energy (the level is calibrate.py's), into
`<TABLE>_STICK_<state>_<velocity>` = [burst, knock] factors that tables_blob.py applies
(each step ×0.5-2, in all ×0.05-4).
"""
import json
import sys
import numpy as np
from scipy import signal
sys.path.insert(0, __file__.rsplit("/", 1)[0].rsplit("\\", 1)[0])
from metrics import load, hit, SR, VELS
from tables import TABLES

STEP = 0.7
SKIP = {"hat_pedal"}


def e(m, on, lo, hi, a_ms, b_ms):
    sos = signal.butter(4, [lo, min(hi, 0.47 * SR)], btype="band", fs=SR, output="sos")
    y = signal.sosfiltfilt(sos, m)[on + int(a_ms * SR / 1000):on + int(b_ms * SR / 1000)]
    return 10 * np.log10(np.mean(y * y) + 1e-30)


def whole(m, on):
    y = m[on:on + int(0.05 * SR)]
    return 10 * np.log10(np.mean(y * y) + 1e-30)


def reads(m, on, kick):
    w = whole(m, on)
    if kick:
        burst = e(m, on, 2000, 8000, 0, 8) - e(m, on, 40, 400, 0, 8)
    else:
        burst = e(m, on, 6000, 20000, 0, 3) - w
    return burst, e(m, on, 200, 1500, 0, 3) - w


def main():
    M, R = load(sys.argv[1]), load(sys.argv[2])
    path = sys.argv[3]
    want = sys.argv[4:]
    gains = json.load(open(path))
    for _, name, parts in TABLES:
        for si, (p, _, _) in enumerate(parts):
            if p in SKIP or (want and p not in want) or p.startswith("hat_o"):
                continue
            for v in VELS:
                a, on_a = hit(M, p, v)
                b, on_b = hit(R, p, v)
                g, r = reads(a, on_a, p == "kick"), reads(b, on_b, p == "kick")
                key = f"{name}_STICK_{si}_{v}"
                old = gains.get(key, [1.0, 1.0])
                step = [float(np.clip(10 ** (STEP * (r[i] - g[i]) / 20), 0.5, 2.0)) for i in range(2)]
                if p == "kick":
                    # a kick's first 3 ms at 0.2-1.5 kHz is its head, not the beater: its knock
                    # (whose skirt is most of the click) follows the click with the burst
                    step[1] = step[0]
                new = [round(float(np.clip(old[i] * step[i], 0.05, 4.0)), 4) for i in range(2)]
                gains[key] = new
                print(f"{name} {p} v{v}: burst {g[0]:+6.1f} rec {r[0]:+6.1f} -> x{new[0]:.2f}; "
                      f"knock {g[1]:+6.1f} rec {r[1]:+6.1f} -> x{new[1]:.2f}")
    json.dump(gains, open(path, "w"), indent=1)


if __name__ == "__main__":
    main()
