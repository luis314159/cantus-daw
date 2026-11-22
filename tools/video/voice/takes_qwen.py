"""Make the narration's takes with Qwen3-TTS (Alibaba Qwen; code and weights Apache-2.0).

Three ways, all with voices that belong to nobody (never a person's voice cloned):

    # a built-in voice (CustomVoice model), steered by a description of the delivery
    python -I tools/video/voice/takes_qwen.py custom --model $WORK/models/qwen3-tts-1.7b-customvoice \
        --speaker Aiden --lang en --script docs/round41/video/script.md --out $WORK/voice/qwen/en

    # a designed voice (VoiceDesign model): the voice described in words, per line
    python -I tools/video/voice/takes_qwen.py design --model $WORK/models/qwen3-tts-1.7b-voicedesign \
        --voice es-latam --lang es ...

    # a designed voice held steady across lines: one reference clip made by VoiceDesign
    # (`design-ref`), then every line spoken by the Base model in that synthetic voice
    python -I tools/video/voice/takes_qwen.py design-ref --model .../voicedesign --voice es-latam \
        --lang es --out $WORK/voice/qwen/ref
    python -I tools/video/voice/takes_qwen.py from-ref --model .../base --ref $WORK/voice/qwen/ref/es-latam-1.wav \
        --lang es --script ... --out $WORK/voice/qwen/es

Each take is `<line>-<take>.wav` at the model's own rate (24 kHz), PCM 24-bit, with a
`.json` of its text, voice, seed and settings, as `takes.py` writes them, so `breathe.py`,
`gate.py` and the A/B pages read both alike.
"""

import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import lines as script_lines  # noqa: E402
from takes import spoken  # noqa: E402  (the same spoken spellings as Chatterbox's)

LANGUAGE = {"en": "English", "es": "Spanish"}

# How the narrator delivers every line: the owner liked the slower cadence (round 2's A takes).
DELIVERY = {
    "en": "Speak as a calm, warm documentary narrator: unhurried, clear and friendly, with natural pauses at the commas and full stops.",
    "es": "Habla como un narrador de documental, cálido y sereno: sin prisa, con dicción clara y pausas naturales en las comas y los puntos.",
}

# The designed voices, described in words (VoiceDesign). Nobody's voice: a description.
VOICES = {
    "en-warm": "A man in his late thirties with a warm, calm, slightly low voice; General American accent; "
    "a documentary narrator speaking unhurriedly, clear diction, friendly and confident.",
    "es-latam": "Un hombre de unos treinta y cinco años con voz cálida, serena y algo grave; acento latinoamericano "
    "neutro, mexicano, sin acento de España; narrador de documental, habla sin prisa, dicción clara, cercano y seguro.",
    "es-latam-f": "Una mujer de unos treinta años con voz cálida y serena; acento latinoamericano neutro, mexicano, "
    "sin acento de España; narradora de documental, habla sin prisa, dicción clara, cercana y segura.",
}

# A neutral passage for a designed voice's reference clip: about ten seconds of plain speech.
REF_TEXT = {
    "en": "Every morning the studio is quiet. The coffee is warm, the windows are bright, and there is time to listen before the first note is played.",
    "es": "Cada mañana el estudio está en silencio. El café está caliente, la luz entra por la ventana, y hay tiempo para escuchar antes de tocar la primera nota.",
}

SEEDS = [3101, 3102, 3103, 3104, 3105, 3106]


def load(path: str):
    import torch
    from qwen_tts import Qwen3TTSModel

    torch.set_num_threads(2)
    return Qwen3TTSModel.from_pretrained(path, device_map="cuda:0", dtype=torch.bfloat16, attn_implementation="sdpa")


def seed(s: int) -> None:
    import torch

    torch.manual_seed(s)
    torch.cuda.manual_seed_all(s)


