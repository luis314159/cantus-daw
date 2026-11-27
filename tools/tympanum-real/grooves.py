#!/usr/bin/env python3
"""The grooves measured (round 42, tympanum-real): the same human performance through every
kit, as a whole.

    python -I tools/tympanum-real/grooves.py <work> [--tags before,after] > table

Per render (`<work>/grooves/<tag>/<groove>--<kit>.wav`): integrated loudness (BS.1770),
the sample peak and the crest (peak − loudness), the loudness range (short-term 3-s loudness,
10th to 95th percentile, as EBU R128's LRA), the side/mid energy (width), the octave bands'
balance (dB re the whole), and the 6–16 kHz spectral flatness (the cymbals' texture). Written
to `<work>/grooves/measures.json`; the table printed groups each groove's references (their
range) against Tympanum's kits.
"""
import json
import sys
from pathlib import Path

import numpy as np
import soundfile as sf
from scipy import signal

sys.path.insert(0, str(Path(__file__).resolve().parent))
from preview import kweight, lufs  # noqa: E402

OCT = [63, 125, 250, 500, 1000, 2000, 4000, 8000, 16000]


def lra(x, fs):
    y = kweight(x, fs)
    n, h = int(3 * fs), int(0.1 * fs)
    st = np.array([-0.691 + 10 * np.log10(np.sum(np.mean(y[i:i + n] ** 2, axis=0)) + 1e-20)
                   for i in range(0, max(1, len(y) - n), h)])
    st = st[st > -70]
    if len(st) < 3:
        return float("nan")
    st = st[st > -0.691 + 10 * np.log10(np.mean(10 ** ((st + 0.691) / 10))) - 20]
    return float(np.percentile(st, 95) - np.percentile(st, 10))


def measure(p):
    x, fs = sf.read(str(p), dtype="float64", always_2d=True)
    m, s = 0.5 * (x[:, 0] + x[:, 1]), 0.5 * (x[:, 0] - x[:, 1])
    L = lufs(x, fs)
    pk = 20 * np.log10(np.abs(x).max() + 1e-20)
    f, P = signal.welch(m, fs, nperseg=8192)
    tot = P[(f > 20)].sum()
    bands = {}
    for c in OCT:
        k = (f >= c / 2 ** 0.5) & (f < c * 2 ** 0.5)
        bands[c] = float(10 * np.log10(P[k].sum() / tot + 1e-20))
    k = (f >= 6000) & (f < 16000)
    flat = float(np.exp(np.mean(np.log(P[k] + 1e-30))) / P[k].mean())
    return {"lufs": L, "peak": float(pk), "crest": float(pk - L), "lra": lra(x, fs),
            "side_db": float(10 * np.log10(np.sum(s ** 2) / (np.sum(m ** 2) + 1e-20) + 1e-20)),
            "flat_6_16": flat, **{f"oct_{c}": v for c, v in bands.items()}}


def main():
    work = Path(sys.argv[1])
    tags = sys.argv[sys.argv.index("--tags") + 1].split(",") if "--tags" in sys.argv else ["before"]
    res = {}
    for t in ["ref"] + tags:
        for p in sorted((work / "grooves" / t).glob("*.wav")):
            g, kit = p.stem.split("--")
            res.setdefault(g, {})[f"{kit}" if t == "ref" else f"{kit}-{t}"] = measure(p)
    (work / "grooves" / "measures.json").write_text(json.dumps(res, indent=1), encoding="utf-8")
    keys = ["crest", "lra", "side_db", "flat_6_16", "oct_63", "oct_250", "oct_1000", "oct_4000", "oct_8000"]
    print("| groove | measure | references (min … max) | " + " | ".join(f"{k}-{t}" for t in tags for k in ("studio", "jazz")) + " |")
    print("|---|---|---|" + "---|" * (2 * len(tags)))
    for g, d in res.items():
        refs = {k: v for k, v in d.items() if not any(k.endswith("-" + t) for t in tags)}
        for key in keys:
            rv = [v[key] for v in refs.values() if np.isfinite(v[key])]
            if not rv:
                continue
            cells = []
            for t in tags:
                for k in ("studio", "jazz"):
                    v = d.get(f"{k}-{t}", {}).get(key)
                    if v is None:
                        cells.append("—")
                    else:
                        out = v < min(rv) or v > max(rv)
                        cells.append(f"{v:.2f}{' **fuera**' if out else ''}")
            print(f"| {g} | {key} | {min(rv):.2f} … {max(rv):.2f} | " + " | ".join(cells) + " |")


if __name__ == "__main__":
    main()
