#!/usr/bin/env python3
"""The whole measurement in one command (round 42, tympanum-real).

    python -I tools/tympanum-real/pipeline.py hits   <work> [--only kit,kit] [--tag before]
    python -I tools/tympanum-real/pipeline.py grooves <work> [--only …] [--tag before]
    python -I tools/tympanum-real/pipeline.py compare <work> <out> [--tags before,after]

`hits` renders every kit of `KITS` through `tympanum_real hits` (the release example; set
`TYMPANUM_REAL_EXE` for another build) into `<work>/hits/<tag>/<kit>.wav`, measures each into
its `.jsonl`; a recorded kit is rendered once (`<work>/hits/ref/`), Tympanum's under the tag.
`grooves` plays the Groove MIDI Dataset excerpts of `GROOVES` through every kit
(`<work>/grooves/<tag>/<groove>--<kit>.wav`). `compare` writes `compare.py`'s tables and
charts for the references and the tags asked for. Kits whose pack is not in the content root
are skipped with a word.
"""
import json
import os
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
sys.path.insert(0, str(HERE.parent / "sound-refs"))
import cantus_paths  # noqa: E402

REFS = cantus_paths.refs()
ROOT = REFS / "drums" / "content"
GMD = REFS / "drums" / "gmd" / "groove"
EXE = os.environ.get("TYMPANUM_REAL_EXE", str(REPO / "target" / "release" / "examples" / "tympanum_real.exe"))
PY = sys.executable

KINDS = "kick,snare,racktom,floortom,hihat,crash,ride"
VELS = "16,32,48,64,80,100,127"  # the soft end dense: 41 % of real snare strokes are under 40
SLOT = "4"
# id, kit text, label, genre, role
KITS = [
    ("drskit", "pack:drskit-tympanum/kit", "DRSKit", "rock/pop (estudio)", "ref"),
    ("muldjord", "pack:dg-muldjordkit/kit", "MuldjordKit", "metal/rock", "ref"),
    ("aasimonster", "pack:dg-aasimonster/kit", "Aasimonster", "death metal", "ref"),
    ("shittykit", "pack:dg-shittykit/kit", "ShittyKit", "lo-fi/garage", "ref"),
    ("bigrusty", "pack:big-rusty-drums/sticks", "Big Rusty", "rock 80s", "ref"),
    ("gogodze", "pack:gogodze-phu-vol-ii/kit", "Gogodze Phu II", "funk/soul 70s", "ref"),
    ("unruly", "pack:unruly-drums/sticks", "Unruly", "indie pop", "ref"),
    ("unruly-brushes", "pack:unruly-drums/brushes", "Unruly escobillas", "jazz (escobillas)", "ref"),
    ("virtuosity", "pack:virtuosity-drums/kit", "Virtuosity", "jazz", "ref"),
    ("studio", "model:studio", "Studio Maple", "Tympanum", "tympanum"),
    ("jazz", "model:jazz", "Jazz Birch", "Tympanum", "tympanum"),
]
# name, GMD file, start s, length s, which kits play it
GROOVES = [
    ("rock", "drummer8/session2/32_rock_117_beat_4-4.mid", 8, 20),
    ("pop", "drummer7/session3/11_pop-soft_83_beat_4-4.mid", 6, 22),
    ("funk", "drummer1/session1/102_funk_95_beat_4-4.mid", 10, 20),
    ("jazz", "drummer10/session1/10_jazz-swing_110_beat_4-4.mid", 8, 20),
    ("samba", "drummer1/session1/28_latin-samba_116_beat_4-4.mid", 10, 18),
    ("soul", "drummer5/session2/11_soul_96_beat_4-4.mid", 8, 20),
    ("gospel", "drummer1/session2/142_gospel_120_beat_4-4.mid", 8, 18),
    ("punk", "drummer1/session2/62_punk_144_beat_4-4.mid", 8, 16),
    ("hiphop", "drummer8/session2/9_hiphop_86_beat_4-4.mid", 8, 20),
]


