#!/usr/bin/env python3
"""Model 6's onset against the recorded kits, one trial at a time (round 42, tympanum-real).

    python -I tools/tympanum-real/onset_fit.py <work> <tag> [--m6 "k=v,…"] [--set "model=2"]
                                               [--grooves rock,jazz,…] [--keep]

Renders Tympanum's studio and jazz kits' kicks, snares and toms (`tympanum_real hits`, the
build in `TYMPANUM_REAL_EXE`, a `tune` build to take `--m6` as `CANTUS_M6`) into
`<work>/onset/<tag>/`, measures each band's peak time and the first 10 ms's crest
(`onset.py`) against the recorded kits' (`<work>/onset/ref/*.wav`, measured once into
`ref.json`), and plays the grooves asked for through both kits to measure their crest
(peak − loudness) against the recordings' (`<work>/grooves/measures.json`). Prints, per piece
and velocity group, the model's medians beside the recordings' median and range, and a score:
the mean |log2(model / recorded median)| of the band peaks, the crest10's mean distance (dB)
and each groove's crest over the recordings' highest.
"""
import json
import os
import subprocess
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import onset  # noqa: E402
from grooves import measure as groove_measure  # noqa: E402

EXE = os.environ.get("TYMPANUM_REAL_EXE", "")
ROOT = Path(r"F:\projects\cantus-refs\drums\content")
KEYS = ["lo", "mid", "hm", "hi", "wave", "crest10"]


def arg(n, d=None):
    return sys.argv[sys.argv.index(n) + 1] if n in sys.argv else d


def table(res):
    out = {}
    for kind in onset.KINDS:
        for g, lo, hi in onset.GROUPS:
            s = [r for r in res if r["kind"] == kind and lo <= r["vel"] <= hi]
            if s:
                out[(kind, g)] = {k: float(np.median([r[k] for r in s])) for k in KEYS}
    return out


def main():
    work, tag = Path(sys.argv[1]), sys.argv[2]
    od = work / "onset"
    refj = od / "ref.json"
    if not refj.exists():
        refs = {w.stem: onset.measure(w) for w in sorted((od / "ref").glob("*.wav"))}
        refj.write_text(json.dumps(refs), encoding="utf-8")
    refs = {k: table(v) for k, v in json.loads(refj.read_text(encoding="utf-8")).items()}
    env = dict(os.environ)
    if arg("--m6"):
        env["CANTUS_M6"] = arg("--m6")
    d = od / tag
    d.mkdir(parents=True, exist_ok=True)
    sets = arg("--set")
    model = {}
    for kid in ("studio", "jazz"):
        wav = d / f"{kid}.wav"
        cmd = [EXE, "hits", str(ROOT), str(wav), f"model:{kid}", "--kinds",
               "kick,snare,racktom,floortom", "--vels", "16,32,48,64,80,100,127", "--slot", "4"]
        if sets:
            cmd += ["--set", sets]
        subprocess.run(cmd, env=env, check=True, capture_output=True)
        model[kid] = table(onset.measure(wav))
        if "--keep" not in sys.argv:
            wav.unlink()
    score = {}
    print(f"{'piece':9} {'vel':5} " + " ".join(f"{k:>16}" for k in KEYS))
    for (kind, g) in model["studio"]:
        rv = {k: [refs[r][(kind, g)][k] for r in refs if (kind, g) in refs[r]] for k in KEYS}
        line = f"{kind:9} {g:5} " + " ".join(
            f"{np.median(rv[k]):5.1f}[{min(rv[k]):4.1f}-{max(rv[k]):4.1f}]" for k in KEYS)
        print(line)
        for kid in ("studio", "jazz"):
            m = model[kid][(kind, g)]
            print(f"  {kid:15} " + " ".join(f"{m[k]:16.1f}" for k in KEYS))
            bands = [abs(np.log2(max(m[k], 0.3) / max(np.median(rv[k]), 0.3))) for k in KEYS[:5]]
            score.setdefault(kid, {"peaks": [], "crest10": []})
            score[kid]["peaks"].append(np.mean(bands))
            score[kid]["crest10"].append(m["crest10"] - np.median(rv["crest10"]))
    grooves = [g for g in (arg("--grooves", "") or "").split(",") if g]
    gm = json.loads((work / "grooves" / "measures.json").read_text(encoding="utf-8"))
    for kid in ("studio", "jazz"):
        s = score[kid]
        print(f"{kid}: band peaks {np.mean(s['peaks']):.2f} octaves off, crest10 {np.mean(s['crest10']):+.2f} dB"
              f" (|{np.mean(np.abs(s['crest10'])):.2f}|)")
    for g in grooves:
        refc = [v["crest"] for k, v in gm[g].items() if "-" not in k and k != "unruly-brushes"]
        cells = []
        for kid in ("studio", "jazz"):
            wav = d / f"{g}--{kid}.wav"
            cmd = [EXE, "notes", str(ROOT), str(wav), f"model:{kid}",
                   str(work / "grooves" / f"{g}--{kid}.csv")]
            if sets:
                cmd += ["--set", sets]
            subprocess.run(cmd, env=env, check=True, capture_output=True)
            c = groove_measure(wav)["crest"]
            if "--keep" not in sys.argv:
                wav.unlink()
            cells.append(f"{kid} {c:.1f} ({c - max(refc):+.1f} over)")
        print(f"groove {g}: recorded {min(refc):.1f}-{max(refc):.1f}; " + ", ".join(cells))


if __name__ == "__main__":
    main()
