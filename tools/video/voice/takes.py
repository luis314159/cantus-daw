"""Make the narration's takes with Chatterbox Multilingual (MIT), 8 per line.

    python -I tools/video/voice/takes.py --script docs/round41/video/script.md --lang en \
        --models F:/projects/video-work/models/chatterbox \
        --out F:/projects/video-work/voice/en [--ref F:/projects/video-work/voice/ref/luis.wav]
        [--takes 6] [--lines 1,4]

With `--ref`, the voice is cloned from that recording (the owner's own, by his consent, and
nobody else's). Without it, the model's stock voice (`conds.pt`). Every take is written as
`<line>-<take>.wav` (the model's 24 kHz, float) with `<line>-<take>.json` beside it: the
text, the seed and the settings, so any take can be made again exactly. Every file carries
Resemble's Perth watermark, as the model writes it.
"""

import argparse
import json
import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import lines as script_lines  # noqa: E402

# The settings each take tries, in turn: seed, exaggeration, cfg weight, temperature.
# Round 1 (exaggeration 0.4-0.6, cfg 0.35-0.5, temperature 0.7-0.8) spoke at 3.5-4 words a
# second and babbled after some lines; Chatterbox's own advice for a slower, deliberate
# read is a lower cfg weight and exaggeration, and a lower temperature keeps it on the text.
GRID = [
    (2101, 0.35, 0.25, 0.70),
    (2102, 0.40, 0.30, 0.70),
    (2103, 0.30, 0.20, 0.65),
    (2104, 0.45, 0.30, 0.75),
    (2105, 0.35, 0.30, 0.60),
    (2106, 0.40, 0.20, 0.70),
    (2107, 0.30, 0.30, 0.60),
    (2108, 0.45, 0.25, 0.65),
]

# What the model is given for what the script writes: names and acronyms as they are said
# (round 1 read "FL" as "Val", "VST3" as "Vistie Street", and dropped "daw" from the address).
SPEAK = {
    "en": {"FL Studio": "F.L. Studio", "VST3": "V.S.T. three", "cantusdaw.com": "cantus-daw dot com"},
    "es": {"FL Studio": "efe ele Studio", "VST3": "uve ese te tres", "cantusdaw.com": "cantus daw punto com",
           "plug-ins": "plugins"},
}


def spoken(text: str, lang: str) -> str:
    for k, v in SPEAK[lang].items():
        text = text.replace(k, v)
    return text


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--script", required=True)
    ap.add_argument("--lang", choices=["en", "es"], required=True)
    ap.add_argument("--models", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--ref", default=None)
    ap.add_argument("--takes", type=int, default=8)
    ap.add_argument("--lines", default=None, help="comma-separated line numbers")
    a = ap.parse_args()
    sys.stdout.reconfigure(encoding="utf-8")

    import soundfile as sf
    import torch

    # The GPU does the work; few CPU threads, so a measurement beside it is not disturbed.
    torch.set_num_threads(2)
    from chatterbox.mtl_tts import ChatterboxMultilingualTTS

    device = "cuda" if torch.cuda.is_available() else "cpu"
    model = ChatterboxMultilingualTTS.from_local(Path(a.models), device)
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    wanted = None if a.lines is None else {int(x) for x in a.lines.split(",")}
    for line in script_lines.read(a.script, a.lang):
        if wanted is not None and line["n"] not in wanted:
            continue
        for k, (seed, exag, cfg, temp) in enumerate(GRID[: a.takes], start=1):
            name = f"{line['n']:02d}-{k}"
            torch.manual_seed(seed)
            if device == "cuda":
                torch.cuda.manual_seed_all(seed)
            said = spoken(line["text"], a.lang)
            wav = model.generate(
                said,
                language_id=a.lang,
                audio_prompt_path=a.ref,
                exaggeration=exag,
                cfg_weight=cfg,
                temperature=temp,
            )
            samples = wav.squeeze(0).detach().cpu().numpy()
            sf.write(out / f"{name}.wav", samples, model.sr, subtype="FLOAT")
            meta = {
                "line": line["n"],
                "take": k,
                "lang": a.lang,
                "text": line["text"],
                "spoken": said,
                "start": line["start"],
                "window": line["window"],
                "seed": seed,
                "exaggeration": exag,
                "cfg_weight": cfg,
                "temperature": temp,
                "voice": os.path.basename(a.ref) if a.ref else "stock (conds.pt)",
                "sr": model.sr,
            }
            (out / f"{name}.json").write_text(json.dumps(meta, ensure_ascii=False, indent=1), encoding="utf-8")
            print(name, f"{len(samples) / model.sr:.2f}s", flush=True)


if __name__ == "__main__":
    main()
