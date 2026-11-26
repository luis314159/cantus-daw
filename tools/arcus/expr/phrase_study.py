"""phrase_study.py - Arcus played like a human (r42 arcus-expr): how real string players shape
notes inside music, measured on the HSD Düsseldorf quartet's separated stems (Mozart K465,
*Andante cantabile*, CC BY 4.0, `$CANTUS_REFS/study/hsd-string-quartet-k465`) and on Arcus's
renders the same way.

Per stem, the pitch is tracked (the study's YIN, `bowlib_core.yin_track`, hop 5 ms) and cut into
notes where the rounded semitone changes for 60 ms or the sound stops. Per note:

- its length; its vibrato's extent peak to trough (the cents track detrended over two cycles,
  the 90th minus the 10th percentile in 0.2 s windows, the windows' 75th percentile) and its
  onset (the first window at half of that, from the note's start);
- its level envelope (RMS, 40 ms): where the peak falls (a share of the note), how far under
  the peak the first and the last 60 ms sit.

Across stems (the quartet takes): each instrument's level against the four's mean over the
windows where all four play, calibrated by each stem's 94 dB SPL reference tone; and the
onset asynchrony — each first violin's onset against the nearest onset of every other part
within 80 ms (median |dt| and its spread).

    python -I tools/arcus/expr/phrase_study.py refs <out.json>
    python -I tools/arcus/expr/phrase_study.py stems <out.json> <name>=<wav>:<family> ...

`stems` measures renders (e.g. a demo's stems) the same way, without the calibration;
`quartet <out.json> <stems-dir>` finds the demo's four stems by name (Violín, Viola,
Violonchelo, Contrabajo).
"""
import json
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "study"))
from bowlib_core import load, rms_db, yin_track, median_filter  # noqa: E402
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "sound-refs"))
import cantus_paths as cp  # noqa: E402

RANGE = {"violin": (180, 2600), "viola": (120, 1300), "cello": (60, 900), "bass": (38, 400)}
HOP = 0.005


def notes_of(x, sr, family):
    """The notes of one stem: (t_on, t_off, midi median, cents track (t, c), level (t, db))."""
    lo, hi = RANGE[family]
    t, f, ap = yin_track(x, sr, lo, hi, hop_s=HOP, thr=0.15)
    lt, ld = rms_db(x, sr, 0.04, HOP)
    lvl = np.interp(t, lt, ld)
    peak = np.percentile(ld, 99)
    voiced = (ap < 0.3) & (lvl > peak - 38) & (f > lo) & (f < hi)
    m = 69 + 12 * np.log2(np.maximum(f, 1.0) / 440.0)
    m = median_filter(m, 7)
    semi = np.round(m)
    out = []
    i, n = 0, len(t)
    hold = int(round(0.06 / HOP))
    while i < n:
        if not voiced[i]:
            i += 1
            continue
        j = i
        cur = semi[i]
        gap = 0
        while j + 1 < n:
            k = j + 1
            if not voiced[k]:
                gap += 1
                if gap * HOP >= 0.03:
                    break
                j = k
                continue
            gap = 0
            if semi[k] != cur:
                # a change that lasts 60 ms ends the note
                w = semi[k:k + hold]
                if len(w) >= hold and np.all(w == semi[k]):
                    break
            j = k
        a, b = i, j
        if (t[b] - t[a]) >= 0.12:
            sel = slice(a, b + 1)
            ok = voiced[sel]
            mm = float(np.median(m[sel][ok])) if ok.any() else float(cur)
            c = 100 * (m[sel] - mm)
            out.append((float(t[a]), float(t[b]), mm, (t[sel], c, ok), (t[sel], lvl[sel])))
        i = j + 1
    return out


