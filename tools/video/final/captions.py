"""The captions and the chapters of the launch video, from the chosen narration as placed.

    python -I tools/video/final/captions.py --chosen F:/projects/video-work/voice/chosen \
        --out F:/projects/video-work/final

The soundtrack (`crates/cantus-ui/examples/soundtrack.rs`) places each line so its speech
begins exactly at the line's `start` (the clip starts 60 ms earlier, its lead kept), and
the speech lasts `speech_s` (`chosen.json`). So each caption is timed to the speech itself.

- **Captions** (`cantus-<lang>.srt`): the script's text (not the spoken spellings given to
  the voice), at most 42 characters a line and two lines a caption. A line too long for one
  caption is cut at its sentences, then its clauses, and the speech's time is shared by the
  characters each caption holds. A caption stays up until the next one, or 0.4 s after the
  speech ends, and at least 1.2 s. Two lines are balanced, broken after punctuation where
  it can, and no line starts with a dash.
- **Chapters**: `chapters-<lang>.txt` (for a YouTube description: the first at 0:00, each
  at least 10 s) and `chapters-<lang>.ffmeta` (ffmpeg's metadata file, to embed them).
"""

import argparse
import json
import re
import sys
from pathlib import Path

MAX_LINE = 42
MAX_LINES = 2
TAIL_S = 0.4
MIN_S = 1.2
END_S = 80.0
# Words a line should not end on.
WEAK = {"a", "an", "and", "the", "of", "to", "y", "de", "en", "el", "la", "un", "una", "sus", "tu", "tus", "your", "its"}

# The chapters follow the cut (script.md's table): each section's start and its title.
CHAPTERS = {
    "en": [(0, "Cantus"), (14, "Instruments: Chorda and Tympanum"), (31, "The mixer and 43 effects"),
           (44, "Record, play live, master"), (60, "Free, for everyone")],
    "es": [(0, "Cantus"), (14, "Instrumentos: Chorda y Tympanum"), (31, "La consola y 43 efectos"),
           (44, "Graba, toca en vivo, masteriza"), (60, "Gratis, para todos")],
}


def wrap(text: str) -> list:
    """One line if it fits; else the two-line break that balances the lines best, preferring
    a break after punctuation and never starting a line with a dash. Longer text is split
    greedily (`chunks` keeps a caption to two lines)."""
    if len(text) <= MAX_LINE:
        return [text]
    words = text.split()
    best = None
    for k in range(1, len(words)):
        a, b = " ".join(words[:k]), " ".join(words[k:])
        if len(a) > MAX_LINE or len(b) > MAX_LINE:
            continue
        score = max(len(a), len(b))
        if a[-1] in ".!?»":
            score -= 20  # a sentence's end is the best break
        elif a[-1] in ",:;—":
            score -= 8
        if words[k - 1].lower() in WEAK:
            score += 10  # never leave "y", "de", "and"... hanging at a line's end
        if b.startswith("—"):
            score += 100
        if best is None or score < best[0]:
            best = (score, [a, b])
    if best:
        return best[1]
    lines, cur = [], ""
    for w in words:
        if cur and len(cur) + 1 + len(w) > MAX_LINE:
            lines.append(cur)
            cur = w
        else:
            cur = f"{cur} {w}" if cur else w
    return lines + ([cur] if cur else [])


def fits(text: str) -> bool:
    return len(wrap(text)) <= MAX_LINES


def chunks(text: str) -> list:
    """The line's text cut into captions that each fit two lines."""
    sentences = re.split(r"(?<=[.!?»])\s+", text)
    if fits(text):
        lines = wrap(text)
        # several sentences on two lines only when the break falls between them
        if len(sentences) == 1 or len(lines) == 1 or lines[0][-1] in ".!?»":
            return [text]
    out = []
    for sentence in sentences:
        if fits(sentence):
            out.append(sentence)
            continue
        # a long sentence: at its clauses (a dash, a colon, a comma), packed greedily
        parts = re.split(r"(?<=[,:;—])\s+", sentence)
        cur = ""
        for p in parts:
            joined = f"{cur} {p}" if cur else p
            if fits(joined):
                cur = joined
            else:
                if cur:
                    out.append(cur)
                cur = p
        if cur:
            out.append(cur)
    # merge neighbours that fit together (a short sentence next to another)
    merged = []
    for c in out:
        # two short sentences share a caption only as its two lines, broken between them
        if merged and len(merged[-1]) <= MAX_LINE and len(c) <= MAX_LINE and merged[-1][-1] in ".!?»":
            merged[-1] += " " + c
        else:
            merged.append(c)
    return merged


def stamp(t: float) -> str:
    ms = int(round(t * 1000))
    h, ms = divmod(ms, 3_600_000)
    m, ms = divmod(ms, 60_000)
    s, ms = divmod(ms, 1000)
    return f"{h:02d}:{m:02d}:{s:02d},{ms:03d}"


def captions(lines: list) -> list:
    caps = []
    for i, line in enumerate(lines):
        start, speech = line["start"], line["speech_s"]
        nxt = lines[i + 1]["start"] if i + 1 < len(lines) else END_S
        parts = chunks(line["text"])
        total = sum(len(p) for p in parts)
        t = start
        for k, p in enumerate(parts):
            share = speech * len(p) / total
            end = t + share if k + 1 < len(parts) else min(t + share + TAIL_S, nxt - 0.1)
            end = max(end, min(t + MIN_S, nxt - 0.1))
            caps.append({"from": t, "to": end, "lines": wrap(p), "line": line["line"]})
            t += share
    return caps


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--chosen", required=True, help="the folder with <lang>/chosen.json")
    ap.add_argument("--out", required=True)
    a = ap.parse_args()
    sys.stdout.reconfigure(encoding="utf-8")
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    for lang in ("en", "es"):
        lines = json.loads((Path(a.chosen) / lang / "chosen.json").read_text(encoding="utf-8"))
        caps = captions(lines)
        srt = []
        for n, c in enumerate(caps, start=1):
            srt.append(f"{n}\n{stamp(c['from'])} --> {stamp(c['to'])}\n" + "\n".join(c["lines"]) + "\n")
        (out / f"cantus-{lang}.srt").write_text("\n".join(srt), encoding="utf-8")
        (out / f"captions-{lang}.json").write_text(json.dumps(caps, ensure_ascii=False, indent=1), encoding="utf-8")
        ch = CHAPTERS[lang]
        (out / f"chapters-{lang}.txt").write_text(
            "\n".join(f"{t // 60}:{t % 60:02d} {name}" for t, name in ch) + "\n", encoding="utf-8")
        meta = [";FFMETADATA1"]
        for k, (t, name) in enumerate(ch):
            end = ch[k + 1][0] if k + 1 < len(ch) else int(END_S)
            meta += ["[CHAPTER]", "TIMEBASE=1/1000", f"START={t * 1000}", f"END={end * 1000}", f"title={name}"]
        (out / f"chapters-{lang}.ffmeta").write_text("\n".join(meta) + "\n", encoding="utf-8")
        longest = max(len(l) for c in caps for l in c["lines"])
        print(f"{lang}: {len(caps)} captions, longest line {longest} characters -> {out / f'cantus-{lang}.srt'}")


if __name__ == "__main__":
    main()
