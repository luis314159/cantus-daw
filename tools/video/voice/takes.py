"""Make the narration's takes with Chatterbox Multilingual (MIT), 4-6 per line.

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

# The settings each take tries, in turn: seed, exaggeration (0.4-0.6), cfg weight
# (0.3-0.5), temperature. A calm presenter: low exaggeration, a moderate cfg.
GRID = [
    (1101, 0.45, 0.40, 0.75),
    (1102, 0.50, 0.40, 0.80),
    (1103, 0.40, 0.30, 0.75),
    (1104, 0.55, 0.45, 0.80),
    (1105, 0.50, 0.50, 0.70),
    (1106, 0.60, 0.35, 0.80),
]


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--script", required=True)
    ap.add_argument("--lang", choices=["en", "es"], required=True)
    ap.add_argument("--models", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--ref", default=None)
    ap.add_argument("--takes", type=int, default=6)
    ap.add_argument("--lines", default=None, help="comma-separated line numbers")
    a = ap.parse_args()

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
            wav = model.generate(
                line["text"],
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
