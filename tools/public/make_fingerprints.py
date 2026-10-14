#!/usr/bin/env python3
"""Write the RVR1960 fingerprints: salted hashes of the RVR1960 text's shingles.

The Reina-Valera 1960 is under copyright (Sociedades Biblicas Unidas) and may not be
published without written permission (crates/cantus-ui/locales/verses/README.md). To check
a repository for it *without publishing it*, this script hashes every run of N consecutive
words of `es.txt` that does **not** also occur in the public-domain Reina-Valera 1909
(`es-1909.txt`) or the World English Bible (`en.txt`), and keeps only the hashes:

  rvr1960.fingerprints     6-word runs: `scan.py` FAILS on any blob holding one
  rvr1960-4.fingerprints   4-word runs: a wider net for short quotations, listed for review

    python3 tools/public/make_fingerprints.py [verses_dir] [out_dir]

Run it from a tree that still holds `es.txt` (the owner's private tree); the output holds
no Bible text and is safe to publish.
"""

import hashlib
import os
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from textnorm import SALT, SHINGLE, SHINGLE_REVIEW, shingles  # noqa: E402

OUT = {SHINGLE: "rvr1960.fingerprints", SHINGLE_REVIEW: "rvr1960-4.fingerprints"}


def verse_texts(path):
    """The text part of each `REF | text` line."""
    with open(path, encoding="utf-8") as f:
        for line in f:
            if "|" in line:
                yield line.split("|", 1)[1]


def write(verses, out, n):
    public = set()
    for name in ("es-1909.txt", "en.txt"):
        for text in verse_texts(os.path.join(verses, name)):
            public |= shingles(text, n)
    own = set()
    seen = 0
    for text in verse_texts(os.path.join(verses, "es.txt")):
        seen += 1
        own |= shingles(text, n) - public
    hashes = sorted(
        hashlib.sha256((SALT + s).encode("utf-8")).hexdigest()[:20] for s in own)
    with open(out, "w", encoding="utf-8", newline="\n") as f:
        f.write("# RVR1960 fingerprints: sha256(salt + normalised %d-word shingle)[:20]\n" % n)
        f.write("# from %d passages; shingles shared with RV1909/WEB left out. No text here.\n"
                % seen)
        for h in hashes:
            f.write(h + "\n")
    print("%d passages, %d %d-word fingerprints -> %s" % (seen, len(hashes), n, out))


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    root = os.path.dirname(os.path.dirname(here))
    verses = sys.argv[1] if len(sys.argv) > 1 else os.path.join(
        root, "crates", "cantus-ui", "locales", "verses")
    out_dir = sys.argv[2] if len(sys.argv) > 2 else here
    for n, name in OUT.items():
        write(verses, os.path.join(out_dir, name), n)


if __name__ == "__main__":
    main()
