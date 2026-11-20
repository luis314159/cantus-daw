"""The narration's pace against each line's window, before any take is made.

    python -I tools/video/voice/pace.py docs/round41/video/script.md

English is counted in words (a calm presenter's 2.3-2.7 a second), Spanish in syllables
(4.5-5.5 a second), estimated from vowel groups: a strong vowel pair (a, e, o with each
other) splits, everything else is one nucleus, and synalepha across words is not merged,
so the Spanish estimate errs long. Figures and acronyms are counted as they are spoken.
"""

import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import lines as script_lines  # noqa: E402

# How the acronyms and figures in the script are spoken.
SPOKEN_EN = {"VST3": "vee-ess-tee three", "CLAP": "clap", "FL": "eff-ell", "cantusdaw.com": "cantus daw dot com"}
SPOKEN_ES = {"VST3": "uve ese te tres", "CLAP": "clap", "FL": "efe ele", "cantusdaw.com": "cantus daw punto com"}

STRONG = set("aeoáéó")
VOWELS = set("aeiouáéíóúü")


def syllables(word: str) -> int:
    w = word.lower()
    n = 0
    prev = ""
    for ch in w:
        if ch in VOWELS:
            if prev and prev in VOWELS:
                # A hiatus: two strong vowels, or an accented weak vowel.
                if (prev in STRONG and ch in STRONG) or ch in "íú" or prev in "íú":
                    n += 1
            else:
                n += 1
        prev = ch
    return max(n, 1)


def spoken(text: str, table: dict) -> list:
    for k, v in table.items():
        text = text.replace(k, v)
    w = r"[A-Za-zÁÉÍÓÚÜÑáéíóúüñ']+"
    # A hyphenated word ("forty-two", a spelled acronym) is one word in English.
    return re.findall(w + r"(?:-" + w + r")*", text)


def main() -> None:
    for lang, title in (("en", "English"), ("es", "Spanish")):
        print(title)
        for line in script_lines.read(sys.argv[1], lang):
            n, lo, hi, unit = count(line["text"], lang)
            need = (n / hi, n / lo)
            flag = "ok" if need[0] <= line["window"] - 0.5 else "OVER"
            print(
                f"  {line['n']}. {line['start']:5.1f}s window {line['window']:4.1f}s  {n:3d} {unit}"
                f"  needs {need[0]:4.1f}-{need[1]:4.1f}s  {flag}"
            )


def count(text: str, lang: str) -> tuple:
    """The line's size and its pace window: words in English, syllables in Spanish."""
    if lang == "en":
        return len(spoken(text, SPOKEN_EN)), 2.3, 2.7, "words"
    return sum(syllables(w) for w in spoken(text, SPOKEN_ES)), 4.5, 5.5, "syll"


if __name__ == "__main__":
    main()
