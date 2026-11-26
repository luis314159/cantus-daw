"""fiddle_report.py - the fiddle study's per-group numbers and markdown (fiddle_measure.py writes
the per-file JSONs under <out>/files; this pools them). Every number is median [p25, p75] (n),
pooled over the group's events (notes, onsets, segments, runs) unless the row says per file."""
import json
from pathlib import Path

import numpy as np

COLS = ["fiddle-bluegrass", "fiddle-tunes", "fiddle-78s", "classical-scales", "classical-music",
        "ours-country", "ours-classical", "ours-room", "control-wav", "control-mp3"]
NOISE = ("burst", "nhr", "steady_share")   # not for the band-limited 78s
DCOLS = ["fiddle-bluegrass", "fiddle-tunes", "fiddle-78s-dstops", "classical-dstops", "ours-dstops", "ours-country",
         "ours-room"]


def q(v):
    v = [float(x) for x in v if x is not None and np.isfinite(x)]
    if not v:
        return None
    p = np.percentile(v, [25, 50, 75])
    return {"n": len(v), "p25": float(p[0]), "med": float(p[1]), "p75": float(p[2])}


def share(v):
    """A yes/no measure: its mean, as med (p25/p75 = the share's 95 % binomial interval)."""
    v = [float(x) for x in v if x is not None]
    if not v:
        return None
    m = float(np.mean(v))
    e = 1.96 * np.sqrt(max(m * (1 - m), 1e-9) / len(v))
    return {"n": len(v), "p25": max(m - e, 0.0), "med": m, "p75": min(m + e, 1.0)}


def fmt(s, d=1):
    if not s:
        return "-"
    return f"{s['med']:.{d}f} [{s['p25']:.{d}f}, {s['p75']:.{d}f}] ({s['n']})"


FAST = ["octave_start", "burst_db", "burst_nhr_db", "burst_ms", "nhr_steady_db", "nhr_steady_rel_db", "dev0_20", "dev0_20_signed",
        "dev20_40", "settle_ms", "steady_share", "dip_db", "dip_ms", "ring30_db", "ring60_db", "ioi"]


