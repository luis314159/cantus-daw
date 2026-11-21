"""The narration the film uses: each line's chosen take, copied out with a manifest.

    python -I tools/video/voice/choose.py --takes F:/projects/video-work/voice/pool/en \
        --out F:/projects/video-work/voice/chosen/en [--pick 1=r2-01-6 ...]

Without `--voice`, a line's take is the first `ab.html` offered (`abpage.best_two(...)[0]`).
With `--voice <name>`, the first take `compare.py` shows for that voice: its passing take
nearest a calm pace. The owner's decision (2026-11-21): ElevenLabs, English Jon and Spanish
Alejandro, the first take of each line as `ab-eleven.html` shows it. A `--pick
<line>=<take>` overrides one line, should he pick another take later.

Writes `<out>/<line>.wav` (the take as made, 24 kHz float) and `<out>/chosen.json`: per line
the source take, the text, where the line starts in the cut, its window, and where its
speech begins and ends inside the file (head and tail from the gate), so the assembly can
place each line with its lead trimmed and know whether it overruns its window.
"""

import argparse
import json
import shutil
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from abpage import MIDDLE, best_two  # noqa: E402


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--takes", required=True, help="a gated folder (gate.json beside the takes)")
    ap.add_argument("--out", required=True)
    ap.add_argument("--pick", action="append", default=[], help="<line>=<take name without .wav>")
    ap.add_argument("--voice", default=None, help="a voice's takes only (their names' prefix, e.g. jon): the first "
                    "take compare.py shows for it, the passing take nearest a calm pace")
    a = ap.parse_args()
    sys.stdout.reconfigure(encoding="utf-8")

    folder, out = Path(a.takes), Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    rows = json.loads((folder / "gate.json").read_text(encoding="utf-8"))
    picks = dict(p.split("=", 1) for p in a.pick)
    chosen = []
    for n in sorted({r["line"] for r in rows}):
        line_rows = [r for r in rows if r["line"] == n]
        if str(n) in picks:
            r = next(r for r in line_rows if r["take"] == picks[str(n)] + ".wav")
        elif a.voice:
            lang = json.loads((folder / line_rows[0]["take"].replace(".wav", ".json")).read_text(encoding="utf-8"))["lang"]
            ok = sorted((r for r in line_rows if r["take"].startswith(a.voice + "-") and r["pass"]),
                        key=lambda r: abs(r["rate"] - MIDDLE[lang]))
            if not ok:
                print(f"line {n}: no take of {a.voice} passed the gate; nothing chosen")
                continue
            r = ok[0]
        else:
            two = best_two(line_rows, json.loads((folder / line_rows[0]["take"].replace(".wav", ".json")).read_text(encoding="utf-8"))["lang"])
            if not two:
                print(f"line {n}: no take passed the gate; nothing chosen")
                continue
            r = two[0]
        meta = json.loads((folder / r["take"].replace(".wav", ".json")).read_text(encoding="utf-8"))
        shutil.copyfile(folder / r["take"], out / f"{n:02d}.wav")
        speech_from = r["head_ms"] / 1000
        entry = {
            "line": n, "source": r["take"], "text": meta["text"], "lang": meta["lang"],
            "start": meta["start"], "window": meta["window"],
            "speech_from": round(speech_from, 3), "speech_s": r["speech_s"],
            "rate": r["rate"], "unit": r["unit"],
            "overruns_by": round(max(0.0, r["speech_s"] - meta["window"]), 2),
            "heard": r["heard"], "names": r.get("names", []),
            "voice": meta.get("voice"), "voice_id": meta.get("voice_id"), "engine": meta.get("engine", "chatterbox"),
        }
        chosen.append(entry)
        print(f"{n:02d} {r['take']:14s} starts {meta['start']:5.1f}s  speech {r['speech_s']:5.2f}s of {meta['window']:4.1f}s")
    (out / "chosen.json").write_text(json.dumps(chosen, ensure_ascii=False, indent=1), encoding="utf-8")
    print(out / "chosen.json")


if __name__ == "__main__":
    main()
