"""strokes.py - Arcus pass 5: what a bow does between notes, measured the same way on the
recordings and on Arcus's renders.

- **The bow change's tick** (`tick`): at each note boundary of a phrase (the level's dip
  between two notes), the high band (4-12 kHz, a fourth-order Butterworth band-pass) in 3 ms
  frames: its loudest frame within 15 ms of the dip over the median frame of the 60-20 ms
  before it, dB. A change that clicks stands over the string's own high end; a clean one
  does not.
- **The spiccato's bounce** (`bounce`): each stroke's level (5 ms RMS) from its peak - the
  dB it has fallen 40, 80, 150 and 250 ms after the peak - and the share of the stroke
  spent within 6 dB of its peak (`held_ms`).

    python -I tools/arcus/pass5/strokes.py tick <file> ...
    python -I tools/arcus/pass5/strokes.py bounce <file> ...
    python -I tools/arcus/pass5/strokes.py refs <downloads> <out.json>

`refs` measures the references (Philharmonia's detache and spiccato phrases, every family;
VSCO's solo violin and solo contrabass spiccato strokes) and writes the distributions. A
`.f32` file is raw little-endian float32 at 48 kHz (the probes' `ARCUS_DUMP`).
"""
import json
import sys
from pathlib import Path

import numpy as np
from scipy.signal import butter, sosfilt

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "study"))
from bowlib_core import load, rms_db  # noqa: E402


def read(path):
    p = str(path)
    if p.endswith(".f32"):
        return np.fromfile(p, dtype="<f4").astype(np.float64), 48000
    return load(p)


def frames_db(x, sr, win_s, hop_s):
    t, d = rms_db(x, sr, win_s, hop_s)
    return t, d


def boundaries(x, sr):
    """The note boundaries of a phrase: the minima of the 10 ms level between the onsets,
    where the level rises 6 dB within 40 ms after a dip of 4 dB or more."""
    t, d = frames_db(x, sr, 0.01, 0.0025)
    peak = np.percentile(d, 99)
    out = []
    i = 8
    n = len(d)
    while i < n - 16:
        # a local minimum, 4 dB under the 40 ms before and 4 dB under the 40 ms after
        if d[i] == d[max(i - 8, 0):i + 9].min() and d[i] > peak - 45:
            before = d[max(i - 16, 0):i].max()
            after = d[i + 1:i + 17].max()
            if before - d[i] >= 4 and after - d[i] >= 4:
                out.append(t[i])
                i += 16
                continue
        i += 1
    return out


def tick_at(x, sr, tb, hp):
    t, d = frames_db(hp, sr, 0.003, 0.001)
    near = (t >= tb - 0.015) & (t <= tb + 0.015)
    pre = (t >= tb - 0.060) & (t <= tb - 0.020)
    if near.sum() < 3 or pre.sum() < 5:
        return None
    return float(d[near].max() - np.median(d[pre]))


def band(x, sr):
    sos = butter(4, [4000, min(12000, 0.45 * sr)], btype="band", fs=sr, output="sos")
    return sosfilt(sos, x)


def ticks(path):
    x, sr = read(path)
    hp = band(x, sr)
    out = []
    for tb in boundaries(x, sr):
        v = tick_at(x, sr, tb, hp)
        if v is not None:
            out.append(v)
    return out


def bounce(path):
    """Each stroke of a file: its fall after the peak at 40, 80, 150, 250 ms, and how long it
    held within 6 dB of its peak."""
    x, sr = read(path)
    t, d = frames_db(x, sr, 0.005, 0.001)
    peak = np.percentile(d, 99.5)
    act = d > peak - 30
    strokes = []
    i, n = 0, len(d)
    while i < n:
        if act[i] and (i == 0 or not act[i - 1]):
            j = i
            # the stroke's peak within 60 ms of its rise
            k = i + int(np.argmax(d[i:min(i + 60, n)]))
            p = d[k]
            row = []
            for ms in (40, 80, 150, 250):
                m = k + ms
                row.append(float(p - d[m]) if m < n else float("nan"))
            hold = k
            while hold + 1 < n and d[hold + 1] > p - 6:
                hold += 1
            strokes.append({"t": float(t[k]), "fall_db": row, "held_ms": float(hold - k)})
            # the next stroke starts after the level fell 10 dB and rose again
            j = k
            while j < n and d[j] > p - 10:
                j += 1
            i = j
        else:
            i += 1
    return strokes


def summary(v):
    v = np.asarray([x for x in v if x == x])
    if len(v) == 0:
        return None
    return {
        "n": int(len(v)),
        "p10": float(np.percentile(v, 10)),
        "p25": float(np.percentile(v, 25)),
        "median": float(np.median(v)),
        "p75": float(np.percentile(v, 75)),
        "p90": float(np.percentile(v, 90)),
    }


def refs(downloads, out):
    root = Path(downloads)
    phil = root / "study" / "philharmonia" / "raw" / "Strings"
    fam = {"cello": "cello", "double-bass": "bass", "viola": "viola", "violin": "violin"}
    res = {"tick": {}, "bounce": {}}
    for d in sorted(phil.iterdir()):
        for f in sorted(d.glob("*_phrase_*arco-detache*.mp3")):
            name = f.name.split("_")[0]
            family = fam.get(name)
            if family:
                res["tick"].setdefault(family, []).extend(ticks(f))
        for f in sorted(d.glob("*_phrase_*arco-spiccato*.mp3")):
            name = f.name.split("_")[0]
            family = fam.get(name)
            if family:
                res["bounce"].setdefault(family, []).extend(bounce(f))
    vsco = [
        ("violin", root / "arcus-upper" / "refs" / "vsco-solo-violin" / "spic"),
        ("bass", root / "strings-rec" / "vsco" / "Strings" / "Solo Contrabass" / "Spic"),
    ]
    single = {}
    for family, folder in vsco:
        for f in sorted(folder.glob("*.wav")):
            s = bounce(f)
            if s:
                single.setdefault(family, []).append(s[0])
    table = {"tick_db": {}, "phrase_bounce": {}, "single_bounce": {}}
    for family, v in res["tick"].items():
        table["tick_db"][family] = summary(v)
    for key, src in (("phrase_bounce", res["bounce"]), ("single_bounce", single)):
        for family, strokes in src.items():
            table[key][family] = {
                f"fall_{ms}ms_db": summary([s["fall_db"][i] for s in strokes])
                for i, ms in enumerate((40, 80, 150, 250))
            }
            table[key][family]["held_ms"] = summary([s["held_ms"] for s in strokes])
    Path(out).write_text(json.dumps(table, indent=1))
    print(json.dumps(table, indent=1))


def main():
    what = sys.argv[1]
    if what == "refs":
        refs(sys.argv[2], sys.argv[3])
        return
    for f in sys.argv[2:]:
        if what == "tick":
            v = ticks(f)
            print(f"{Path(f).name:32s} ticks {len(v):3d}: median {np.median(v) if v else float('nan'):+.1f} dB, "
                  f"p90 {np.percentile(v, 90) if v else float('nan'):+.1f}, all {[round(x, 1) for x in v]}")
        else:
            s = bounce(f)
            for k in s:
                print(f"{Path(f).name:32s} {k['t']:.2f}s fall {[round(x, 1) for x in k['fall_db']]} held {k['held_ms']:.0f} ms")


if __name__ == "__main__":
    main()