def pool(files):
    g = {}
    fast = [r for f in files for r in f.get("fast", [])]
    for k in FAST:
        g["fast." + k] = q([r.get(k) for r in fast])
    g["fast.octave_start"] = share([r.get("octave_start") for r in fast])
    ft = [r for f in files for r in f.get("fast_truth", [])]
    for k in FAST:
        if ft:
            g["fast_truth." + k] = q([r.get(k) for r in ft])
    g["fast.flux_cue_share"] = share([1.0 if r["cue"] == 0 else 0.0 for r in fast])
    f2 = [r for r in fast if r["ioi"] < 0.2]
    for k in FAST:
        g["fast200." + k] = q([r.get(k) for r in f2])
    f1 = [r for r in fast if r.get("dyad_share", 0) < 0.3]
    for k in FAST:
        g["fast1." + k] = q([r.get(k) for r in f1])
    g["fast.dyad_note_share"] = share([1.0 if r.get("dyad_share", 0) >= 0.3 else 0.0 for r in fast])
    ds = [f["double"] for f in files if "double" in f]
    if ds:
        fr = sum(d["frames"] for d in ds)
        g["double.dyad_share_pooled"] = {"n": len(ds), "med": sum(d["dyad_share"] * d["frames"] for d in ds) / max(fr, 1),
                                         "p25": min(d["dyad_share"] for d in ds), "p75": max(d["dyad_share"] for d in ds)}
        cls = {}
        tot = 0
        for d in ds:
            n = sum(d["semitones"].values())
            tot += n
            for c, s in d["classes"].items():
                cls[c] = cls.get(c, 0) + s * n
        g["double.classes"] = {c: v / max(tot, 1) for c, v in sorted(cls.items())}
        rows = [r for d in ds for r in d["rows"]]
        g["double.segments"] = {"n": len(rows)}
        for k in ("corr_raw", "corr_fast", "dips_shared", "dur", "level_lo_minus_hi_db", "et_dev_c",
                  "just_dev_c", "interval_sd_c", "pitch_corr"):
            g["double." + k] = q([r.get(k) for r in rows])
        g["double.onset_lo_minus_hi_ms"] = q([r.get("onset_lo_minus_hi_ms") for r in rows])
        g["double.abs_et_dev_c"] = q([abs(r["et_dev_c"]) for r in rows if "et_dev_c" in r])
        g["double.abs_just_dev_c"] = q([abs(r["just_dev_c"]) for r in rows if "just_dev_c" in r])
        g["double.onset_abs_ms"] = q([abs(r["onset_lo_minus_hi_ms"]) for r in rows if r.get("onset_lo_minus_hi_ms") is not None])
    tb = [f["timbre"] for f in files if f.get("timbre")]
    for k in ("centroid_hz", "share_above_2k_db", "share_above_4k_db", "centroid_over_f0"):
        g["timbre." + k] = q([t.get(k) for t in tb])
    if tb:
        keys = list(tb[0]["ltas_third_oct_db"].keys())
        g["timbre.ltas"] = {k: float(np.mean([t["ltas_third_oct_db"][k] for t in tb])) for k in keys}
    for reg in "GDAE":
        nz = [f["noise"][reg] for f in files if f.get("noise", {}).get(reg)]
        g[f"noise.{reg}.hnr_bb_db"] = q([n["hnr_bb_db"][1] for n in nz])
        g[f"noise.{reg}.hnr_2_8k_db"] = q([n["hnr_2_8k_db"][1] for n in nz])
    vib = [v for f in files for v in f.get("vibrato", [])]
    g["vibrato.share"] = share([1.0 if v["has"] else 0.0 for v in vib])
    g["vibrato.extent_c"] = q([v["extent_c"] for v in vib if v["has"]])
    g["vibrato.rate_hz"] = q([v["rate_hz"] for v in vib if v["has"]])
    g["vibrato.extent_all_c"] = q([v["extent_c"] for v in vib])
    sl = [s for f in files for s in f.get("slides", [])]
    g["slides.share"] = share([1.0 if abs(s["from_c"]) >= 50 else 0.0 for s in sl if not s.get("octave")])
    g["slides.octave_start_share"] = share([1.0 if s.get("octave") else 0.0 for s in sl])
    g["slides.from_c"] = q([s["from_c"] for s in sl if abs(s["from_c"]) >= 50])
    g["slides.ms"] = q([s["ms"] for s in sl if abs(s["from_c"]) >= 50])
    it = [f["intonation"] for f in files if f.get("intonation")]
    for deg in ("third", "seventh"):
        g[f"intonation.{deg}_above_minor_c"] = q([i[deg]["cents_above_minor"][1] for i in it if deg in i])
        g[f"intonation.{deg}_neutral_share"] = q([i[deg]["neutral_share"] for i in it if deg in i])
    g["intonation.dev_sd_c"] = q([i.get("dev_from_et_c_sd") for i in it])
    rr = [r for f in files for r in f.get("rhythm", [])]
    g["rhythm.runs"] = {"n": len(rr)}
    for k in ("swing", "ioi_sd_ms", "accent_long_minus_short_db", "level_sd_db"):
        g["rhythm." + k] = q([r.get(k) for r in rr])
    g["rhythm.tempo_bpm"] = q([60 / (2 * r["eighth_s"]) for r in rr])
    g["notes.ioi_median_s"] = q([f["ioi_q"][2] for f in files if f.get("ioi_q")])
    g["notes.fast_share"] = q([f.get("fast_share") for f in files if "fast_share" in f])
    dc = [d for f in files for d in f.get("decays", [])]
    g["room.t60_s"] = q([d["t60_s"] for d in dc])
    dv = [f["detector_vs_truth"] for f in files if f.get("detector_vs_truth")]
    if dv:
        g["detector.recall"] = q([d["recall"] for d in dv])
        g["detector.precision"] = q([d["precision"] for d in dv])
        g["detector.lag_ms"] = q([d["lag_ms"] for d in dv])
    return g


