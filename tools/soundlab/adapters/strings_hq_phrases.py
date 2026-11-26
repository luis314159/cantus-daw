"""Real players' phrases for the bowed-strings subjects (round 42, strings-refs): which
stretches of the Musopen Mozart K465 session (`$CANTUS_REFS/strings-hq/musopen-k465`, public
domain) and of HSD's K465 stems (`study/hsd-string-quartet-k465`, CC BY 4.0) each subject
reads, and the phrase measures on them.

    python -I tools/soundlab/adapters/strings_hq_phrases.py pick <refs root> <subjects dir>
    python -I tools/soundlab/adapters/strings_hq_phrases.py measure <subject dir> <refs root> <out.json> [name=<wav>:<family> ...]

`pick`: the Musopen session has one spot microphone per player ("1st violin F", "2nd violin
F", "viola F", "celli F" — the session template's names; this is a quartet), the four takes
sample-aligned. Each mic also hears the other three: when a player rests its own mic drops only
~13 dB, so the others' bleed sits ~10-13 dB under the player. Per 50 ms frame, a player's
**dominance** is its own mic's level (re that mic's 95th percentile: the gains differ) over the
loudest of the other three mics, measured the same way; the windows (8 s, 1 s steps) where the
player sounds (within 25 dB of its loud level) in at least 80 % of the frames and its median
dominance is highest are its phrases (at most three per take, none overlapping, a dominance of
1 dB at least: K465 is dense, no player is alone for long). Writes `<subjects dir>/arcus-<family>/phrases.json`; HSD's
stems are added whole (they were recorded one instrument at a time over a guide track).

`measure`: per phrase, the arcus-expr phrase study's note cutter and per-note measures
(`tools/arcus/expr/phrase_study.py`: YIN pitch, 5 ms hop; notes cut where the semitone changes
for 60 ms): the pitch of each note of 0.25 s or more against the nearest semitone (and against
the phrase's own tuning, its median), the vibrato's extent and onset and the level envelope on
the notes of 0.6 s or more. Each phrase is read at 48 kHz. The extra `name=<wav>:<family>`
items (e.g. Arcus's solo renders) are measured the same way, for the comparison.
"""
import json
import sys
from pathlib import Path

import numpy as np
import soundfile as sf
from scipy import signal

HERE = Path(__file__).resolve()
sys.path.insert(0, str(HERE.parents[2] / "arcus" / "expr"))
import phrase_study as ps  # noqa: E402

MUSOPEN = "strings-hq/musopen-k465"
HSD = "study/hsd-string-quartet-k465/Streicherquartett_Complete_Project/Audiodateien"
SPOTS = {"violin1": "1st violin F", "violin2": "2nd violin F", "viola": "viola F", "cello": "celli F"}
FAMILY = {"violin1": "violin", "violin2": "violin", "viola": "viola", "cello": "cello"}
# The session numbers each track's takes on its own: the cello's counter runs one behind (its
# _01 is the others' _02, the same length to the sample). A take, by the violins' and the
# viola's number -> each player's file number.
TAKES = {"02": {"violin1": "02", "violin2": "02", "viola": "02", "cello": "01"},
         "05": {"violin1": "05", "violin2": "05", "viola": "05", "cello": "04"},
         "09": {"violin1": "09", "violin2": "09", "viola": "09", "cello": "08"}}
FRAME, WIN, STEP, MIN_DB, PER_TAKE = 0.05, 8.0, 1.0, 1.0, 3
MUSOPEN_CREDIT = ("Musopen (musopen.org), Mozart String Quartet in C major K. 465, the 2012 Kickstarter "
                  "session (archive.org item musopen-mozart-quartet-in-c-major-k465-compressed), "
                  "released into the public domain")
HSD_CREDIT = "D. Leckschat, C. Epe, Hochschule Duesseldorf: string-quartet recording (Mozart K465), Zenodo 3601097"
HSD_STEMS = {
    "violin": ["Violin Phrase 1/Violin_Phrase_1_Vl2.wav", "Violin Phrase 2/Violin_Phrase_2_Vl2.wav",
               "Violin Phrase 3/Violin_Phrase_3_Vl1.wav", "Quartet Long Excerpt/Quartet_Long_Vl1.wav"],
    "viola": ["Quartet Phrase 1/Quartet_Phrase_1_Vla.wav", "Quartet Phrase 2/Quartet_Phrase_2_Vla.wav",
              "Quartet Phrase 3/Quartet_Phrase_3_Vla.wav", "Quartet Long Excerpt/Quartet_Long_Vla.wav"],
    "cello": ["Cello Phrase 1/Cello_Phrase_1_Vlc.wav", "Cello Phrase 2/Cello_Phrase_2_Vlc.wav",
              "Cello Phrase 3/Cello_Phrase_3_Vlc.wav", "Quartet Long Excerpt/Quartet_Long_Vlc.wav"],
}