def vibrato(tc, cc, ok):
    """(extent p-p cents, onset s) of one note's cents track."""
    if len(tc) < 40 or ok.mean() < 0.6:
        return None, None
    c = cc.copy()
    if (~ok).any():
        c[~ok] = np.interp(tc[~ok], tc[ok], c[ok])
    k = max(int(round(0.36 / HOP)), 3)
    ker = np.ones(k) / k
    tr = np.convolve(np.pad(c, (k // 2, k - 1 - k // 2), mode="edge"), ker, mode="valid")
    v = c - tr
    w = int(round(0.2 / HOP))
    if len(v) < w + 2:
        return None, None
    pp = np.array([np.percentile(v[s:s + w], 90) - np.percentile(v[s:s + w], 10)
                   for s in range(0, len(v) - w, 2)])
    ext = float(np.percentile(pp, 75))
    first = np.nonzero(pp >= 0.5 * ext)[0]
    onset = float(2 * first[0] * HOP) if len(first) else None
    return ext, onset


def envelope(tl, dl):
    """(peak share, start under peak dB, end under peak dB) of one note's level."""
    if len(dl) < 20:
        return None
    k = int(np.argmax(dl))
    n6 = max(int(round(0.06 / HOP)), 2)
    return (float(k / max(len(dl) - 1, 1)), float(dl[k] - np.mean(dl[:n6])),
            float(dl[k] - np.mean(dl[-n6:])))


BINS = [(0.12, 0.3), (0.3, 0.6), (0.6, 1.2), (1.2, 99.0)]


def per_note(notes):
    rows = []
    for (a, b, mm, (tc, cc, ok), (tl, dl)) in notes:
        ext, on = vibrato(tc, cc, ok)
        env = envelope(tl, dl)
        rows.append({"t": a, "dur": b - a, "midi": mm, "vib_pp": ext, "vib_onset": on,
                     "env": env})
    return rows


def q(v, p=(25, 50, 75)):
    v = [x for x in v if x is not None and x == x]
    if not v:
        return None
    return [round(float(np.percentile(v, pp)), 3) for pp in p] + [len(v)]


def summary(rows):
    out = {}
    for lo, hi in BINS:
        r = [x for x in rows if lo <= x["dur"] < hi]
        key = f"{lo}-{hi if hi < 99 else 'inf'}s"
        out[key] = {
            "notes": len(r),
            "vib_pp_cents": q([x["vib_pp"] for x in r]),
            "vib_share_over_8ct": round(float(np.mean([1.0 if (x["vib_pp"] or 0) >= 8 else 0.0
                                                       for x in r])), 3) if r else None,
            "vib_onset_s": q([x["vib_onset"] for x in r if (x["vib_pp"] or 0) >= 8]),
            "env_peak_share": q([x["env"][0] for x in r if x["env"]]),
            "env_start_under_peak_db": q([x["env"][1] for x in r if x["env"]]),
            "env_end_under_peak_db": q([x["env"][2] for x in r if x["env"]]),
        }
    return out


def calib(path):
    x, sr = load(path)
    return 10 * np.log10(np.mean(x[int(0.5 * sr):-int(0.5 * sr) or None] ** 2) + 1e-20)


def balance(stems, cal):
    """Each part's level against the parts' mean where all play (0.5 s windows), dB."""
    lv = {}
    for name, (x, sr) in stems.items():
        w = int(0.5 * sr)
        e = np.array([np.mean(x[i:i + w] ** 2) for i in range(0, len(x) - w, w)])
        lv[name] = 10 * np.log10(e + 1e-20) - cal.get(name, 0.0)
    n = min(len(v) for v in lv.values())
    mat = np.array([lv[k][:n] for k in lv])
    act = np.all(mat > (mat.max(axis=1, keepdims=True) - 30), axis=0)
    mean = mat[:, act].mean(axis=0)
    return {k: q(list(mat[i, act] - mean)) for i, k in enumerate(lv)}, int(act.sum())


def asynchrony(onsets, lead):
    out = {}
    for k, v in onsets.items():
        if k == lead:
            continue
        d = []
        for t in onsets[lead]:
            j = np.argmin(np.abs(v - t)) if len(v) else None
            if j is not None and abs(v[j] - t) <= 0.08:
                d.append(1000 * (v[j] - t))
        out[k] = {"matched": len(d), "median_ms": q(d), "abs_ms": q([abs(x) for x in d]),
                  "sd_ms": round(float(np.std(d)), 2) if d else None}
    return out


def take(files, cal=None):
    stems, rows, onsets = {}, {}, {}
    for name, (path, fam) in files.items():
        x, sr = load(path)
        stems[name] = (x, sr)
        ns = notes_of(x, sr, fam)
        rows[name] = per_note(ns)
        onsets[name] = np.array([n[0] for n in ns])
        print(f"  {name}: {len(ns)} notes", flush=True)
    bal, nwin = balance(stems, cal or {})
    lead = next(iter(files))
    return {"per_part": {k: summary(v) for k, v in rows.items()},
            "all_notes": summary([r for v in rows.values() for r in v]),
            "balance_db": bal, "balance_windows": nwin,
            "asynchrony_vs_" + lead: asynchrony(onsets, lead)}


def refs(out):
    root = cp.refs("study", "hsd-string-quartet-k465", "Streicherquartett_Complete_Project",
                   "Audiodateien")
    fam = {"Vl1": "violin", "Vl2": "violin", "Vla": "viola", "Vlc": "cello"}
    cal = {k: calib(root / f"Reference_Level_{k}_94dBSPL.wav") for k in fam}
    res = {"calibration_db": {k: round(v, 2) for k, v in cal.items()}}
    for take_name, folder, stem in [("long", "Quartet Long Excerpt", "Quartet_Long"),
                                    ("phrase1", "Quartet Phrase 1", "Quartet_Phrase_1"),
                                    ("phrase2", "Quartet Phrase 2", "Quartet_Phrase_2"),
                                    ("phrase3", "Quartet Phrase 3", "Quartet_Phrase_3")]:
        print(take_name, flush=True)
        files = {k: (root / folder / f"{stem}_{k}.wav", f) for k, f in fam.items()}
        res[take_name] = take(files, cal)
    solo = {}
    for i in (1, 2, 3):
        for inst, part, f in (("Cello", "Vlc", "cello"), ("Violin", "Vl1" if i == 3 else "Vl2",
                                                           "violin")):
            p = root / f"{inst} Phrase {i}" / f"{inst}_Phrase_{i}_{part}.wav"
            if p.exists():
                x, sr = load(p)
                solo[f"{inst.lower()}{i}"] = per_note(notes_of(x, sr, f))
    res["solo_phrases"] = summary([r for v in solo.values() for r in v])
    Path(out).write_text(json.dumps(res, indent=1))
    print("wrote", out)


def stems_cmd(out, specs):
    files = {}
    for s in specs:
        name, rest = s.split("=", 1)
        path, fam = rest.rsplit(":", 1)
        files[name] = (Path(path), fam)
    res = take(files)
    Path(out).write_text(json.dumps(res, indent=1))
    print("wrote", out)


def quartet(out, d):
    d = Path(d)
    pick = {"Vl": ("Violín", "violin"), "Vla": ("Viola", "viola"),
            "Vlc": ("Violonchelo", "cello"), "Cb": ("Contrabajo", "bass")}
    files = {}
    for k, (name, fam) in pick.items():
        hit = [f for f in d.glob("*.wav") if f.stem.split(" ", 1)[-1] == name]
        files[k] = (hit[0], fam)
    res = take(files)
    Path(out).write_text(json.dumps(res, indent=1))
    print("wrote", out)


def main():
    if len(sys.argv) >= 4 and sys.argv[1] == "quartet":
        quartet(sys.argv[2], sys.argv[3])
    elif len(sys.argv) >= 3 and sys.argv[1] == "refs":
        refs(sys.argv[2])
    elif len(sys.argv) >= 4 and sys.argv[1] == "stems":
        stems_cmd(sys.argv[2], sys.argv[3:])
    else:
        print(__doc__)
        sys.exit(2)


if __name__ == "__main__":
    main()
