"""The owner's named listening pairs for the bowed strings (round 42, strings-refs): a better
reference beside the matching Arcus clip, per instrument, 16-bit 48 kHz, loudness-matched.

    python -I tools/listening/strings_hq_pairs.py <refs root> <arcus renders> <arcus phrases> <out dir>

- <arcus renders>: `arcus_match render <dir> cello bass violin viola` (the lab's held-out cells).
- <arcus phrases>: a folder with `ahora-<violin|viola|violonchelo|contrabajo>.wav` (Arcus's
  cantabile phrase, as the owner last heard it).
Per instrument: **a held note** at mf, the Iowa MIS 2012 recording (anechoic, 24/96) of the same
key cut from its run, beside Arcus's render of that key (and, for the bass, the Good-sounds bass
too); **a phrase** (violin, viola, cello), the Musopen K465 spot-mic window where that player
dominates most (`tools/soundlab/subjects/arcus-<family>/phrases.json`), beside Arcus's phrase.
Each clip: mono, 48 kHz, fades 10 ms in and 80 ms out, integrated loudness -23 LUFS (ITU-R
BS.1770-4, the lab's meter); a pair is lowered together if either would pass -1 dBTP; TPDF
dither to 16 bits. `pairs.json` beside the clips records every source, cut and level.
"""
import json
import sys
from pathlib import Path

import numpy as np
import soundfile as sf
from scipy import signal

LAB = Path(__file__).resolve().parents[1] / "soundlab"
sys.path.insert(0, str(LAB))
from soundlab import loudness  # noqa: E402
from soundlab.core import segment  # noqa: E402
from soundlab.notes import _f0_steady  # noqa: E402

SR, TARGET, CEIL = 48000, -23.0, -1.0
ES = {"violin": "violin", "viola": "viola", "cello": "violonchelo", "bass": "contrabajo"}
# One held-out mf cell each ((key + 1) % 3 == 0): a stopped note in the middle of the range.
KEY = {"violin": 71, "viola": 65, "cello": 53, "bass": 41}
F_RANGE = {"violin": (180, 3200), "viola": (120, 2200), "cello": (60, 1100), "bass": (30, 600)}
NAMES = "C C# D D# E F F# G G# A A# B".split()


def name(k):
    return f"{NAMES[k % 12]}{k // 12 - 1}"


