"""Render a design sheet with Edge headless at 1:1 device pixels, and its 4x by nearest
neighbour (so every device pixel stays visible — a vector re-render at 4x would hide the
pixel-snapping this family is about).

    python render.py sheet.html "t=night&groups=transport" out-1x.png [--x4 out-4x.png]

The page reports its own size in <body data-w data-h>; we read it with --dump-dom first.
"""
import os
import re
import subprocess
import sys
import tempfile

from PIL import Image

EDGE = r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"
HERE = os.path.dirname(os.path.abspath(__file__))
PROFILE = os.path.join(os.environ.get("TEMP", tempfile.gettempdir()), "cantus-icons-edge")


def url_for(page, query):
    path = os.path.abspath(os.path.join(HERE, page)).replace("\\", "/")
    return f"file:///{path}" + (f"?{query}" if query else "")


def edge(args):
    base = [EDGE, "--headless=new", "--disable-gpu", "--hide-scrollbars",
            f"--user-data-dir={PROFILE}", "--virtual-time-budget=3000"]
    return subprocess.run(base + args, capture_output=True, text=True, timeout=120)


def size_of(url):
    dom = edge(["--dump-dom", url]).stdout
    w = re.search(r'data-w="(\d+)"', dom)
    h = re.search(r'data-h="(\d+)"', dom)
    if not (w and h):
        raise SystemExit("page did not report its size:\n" + dom[:2000])
    return int(w.group(1)), int(h.group(1))


def main():
    page, query, out = sys.argv[1], sys.argv[2], sys.argv[3]
    x4 = sys.argv[sys.argv.index("--x4") + 1] if "--x4" in sys.argv else None
    url = url_for(page, query)
    w, h = size_of(url)
    out = os.path.abspath(out)
    edge(["--force-device-scale-factor=1", f"--window-size={w},{h}", f"--screenshot={out}", url])
    im = Image.open(out).convert("RGB").crop((0, 0, w, h))
    im.save(out)
    print(f"{out}: {w}x{h}")
    if x4:
        im.resize((w * 4, h * 4), Image.NEAREST).save(os.path.abspath(x4))
        print(f"{x4}: {w * 4}x{h * 4}")


if __name__ == "__main__":
    main()
