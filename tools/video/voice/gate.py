"""The gate every take passes before the owner hears it (Claude cannot hear).

    python -I tools/video/voice/gate.py --takes F:/projects/video-work/voice/en \
        --whisper F:/projects/video-work/models/whisper [--model large-v3]

For each `<line>-<take>.wav` with its `.json` (from `takes.py`):

- **Words:** Whisper (MIT) transcribes it in the line's language; after normalising (case,
  punctuation, the figures below) it must equal the script word for word; a name (Cantus,
  Chorda, Tympanum) may be spelled within two letters, with the same first sound.
- **Clipping:** samples at or past 0.999 of full scale; none allowed.
- **DC:** the mean; at most 0.002 of full scale.
- **Head and tail:** the silence before the first and after the last sample within 40 dB
  of the take's peak: 10-400 ms at the head, 30-400 ms at the tail (none means a cut word).
- **Pace:** the words (English) or syllables (Spanish, `pace.py`'s count) over the speech's
  length head to tail, its pauses included: 2.0-3.0 and 4.0-6.0 a second. The speech must
  end at least 0.3 s before the line's window does.
- **Loudness:** integrated (ITU-R BS.1770, pyloudnorm) and true peak (4x oversampled),
  reported, not gated: the mix sets the level.

Writes `gate.json` beside the takes and prints one row a take.
"""

import argparse
import json
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import pace  # noqa: E402

# Spellings Whisper may choose for what the script says, mapped to the script's (after
# hyphens became spaces and a dot inside a word became the spoken "dot" / "punto").
SAME = {
    "en": {
        "42": "forty two", "11": "eleven", "cantus daw": "cantusdaw",
        "vst 3": "vst3", "vst three": "vst3", "v s t three": "vst3", "v s t 3": "vst3",
        "plugins": "plug ins", "f l studio": "fl studio", "modeled": "modelled",
    },
    "es": {
        "42": "cuarenta y dos", "11": "once", "cantus daw": "cantusdaw",
        "vst 3": "vst3", "uve ese te tres": "vst3", "plugins": "plug ins",
        "efe ele studio": "fl studio", "máster": "master",
    },
}
# The names, which Whisper spells as it can: one heard within two letters of the script's,
# with the same first sound, is the name (a name further off was likely said wrong).
NAMES = {"cantus": "cqk", "chorda": "ck", "tympanum": "t"}
DOT = {"en": " dot ", "es": " punto "}


def lev(a: str, b: str) -> int:
    row = list(range(len(b) + 1))
    for i, ca in enumerate(a, 1):
        prev, row[0] = row[0], i
        for j, cb in enumerate(b, 1):
            prev, row[j] = row[j], min(row[j] + 1, row[j - 1] + 1, prev + (ca != cb))
    return row[-1]


def normal(text: str, lang: str) -> list:
    t = text.lower().replace("—", " ").replace("–", " ")
    t = re.sub(r"(?<=\w)\.(?=\w)", DOT[lang], t)
    t = re.sub(r"[«»“”\"¿?¡!,.;:()]+(?=\s|$)", " ", t + " ")
    t = t.replace("-", " ")
    t = re.sub(r"\s+", " ", t).strip()
    for k, v in sorted(SAME[lang].items(), key=lambda kv: -len(kv[0])):
        t = re.sub(r"(?<![\w])" + re.escape(k) + r"(?![\w])", v, t)
    return t.split()


def same_words(said: list, heard: list) -> bool:
    if len(said) != len(heard):
        return False
    for a, b in zip(said, heard):
        if a == b:
            continue
        first = NAMES.get(a)
        if first is None or not b or b[0] not in first or lev(a, b) > 2:
            return False
    return True


def edges(x, sr: int):
    """The silence at the head and the tail, in seconds: to the first and from the last
    sample within 40 dB of the peak."""
    import numpy as np

    a = np.abs(x)
    peak = float(a.max()) if len(a) else 0.0
    if peak == 0.0:
        return len(x) / sr, len(x) / sr
    loud = np.nonzero(a >= peak * 0.01)[0]
    return loud[0] / sr, (len(x) - 1 - loud[-1]) / sr


