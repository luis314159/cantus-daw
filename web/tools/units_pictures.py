"""The units' pictures for the site and the README, from `shoot_skins`' PNGs.

    cargo run --release -p cantus-ui --example shoot_skins -- <shots>
    python web/tools/units_pictures.py <shots>          # needs Pillow; oxipng if installed
    cargo run --release -p cantus-ui --example shoot_skins -- <shots-es> --es
    python web/tools/units_pictures.py <shots-es> --es  # the Spanish page's: <name>-*-es.webp

Writes, for every instrument and effect:
- web/assets/units/<name>-thumb.webp  (Slate at 100 %, 640 px wide: the card)
- web/assets/units/<name>-{dark,light}.webp  (Slate / Paper at 200 %, at most 1800 px: the page)
- docs/images/units/<name>-{dark,light}.png  (100 %, lossless: the README's large picture)
- docs/images/units/thumbs/<name>.jpg  (480 px: the README's grid)

The pictures' sizes are written into web/src/units_data.rs (`big`, `thumb`): change them there
if a unit's window changes size.
"""
import os
import shutil
import subprocess
import sys

from PIL import Image

ROOT = os.path.normpath(os.path.join(os.path.dirname(__file__), "..", ".."))
WEB = os.path.join(ROOT, "web", "assets", "units")
DOC = os.path.join(ROOT, "docs", "images", "units")

NAMES = """aequalis comprimo terminus culmen multiplex sibilus porta tonsor impetus discrimen spatium
fervor latitudo speculum tonus concentus flexus phasis tremor vibrans anulus translatio fluctus
unda calor modulus exemplar gravis organum lamina chorda pulsus""".split()


def fit(im, w):
    return im if im.width <= w else im.resize((w, round(im.height * w / im.width)), Image.LANCZOS)


def main(src, es=False):
    os.makedirs(WEB, exist_ok=True)
    if es:
        # The Spanish page's pictures only: the README is in English.
        for n in NAMES:
            for theme, tag in (("slate", "dark"), ("paper", "light")):
                big = Image.open(os.path.join(src, f"{n}-{theme}-200.png")).convert("RGB")
                fit(big, 1800).save(os.path.join(WEB, f"{n}-{tag}-es.webp"), "WEBP", quality=82, method=6)
            one = Image.open(os.path.join(src, f"{n}-slate-100.png")).convert("RGB")
            fit(one, 640).save(os.path.join(WEB, f"{n}-thumb-es.webp"), "WEBP", quality=80, method=6)
            print(n, flush=True)
        return
    os.makedirs(os.path.join(DOC, "thumbs"), exist_ok=True)
    for n in NAMES:
        for theme, tag in (("slate", "dark"), ("paper", "light")):
            big = Image.open(os.path.join(src, f"{n}-{theme}-200.png")).convert("RGB")
            fit(big, 1800).save(os.path.join(WEB, f"{n}-{tag}.webp"), "WEBP", quality=82, method=6)
            one = Image.open(os.path.join(src, f"{n}-{theme}-100.png")).convert("RGB")
            if tag == "dark":
                fit(one, 640).save(os.path.join(WEB, f"{n}-thumb.webp"), "WEBP", quality=80, method=6)
                fit(one, 480).save(os.path.join(DOC, "thumbs", f"{n}.jpg"), "JPEG", quality=86,
                                   optimize=True, progressive=True)
            one.save(os.path.join(DOC, f"{n}-{tag}.png"), optimize=True)
        print(n, flush=True)
    if shutil.which("oxipng"):
        pngs = [os.path.join(DOC, f) for f in os.listdir(DOC) if f.endswith(".png")]
        subprocess.run(["oxipng", "-o", "4", "--strip", "safe", "-q", *pngs], check=False)


if __name__ == "__main__":
    args = [a for a in sys.argv[1:] if a != "--es"]
    main(args[0] if args else os.path.join(ROOT, "target", "shots", "skins"), "--es" in sys.argv)
