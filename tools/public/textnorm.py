"""Text normalisation shared by make_fingerprints.py and scan.py (keep them in step)."""

import html
import re
import unicodedata

SALT = "cantus-rvr1960-v1:"
SHINGLE = 6          # the failing check
SHINGLE_REVIEW = 4   # a wider net, listed for review only
_WORD = re.compile(r"[a-z0-9]+")
_U_BRACE = re.compile(r"\\u\{([0-9A-Fa-f]{1,6})\}")    # Rust: \u{e1}
_U_FOUR = re.compile(r"\\u([0-9A-Fa-f]{4})")           # JSON, JS, Python: \u00e1
_HYPHEN_BREAK = re.compile(r"(\w)-[ \t]*\r?\n[ \t]*(\w)")  # "cora-\nzón" -> "corazón"
_ESCAPED_WS = re.compile(r"\\[ntr]")                   # a literal "\n" inside a string


def _chr(m):
    try:
        c = chr(int(m.group(1), 16))
    except (ValueError, OverflowError):
        return " "
    return " " if 0xD800 <= ord(c) <= 0xDFFF else c


def decode(text):
    """Undo the encodings a quotation may hide behind: HTML entities, \\u escapes, a word
    hyphenated across lines, escaped newlines and tabs."""
    if "&" in text:
        text = html.unescape(text)
    if "\\u" in text:
        text = _U_FOUR.sub(_chr, _U_BRACE.sub(_chr, text))
    if "-" in text:
        text = _HYPHEN_BREAK.sub(r"\1\2", text)
    if "\\" in text:
        text = _ESCAPED_WS.sub(" ", text)
    return text


def words(text):
    """Lower-case ASCII words without accents or punctuation ("Jehová;" -> "jehova")."""
    plain = unicodedata.normalize("NFKD", decode(text)).encode("ascii", "ignore")
    return _WORD.findall(plain.decode("ascii").lower())


def shingles(text, n=SHINGLE):
    """Every run of n consecutive words, joined by single spaces."""
    w = words(text)
    return {" ".join(w[i:i + n]) for i in range(len(w) - n + 1)}
