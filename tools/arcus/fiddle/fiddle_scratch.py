"""fiddle_scratch.py - r42 fiddle, the owner's "too scratchy": the noise between the harmonics
by band, the periodicity, and the top's third-octaves, for a few files beside the references.

    python -I tools/arcus/fiddle/fiddle_scratch.py refs            # the references' table (cached)
    python -I tools/arcus/fiddle/fiddle_scratch.py <wav> [<wav> ...]

Per file (both sides summed, as fiddle_lib loads):

- `nN dB` (N = 2-4, 4-8, 8-12 kHz): the band's noise (its comb residual against one period
  later, `fiddle_lib.band_aperiodicity`) re the frame's whole level, the median over the
  steady frames (voiced, the pitch moving < 30 cents per 10 ms, within 25 dB of the peak).
  This is the aperiodic energy the ear hears as the hair's hiss and the scratch.
- `hN dB`: the same band's noise-to-harmonic ratio (noise minus the band's periodic part).
- `act4-8 p90`: the 4-8 kHz noise re the frame's level over every sounding frame (onsets and
  fast notes included), its 90th percentile: the attacks' and crossings' scratch.
- `ap`: YIN's aperiodicity d' over the sounding frames (median), and `rauc`: the share of
  them over 0.25 (a raucous or multi-slip frame loses the period: d' rises).
- `L4k ... L10k`: the long-term third-octaves (dB re total), and centroid / f0, > 2 kHz.

Single lines only: a double stop's second voice counts as noise in the comb residual.
"""
import json
import sys
from pathlib import Path

import numpy as np
from scipy import signal

sys.path.insert(0, str(Path(__file__).resolve().parent))
import fiddle_groups as fg  # noqa: E402
import fiddle_lib as fl  # noqa: E402
import fiddle_measure as fm  # noqa: E402

BANDS = [("2-4", 2000.0, 4000.0), ("4-8", 4000.0, 8000.0), ("8-12", 8000.0, 12000.0)]
LT = ["4000", "5040", "6350", "8000", "10079"]
CNSM = fg.CNSM
PHX = fg.PHX
# single lines only (the double-stop and harmonic clips left out)
TFF_SINGLE = ["621593_Bluegrass_Fiddle", "621592_Bluegrass_Fiddle_Ending", "621596_Bluesy_Violin_Fast_liks",
              "621590_Bluesy_Violin_Ascending_Arpegio", "621599_Bluesy_Violin_descending_lick",
              "621597_Bluesy_Violin_Descending_lick_3", "621594_Bluesy_Violin_descending_lick_2",
              "641690_Solo_fiddle_playing_part_of_a_hornpipe"]


def ref_files():
    g = {"dry-classical": [(fg.refs(CNSM + f"{v}/Player {p}/scale_01.wav"), None)
                           for v, p in (("Klimke", 0), ("Klimke", 11), ("Levaggi", 2), ("Levaggi", 7),
                                        ("Stoppani", 4), ("Stoppani", 9))]
         + [(fg.refs(PHX + "mozart/violin1.wav"), 40.0), (fg.refs(PHX + "beethoven/violin1.wav"), 40.0)],
         "bluegrass": [(fg.refs(fg.FS + n + ".preview-hq.mp3"), None) for n in TFF_SINGLE]}
    return g


def period_of_frames(tr):
    f = tr.f_raw
    fb = np.where(tr.voiced, f, np.nan)
    idx = np.flatnonzero(np.isfinite(fb))
    if not len(idx):
        return np.zeros(len(f))
    ar = np.arange(len(fb))
    near = idx[np.clip(np.searchsorted(idx, ar), 0, len(idx) - 1)]
    near2 = idx[np.clip(np.searchsorted(idx, ar) - 1, 0, len(idx) - 1)]
    pick = np.where(np.abs(near - ar) <= np.abs(near2 - ar), near, near2)
    ok = tr.active & (np.abs(pick - ar) * fl.HOP <= 0.06)
    return np.where(ok, f[pick], 0.0)


