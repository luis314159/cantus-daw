"""Make the narration's takes with ElevenLabs (the owner's Starter plan), counting characters.

    python -I tools/video/voice/takes_eleven.py --lang es --voice NWqMOQLlMBaUbjKYdhbW \
        --name Alejandro --out F:/projects/video-work/voice/eleven/es --budget 20000

Each call goes through `eleven-api.ps1`, which reads the key from Windows Credential Manager
at the moment of use; the key is never on a command line, in a log or in a file here. The
text of a line is given as the voice should say it (`takes.spoken`: Korda, Kantus,
Timpanum...). Before every call the characters it will spend are added to a ledger
(`<out>/../ledger.json`, shared by every voice); a call that would pass `--budget` is not made.

Each take is `<voice>-<line>-<take>.wav` (48 kHz WAV, as the API gives it) with its `.json`,
as `takes.py` writes them, so `gate.py` and `compare.py` read it alike. The voices are
ElevenLabs's library voices, used under the plan's licence: never a voice cloned here.
"""

import argparse
import json
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import lines as script_lines  # noqa: E402
from takes import spoken  # noqa: E402

HERE = Path(__file__).resolve().parent
MODEL = "eleven_multilingual_v2"  # "best for voice overs" (the API's own description)
# Three takes a line: a seed each, a calm narrator's settings around ElevenLabs's defaults.
TAKES = [
    (11, {"stability": 0.55, "similarity_boost": 0.8, "style": 0.0, "use_speaker_boost": True, "speed": 0.95}),
    (12, {"stability": 0.45, "similarity_boost": 0.8, "style": 0.1, "use_speaker_boost": True, "speed": 0.95}),
    (13, {"stability": 0.65, "similarity_boost": 0.8, "style": 0.0, "use_speaker_boost": True, "speed": 0.92}),
]


def call(path: str, body: dict, out_file: Path, work: Path) -> None:
    body_file = work / "body.json"
    body_file.write_text(json.dumps(body, ensure_ascii=False), encoding="utf-8")
    subprocess.run(
        ["pwsh", "-NoProfile", "-File", str(HERE / "eleven-api.ps1"), "POST", path,
         "-BodyFile", str(body_file), "-OutFile", str(out_file)],
        check=True, capture_output=True, text=True,
    )
    body_file.unlink()


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--script", default="docs/round41/video/script.md")
    ap.add_argument("--lang", choices=["en", "es"], required=True)
    ap.add_argument("--voice", required=True, help="the voice id")
    ap.add_argument("--name", required=True, help="the voice's name, for the takes' names")
    ap.add_argument("--out", required=True)
    ap.add_argument("--budget", type=int, default=20000, help="characters, for every voice together")
    ap.add_argument("--lines", default=None)
    ap.add_argument("--takes", type=int, default=3)
    ap.add_argument("--format", default="wav_48000")
    ap.add_argument("--only-take", type=int, default=None, help="make only this take of TAKES (its seed and settings)")
    ap.add_argument("--suffix", default="", help="appended to the take names (a line re-made: jon-02-2b)")
    ap.add_argument("--ledger", default=None, help="the ledger (default: <out>/../ledger.json), shared by every run")
    a = ap.parse_args()
    sys.stdout.reconfigure(encoding="utf-8")

    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    ledger_path = Path(a.ledger) if a.ledger else out.parent / "ledger.json"
    ledger = json.loads(ledger_path.read_text(encoding="utf-8")) if ledger_path.exists() else {"spent": 0, "calls": []}
    wanted = None if a.lines is None else {int(x) for x in a.lines.split(",")}
    tag = a.name.lower()
    for line in script_lines.read(a.script, a.lang):
        if wanted is not None and line["n"] not in wanted:
            continue
        said = spoken(line["text"], a.lang)
        for k, (seed, settings) in enumerate(TAKES[: a.takes], start=1):
            if a.only_take is not None and k != a.only_take:
                continue
            name = f"{tag}-{line['n']:02d}-{k}{a.suffix}"
            if (out / f"{name}.wav").exists():
                continue  # made already: never pay twice
            if ledger["spent"] + len(said) > a.budget:
                print(f"budget: {ledger['spent']} spent, {name} ({len(said)}) would pass {a.budget}; stopping")
                ledger_path.write_text(json.dumps(ledger, indent=1), encoding="utf-8")
                return
            body = {"text": said, "model_id": MODEL, "voice_settings": settings, "seed": seed}
            call(f"v1/text-to-speech/{a.voice}?output_format={a.format}", body, out / f"{name}.wav", out)
            ledger["spent"] += len(said)
            ledger["calls"].append({"take": name, "chars": len(said)})
            ledger_path.write_text(json.dumps(ledger, indent=1), encoding="utf-8")
            meta = {
                "line": line["n"], "take": k, "lang": a.lang, "text": line["text"], "spoken": said,
                "start": line["start"], "window": line["window"], "seed": seed,
                "voice": f"ElevenLabs {a.name}", "voice_id": a.voice, "engine": MODEL,
                "voice_settings": settings, "format": a.format,
            }
            (out / f"{name}.json").write_text(json.dumps(meta, ensure_ascii=False, indent=1), encoding="utf-8")
            print(name, len(said), "chars; spent", ledger["spent"], flush=True)


if __name__ == "__main__":
    main()
