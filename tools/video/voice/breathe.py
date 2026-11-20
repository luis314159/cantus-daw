"""Lengthen a take's own pauses, so a line read quickly breathes like a presenter's.

    python -I tools/video/voice/breathe.py --from F:/projects/video-work/voice/round2/en \
        --to F:/projects/video-work/voice/pool/en --tag r2

The stock voice reads long lines at 3.5-4.5 words a second (rounds 1-2); made phrase by
phrase it babbles (round 3). So the takes are made whole and only their silences are
touched: every pause inside the line (a run under 40 dB below the take's peak, between
speech) of 100 ms or more is opened to 180 ms, and one of 150 ms or more, where the reader
already stopped for a full stop or a dash, to 320 ms. The silence is lengthened in its
middle with silence; no sound of the voice is stretched, cut or moved in pitch.

Each take is copied as `<tag>-<line>-<take>.wav` with its `.json`, which gains `"breath"`:
the pauses opened, as [seconds into the source, from ms, to ms].
"""

import argparse
import json
import sys
from pathlib import Path

import numpy as np

FRAME = 0.01
SHORT, LONG = 0.10, 0.15  # pauses at least this long are opened
TO_SHORT, TO_LONG = 0.18, 0.32


def pauses(x, sr: int) -> list:
    """The pauses between speech: (start sample, end sample) of each quiet run inside."""
    hop = int(sr * FRAME)
    n = len(x) // hop
    rms = np.sqrt(np.mean(x[: n * hop].reshape(n, hop) ** 2, axis=1) + 1e-12)
    quiet = 20 * np.log10(rms / rms.max()) < -40
    loud = np.nonzero(~quiet)[0]
    if len(loud) == 0:
        return []
    found = []
    i = loud[0]
    while i <= loud[-1]:
        if quiet[i]:
            j = i
            while j <= loud[-1] and quiet[j]:
                j += 1
            found.append((i * hop, j * hop))
            i = j
        else:
            i += 1
    return found


def breathe(x, sr: int):
    out, made, last = [], [], 0
    for a, b in pauses(x, sr):
        length = (b - a) / sr
        if length < SHORT:
            continue
        target = TO_LONG if length >= LONG else TO_SHORT
        if target <= length:
            continue
        mid = (a + b) // 2
        out.append(x[last:mid])
        out.append(np.zeros(int((target - length) * sr), dtype=x.dtype))
        last = mid
        made.append([round(a / sr, 3), round(length * 1000), round(target * 1000)])
    out.append(x[last:])
    return np.concatenate(out), made


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--from", dest="src", required=True)
    ap.add_argument("--to", dest="dst", required=True)
    ap.add_argument("--tag", required=True)
    ap.add_argument("--lines", default=None, help="comma-separated line numbers")
    a = ap.parse_args()
    sys.stdout.reconfigure(encoding="utf-8")
    import soundfile as sf

    wanted = None if a.lines is None else {int(v) for v in a.lines.split(",")}
    dst = Path(a.dst)
    dst.mkdir(parents=True, exist_ok=True)
    for meta_path in sorted(Path(a.src).glob("[0-9]*-*.json")):
        meta = json.loads(meta_path.read_text(encoding="utf-8"))
        if wanted is not None and meta["line"] not in wanted:
            continue
        x, sr = sf.read(meta_path.with_suffix(".wav"), dtype="float32")
        y, made = breathe(x, sr)
        name = f"{a.tag}-{meta_path.stem}"
        sf.write(dst / f"{name}.wav", y, sr, subtype="FLOAT")
        meta["breath"] = made
        meta["source"] = str(meta_path.with_suffix(".wav"))
        (dst / f"{name}.json").write_text(json.dumps(meta, ensure_ascii=False, indent=1), encoding="utf-8")
        print(name, f"{len(x) / sr:.2f}s -> {len(y) / sr:.2f}s", len(made), "pauses opened")


if __name__ == "__main__":
    main()