ROWS = [("Fast notes (IOI < 0.3 s), per onset", None),
        ("noise burst 2-8 kHz re note's steady noise, dB", "fast.burst_db", 1),
        ("noise-to-harmonic burst 2-8 kHz re steady, dB", "fast.burst_nhr_db", 1),
        ("burst length (> steady + 3 dB), ms", "fast.burst_ms", 0),
        ("steady noise-to-harmonic 2-8 kHz, dB", "fast.nhr_steady_db", 1),
        ("share of fast notes starting an octave off (first 20 ms)", "fast.octave_start", 2),
        ("pitch |cents| 0-20 ms (octave-off frames left out)", "fast.dev0_20", 0), ("pitch signed cents 0-20 ms", "fast.dev0_20_signed", 0),
        ("pitch |cents| 20-40 ms", "fast.dev20_40", 0), ("pitch settle (within 20 c), ms", "fast.settle_ms", 0),
        ("steady share of the note", "fast.steady_share", 2),
        ("level dip at the change, dB", "fast.dip_db", 1), ("dip length (< lower note - 3 dB), ms", "fast.dip_ms", 0),
        ("previous pitch at +30 ms re before, dB", "fast.ring30_db", 1),
        ("previous pitch at +60 ms re before, dB", "fast.ring60_db", 1),
        ("share of onsets with a flux (bow) cue", "fast.flux_cue_share", 2),
        ("IOI < 0.2 s only: noise burst, dB", "fast200.burst_db", 1),
        ("IOI < 0.2 s only: burst length, ms", "fast200.burst_ms", 0),
        ("IOI < 0.2 s only: settle, ms", "fast200.settle_ms", 0),
        ("IOI < 0.2 s only: steady share", "fast200.steady_share", 2),
        ("IOI < 0.2 s only: dip, dB", "fast200.dip_db", 1),
        ("share of fast notes with a second pitch (>= 30 % of the note)", "fast.dyad_note_share", 2),
        ("single-voice fast notes: noise burst, dB", "fast1.burst_db", 1),
        ("single-voice fast notes: burst length, ms", "fast1.burst_ms", 0),
        ("single-voice fast notes: steady NHR 2-8 kHz, dB", "fast1.nhr_steady_db", 1),
        ("single-voice fast notes: steady share", "fast1.steady_share", 2),
        ("single-voice fast notes: settle, ms", "fast1.settle_ms", 0),
        ("single-voice fast notes: dip, dB", "fast1.dip_db", 1),
        ("single-voice fast notes: previous pitch +60 ms, dB", "fast1.ring60_db", 1),
        ("Timbre (per file; 78s excluded)", None),
        ("spectral centroid, Hz", "timbre.centroid_hz", 0), ("centroid / f0", "timbre.centroid_over_f0", 2),
        ("energy above 2 kHz, dB re total", "timbre.share_above_2k_db", 1),
        ("energy above 4 kHz, dB re total", "timbre.share_above_4k_db", 1),
        ("Noise in steady notes (per file medians)", None),
        ("HNR broadband, G range, dB", "noise.G.hnr_bb_db", 1), ("HNR broadband, D range", "noise.D.hnr_bb_db", 1),
        ("HNR broadband, A range", "noise.A.hnr_bb_db", 1), ("HNR broadband, E range", "noise.E.hnr_bb_db", 1),
        ("HNR 2-8 kHz, D range", "noise.D.hnr_2_8k_db", 1), ("HNR 2-8 kHz, A range", "noise.A.hnr_2_8k_db", 1),
        ("HNR 2-8 kHz, E range", "noise.E.hnr_2_8k_db", 1),
        ("Rhythm in runs (per run)", None),
        ("swing (long/short eighth)", "rhythm.swing", 2), ("IOI spread after swing, ms sd", "rhythm.ioi_sd_ms", 1),
        ("accent: long-IOI note minus short, dB", "rhythm.accent_long_minus_short_db", 1),
        ("note-peak level spread, dB sd", "rhythm.level_sd_db", 1), ("tempo, quarter bpm", "rhythm.tempo_bpm", 0),
        ("median IOI per file, s", "notes.ioi_median_s", 3), ("share of IOIs < 0.2 s per file", "notes.fast_share", 2),
        ("Vibrato, slides, intonation", None),
        ("share of notes >= 0.3 s with vibrato", "vibrato.share", 2), ("vibrato extent p10-p90, cents", "vibrato.extent_c", 0),
        ("vibrato rate, Hz", "vibrato.rate_hz", 1), ("share of notes with a slide in (50-400 c)", "slides.share", 2),
        ("share of notes starting an octave off", "slides.octave_start_share", 2),
        ("slide start, cents", "slides.from_c", 0), ("slide length, ms", "slides.ms", 0),
        ("third: cents above the minor third (per file)", "intonation.third_above_minor_c", 0),
        ("third: neutral share (25-75 c)", "intonation.third_neutral_share", 2),
        ("seventh: cents above the minor seventh", "intonation.seventh_above_minor_c", 0),
        ("seventh: neutral share", "intonation.seventh_neutral_share", 2),
        ("Room", None), ("decay after the sound stops, T60 s", "room.t60_s", 2),
        ("Onset detector against our notes (ours only)", None),
        ("recall (35 ms)", "detector.recall", 2), ("precision", "detector.precision", 2),
        ("lag render - note start, ms", "detector.lag_ms", 0)]

