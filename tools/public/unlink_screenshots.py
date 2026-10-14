#!/usr/bin/env python3
"""The day the repository goes public: turn Markdown links to the research screenshots into
plain text, since the screenshots stay out of the public repository (ADR 0049).

    python3 tools/public/unlink_screenshots.py [repo_dir]      # rewrites research/**/*.md

- inline: `[family-01](screenshots/family-01-lineup.jpg)` becomes `family-01`; an image
  with no alt text (`![](screenshots/x.png)`) becomes `x.png`;
- reference-style: a definition `[fam1]: screenshots/x.png` is removed, and its uses
  `[text][fam1]`, `[fam1][]` and `[fam1]` become `text` / `fam1`.
Matching is case-insensitive (`Screenshots/` too). Nothing else in the files changes.
Prints each file it changed with the number of links.
"""

import os
import re
import sys

INLINE = re.compile(r"(?i)!?\[([^\]]*)\]\(<?([^)\s>]*screenshots/[^)\s>]*)>?(?:\s+\"[^\"]*\")?\)")
DEFINITION = re.compile(r"(?im)^[ \t]{0,3}\[([^\]]+)\]:[ \t]*<?[^\s>]*screenshots/\S*.*(?:\r?\n|$)")


def plain(m):
    label, target = m.group(1), m.group(2)
    return label if label.strip() else os.path.basename(target.rstrip("/")) or target


def unlink(text):
    labels = {m.group(1).strip().lower() for m in DEFINITION.finditer(text)}
    text, n = INLINE.subn(plain, text)
    text, d = DEFINITION.subn("", text)
    if labels:
        def use(m):
            text_part, ref = m.group(1), m.group(2)
            key = (ref if ref else text_part).strip().lower()
            return text_part if key in labels else m.group(0)
        # [text][label], [label][] and a bare [label] not followed by ( or :
        text, u = re.subn(r"!?\[([^\]]+)\]\[([^\]]*)\]", use, text)
        bare = re.compile(r"!?\[([^\]]+)\](?![\[(:])")
        text, b = bare.subn(lambda m: m.group(1) if m.group(1).strip().lower() in labels
                            else m.group(0), text)
        n += u + b
    return text, n + d


def main():
    root = sys.argv[1] if len(sys.argv) > 1 else "."
    total = 0
    for dirpath, _dirs, files in os.walk(os.path.join(root, "research")):
        for name in sorted(files):
            if not name.lower().endswith(".md"):
                continue
            path = os.path.join(dirpath, name)
            with open(path, encoding="utf-8", newline="") as f:
                text = f.read()
            new, n = unlink(text)
            if n:
                with open(path, "w", encoding="utf-8", newline="") as f:
                    f.write(new)
                print("%4d  %s" % (n, os.path.relpath(path, root)))
                total += n
    print("%d links made plain" % total)


if __name__ == "__main__":
    main()
