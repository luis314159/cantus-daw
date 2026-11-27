"""The Paganini comparison's files to hear (round 42): each Arcus render dry and in a small
hall (Aula's own factory response "chamber", 1.3 s, cut under 160 Hz and over 8 kHz as the
demos' hall is — `paganini_render ir 5 160 8000`, convolved here path by path, true
stereo), everything at -23 LUFS integrated as 16-bit WAV, cut to the original's length; and
the short A/B: two or three bars of the original, then the same bars on Arcus.

    python -I tools/listening/paganini_mix.py <work dir> <out dir>
"""
import json
import os
import sys

import numpy as np
from scipy import signal

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import paganini_common as pc  # noqa: E402

# the hall's send: its wet signal this many dB under the dry (integrated loudness)
WET_DB = -9.0
# the A/B's excerpts in passage time (s): the theme's first two bars, the second half's
# first two bars, variation 2's first three bars
AB = [("tema-c1-2", 0.15, 2.40), ("tema-c9-10", 8.05, 10.15), ("var2-c1-3", 17.35, 20.35)]


def read(path):
    x, sr = pc.audio.read(path)
    assert sr == pc.SR, path
    return x


def hall(dry, ir):
    """True stereo: L = dry_L * LL + dry_R * RL, R = dry_L * LR + dry_R * RR."""
    ll, lr, rl, rr = (ir[:, i] for i in range(4))
    n = len(dry) + len(ir) - 1
    wl = signal.fftconvolve(dry[:, 0], ll)[:n] + signal.fftconvolve(dry[:, 1], rl)[:n]
    wr = signal.fftconvolve(dry[:, 0], lr)[:n] + signal.fftconvolve(dry[:, 1], rr)[:n]
    wet = np.stack([wl, wr], axis=1)[: len(dry)]
    g = WET_DB - (pc.lufs(wet, pc.SR) - pc.lufs(dry, pc.SR))
    return dry + wet * 10 ** (g / 20)


def fit(x, n):
    if len(x) >= n:
        x = x[:n].copy()
    else:
        x = np.concatenate([x, np.zeros((n - len(x), x.shape[1]))])
    f = int(0.05 * pc.SR)
    x[-f:] *= np.linspace(1, 0, f)[:, None]
    return x


def main(work, out):
    os.makedirs(out, exist_ok=True)
    orig = read(os.path.join(work, "pasaje-original.wav"))
    n = len(orig)
    ir = read(os.path.join(work, "aula-chamber.wav"))
    made = {}
    o, g = pc.to_lufs(orig, pc.SR)
    pc.write16(os.path.join(out, "original.wav"), o, pc.SR)
    made["original"] = o
    for ver in ("actual", "mejora"):
        for take in ("alineado", "partitura"):
            dry = read(os.path.join(work, f"{ver}-{take}-seco.wav"))
            room = hall(dry, ir)
            # the score's take keeps its own length (it is shorter); the aligned one the
            # original's, so the page can switch between them at the same moment
            m = n if take == "alineado" else len(dry)
            for kind, x in (("seco", dry), ("sala", room)):
                y, _ = pc.to_lufs(fit(x, m), pc.SR)
                name = f"arcus-{ver}-{take}-{kind}"
                pc.write16(os.path.join(out, name + ".wav"), y, pc.SR)
                made[name] = y
    # the A/B: the original's bars, 0.4 s of silence, the same bars on Arcus (aligned, hall)
    for ver in ("actual", "mejora"):
        for tag, a, b in AB:
            i0, i1 = int(a * pc.SR), int(b * pc.SR)
            f = int(0.03 * pc.SR)
            ramp = np.ones(i1 - i0)
            ramp[:f] = np.linspace(0, 1, f)
            ramp[-f:] = np.linspace(1, 0, f)
            seg_o = made["original"][i0:i1] * ramp[:, None]
            seg_a = made[f"arcus-{ver}-alineado-sala"][i0:i1] * ramp[:, None]
            gap = np.zeros((int(0.4 * pc.SR), 2))
            pc.write16(os.path.join(out, f"ab-{tag}-{ver}.wav"),
                       np.concatenate([seg_o, gap, seg_a]), pc.SR)
            if ver == "actual":
                pc.write16(os.path.join(out, f"ab-{tag}-original.wav"), seg_o, pc.SR)
            pc.write16(os.path.join(out, f"ab-{tag}-arcus-{ver}.wav"), seg_a, pc.SR)
    info = {k: dict(seconds=len(v) / pc.SR, lufs=round(pc.lufs(v, pc.SR), 2),
                    peak_db=round(pc.audio.peak_db(v), 2)) for k, v in made.items()}
    info["wet_db"] = WET_DB
    info["ab"] = AB
    info["original_gain_db"] = g
    json.dump(info, open(os.path.join(out, "archivos.json"), "w", encoding="utf-8"), indent=1)
    for k, v in info.items():
        print(k, v)


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
