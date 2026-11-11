"""The units' pictures against the sizes the site declares (`big`, `thumb` in
web/src/units_data.rs), so a window that changed size cannot go out with a wrong aspect.

    python tools/shots/check_units.py          # report
    python tools/shots/check_units.py --fix    # write the measured sizes into units_data.rs
"""
import os
import re
import sys

from PIL import Image

ROOT = os.path.normpath(os.path.join(os.path.dirname(__file__), "..", ".."))
DATA = os.path.join(ROOT, "web", "src", "units_data.rs")
UNITS = os.path.join(ROOT, "web", "assets", "units")
ENTRY = re.compile(
    r'(file: "(?P<file>[a-z]+)",.*?big: \()(?P<bw>\d+), (?P<bh>\d+)(\),\s*thumb: \()(?P<tw>\d+), (?P<th>\d+)\)',
    re.S,
)


def main(fix):
    text = open(DATA, encoding="utf-8").read()
    bad = []

    def check(m):
        f = m["file"]
        out = m.group(0)
        sizes = {}
        for kind in ("dark", "light", "thumb", "dark-es", "light-es", "thumb-es"):
            p = os.path.join(UNITS, f"{f}-{kind}.webp")
            sizes[kind] = Image.open(p).size if os.path.exists(p) else None
        big = sizes["dark"]
        thumb = sizes["thumb"]
        for kind, s in sizes.items():
            want = thumb if kind.startswith("thumb") else big
            if s is None or s != want:
                bad.append(f"{f}-{kind}.webp is {s}, its twin {want}")
        declared = ((int(m["bw"]), int(m["bh"])), (int(m["tw"]), int(m["th"])))
        if (big, thumb) != declared:
            bad.append(f"{f}: declared big {declared[0]} thumb {declared[1]}, pictures {big} {thumb}")
            if fix and big and thumb:
                out = (f"{m.group(1)}{big[0]}, {big[1]}{m.group(5)}{thumb[0]}, {thumb[1]})")
        return out

    new = ENTRY.sub(check, text)
    n = len(ENTRY.findall(text))
    for b in bad:
        print(b)
    if fix and new != text:
        open(DATA, "w", encoding="utf-8", newline="").write(new)
        print("units_data.rs: sizes written")
    print(f"{n} units checked, {len(bad)} difference(s)")
    return 0 if not bad or fix else 1


if __name__ == "__main__":
    sys.exit(main("--fix" in sys.argv))