def true_peak_db(x, sr: int) -> float:
    import numpy as np
    from scipy.signal import resample_poly

    up = resample_poly(x, 4, 1)
    p = float(np.abs(up).max())
    return 20 * np.log10(p) if p > 0 else -200.0


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--takes", required=True)
    ap.add_argument("--whisper", required=True)
    ap.add_argument("--model", default="large-v3")
    a = ap.parse_args()
    sys.stdout.reconfigure(encoding="utf-8")

    import numpy as np
    import torch

    torch.set_num_threads(2)
    import pyloudnorm
    import soundfile as sf
    import whisper

    asr = whisper.load_model(a.model, download_root=a.whisper)
    rows = []
    for meta_path in sorted(Path(a.takes).glob("*-*.json")):
        meta = json.loads(meta_path.read_text(encoding="utf-8"))
        wav = meta_path.with_suffix(".wav")
        x, sr = sf.read(wav, dtype="float32")
        if x.ndim > 1:
            x = x.mean(axis=1)
        lang = meta["lang"]
        # Whisper reads 16 kHz samples; handed the array, it needs no ffmpeg.
        from math import gcd
        from scipy.signal import resample_poly

        g = gcd(16000, sr)
        x16 = resample_poly(x, 16000 // g, sr // g).astype(np.float32)
        heard = asr.transcribe(x16, language=lang, temperature=0.0, fp16=False)["text"]
        said, got = normal(meta["text"], lang), normal(heard, lang)
        clip = int((np.abs(x) >= 0.999).sum())
        dc = float(x.mean())
        head, tail = edges(x, sr)
        speech = len(x) / sr - head - tail
        n, lo, hi, unit = pace.count(meta["text"], lang)
        # The rate is over the speech with its own pauses, so the window is the plan's
        # "about" widened: 2.0-3.0 words a second in English, 4.0-6.0 syllables in Spanish.
        lo, hi = (2.0, 3.0) if lang == "en" else (4.0, 6.0)
        rate = n / speech if speech > 0 else 0.0
        meter = pyloudnorm.Meter(sr)
        lufs = float(meter.integrated_loudness(x.astype(np.float64)))
        tp = true_peak_db(x, sr)
        checks = {  # plain bools: numpy's do not go into JSON
            "words": same_words(said, got),
            "clip": clip == 0,
            "dc": abs(dc) <= 0.002,
            "head": 0.01 <= head <= 0.4,
            "tail": 0.03 <= tail <= 0.4,
            "pace": lo <= rate <= hi,
            "fits": head + speech <= meta["window"] - 0.3,
        }
        row = {
            "take": wav.name,
            "line": meta["line"],
            "pass": bool(all(checks.values())),
            "checks": checks,
            "heard": heard.strip(),
            "rate": round(rate, 2),
            "unit": unit + "/s",
            "speech_s": round(speech, 2),
            "head_ms": round(head * 1000),
            "tail_ms": round(tail * 1000),
            "dc": round(dc, 5),
            "clipped": clip,
            "lufs": round(lufs, 1),
            "true_peak_db": round(tp, 1),
            "seed": meta["seed"],
            "exaggeration": meta["exaggeration"],
            "cfg_weight": meta["cfg_weight"],
        }
        rows.append(row)
        failed = [k for k, ok in checks.items() if not ok]
        print(
            f"{wav.name:10s} {'PASS' if row['pass'] else 'fail':4s} {rate:4.2f} {unit}/s "
            f"{speech:5.2f}s head {row['head_ms']:3d} tail {row['tail_ms']:3d} "
            f"{lufs:6.1f} LUFS {tp:5.1f} dBTP  {' '.join(failed)}",
            flush=True,
        )
        if not checks["words"]:
            print(f"           heard: {heard.strip()}")
    out = Path(a.takes) / "gate.json"
    out.write_text(json.dumps(rows, ensure_ascii=False, indent=1), encoding="utf-8")
    print(out)


if __name__ == "__main__":
    main()
