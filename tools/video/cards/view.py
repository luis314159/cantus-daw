"""Lay card stills (transparent PNGs from `render.mjs --still`) on a plain slate plate, so
they can be looked at as opaque pictures; the film will be under them in the video.

    python -I tools/video/cards/view.py F:/projects/video-work/cards/stills

Reads <dir>/<lang>/<card>-still.png and writes <dir>/view/<lang>-<card>.png.
"""

import os
import sys
from pathlib import Path

from PIL import Image

PLATE = (70, 84, 98, 255)  # a mid slate, darker than any card text, lighter than its panels


def main() -> None:
    root = Path(sys.argv[1])
    out = root / "view"
    out.mkdir(exist_ok=True)
    for f in sorted(root.glob("*/*-still.png")):
        im = Image.open(f).convert("RGBA")
        plate = Image.new("RGBA", im.size, PLATE)
        plate.alpha_composite(im)
        name = f"{f.parent.name}-{f.name.replace('-still', '')}"
        plate.convert("RGB").save(out / name)
        print(out / name)


if __name__ == "__main__":
    os.environ.setdefault("PYTHONIOENCODING", "utf-8")
    main()