def vibrato_onset(tr, min_s=0.5):
    """The vibrato's onset on held notes (>= `min_s` of one rounded semitone, voiced): the
    pitch detrended over 0.25 s, its extent (p90 - p10) in 0.15 s windows; a note has vibrato
    when its widest window reaches 20 cents at 3.5-9 Hz, and its onset is the first window
    at half that extent (s from the note's start). Returns the share of held notes with
    vibrato and the median onset."""
    semi = np.where(tr.voiced, np.round(tr.midi - tr.tuning), np.nan)
    runs, i = [], 0
    while i < len(semi):
        if np.isnan(semi[i]):
            i += 1
            continue
        j = i
        while j + 1 < len(semi) and semi[j + 1] == semi[i]:
            j += 1
        if (j - i + 1) * fl.HOP >= min_s:
            runs.append((i, j))
        i = j + 1
    w = int(0.15 / fl.HOP)
    has, on = [], []
    for a, b in runs:
        c = 100 * (tr.midi[a:b + 1] - np.round(tr.midi[a:b + 1]))
        k = int(0.25 / fl.HOP) | 1
        det = c - np.convolve(np.pad(c, k // 2, mode="edge"), np.ones(k) / k, mode="valid")
        ext = np.array([np.percentile(det[s:s + w], 90) - np.percentile(det[s:s + w], 10)
                        for s in range(0, len(det) - w, w // 3)])
        if not len(ext):
            continue
        zc = np.sum(np.diff(np.sign(det)) != 0) / 2.0 / (len(det) * fl.HOP)
        ok = ext.max() >= 20 and 3.5 <= zc <= 9.0
        has.append(ok)
        if ok:
            on.append(float(np.argmax(ext >= 0.5 * ext.max()) * (w // 3) * fl.HOP))
    return {"vib_share": float(np.mean(has)) if has else float("nan"),
            "vib_on_s": float(np.median(on)) if on else float("nan"),
            "held_notes": len(has)}


def measure(path, t1=None):
    x, sr = fl.load(path, None, t1)
    tr = fl.Track(x, sr)
    fb = period_of_frames(tr)
    dm = np.abs(np.gradient(tr.midi)) * (0.01 / fl.HOP) * 100
    steady = tr.voiced & (dm < 30) & (tr.lvl > tr.peak - 25)
    act = tr.active & (fb > 0)
    out = {}
    for name, a, b in BANDS:
        if b > 0.45 * sr:
            continue
        sos = signal.butter(6, [a, b], "bandpass", fs=sr, output="sos")
        y = signal.sosfiltfilt(sos, x)
        E, R = fl.band_aperiodicity(y, sr, tr.t, fb)
        r = np.clip(R, 1e-4, 0.9999)
        N = E + 10 * np.log10(r)
        H = E + 10 * np.log10(1 - r)
        rel = N - tr.lvl
        out["n" + name] = float(np.median(rel[steady])) if steady.sum() > 20 else float("nan")
        out["h" + name] = float(np.median((N - H)[steady])) if steady.sum() > 20 else float("nan")
        if name == "4-8":
            out["act4-8 p90"] = float(np.percentile(rel[act], 90)) if act.sum() > 20 else float("nan")
    out.update(vibrato_onset(tr))
    ap = tr.ap[tr.active]
    out["ap"] = float(np.median(ap))
    out["rauc"] = float(np.mean(ap > 0.25))
    t = fm.timbre(tr)
    for k in LT:
        out["L" + k] = t["ltas_third_oct_db"].get(k, float("nan"))
    out["c/f0"] = t.get("centroid_over_f0", float("nan"))
    out[">2k"] = t["share_above_2k_db"]
    out["L1587-3175"] = float(np.mean([t["ltas_third_oct_db"][k] for k in ("1587", "2000", "2520", "3175")]))
    return out


KEYS = ["n2-4", "n4-8", "n8-12", "h2-4", "h4-8", "h8-12", "act4-8 p90", "ap", "rauc",
        "L4000", "L5040", "L6350", "L8000", "L10079", "c/f0", ">2k", "L1587-3175", "vib_share", "vib_on_s"]


def show(rows):
    w = max([len(n) for n, _ in rows] + [10]) + 1
    print("file".ljust(w) + "".join(k.rjust(11) for k in KEYS))
    for n, r in rows:
        print(n.ljust(w) + "".join(f"{r.get(k, float('nan')):11.2f}" for k in KEYS))


def refs_table():
    cache = fg.cp.work("arcus", "fiddle", "scratch_refs.json")
    if cache.exists():
        data = json.loads(cache.read_text(encoding="utf-8"))
    else:
        data = {}
        for grp, files in ref_files().items():
            data[grp] = []
            for p, t1 in files:
                if not Path(p).exists():
                    print("missing", p, file=sys.stderr)
                    continue
                data[grp].append((Path(p).stem[:28], measure(p, t1)))
        cache.write_text(json.dumps(data, indent=1), encoding="utf-8")
    rows = []
    for grp, rs in data.items():
        rows += [(f"{grp[:4]}:{n}", r) for n, r in rs]
        med = {k: float(np.nanmedian([r.get(k, np.nan) for _, r in rs])) for k in KEYS}
        lo = {k: float(np.nanpercentile([r.get(k, np.nan) for _, r in rs], 25)) for k in KEYS}
        hi = {k: float(np.nanpercentile([r.get(k, np.nan) for _, r in rs], 75)) for k in KEYS}
        rows += [(f"{grp} p25", lo), (f"{grp} MEDIAN", med), (f"{grp} p75", hi)]
    show(rows)


def main(argv):
    if argv and argv[0] == "refs":
        refs_table()
        return
    show([(Path(p).stem, measure(p)) for p in argv])


if __name__ == "__main__":
    main(sys.argv[1:])
