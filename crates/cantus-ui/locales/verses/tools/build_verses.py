#!/usr/bin/env python3
"""Build the verse files from their sources — never from memory.

    py -3 build_verses.py <work-dir>

<work-dir> holds what was downloaded (see ../README.md for the URLs and dates):

- `engwebu_vpl.txt` and `usfm_engwebu/` — the World English Bible (eBible.org, public
  domain): the text, and the Psalm titles (USFM `\\d`) to take off verse 1.
- `spaRV1909_vpl.txt` — the Reina-Valera 1909 (eBible.org, public domain).
- `bg/` — one page per passage of the Reina-Valera 1960 from Bible Gateway, fetched with
  curl (the system's TLS) and kept, so a build can be repeated without the network.

Writes `../en.txt`, `../es.txt` (RVR1960) and `../es-1909.txt`, one passage per line:
`PSA 33:3 | text`. Fails, rather than drop a passage, when one is missing or too long.
"""

import html
import html.parser
import os
import re
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.dirname(HERE)

# The longest a passage may be, in characters, in any translation: what the splash can
# show in its verse band without shrinking the type below its floor.
MAX_CHARS = 250

# Bible Gateway's English names for the USFM codes used in the list.
BG_NAME = {
    "GEN": "Genesis", "EXO": "Exodus", "NUM": "Numbers", "DEU": "Deuteronomy",
    "JOS": "Joshua", "JDG": "Judges", "1SA": "1 Samuel", "2SA": "2 Samuel",
    "1KI": "1 Kings", "2KI": "2 Kings", "1CH": "1 Chronicles", "2CH": "2 Chronicles",
    "EZR": "Ezra", "NEH": "Nehemiah", "JOB": "Job", "PSA": "Psalm", "PRO": "Proverbs",
    "ECC": "Ecclesiastes", "ISA": "Isaiah", "JER": "Jeremiah", "LAM": "Lamentations",
    "DAN": "Daniel", "MIC": "Micah", "NAM": "Nahum", "HAB": "Habakkuk",
    "ZEP": "Zephaniah", "HAG": "Haggai", "MAT": "Matthew", "MRK": "Mark", "LUK": "Luke",
    "JHN": "John", "ACT": "Acts", "ROM": "Romans", "1CO": "1 Corinthians",
    "2CO": "2 Corinthians", "GAL": "Galatians", "EPH": "Ephesians",
    "PHP": "Philippians", "COL": "Colossians", "1TH": "1 Thessalonians",
    "2TH": "2 Thessalonians", "2TI": "2 Timothy", "TIT": "Titus", "HEB": "Hebrews",
    "JAS": "James", "1PE": "1 Peter", "1JN": "1 John", "REV": "Revelation",
}


def read_list():
    refs = []
    for line in open(os.path.join(HERE, "list.txt"), encoding="utf-8"):
        line = line.split("#", 1)[0].strip()
        if not line:
            continue
        m = re.fullmatch(r"([1-3A-Z]{3}) (\d+):(\d+)(?:-(\d+))?", line)
        if not m:
            sys.exit(f"bad reference: {line!r}")
        book, c, v1, v2 = m.group(1), int(m.group(2)), int(m.group(3)), m.group(4)
        v2 = int(v2) if v2 else v1
        refs.append((book, c, v1, v2, line))
    seen = set()
    for r in refs:
        if r[4] in seen:
            sys.exit(f"listed twice: {r[4]}")
        seen.add(r[4])
    return refs


# The VPL files name books BibleWorks' way; the list uses USFM codes.
VPL_TO_USFM = {
    "SOL": "SNG", "EZE": "EZK", "JOE": "JOL", "NAH": "NAM", "MAR": "MRK", "JOH": "JHN",
    "PHI": "PHP", "JAM": "JAS", "1JO": "1JN", "2JO": "2JN", "3JO": "3JN",
}


def read_vpl(path):
    verses = {}
    for line in open(path, encoding="utf-8-sig"):
        m = re.match(r"([1-4A-Z]{3}) (\d+):(\d+) (.*)", line.rstrip("\n"))
        if m:
            book = VPL_TO_USFM.get(m.group(1), m.group(1))
            verses[(book, int(m.group(2)), int(m.group(3)))] = m.group(4).strip()
    return verses


def web_psalm_titles(usfm_dir):
    """Psalm number -> its title (`\\d`), which the VPL file runs into verse 1."""
    path = next(
        os.path.join(usfm_dir, f) for f in os.listdir(usfm_dir) if "PSA" in f
    )
    titles, chapter = {}, 0
    for line in open(path, encoding="utf-8-sig"):
        line = line.strip()
        if line.startswith("\\c "):
            chapter = int(line.split()[1])
        elif line.startswith("\\d "):
            text = re.sub(r"\\w ([^|\\]*)\|[^\\]*\\w\*", r"\1", line[3:])
            text = re.sub(r"\\f .*?\\f\*", "", text)
            titles[chapter] = " ".join(text.split())
    return titles


def tidy(s):
    return " ".join(s.replace(" ", " ").split())


def web_text(verses, titles, book, c, v1, v2):
    parts = []
    for v in range(v1, v2 + 1):
        t = verses[(book, c, v)]
        if book == "PSA" and v == 1 and c in titles:
            title = titles[c]
            if not t.startswith(title):
                sys.exit(f"WEB Psalm {c}: the title {title!r} does not open verse 1")
            t = t[len(title):]
        parts.append(tidy(t))
    return " ".join(parts)


