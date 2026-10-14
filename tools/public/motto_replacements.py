#!/usr/bin/env python3
"""Write a `git filter-repo --replace-text` file that turns the RVR1960 quotations found in
the history into the public-domain Reina-Valera 1909 — without this repository ever holding
the RVR1960 wording: the rules are built from the private tree's `es.txt`.

    python3 tools/public/motto_replacements.py <verses_dir_with_es.txt> <out_file>

The out file goes OUTSIDE the repository (it quotes the RVR1960). Rules, in order:
  1. Colossians 3:23 (the motto), whole (any line wrapping)   -> the RV1909 verse
  2. Colossians 3:23 up to "Señor" (the release notes)         -> the RV1909, up to "Señor"
  3. "Colosenses 3:23 (RV1909)"                               -> "Colosenses 3:23 (RV1909)"
  4. Psalm 33:3's first five words (docs/round20/verse.md)     -> the RV1909's first five
"""

import re
import sys


def verse(path, ref):
    with open(path, encoding="utf-8") as f:
        for line in f:
            if line.startswith(ref + " |"):
                return line.split("|", 1)[1].strip().rstrip(";.")
    raise SystemExit("%s not in %s" % (ref, path))


def pattern(text):
    """The words of `text` with any run of whitespace between them (line wrapping)."""
    return r"\s+".join(re.escape(w) for w in text.split())


def upto(text, word):
    words = text.split()
    i = next(i for i, w in enumerate(words) if w.strip(",;.") == word)
    return " ".join(words[: i + 1]).rstrip(",")


def first(text, n):
    return " ".join(text.split()[:n]).rstrip(",;:.")


def main():
    if len(sys.argv) != 3:
        raise SystemExit(__doc__)
    d, out = sys.argv[1], sys.argv[2]
    rvr, rv = verse(d + "/es.txt", "COL 3:23"), verse(d + "/es-1909.txt", "COL 3:23")
    psa, psa_rv = verse(d + "/es.txt", "PSA 33:3"), verse(d + "/es-1909.txt", "PSA 33:3")
    rules = [
        "regex:%s==>%s" % (pattern(rvr), rv),
        "regex:%s==>%s" % (pattern(upto(rvr, "Señor")), upto(rv, "Señor")),
        "Colosenses 3:23 (RV1909)==>Colosenses 3:23 (RV1909)",
        "regex:%s==>%s" % (pattern(first(psa, 5)), first(psa_rv, 5)),
    ]
    with open(out, "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(rules) + "\n")
    print("%d rules -> %s" % (len(rules), out))


if __name__ == "__main__":
    main()
