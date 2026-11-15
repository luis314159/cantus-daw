"""A listen's files (round 40, drums-gen): pieces alone at four velocities — the previous
model, the new one and the recording, the three at the same loudness — and the groove
three ways.

    python -I make_listen.py <out dir> <prev hits> <new hits> <recorded hits>
        <prev groove> <new groove> <recorded groove> <kit tag> [piece,piece,...]

Files: `<kit>-<piece>-1-antes.flac`, `-2-nuevo`, `-3-grabado`; `<kit>-groove-…`.
"""
import os
import sys
import numpy as np
import soundfile as sf
sys.path.insert(0, __file__.rsplit("/", 1)[0].rsplit("\\", 1)[0])
from metrics import load, SR
from make_b4 import NAMES, cut

TAGS = ["1-antes", "2-nuevo", "3-grabado"]


def main():
    out = sys.argv[1]
    hits = [load(p) for p in sys.argv[2:5]]
    grooves = sys.argv[5:8]
    kit = sys.argv[8]
    only = sys.argv[9].split(",") if len(sys.argv) > 9 else list(NAMES)
    os.makedirs(out, exist_ok=True)
    for p in only:
        segs = [cut(x, p) for x in hits]
        ref = np.sqrt(np.mean(segs[2] ** 2))
        segs = [s * ref / max(np.sqrt(np.mean(s ** 2)), 1e-12) for s in segs]
        g = 10 ** (-3 / 20) / max(np.abs(s).max() for s in segs)
        for tag, s in zip(TAGS, segs):
            sf.write(os.path.join(out, f"{kit}-{NAMES[p]}-{tag}.flac"), s * g, SR, subtype="PCM_24")
    # the grooves: each already at -28 dBFS RMS (tympanum_measure); the same peak headroom
    xs = [sf.read(g, always_2d=True)[0] for g in grooves]
    g = min(1.0, 10 ** (-1 / 20) / max(np.abs(x).max() for x in xs))
    for tag, x in zip(TAGS, xs):
        sf.write(os.path.join(out, f"{kit}-groove-{tag}.flac"), x * g, SR, subtype="PCM_24")
    print("written", out)


if __name__ == "__main__":
    main()