CAPS = re.compile(r"\b([A-ZÁÉÍÓÚÑÜ]{2,})\b")


def rv1909_text(verses, book, c, v1, v2):
    """The 1909's own conventions, undone for display: the first word of a chapter is set
    in capitals (and in the Psalms it follows the title, which is run into verse 1);
    words the translators supplied are in [brackets], printed in italics in the book."""
    parts = []
    for v in range(v1, v2 + 1):
        t = verses[(book, c, v)]
        if v == 1:
            m = CAPS.search(t)
            if m:
                if book == "PSA":
                    t = t[m.start():]
                    m = CAPS.search(t)
                w = m.group(1)
                t = t[: m.start()] + w[0] + w[1:].lower() + t[m.end():]
        t = t.replace("[", "").replace("]", "")
        parts.append(tidy(t))
    return " ".join(parts)


class Passage(html.parser.HTMLParser):
    """The verse text of a Bible Gateway passage page: the spans of class `text`, without
    headings, Psalm titles, verse and chapter numbers, footnotes or cross-references."""

    SKIP_TAGS = {"h1", "h2", "h3", "h4", "h5", "h6", "sup", "script", "style"}
    SKIP_CLASSES = {"chapternum", "versenum", "footnote", "crossreference",
                    "footnotes", "crossrefs", "passage-other-trans", "full-chap-link"}

    def __init__(self):
        super().__init__()
        self.stack = []
        self.out = []
        self.in_passage = 0

    def handle_starttag(self, tag, attrs):
        a = dict(attrs)
        classes = set((a.get("class") or "").split())
        skip = tag in self.SKIP_TAGS or bool(classes & self.SKIP_CLASSES)
        is_text = tag == "span" and "text" in classes
        if tag in ("br", "img", "meta", "link", "input", "hr"):
            return
        self.stack.append((tag, skip, is_text, "passage-text" in classes))

    def handle_endtag(self, tag):
        while self.stack:
            t = self.stack.pop()
            if t[0] == tag:
                # A poem's lines are spans of their own: a space between them.
                if t[2] and not any(s[2] for s in self.stack):
                    self.out.append(" ")
                break

    def handle_data(self, data):
        if any(s[1] for s in self.stack):
            return
        if not any(s[3] for s in self.stack):
            return
        if any(s[2] for s in self.stack):
            self.out.append(data)


def bg_fetch(work, book, c, v1, v2):
    os.makedirs(os.path.join(work, "bg"), exist_ok=True)
    tag = f"{book}_{c}_{v1}" + (f"-{v2}" if v2 != v1 else "")
    path = os.path.join(work, "bg", tag + ".html")
    if not os.path.exists(path) or os.path.getsize(path) < 20000:
        ref = f"{BG_NAME[book]} {c}:{v1}" + (f"-{v2}" if v2 != v1 else "")
        url = (
            "https://www.biblegateway.com/passage/?search="
            + ref.replace(" ", "%20").replace(":", "%3A")
            + "&version=RVR1960"
        )
        for attempt in range(5):
            r = subprocess.run(
                ["curl", "-sS", "-m", "60", "-A", "Mozilla/5.0", "-o", path, url]
            )
            if r.returncode == 0:
                break
            time.sleep(5.0 * (attempt + 1))
        else:
            sys.exit(f"could not fetch {url}")
        time.sleep(1.0)
    page = open(path, encoding="utf-8").read()
    if "RVR1960" not in page:
        sys.exit(f"{path}: not a Bible Gateway RVR1960 page (proxy?)")
    p = Passage()
    p.feed(page)
    text = tidy(html.unescape("".join(p.out)))
    if not text:
        sys.exit(f"{path}: no verse text found")
    return text


def main():
    work = sys.argv[1]
    refs = read_list()
    web = read_vpl(os.path.join(work, "engwebu_vpl.txt"))
    titles = web_psalm_titles(os.path.join(work, "usfm_engwebu"))
    rv = read_vpl(os.path.join(work, "spaRV1909_vpl.txt"))
    files = {"en.txt": [], "es.txt": [], "es-1909.txt": []}
    too_long = []
    for book, c, v1, v2, ref in refs:
        texts = {
            "en.txt": web_text(web, titles, book, c, v1, v2),
            "es.txt": bg_fetch(work, book, c, v1, v2),
            "es-1909.txt": rv1909_text(rv, book, c, v1, v2),
        }
        for name, t in texts.items():
            # A passage that begins mid-sentence still begins with a capital on screen.
            t = t[:1].upper() + t[1:]
            if len(t) > MAX_CHARS:
                too_long.append(f"{ref} ({name}, {len(t)})")
            files[name].append(f"{ref} | {t}")
    if too_long:
        sys.exit("too long for the splash:\n  " + "\n  ".join(too_long))
    for name, lines in files.items():
        with open(os.path.join(OUT, name), "w", encoding="utf-8", newline="\n") as f:
            f.write("\n".join(lines) + "\n")
    verses = sum(v2 - v1 + 1 for _, _, v1, v2, _ in refs)
    print(f"{len(refs)} passages, {verses} verses")


if __name__ == "__main__":
    main()