def arg(name, default=None):
    return sys.argv[sys.argv.index(name) + 1] if name in sys.argv else default


def present(kit):
    if kit.startswith("model:"):
        return True
    pack = kit.split(":", 1)[1].split("/")[0]
    return (ROOT / pack / "cantus-pack.json").exists()


def chosen():
    only = arg("--only")
    ks = [k for k in KITS if not only or k[0] in only.split(",")]
    out = []
    for k in ks:
        if present(k[1]):
            out.append(k)
        else:
            print(f"skip {k[0]}: {k[1]} not in {ROOT}")
    return out


def run(cmd):
    print("$", " ".join(str(c) for c in cmd), flush=True)
    subprocess.run([str(c) for c in cmd], check=True)


def info(work):
    p = work / "info.jsonl"
    have = {json.loads(l)["kit"] for l in open(p, encoding="utf-8")} if p.exists() else set()
    need = [k[1] for k in KITS if present(k[1]) and k[1] not in have]
    if need:
        r = subprocess.run([EXE, "info", str(ROOT)] + need, capture_output=True, text=True, check=True)
        with open(p, "a", encoding="utf-8") as fh:
            fh.write(r.stdout)
    return p


def hits(work):
    tag = arg("--tag", "before")
    sets = arg("--set", "")
    for kid, kit, *_ , role in chosen():
        d = work / "hits" / ("ref" if role == "ref" else tag)
        d.mkdir(parents=True, exist_ok=True)
        wav = d / f"{kid}.wav"
        if not wav.with_suffix(".jsonl").exists() or role != "ref":
            cmd = [EXE, "hits", ROOT, wav, kit, "--kinds", KINDS, "--vels", VELS, "--slot", SLOT]
            if sets and role != "ref":
                cmd += ["--set", sets]
            run(cmd)
        if not wav.with_suffix(".jsonl").exists() or role != "ref":
            run([PY, "-I", HERE / "measure.py", wav])
            if os.environ.get("TYMPANUM_REAL_KEEP") != "1":
                wav.unlink()  # 0.5-1 GB of float a kit: the measures are what is kept


def grooves(work):
    tag = arg("--tag", "before")
    sets = arg("--set", "")
    ip = info(work)
    for kid, kit, *_ , role in chosen():
        d = work / "grooves" / ("ref" if role == "ref" else tag)
        d.mkdir(parents=True, exist_ok=True)
        for name, mid, start, ln in GROOVES:
            wav = d / f"{name}--{kid}.wav"
            if wav.exists() and role == "ref":
                continue
            notes = work / "grooves" / f"{name}--{kid}.csv"
            run([PY, "-I", HERE / "midi.py", GMD / mid, ip, kit, notes, "--start", start, "--len", ln])
            cmd = [EXE, "notes", ROOT, wav, kit, notes]
            if sets and role != "ref":
                cmd += ["--set", sets]
            run(cmd)


def compare(work, out):
    tags = arg("--tags", "before").split(",")
    spec = []
    for kid, kit, label, genre, role in KITS:
        if role == "ref":
            p = work / "hits" / "ref" / f"{kid}.jsonl"
            if p.exists():
                spec.append({"id": kid, "label": label, "genre": genre, "role": "ref", "hits": str(p)})
        else:
            for t in tags:
                p = work / "hits" / t / f"{kid}.jsonl"
                if p.exists():
                    spec.append({"id": f"{kid}-{t}", "label": f"{label} ({t})", "genre": genre,
                                 "role": "before" if t == tags[0] else "after", "hits": str(p)})
    sp = Path(out) / "kits.json"
    sp.parent.mkdir(parents=True, exist_ok=True)
    sp.write_text(json.dumps(spec, indent=1), encoding="utf-8")
    run([PY, "-I", HERE / "compare.py", sp, out])


if __name__ == "__main__":
    mode, work = sys.argv[1], Path(sys.argv[2])
    work.mkdir(parents=True, exist_ok=True)
    {"hits": hits, "grooves": grooves}.get(mode, lambda w: compare(w, sys.argv[3]))(work)
