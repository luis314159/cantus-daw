"""The worked example's sounds: a hypothetical struck bar ("Tibia"), synthetic, so the next
builder sees a subject's whole shape without downloading anything.

    python -I synth_bar.py refs <out dir>        three "recorded" bars (the references)
    python -I synth_bar.py candidate <out dir>   the candidate's renders (what a cargo example would write)

A bar's partials sit at 1 : 3.93 : 9.2 (a tuned marimba-like bar), each decaying with its own
T60, a short mallet click (noise, 3 ms); every note at p, mf, f. The three reference bars
differ a little (their decays, their partials' levels); the candidate decays too long and is
brighter — what the report must find out of the band. CC0: written by this script.
"""
import sys
from pathlib import Path

import numpy as np
import soundfile as sf

SR = 48000
NOTES = list(range(60, 85, 2))
DYN = {"p": 0.35, "mf": 0.6, "f": 0.9}
RATIOS = (1.0, 3.93, 9.2)


def bar(note, vel, t60_f1, bright, seed, dur=2.5):
    rng = np.random.default_rng(seed)
    f0 = 440.0 * 2 ** ((note - 69) / 12)
    t = np.arange(int(dur * SR)) / SR
    x = np.zeros_like(t)
    for k, r in enumerate(RATIOS):
        f = f0 * r
        if f > 0.45 * SR:
            break
        level = 10 ** ((-14 * k + bright * k * vel) / 20)
        t60 = t60_f1 * (1.0 - 0.15 * (note - 60) / 24) / (1 + 2.5 * k)
        x += level * np.exp(-6.91 * t / t60) * np.sin(2 * np.pi * f * t + rng.uniform(0, 2 * np.pi))
    click = rng.standard_normal(len(t)) * np.exp(-t / 0.003) * 0.3 * vel
    y = vel * (x + click)
    pre = np.zeros(int(0.05 * SR))
    return 0.3 * np.concatenate([pre, y]) + 1e-6 * rng.standard_normal(len(pre) + len(y))


def phrase(t60_f1, bright, seed):
    """A short phrase: eight notes, 0.3 s apart, mf with an accent."""
    out = np.zeros(int(4.0 * SR))
    for i, (n, v) in enumerate(zip([72, 74, 76, 79, 76, 74, 72, 67], [0.6, 0.5, 0.6, 0.9, 0.6, 0.5, 0.6, 0.7])):
        y = bar(n, v, t60_f1, bright, seed + i, dur=1.6)
        a = int((0.1 + 0.3 * i) * SR)
        out[a:a + len(y)] += y[: len(out) - a]
    return out


def main(what, out):
    out = Path(out)
    out.mkdir(parents=True, exist_ok=True)
    if what == "refs":
        for b, (t60, bright) in enumerate([(1.4, 6.0), (1.1, 8.0), (1.7, 5.0)]):
            for n in NOTES:
                for d, v in DYN.items():
                    sf.write(out / f"bar{b + 1}-{n}-{d}.wav", bar(n, v, t60, bright, 100 * b + n), SR, subtype="PCM_24")
            sf.write(out / f"bar{b + 1}-phrase.wav", phrase(t60, bright, 7 * b), SR, subtype="PCM_24")
    else:
        for n in NOTES:
            for d, v in DYN.items():
                sf.write(out / f"tibia-{n}-{d}.wav", bar(n, v, 3.2, 12.0, 900 + n), SR, subtype="PCM_24")
        sf.write(out / "tibia-phrase.wav", phrase(3.2, 12.0, 99), SR, subtype="PCM_24")
    print(f"{what}: written to {out}")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
