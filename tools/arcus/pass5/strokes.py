"""strokes.py - Arcus pass 5: what a bow does between notes, measured the same way on the
recordings and on Arcus's renders.

- **The bow change's tick** (`tick`, `parts`): at each note boundary of a phrase (the
  level's dip between two notes), the high band (4-12 kHz, a fourth-order Butterworth
  band-pass) in 3 ms frames: its loudest frame within 15 ms of the dip over the median
  frame of the 60-20 ms before (`over_pre`) and of the 25-65 ms after (`over_post`), dB,
  and how long the band stays within 6 dB of that frame (`width_ms`). A change that clicks
  stands over both notes' own high end for a few ms; the next note's bite stays.
- **The spiccato's bounce** (`bounce`): each stroke's level (5 ms RMS) from its peak - the
  dB it has fallen 40, 80, 150 and 250 ms after the peak - and the share of the stroke
  spent within 6 dB of its peak (`held_ms`).

    python -I tools/arcus/pass5/strokes.py tick <file> ...
    python -I tools/arcus/pass5/strokes.py parts <file> ...    (the ticks' parts, pooled)
    python -I tools/arcus/pass5/strokes.py bounce|single <file> ...
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
    """The high band's loudest 3 ms within 15 ms of the boundary over the louder of its
    median before (60-20 ms) and after (25-65 ms): a click stands over both notes' own high
    end; the next note's bite, which stays, does not."""
    t, d = frames_db(hp, sr, 0.003, 0.001)
    near = (t >= tb - 0.015) & (t <= tb + 0.015)
    pre = (t >= tb - 0.060) & (t <= tb - 0.020)
    post = (t >= tb + 0.025) & (t <= tb + 0.065)
    if near.sum() < 3 or pre.sum() < 5 or post.sum() < 5:
        return None
    return float(d[near].max() - max(np.median(d[pre]), np.median(d[post])))


def tick_parts(x, sr, tb, hp):
    """The tick's parts at a boundary: the high band's loudest 3 ms within 15 ms over its
    median 60-20 ms before (`pre`) and 25-65 ms after (`post`), and how long (ms) the band
    stays within 6 dB of that loudest frame (`width`: a click is a few ms, a bite tens)."""
    t, d = frames_db(hp, sr, 0.003, 0.001)
    near = np.nonzero((t >= tb - 0.015) & (t <= tb + 0.015))[0]
    pre = (t >= tb - 0.060) & (t <= tb - 0.020)
    post = (t >= tb + 0.025) & (t <= tb + 0.065)
    if len(near) < 3 or pre.sum() < 5 or post.sum() < 5:
        return None
    k = near[np.argmax(d[near])]
    a = b = k
    while a > 0 and d[a - 1] > d[k] - 6:
        a -= 1
    while b + 1 < len(d) and d[b + 1] > d[k] - 6:
        b += 1
    return float(d[k] - np.median(d[pre])), float(d[k] - np.median(d[post])), float(b - a + 1)


def band(x, sr):
    sos = butter(4, [4000, min(12000, 0.45 * sr)], btype="band", fs=sr, output="sos")
    return sosfilt(sos, x)


def ticks(path):
    """Every boundary's `tick_parts` (over pre, over post, width)."""
    x, sr = read(path)
    hp = band(x, sr)
    out = []
    for tb in boundaries(x, sr):
        v = tick_parts(x, sr, tb, hp)
        if v is not None:
            out.append(v)
    return out


def stroke_at(t, d, k, end):
    """A stroke peaking at frame `k` (1 ms frames) and lasting to frame `end`."""
    p = d[k]
    row = []
    for ms in (40, 80, 150, 250):
        m = k + ms
        row.append(float(p - d[m]) if m < end else float("nan"))
    hold = k
    while hold + 1 < end and d[hold + 1] > p - 6:
        hold += 1
    gap = float(p - d[k:end].min()) if end > k else float("nan")
    return {"t": float(t[k]), "fall_db": row, "held_ms": float(hold - k), "gap_db": gap,
            "ioi_ms": float(end - k)}


