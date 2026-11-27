#!/usr/bin/env python3
"""A quick listening set for one Tympanum change (round 42, tympanum-real: model 6).

    python -I tools/tympanum-real/quick_preview.py <work> <out dir> <exe> [--tag m6]
           [--before after] [--set model=2] [--grooves rock:studio,funk:studio,jazz:jazz]

For each `groove:kit`: the recorded kits of its genre (`preview.PAIRS`, `<work>/grooves/ref`),
Tympanum's render under `--before` (`<work>/grooves/<before>/`, model 5's) and a new render
with `<exe>` (`notes`, the groove's own CSV) into `<work>/grooves/<tag>/`; every clip at
−20 LUFS (lowered together so no peak passes −1 dBFS), 16-bit, into `<out dir>/<groove>/`,
with `index.html` and `numbers.json` — each clip's crest (peak − loudness, `grooves.measure`)
and, per kit, the drums' per-band peak times (`onset.py`) from `<work>/onset/`.
"""
import html
import json
import subprocess
import sys
from pathlib import Path

import numpy as np
import soundfile as sf

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import preview as PV  # noqa: E402
from grooves import measure  # noqa: E402

ROOT = Path(r"F:\projects\cantus-refs\drums\content")
LABELS = {"drskit": "DRSKit", "muldjord": "MuldjordKit", "bigrusty": "Big Rusty", "gogodze": "Gogodze Phu II",
          "unruly": "Unruly", "virtuosity": "Virtuosity", "unruly-brushes": "Unruly escobillas"}
TNAME = {"studio": "Tympanum Studio Maple", "jazz": "Tympanum Jazz Birch"}


def arg(n, d=None):
    return sys.argv[sys.argv.index(n) + 1] if n in sys.argv else d


def main():
    work, out, exe = Path(sys.argv[1]), Path(sys.argv[2]), sys.argv[3]
    tag, before = arg("--tag", "m6"), arg("--before", "after")
    sets = arg("--set", "model=2")
    rng = np.random.default_rng(42)
    numbers, page = {}, []
    for gk in arg("--grooves", "rock:studio,funk:studio,jazz:jazz").split(","):
        g, kit = gk.split(":")
        new = work / "grooves" / tag / f"{g}--{kit}.wav"
        new.parent.mkdir(parents=True, exist_ok=True)
        subprocess.run([exe, "notes", str(ROOT), str(new), f"model:{kit}",
                        str(work / "grooves" / f"{g}--{kit}.csv"), "--set", sets],
                       check=True, capture_output=True)
        clips = [(f"grabada · {LABELS.get(r, r)}", work / "grooves" / "ref" / f"{g}--{r}.wav")
                 for r in PV.PAIRS[g][0]]
        clips = [c for c in clips if c[1].exists()]
        clips += [(f"{TNAME[kit]} · modelo 5 (el «después» anterior)", work / "grooves" / before / f"{g}--{kit}.wav"),
                  (f"{TNAME[kit]} · modelo 6", new)]
        audio = []
        for lab, p in clips:
            x, fs = sf.read(str(p), dtype="float64", always_2d=True)
            x = x[: int(22.0 * fs)]
            fade = int(0.1 * fs)
            x[-fade:] *= np.linspace(1, 0, fade)[:, None]
            audio.append((lab, p, x, fs))
        gains = [PV.TARGET - PV.lufs(x, fs) for _, _, x, fs in audio]
        peaks = [20 * np.log10(np.abs(x).max() + 1e-12) + gg for (_, _, x, _), gg in zip(audio, gains)]
        drop = max(0.0, max(peaks) - PV.CEIL)
        d = out / g
        d.mkdir(parents=True, exist_ok=True)
        items = []
        for i, ((lab, p, x, fs), gg) in enumerate(zip(audio, gains)):
            fn = f"{i + 1:02d}.wav"
            sf.write(str(d / fn), PV.to16(x * 10 ** ((gg - drop) / 20), rng), fs, subtype="PCM_16")
            m = measure(p)
            numbers.setdefault(g, []).append({"clip": lab, "file": f"{g}/{fn}", "crest_db": round(m["crest"], 2)})
            items.append((lab, f"{g}/{fn}", m["crest"]))
        page.append((g, PV.PAIRS[g][1], items))
    (out / "numbers.json").write_text(json.dumps(numbers, indent=1, ensure_ascii=False), encoding="utf-8")
    rows = ["<!doctype html><html lang='es'><head><meta charset='utf-8'><meta name='viewport' content='width=device-width,initial-scale=1'>"
            "<title>Tympanum modelo 6</title><style>body{font:16px/1.5 system-ui;max-width:860px;margin:0 auto;padding:16px}"
            ".r{display:grid;grid-template-columns:minmax(0,1fr) minmax(0,1.2fr) 6em;gap:10px;align-items:center;margin:6px 0}"
            "audio{width:100%}small{color:#666}</style></head><body><h1>Tympanum: modelo 6 (escucha rápida)</h1>"
            "<p>Cada ritmo es una interpretación humana (Groove MIDI Dataset). Todos a −20 LUFS. "
            "<b>Cresta</b> = pico − sonoridad (dB): las baterías grabadas quedan más bajo que el modelo 5.</p>"]
    for g, hint, items in page:
        rows.append(f"<h2>{html.escape(g)}</h2><p><small>Fíjate en: {html.escape(hint)}</small></p>")
        for lab, f, c in items:
            rows.append(f"<div class='r'><div>{html.escape(lab)}</div><audio controls preload='none' src='{f}'></audio>"
                        f"<div>cresta {c:.1f} dB</div></div>")
    rows.append("</body></html>")
    (out / "index.html").write_text("\n".join(rows), encoding="utf-8")
    print(json.dumps(numbers, indent=1, ensure_ascii=False))


if __name__ == "__main__":
    main()
