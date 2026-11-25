"""r42:arcus-hair — the hair and face sheet (the owner's review of sheet v8: her hair read as
camouflage, a hard hairline, the face slightly porcelain): the woman with the violin and the
cello and the man with the violin, Photographic, Night and Paper, at 200 %; each face
cropped and doubled; and sheet v8's faces beside the new ones.

    python tools/arcus-figures/hair_sheet.py <shoot_arcus.exe> <out dir> [<sheet v8 dir>] [--no-shoot]

The crops are the picture's pixels at 200 % (``FACES``); ``nup.py`` sets them side by side.
"""

import os
import subprocess
import sys

from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))

# Each face's box in the 200 % shot (x0, y0, x1, y1), by instrument and figure.
FACES = {
    ("violin", "woman"): (370, 170, 650, 450),
    ("cello", "woman"): (510, 130, 690, 290),
    ("violin", "man"): (370, 150, 650, 430),
    ("cello", "man"): (500, 110, 700, 290),
}
# The head and shoulders, wider (the hair whole, the hairline, the chignon).
HEADS = {
    ("violin", "woman"): (300, 100, 720, 520),
    ("cello", "woman"): (440, 80, 760, 400),
    ("violin", "man"): (300, 80, 720, 500),
    ("cello", "man"): (440, 60, 760, 380),
}


def shot(out, inst, who, theme):
    return os.path.join(out, f"arcus-{inst}-play-{who}-photo-{theme}-en-200.png")


def crop2(src, box, dst):
    im = Image.open(src).convert("RGB").crop(box)
    im.resize((im.width * 2, im.height * 2), Image.LANCZOS).save(dst)


def main():
    exe, out = sys.argv[1], sys.argv[2]
    v8 = sys.argv[3] if len(sys.argv) > 3 and not sys.argv[3].startswith("--") else None
    os.makedirs(out, exist_ok=True)
    if "--no-shoot" not in sys.argv:
        for inst in ("violin", "cello"):
            subprocess.run([exe, out, "--hold=woman,man", f"--only={inst}-play+photo",
                            "--scales=2"], check=True, stdout=subprocess.DEVNULL)
    nup = os.path.join(HERE, "nup.py")
    for (inst, who), box in FACES.items():
        for theme in ("night", "paper"):
            src = shot(out, inst, who, theme)
            if not os.path.exists(src):
                continue
            face = os.path.join(out, f"face-{who}-{inst}-photo-{theme}-200-x2.png")
            crop2(src, box, face)
            crop2(src, HEADS[(inst, who)], os.path.join(
                out, f"head-{who}-{inst}-photo-{theme}-200.png"))
            # (Sheet v8's own shot cropped the same way, so the two match to the pixel.)
            old = None
            if v8 and os.path.exists(shot(v8, inst, who, theme)):
                old = os.path.join(out, f"before-face-{who}-{inst}-{theme}.png")
                crop2(shot(v8, inst, who, theme), box, old)
            if old:
                subprocess.run([sys.executable, nup, os.path.join(
                    out, f"before-after-{who}-{inst}-{theme}.png"), "560",
                    f"{old}=Before (sheet v8) - {who}, {inst}, Photographic, {theme}, 200 % x2",
                    f"{face}=After (arcus-hair)"], check=True)


if __name__ == "__main__":
    main()
