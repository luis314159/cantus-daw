"""The measured voice in Python: the same algorithm the engine runs (pulsus_measured.rs),
for quick listening and measuring before the Rust port.

    python -I synth.py <tables.json> <base hits.flac> <out hits.flac> [seed]

Renders every piece the tables hold, at the hits file's four velocities, into a copy of
the base hits file (the other slots untouched).
"""
import json
import sys
import numpy as np
import soundfile as sf
from scipy import signal
sys.path.insert(0, __file__.rsplit("/", 1)[0].rsplit("\\", 1)[0])
from metrics import SR, PIECES, VELS


def bandpass(c, q):
    w = 2 * np.pi * c / SR
    al = np.sin(w) / (2 * q)
    b = np.array([al, 0, -al]) / (1 + al)
    a = np.array([1, -2 * np.cos(w) / (1 + al), (1 - al) / (1 + al)])
    return b, a


Q = 1 / (2 ** (1 / 8) - 2 ** (-1 / 8))
# Each band is SECTIONS identical band-passes in a row (skirts 6 dB/octave each): a soft
# cymbal's strong mids must not leak into its quiet highs. Their Q keeps the bandwidth.
SECTIONS = 2
QS = Q * {1: 1.0, 2: 0.644, 3: 0.510}[SECTIONS]


def band_filter(c, x):
    b, a = bandpass(c, QS)
    for _ in range(SECTIONS):
        x = signal.lfilter(b, a, x)
    return x


def band_norm(c):
    imp = np.zeros(1 << 15)
    imp[0] = 1
    h = band_filter(c, imp)
    return 1 / np.sqrt((h * h).sum())


def env_curve(db, bp_ms, n):
    t = np.arange(n) / SR * 1000
    return 10 ** (np.interp(t, bp_ms, db, right=np.nan) / 20)


def layer_at(layers, vel):
    vs = [l["vel"] for l in layers]
    if vel <= vs[0]:
        lo = hi = 0
        w = 0.0
    elif vel >= vs[-1]:
        lo = hi = len(vs) - 1
        w = 0.0
    else:
        hi = next(i for i, v in enumerate(vs) if v >= vel)
        lo = hi - 1
        w = (vel - vs[lo]) / (vs[hi] - vs[lo])
    env = (1 - w) * np.array(layers[lo]["env"]) + w * np.array(layers[hi]["env"])
    near = layers[lo] if w < 0.5 else layers[hi]
    return env, near["modes"]


def stick(amps, vel, n, rng):
    """The stick's contact: a half-sine of noise (broadband, shorter the harder) and a dull
    knock (a 2 ms half-sine of noise through two one-pole low-passes at 1.2 kHz)."""
    y = np.zeros(n)
    tc = 0.0004 * (1.4 - 0.6 * vel / 127)
    k = max(2, int(tc * SR))
    y[:k] = amps[0] * np.sin(np.pi * np.arange(k) / k) * rng.uniform(-1, 1, k) * np.sqrt(3)
    k2 = int(0.002 * SR)
    knock = np.sin(np.pi * np.arange(k2) / k2) * rng.uniform(-1, 1, k2) * np.sqrt(3)
    a1 = np.exp(-2 * np.pi * 1200 / SR)
    knock = signal.lfilter([1 - a1], [1, -a1], knock)
    y[:k2] += amps[1] * signal.lfilter([1 - a1], [1, -a1], knock)
    return y


def render(tab, piece, vel, n, rng):
    env, modes = layer_at(tab["pieces"][piece], vel)
    bp = np.array(tab["bp_ms"], dtype=float)
    noise = rng.uniform(-1, 1, n) * np.sqrt(3)
    out = np.zeros(n)
    for k, c in enumerate(tab["centres"]):
        if c * 2 ** (1 / 8) > 0.47 * SR or env[k].max() < -150:
            continue
        y = band_filter(c, noise) * band_norm(c)
        db = env[k] + rng.normal(0, 0.7)
        # the power envelope (dB) is 10 log10 p: amplitude = sqrt(p)
        e = 10 ** (np.interp(np.arange(n) / SR * 1000, bp, db) / 20)
        # past the last breakpoint, the last slope continues
        last = bp[-1]
        slope = (db[-1] - db[-2]) / (bp[-1] - bp[-2])
        t = np.arange(n) / SR * 1000
        e = np.where(t > last, 10 ** ((db[-1] + slope * (t - last)) / 20), e)
        out += y * e
    t = np.arange(n) / SR
    lays = tab["pieces"][piece]
    nl = min(lays, key=lambda l: abs(l["vel"] - vel))
    gl_g, gl_tau = nl.get("glide", [0.0, 0.05])
    # a drum's onset (early.py): its phases and early parts, the nearest layer's
    onset = nl.get("ets", 0.0) > 0 and len(nl.get("ph", [])) == len(modes)
    for j, (hz, amp, t60, rise, direct) in enumerate(modes):
        ph = nl["ph"][j] if onset else rng.uniform(0, 2 * np.pi)
        g = amp * 10 ** (rng.normal(0, 1.5) / 20)
        e = np.exp(-6.9078 * t / t60)
        if rise > 0 and not onset:
            e = e - (1 - direct) * np.exp(-t * (6.9078 / t60 + 1 / rise))
        arg = 2 * np.pi * hz * (t + gl_g * gl_tau * (1 - np.exp(-t / gl_tau)))
        out += g * e * np.cos(arg + ph)
        if onset:
            out += nl["ea"][j] * np.exp(-t / nl["ets"]) * np.cos(arg + nl["ep"][j])
    lays = tab["pieces"][piece]
    near = min(lays, key=lambda l: abs(l["vel"] - vel))
    st = near.get("stick", [0.0, 0.0])
    if max(st) > 0:
        out += stick(st, vel, n, rng)
    return out


def main():
    tab = json.load(open(sys.argv[1]))
    x, sr = sf.read(sys.argv[2], always_2d=True)
    seed = int(sys.argv[4]) if len(sys.argv) > 4 else 1
    rng = np.random.default_rng(seed)
    for p in [q for q in tab["pieces"] if q in PIECES]:
        for v in VELS:
            i = PIECES.index(p) * 4 + VELS.index(v)
            a = int(i * 4 * SR)
            # the onset where the base file's hit starts (the same note time)
            seg = np.abs(x[a:a + 4 * SR]).sum(1)
            on = int(np.argmax(seg > 0.05 * seg.max())) if seg.max() > 0 else int(0.25 * SR)
            n = 4 * SR - on
            y = render(tab, p, v, n, rng)
            x[a:a + 4 * SR] = 0
            x[a + on:a + 4 * SR, 0] = y
            x[a + on:a + 4 * SR, 1] = y
    sf.write(sys.argv[3], x, SR, subtype="PCM_24")


if __name__ == "__main__":
    main()