def read48(path, t0=None, t1=None):
    info = sf.info(str(path))
    a = int(t0 * info.samplerate) if t0 is not None else 0
    b = int(t1 * info.samplerate) if t1 is not None else None
    x, sr = sf.read(str(path), start=a, stop=b, dtype="float64", always_2d=True)
    x = x.mean(axis=1)
    if sr != SR:
        g = np.gcd(SR, sr)
        x = signal.resample_poly(x, SR // g, sr // g)
    return x, info


def iowa_note(root, fam, key):
    """The longest note at `key` in the mf arco runs (a key is on two strings' runs), cut at
    its silences."""
    best = None
    for p in sorted((root / "strings-hq" / "iowa2012" / fam).glob("*.arco.*mf*.aif")):
        if ".mf." not in p.name:
            continue
        x, info = read48(p)
        for a, b in segment(x, SR):
            f = _f0_steady(x[a:b], SR, *F_RANGE[fam])
            if f and round(69 + 12 * np.log2(f / 440.0)) == key and (best is None or b - a > len(best[0])):
                best = (x[a:b], p, a / SR, info)
    if best is None:
        raise SystemExit(f"{fam}: no Iowa 2012 mf arco note at {name(key)}")
    return best


def gs_bass_note(root, key):
    import sqlite3
    db = root / "study/good-sounds/zenodo/good-sounds/database.sqlite"
    c = sqlite3.connect(f"file:{db.as_posix()}?mode=ro", uri=True)
    rows = list(c.execute("select s.note, s.octave, t.filename from sounds s join takes t on t.sound_id = s.id "
                          "where s.instrument = 'bass' and trim(s.klass) = 'good-sound-bow' and t.microphone = 'neumann'"))
    c.close()
    for note, octave, fn in rows:
        if 12 * (int(octave) + 1) + NAMES.index(note.strip()) == key:
            p = root / "strings-hq" / "good-sounds-bass" / fn
            x, info = read48(p)
            seg = segment(x, SR, single=True)
            a, b = seg[0] if seg else (0, len(x))
            return x[a:b], p, a / SR, info
    raise SystemExit(f"bass: no Good-sounds bowed note at {name(key)}")


def shape(x, max_s):
    x = x[: int(max_s * SR)].copy()
    fi, fo = int(0.01 * SR), int(0.08 * SR)
    x[:fi] *= np.linspace(0, 1, fi)
    x[-fo:] *= np.linspace(1, 0, fo)
    return x


def write_pair(out, clips):
    """clips: [(file name, x)] -> loudness-matched 16-bit files; returns their levels."""
    gains = [TARGET - loudness.lufs(x, SR) for _, x in clips]
    tps = [loudness.true_peak_db(x * 10 ** (g / 20)) for (_, x), g in zip(clips, gains)]
    down = min(0.0, CEIL - max(tps))
    rng = np.random.default_rng(42)
    lv = []
    for (fn, x), g in zip(clips, gains):
        y = x * 10 ** ((g + down) / 20)
        lsb = 1 / 32768
        y = y + (rng.random(len(y)) - rng.random(len(y))) * lsb
        sf.write(str(out / fn), np.clip(y, -1, 1 - lsb), SR, subtype="PCM_16")
        z, _ = sf.read(str(out / fn), dtype="float64")
        lv.append({"file": fn, "lufs": round(loudness.lufs(z, SR), 2), "true_peak_dbtp": round(loudness.true_peak_db(z), 2),
                   "seconds": round(len(z) / SR, 2)})
    return lv


def main(root, renders, phrases_dir, out):
    root, renders, phrases_dir, out = Path(root), Path(renders), Path(phrases_dir), Path(out)
    out.mkdir(parents=True, exist_ok=True)
    subjects = LAB / "subjects"
    log = {}
    for fam in ("violin", "viola", "cello", "bass"):
        k, es = KEY[fam], ES[fam]
        rx, rp, rt, ri = iowa_note(root, fam, k)
        ax, _ = read48(renders / f"arcus-{fam}-arco-{k}-mf.wav")
        a0 = segment(ax, SR, single=True)[0][0]
        clips = [(f"{es}-nota-grabacion.wav", shape(rx, 4.0)), (f"{es}-nota-arcus.wav", shape(ax[a0:], 4.0))]
        src = [{"file": clips[0][0], "path": rp.relative_to(root).as_posix(), "from_s": round(rt, 3),
                "format": f"{ri.samplerate} Hz {ri.subtype} mono", "licence": "Iowa-MIS"},
               {"file": clips[1][0], "path": f"arcus_match render: arcus-{fam}-arco-{k}-mf.wav"}]
        if fam == "bass":
            gx, gp, gt, gi = gs_bass_note(root, k)
            clips.append((f"{es}-nota-grabacion2.wav", shape(gx, 4.0)))
            src.append({"file": clips[2][0], "path": gp.relative_to(root).as_posix(), "from_s": round(gt, 3),
                        "format": f"{gi.samplerate} Hz {gi.subtype} mono", "licence": "CC-BY-4.0"})
        log[f"{fam}-nota"] = {"key": name(k), "sources": src, "levels": write_pair(out, clips)}
        if fam == "bass":
            continue
        ph = json.loads((subjects / f"arcus-{fam}" / "phrases.json").read_text(encoding="utf-8"))
        best = max((e for e in ph if e["source"] == "musopen"), key=lambda e: e["dominance_db"])
        px, pi = read48(root / best["path"], best["t0"], best["t1"])
        qx, _ = read48(phrases_dir / f"ahora-{es}.wav")
        clips = [(f"{es}-frase-grabacion.wav", shape(px, 12.0)), (f"{es}-frase-arcus.wav", shape(qx, 15.0))]
        log[f"{fam}-frase"] = {"sources": [dict(best, file=clips[0][0]),
                                           {"file": clips[1][0], "path": str(phrases_dir / f"ahora-{es}.wav")}],
                               "levels": write_pair(out, clips)}
        print(f"{fam}: note {name(k)} from {rp.name}; phrase {best['path'].rsplit('/', 1)[1]} "
              f"{best['t0']}-{best['t1']} s ({best['dominance_db']} dB)", flush=True)
    (out / "pairs.json").write_text(json.dumps(log, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"-> {out}")


if __name__ == "__main__":
    if len(sys.argv) != 5:
        print(__doc__)
        sys.exit(2)
    main(*sys.argv[1:5])