DROWS = [("share of sounding frames with two pitches", "double.dyad_share_pooled", 2),
         ("dyad segments >= 150 ms", "double.segments", 0),
         ("voices' envelope correlation, raw", "double.corr_raw", 2),
         ("voices' envelope correlation, 100 ms trend removed", "double.corr_fast", 2),
         ("share of dips shared by both voices (within 15 ms)", "double.dips_shared", 2),
         ("level lower minus upper voice, dB", "double.level_lo_minus_hi_db", 1),
         ("interval minus equal temperament, cents", "double.et_dev_c", 1),
         ("|interval - ET|, cents", "double.abs_et_dev_c", 1),
         ("|interval - nearest just ratio|, cents", "double.abs_just_dev_c", 1),
         ("interval wobble in a segment, cents sd", "double.interval_sd_c", 1),
         ("onset lower minus upper voice, ms", "double.onset_lo_minus_hi_ms", 0),
         ("|onset offset|, ms", "double.onset_abs_ms", 0)]


def write(out):
    out = Path(out)
    files = [json.loads(p.read_text(encoding="utf-8")) for p in sorted((out / "files").glob("*.json"))
             if p.name != "ours-stems.json"]
    by = {}
    for f in files:
        by.setdefault(f["group"], []).append(f)
    res = {k: pool(v) for k, v in by.items()}
    res["_files"] = {k: [f["path"] for f in v] for k, v in by.items()}
    st = out / "files" / "ours-stems.json"
    if st.exists():
        res["ours-stems"] = json.loads(st.read_text(encoding="utf-8"))
    (out / "fiddle_measures.json").write_text(json.dumps(res, indent=1), encoding="utf-8")
    L = ["# The fiddle study: real fiddle vs classical violin vs Arcus", "",
         "median [p25, p75] (n). Generated by tools/arcus/fiddle/fiddle_measure.py.", ""]
    cols = [c for c in COLS if c in res]
    L.append("| measure | " + " | ".join(cols) + " |")
    L.append("|---|" + "---|" * len(cols))
    for row in ROWS:
        if row[1] is None:
            L.append(f"| **{row[0]}** |" + " |" * len(cols))
            continue
        L.append(f"| {row[0]} | " + " | ".join(
            "-" if ("78s" in c and any(w in row[1] for w in NOISE)) else fmt(res[c].get(row[1]), row[2])
            for c in cols) + " |")
    L += ["", "## Double stops", ""]
    dcols = [c for c in DCOLS if c in res]
    L.append("| measure | " + " | ".join(dcols) + " |")
    L.append("|---|" + "---|" * len(dcols))
    for name, key, d in DROWS:
        L.append(f"| {name} | " + " | ".join(
            (str(res[c].get(key, {}).get("n", "-")) if key == "double.segments" else fmt(res[c].get(key), d))
            for c in dcols) + " |")
    L.append("| interval classes (share of dyad frames) | " + " | ".join(
        ", ".join(f"{k} {v:.2f}" for k, v in res[c].get("double.classes", {}).items()) for c in dcols) + " |")
    if "ours-stems" in res:
        L += ["", "## Our double stops, stem against stem (exact)", "", "```", json.dumps(res["ours-stems"], indent=1), "```"]
    (out / "fiddle_measures.md").write_text("\n".join(L) + "\n", encoding="utf-8")
    print("wrote", out / "fiddle_measures.md")
