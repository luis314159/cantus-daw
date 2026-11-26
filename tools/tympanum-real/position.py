#!/usr/bin/env python3
"""Where the stick lands, measured off recorded snares (round 42, tympanum-real).

    python -I tools/tympanum-real/position.py <content dir> <out.json>
    python -I tools/tympanum-real/position.py --tympanum <hits.jsonl …> (the same measures of renders)

Centre against edge (Big Rusty's 14×8 snare: `center` 10 layers, `edge` 7, four takes each;
Virtuosity's: `center` against `offcenter`, up to 16 layers), read from the packs' own sample
files — the close top mic plus the overheads, summed as recorded — and each stroke measured by
`features.features`. Per layer index (soft → hard, the two articulations' layers matched by
their place in their own range) the asymmetric modes' share (`upper_db`), the fundamental, the
centroid at 20–80 ms, the 2 kHz decay, the level. What a membrane does (Rossing, *Science of
Percussion Instruments*, ch. 2–3; Fletcher & Rossing §18): a centre stroke drives the (0,1)
family and little else, an edge stroke the (1,1), (2,1), (3,1) … nodal-diameter modes — more
energy above the fundamental, a brighter, ringier, less "thick" sound.
"""
import glob
import json
import re
import sys
from pathlib import Path

import numpy as np
import soundfile as sf

sys.path.insert(0, str(Path(__file__).resolve().parent))
from features import features  # noqa: E402

SETS = {
    "bigrusty": {
        "dir": "big-rusty-drums/Samples/snare_14",
        "arts": {"center": "center", "edge": "edge"},
        "mics": ["top", "oh"],
        "pat": "{art}/{mic}/sn_{art}_vl{v}_rr*.flac",
    },
    "virtuosity": {
        "dir": "virtuosity-drums/Samples",
        "arts": {"center": "center", "edge": "offcenter"},
        "mics": ["snaremic", "oh"],
        "pat": "{mic}/snare/{mic}_snare_{art}_vl{v}.flac",
    },
    # the same strokes with the wires off (the head's modes alone), and a rack tom
    "virtuosity-wires-off": {
        "dir": "virtuosity-drums/Samples",
        "arts": {"center": "center", "edge": "offcenter"},
        "mics": ["snaremic", "oh"],
        "pat": "{mic}/snareoff/{mic}_snareoff_{art}_vl{v}.flac",
    },
    "virtuosity-tom": {
        "dir": "virtuosity-drums/Samples",
        "arts": {"center": "center", "edge": "offcenter"},
        "mics": ["snaremic", "oh"],
        "pat": "{mic}/htom/{mic}_htom_{art}_vl{v}.flac",
        "kind": "racktom",
    },
}


def asym_db(st, fs, f0):
    """The (1,1) mode's region (1.35–1.75 × f0) over the fundamental's (± 12 %), 20–200 ms:
    an edge stroke drives the nodal-diameter modes a centre stroke leaves still."""
    m = st.mean(axis=1)
    m = m[int(0.02 * fs):int(0.2 * fs)]
    X = np.abs(np.fft.rfft(m * np.hanning(len(m)), 1 << 15)) ** 2
    f = np.fft.rfftfreq(1 << 15, 1 / fs)
    e = lambda a, b: X[(f >= a * f0) & (f < b * f0)].sum() + 1e-30  # noqa: E731
    return float(10 * np.log10(e(1.35, 1.75) / e(0.88, 1.12)))


def stereo(x):
    return x if x.ndim == 2 and x.shape[1] == 2 else np.stack([x.reshape(-1)] * 2, axis=1) if x.ndim == 1 \
        else np.stack([x[:, 0]] * 2, axis=1)


def layers(root, s, art):
    out = {}
    for v in range(1, 40):
        takes = None
        for mic in s["mics"]:
            files = sorted(glob.glob(str(root / s["dir"] / s["pat"].format(art=art, mic=mic, v=v))))
            if not files:
                continue
            got = []
            for f in files:
                x, fs = sf.read(f, dtype="float64", always_2d=True)
                got.append(stereo(x))
            if takes is None:
                takes = got
            else:
                takes = [a[: min(len(a), len(b))] + b[: min(len(a), len(b))] for a, b in zip(takes, got)]
        if takes:
            out[v] = (takes, fs)
    return out


def measure(root):
    res = {}
    for name, s in SETS.items():
        res[name] = {}
        for label, art in s["arts"].items():
            ls = layers(root, s, art)
            rows = []
            n = len(ls)
            for i, (v, (takes, fs)) in enumerate(sorted(ls.items())):
                kind = s.get("kind", "snare")
                fs_ = [features(t, fs, kind) for t in takes]
                fs_ = [f for f in fs_ if f]
                for t, f in zip(takes, fs_):
                    if isinstance(f.get("f0"), float) and np.isfinite(f["f0"]):
                        f["asym_db"] = asym_db(t, fs, f["f0"])
                row = {"layer": v, "place": i / max(1, n - 1)}
                for k in ("upper_db", "asym_db", "f0", "cent_c", "t30_2000", "rms80_db", "wires_db", "r2", "glide_c"):
                    vals = [f.get(k) for f in fs_ if isinstance(f.get(k), float) and np.isfinite(f.get(k))]
                    row[k] = float(np.median(vals)) if vals else None
                rows.append(row)
            res[name][label] = rows
            print(name, label, len(rows), "layers")
    return res


def main():
    root, out = Path(sys.argv[1]), Path(sys.argv[2])
    res = measure(root)
    out.write_text(json.dumps(res, indent=1), encoding="utf-8")
    for name, r in res.items():
        for label in ("center", "edge"):
            rows = r[label]
            print(f"{name:10} {label:6} " + " ".join(
                f"{x['place']:.1f}:{x['asym_db'] if x['asym_db'] is not None else float('nan'):+.1f}/{x['cent_c'] or 0:.0f}"
                for x in rows))


if __name__ == "__main__":
    main()
