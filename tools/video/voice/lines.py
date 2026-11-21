"""The narration's lines, read from the script, for the take generator and the gate.

Each line: its number, its start in seconds, its window (to the next line's start, the
last to 1:20) and its text with the brackets taken off. One parser for every tool, so a
change to `script.md` reaches the takes, the gate and the captions alike.
"""

import re

END = 80.0
HEADINGS = {"en": "### English", "es": "### Español"}


def read(script_path: str, lang: str) -> list:
    md = open(script_path, encoding="utf-8").read().replace("\r\n", "\n")
    part = md.split(HEADINGS[lang], 1)[1].split("\n### ", 1)[0]
    found = []
    pattern = r"^(\d+)\. \*\((\d+):(\d+(?:\.\d+)?)\)\* (.+?)(?=^\d+\. |\Z)"
    for m in re.finditer(pattern, part, re.S | re.M):
        start = int(m.group(2)) * 60 + float(m.group(3))
        text = " ".join(m.group(4).split()).strip()
        text = text.strip("\"«»“”").replace("[", "").replace("]", "")
        found.append({"n": int(m.group(1)), "start": start, "text": text})
    for i, line in enumerate(found):
        nxt = found[i + 1]["start"] if i + 1 < len(found) else END
        line["window"] = nxt - line["start"]
    return found