def levels(path):
    """Frame levels (dB) of a long file, read in blocks."""
    info = sf.info(str(path))
    n = int(round(FRAME * info.samplerate))
    out = []
    for b in sf.blocks(str(path), blocksize=n, dtype="float64", always_2d=True):
        x = b[:, 0]
        out.append(10 * np.log10(np.mean(x * x) + 1e-20))
    return np.array(out), info


def pick_take(root, take):
    lv, infos, files = {}, {}, {}
    for p, name in SPOTS.items():
        files[p] = f"{MUSOPEN}/{name}_{TAKES[take][p]}.wav"
        lv[p], infos[p] = levels(root / files[p])
        lv[p] = lv[p] - np.percentile(lv[p], 95)  # the mics' gains differ: each re its own loud level
    if len({i.frames for i in infos.values()}) != 1:
        raise SystemExit(f"take {take}: the four spot files differ in length: "
                         + ", ".join(f"{files[p]} {infos[p].frames}" for p in SPOTS))
    n = min(len(v) for v in lv.values())
    lv = {p: v[:n] for p, v in lv.items()}
    out = []
    w, s = int(WIN / FRAME), int(STEP / FRAME)
    for p in SPOTS:
        others = np.max(np.stack([lv[q] for q in SPOTS if q != p]), axis=0)
        dom = lv[p] - others
        active = lv[p] > -25
        cands = []
        for a in range(0, n - w, s):
            act = active[a:a + w]
            if act.mean() < 0.8:
                continue
            cands.append((float(np.median(dom[a:a + w][act])), a))
        cands.sort(reverse=True)
        chosen = []
        for d, a in cands:
            if d < MIN_DB or len(chosen) == PER_TAKE:
                break
            if all(abs(a - b) >= w for _, b in chosen):
                chosen.append((d, a))
        for d, a in sorted(chosen, key=lambda c: c[1]):
            info = infos[p]
            out.append({"path": files[p], "take": take, "player": p, "family": FAMILY[p],
                        "t0": round(a * FRAME, 2), "t1": round(a * FRAME + WIN, 2),
                        "dominance_db": round(d, 1), "source": "musopen", "licence": "PD",
                        "credit": MUSOPEN_CREDIT, "inst": f"musopen-k465-{p}",
                        "format": f"{info.samplerate} Hz, {info.subtype}, mono; spot microphone"})
    return out