def write(out: Path, name: str, wav, sr: int, meta: dict) -> None:
    import numpy as np
    import soundfile as sf

    x = np.asarray(wav, dtype=np.float32)
    peak = float(np.abs(x).max()) if len(x) else 0.0
    if peak > 0.98:  # PCM can't hold an overshoot; bring it under, and say so
        x = x * (0.98 / peak)
        meta["scaled_from_peak"] = round(peak, 4)
    sf.write(out / f"{name}.wav", x, sr, subtype="PCM_24")
    meta["sr"] = sr
    (out / f"{name}.json").write_text(json.dumps(meta, ensure_ascii=False, indent=1), encoding="utf-8")
    print(name, f"{len(x) / sr:.2f}s", flush=True)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("mode", choices=["custom", "design", "design-ref", "from-ref"])
    ap.add_argument("--model", required=True)
    ap.add_argument("--lang", choices=["en", "es"], required=True)
    ap.add_argument("--script", default="docs/round41/video/script.md")
    ap.add_argument("--out", required=True)
    ap.add_argument("--speaker", default=None, help="custom: a built-in speaker")
    ap.add_argument("--voice", default=None, help="design, design-ref: a key of VOICES")
    ap.add_argument("--ref", default=None, help="from-ref: a clip made by design-ref")
    ap.add_argument("--takes", type=int, default=6)
    ap.add_argument("--lines", default=None)
    ap.add_argument("--tag", default="", help="prefix of the take names")
    a = ap.parse_args()
    sys.stdout.reconfigure(encoding="utf-8")

    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    model = load(a.model)
    lang = LANGUAGE[a.lang]

    if a.mode == "design-ref":
        for k, s in enumerate(SEEDS[: a.takes], start=1):
            seed(s)
            wavs, sr = model.generate_voice_design(text=REF_TEXT[a.lang], language=lang, instruct=VOICES[a.voice])
            write(out, f"{a.voice}-{k}", wavs[0], sr, {"voice": a.voice, "instruct": VOICES[a.voice],
                                                       "text": REF_TEXT[a.lang], "seed": s, "lang": a.lang,
                                                       # so gate.py can check a reference like a line
                                                       "line": 0, "take": k, "start": 0.0, "window": 30.0})
        return

    prompt = None
    if a.mode == "from-ref":
        ref_meta = json.loads(Path(a.ref).with_suffix(".json").read_text(encoding="utf-8"))
        prompt = model.create_voice_clone_prompt(ref_audio=a.ref, ref_text=ref_meta["text"])

    wanted = None if a.lines is None else {int(x) for x in a.lines.split(",")}
    for line in script_lines.read(a.script, a.lang):
        if wanted is not None and line["n"] not in wanted:
            continue
        said = spoken(line["text"], a.lang)
        for k, s in enumerate(SEEDS[: a.takes], start=1):
            seed(s)
            if a.mode == "custom":
                wavs, sr = model.generate_custom_voice(text=said, language=lang, speaker=a.speaker,
                                                       instruct=DELIVERY[a.lang])
                voice = f"built-in {a.speaker}"
            elif a.mode == "design":
                wavs, sr = model.generate_voice_design(text=said, language=lang,
                                                       instruct=VOICES[a.voice] + " " + DELIVERY[a.lang])
                voice = f"designed {a.voice}"
            else:
                wavs, sr = model.generate_voice_clone(text=said, language=lang, voice_clone_prompt=prompt)
                voice = f"designed {ref_meta['voice']} (reference {Path(a.ref).name}, a synthetic voice)"
            name = f"{a.tag}{line['n']:02d}-{k}"
            write(out, name, wavs[0], sr, {
                "line": line["n"], "take": k, "lang": a.lang, "text": line["text"], "spoken": said,
                "start": line["start"], "window": line["window"], "seed": s, "voice": voice,
                "engine": "Qwen3-TTS " + Path(a.model).name, "delivery": DELIVERY[a.lang],
            })


if __name__ == "__main__":
    main()
