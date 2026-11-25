"""r42:arcus-upper — design sheet v8 (the owner's review of v7, 2026-11-24): the woman and
the man with the violin and the cello, High and Light (*Figure detail*), Photographic and
Drawn, Night and Paper, 100 % and 200 %; High beside Light; the violin's left hand beside
the owner's photographs (``hand_sheet.py``); her face at 200 % in the Photographic look.

    python tools/arcus-figures/sheet_v8.py <shoot_arcus.exe> <dump.json> <out dir> [--no-shoot]

``dump.json`` is the left hand's inputs (``left_hand.py``; its solved caches beside it).
"""

import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))


def py(*a):
    subprocess.run([sys.executable, *a], check=True)


def main():
    exe, dump, out = sys.argv[1:4]
    os.makedirs(out, exist_ok=True)
    if "--no-shoot" not in sys.argv:
        for q in ("high", "light"):
            for name in ("violin-play", "cello-play"):
                subprocess.run([exe, out, "--hold=woman,man", f"--only={name}", "--scales=1,2",
                                f"--quality={q}"], check=True, stdout=subprocess.DEVNULL)
    from PIL import Image
    nup = os.path.join(HERE, "nup.py")
    # High beside Light, each figure and instrument, Photographic / Night at 200 %.
    for who in ("woman", "man"):
        for inst in ("violin", "cello"):
            for look in ("photo", "drawn"):
                for theme in ("night", "paper"):
                    hi = os.path.join(out, f"arcus-{inst}-play-{who}-{look}-{theme}-en-200.png")
                    lo = os.path.join(out, f"arcus-{inst}-play-{who}-light-{look}-{theme}-en-200.png")
                    if not (os.path.exists(hi) and os.path.exists(lo)):
                        continue
                    box = "@20,120,1200,960"
                    py(nup, os.path.join(out, f"side-{inst}-{who}-{look}-{theme}-high-vs-light.png"),
                       "520", f"{hi}{box}=High (the default) - {who}, {inst}, {look}, {theme}, 200 %",
                       f"{lo}{box}=Light (within the +1 ms budget)")
    # Her face at 200 %, Photographic, both themes (an original face: never the photo's).
    for theme in ("night", "paper"):
        for inst, box in (("violin", (370, 170, 650, 450)), ("cello", (510, 130, 690, 290))):
            p = os.path.join(out, f"arcus-{inst}-play-woman-photo-{theme}-en-200.png")
            if os.path.exists(p):
                im = Image.open(p).convert("RGB").crop(box)
                im.resize((im.width * 2, im.height * 2), Image.LANCZOS).save(
                    os.path.join(out, f"face-woman-{inst}-photo-{theme}-200-x2.png"))
    # The left hand beside the photographs, both figures, both looks, High and Light.
    for who in ("woman", "man"):
        for look, theme in (("photo", "night"), ("drawn", "paper")):
            for extra in ([], ["--quality=light"]):
                py(os.path.join(HERE, "hand_sheet.py"), exe, dump, out, who, look, theme, *extra)


if __name__ == "__main__":
    main()