def bounce(path, single=False):
    """Each stroke of a file: its fall from its peak at 40, 80, 150 and 250 ms, how long it
    held within 6 dB of the peak, and how far the level fell before the next stroke. A
    phrase's strokes run from one boundary (`boundaries`) to the next, their peak within
    60 ms of it; `single`: the file is one stroke, from its loudest frame on."""
    x, sr = read(path)
    t, d = frames_db(x, sr, 0.005, 0.001)
    n = len(d)
    if single:
        k = int(np.argmax(d))
        return [stroke_at(t, d, k, n)]
    edges = [int(np.searchsorted(t, b)) for b in boundaries(x, sr)]
    out = []
    for a, b in zip(edges, edges[1:] + [n]):
        if b - a < 80:
            continue
        k = a + int(np.argmax(d[a:min(a + 60, b)]))
        out.append(stroke_at(t, d, k, b))
    return out


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
    phils = [root / "study" / "philharmonia" / "raw" / "Strings",
             root / "study" / "philharmonia-violin-viola"]
    fam = {"cello": "cello", "double-bass": "bass", "viola": "viola", "violin": "violin"}
    res = {"tick": {}, "bounce": {}}
    folders = sorted({p.parent for ph in phils if ph.exists() for p in ph.rglob("*_phrase_*.mp3")})
    for d in folders:
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
            single.setdefault(family, []).extend(bounce(f, single=True))
    table = {"tick": {}, "phrase_bounce": {}, "single_bounce": {}}
    for family, v in res["tick"].items():
        table["tick"][family] = {
            "over_pre_db": summary([r[0] for r in v]),
            "over_post_db": summary([r[1] for r in v]),
            "width_ms": summary([r[2] for r in v]),
        }
    for key, src in (("phrase_bounce", res["bounce"]), ("single_bounce", single)):
        for family, strokes in src.items():
            table[key][family] = {
                f"fall_{ms}ms_db": summary([s["fall_db"][i] for s in strokes])
                for i, ms in enumerate((40, 80, 150, 250))
            }
            for m in ("held_ms", "gap_db", "ioi_ms"):
                table[key][family][m] = summary([s[m] for s in strokes])
    Path(out).write_text(json.dumps(table, indent=1))
    print(json.dumps(table, indent=1))


def main():
    what = sys.argv[1]
    if what == "refs":
        refs(sys.argv[2], sys.argv[3])
        return
    if what == "parts":
        rows = []
        for f in sys.argv[2:]:
            x, sr = read(f)
            hp = band(x, sr)
            for tb in boundaries(x, sr):
                v = tick_parts(x, sr, tb, hp)
                if v:
                    rows.append(v)
        r = np.array(rows)
        for i, name in enumerate(("over pre", "over post", "width ms")):
            print(f"{name:10s} n {len(r)}: p10 {np.percentile(r[:, i], 10):+.1f} median {np.median(r[:, i]):+.1f} "
                  f"p90 {np.percentile(r[:, i], 90):+.1f}")
        return
    for f in sys.argv[2:]:
        if what == "tick":
            v = [r[1] for r in ticks(f)]
            print(f"{Path(f).name:32s} ticks {len(v):3d}: over post median "
                  f"{np.median(v) if v else float('nan'):+.1f} dB, "
                  f"p90 {np.percentile(v, 90) if v else float('nan'):+.1f}, all {[round(x, 1) for x in v]}")
        else:
            s = bounce(f, single=what == "single")
            for k in s:
                print(f"{Path(f).name:32s} {k['t']:.2f}s fall {[round(x, 1) for x in k['fall_db']]} "
                      f"held {k['held_ms']:.0f} ms gap {k['gap_db']:.1f} dB ioi {k['ioi_ms']:.0f} ms")


if __name__ == "__main__":
    main()