def pick(root, subjects):
    root, subjects = Path(root), Path(subjects)
    found = []
    for take in TAKES:
        got = pick_take(root, take)
        print(f"take {take}: " + ", ".join(f"{e['player']} {e['t0']:.0f}-{e['t1']:.0f} s ({e['dominance_db']} dB)"
                                         for e in got), flush=True)
        found += got
    for fam in ("violin", "viola", "cello"):
        rows = [e for e in found if e["family"] == fam]
        for rel in HSD_STEMS[fam]:
            p = root / HSD / rel
            info = sf.info(str(p))
            rows.append({"path": f"{HSD}/{rel}", "player": Path(rel).stem.rsplit("_", 1)[1].lower(),
                         "family": fam, "t0": None, "t1": None, "dominance_db": None, "source": "hsd",
                         "licence": "CC-BY-4.0", "credit": HSD_CREDIT,
                         "inst": f"hsd-k465-{Path(rel).stem.rsplit('_', 1)[1].lower()}",
                         "format": f"{info.samplerate} Hz, {info.subtype}, mono; spot microphone, one instrument at a time"})
        d = subjects / f"arcus-{fam}"
        d.mkdir(parents=True, exist_ok=True)
        (d / "phrases.json").write_text(json.dumps(rows, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
        print(f"{fam}: {len(rows)} phrases -> {d / 'phrases.json'}")


def read48(path, t0=None, t1=None):
    info = sf.info(str(path))
    a = int(t0 * info.samplerate) if t0 is not None else 0
    b = int(t1 * info.samplerate) if t1 is not None else -1
    x, sr = sf.read(str(path), start=a, stop=b if b > 0 else None, dtype="float64", always_2d=True)
    x = x[:, 0]
    if sr != 48000:
        g = np.gcd(48000, sr)
        x = signal.resample_poly(x, 48000 // g, sr // g)
    return np.ascontiguousarray(x), 48000


def one(x, sr, fam):
    """The analysis's phrase measures (cantus-work/arcus-analisis/scripts/phrase.py), kept."""
    rows = ps.per_note(ps.notes_of(x, sr, fam))
    dev = np.array([100 * (r["midi"] - round(r["midi"])) for r in rows if r["dur"] >= 0.25])
    off = float(np.median(dev)) if len(dev) else None
    rel = dev - (off or 0.0)
    long = [r for r in rows if r["dur"] >= 0.6]
    return {"notes": len(rows), "notes_025": int(len(dev)), "tuning_offset_cents": off,
            "dev": [round(float(v), 2) for v in dev], "rel": [round(float(v), 2) for v in rel],
            "vib_pp": [r["vib_pp"] for r in long], "vib_onset": [r["vib_onset"] for r in long if (r["vib_pp"] or 0) >= 8],
            "env_start": [r["env"][1] for r in long if r["env"]], "env_end": [r["env"][2] for r in long if r["env"]],
            "long": len(long)}


def pooled(items):
    """Pool the per-note values of several phrases (one player or one source)."""
    cat = {k: [v for it in items for v in it[k] if v is not None] for k in
           ("dev", "rel", "vib_pp", "vib_onset", "env_start", "env_end")}
    a = lambda k: np.abs(np.array(cat[k])) if cat[k] else np.array([])  # noqa: E731
    long = sum(it["long"] for it in items)
    return {"phrases": len(items), "notes_025": len(cat["dev"]), "notes_long": long,
            "tuning_offset_cents_median": ps.q([it["tuning_offset_cents"] for it in items], (50,)),
            "rel_dev_abs_cents": ps.q(list(a("rel")), (50, 90)),
            "vib_pp_cents": ps.q(cat["vib_pp"]),
            "vib_share_over_8ct": round(float(np.mean([(v or 0) >= 8 for v in cat["vib_pp"]])), 3) if cat["vib_pp"] else None,
            "vib_onset_s": ps.q(cat["vib_onset"]),
            "env_start_under_peak_db": ps.q(cat["env_start"]),
            "env_end_under_peak_db": ps.q(cat["env_end"])}


def measure(subject_dir, root, out, extra):
    root = Path(root)
    phrases = json.loads((Path(subject_dir) / "phrases.json").read_text(encoding="utf-8"))
    res, by = {"phrases": [], "pooled": {}}, {}
    for e in phrases:
        x, sr = read48(root / e["path"], e["t0"], e["t1"])
        m = one(x, sr, e["family"])
        res["phrases"].append(dict(e, measures={k: m[k] for k in ("notes", "notes_025", "tuning_offset_cents", "long")}))
        by.setdefault(e["source"], []).append(m)
        by.setdefault("all", []).append(m)
        print(f"  {e['inst']} {e['path'].rsplit('/', 1)[1]} {e['t0']}: {m['notes_025']} notes, {m['long']} long", flush=True)
    for spec in extra:
        name, rest = spec.split("=", 1)
        path, fam = rest.rsplit(":", 1)
        x, sr = read48(Path(path))
        by[name] = [one(x, sr, fam)]
    res["pooled"] = {k: pooled(v) for k, v in by.items()}
    Path(out).write_text(json.dumps(res, indent=1) + "\n", encoding="utf-8")
    for k, v in res["pooled"].items():
        print(f"{k:28s} {v['notes_025']:4d} notes; |rel dev| p50/p90 {v['rel_dev_abs_cents']}; vib p-p {v['vib_pp_cents']}; "
              f"onset {v['vib_onset_s']}; start {v['env_start_under_peak_db']}")


if __name__ == "__main__":
    a = sys.argv[1:]
    if a and a[0] == "pick":
        pick(a[1], a[2])
    elif a and a[0] == "measure":
        measure(a[1], a[2], a[3], a[4:])
    else:
        print(__doc__)
        sys.exit(2)
