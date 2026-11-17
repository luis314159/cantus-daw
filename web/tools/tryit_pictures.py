"""Try it's pictures (fx side round, docs/round-fx/site.md): the PNGs `render_tryit` shoots,
one per preset, theme and language, as the WebP the site shows beside each preset.

    cargo run --release -p cantus-ui --example render_tryit -- <SampleData> web target/shots/tryit
    python web/tools/tryit_pictures.py target/shots/tryit        # needs Pillow

Reads <shots>/<unit>/<preset>-{slate,paper}-{en,es}.png and writes
web/assets/tryit/<unit>/<preset>-{dark,light}[-es].webp (at most 1200 px wide, quality 78),
then checks that every manifest's takes and pictures are there and reports the sizes.
"""
import json
import os
import sys

from PIL import Image

ROOT = os.path.normpath(os.path.join(os.path.dirname(__file__), "..", ".."))
TRY = os.path.join(ROOT, "web", "assets", "tryit")
WIDTH = 1200
NAMES = {("slate", "en"): "dark", ("paper", "en"): "light", ("slate", "es"): "dark-es", ("paper", "es"): "light-es"}


def convert(shots):
    made = 0
    for unit in sorted(os.listdir(shots)):
        src = os.path.join(shots, unit)
        if not os.path.isdir(src):
            continue
        out = os.path.join(TRY, unit)
        os.makedirs(out, exist_ok=True)
        for f in sorted(os.listdir(src)):
            if not f.endswith(".png"):
                continue
            stem, theme, lang = f[:-4].rsplit("-", 2)
            im = Image.open(os.path.join(src, f)).convert("RGB")
            if im.width > WIDTH:
                im = im.resize((WIDTH, round(im.height * WIDTH / im.width)), Image.LANCZOS)
            im.save(os.path.join(out, f"{stem}-{NAMES[(theme, lang)]}.webp"), "WEBP", quality=78, method=6)
            made += 1
    return made


def check():
    bad, total = [], 0
    for m in sorted(f for f in os.listdir(TRY) if f.endswith(".json")):
        man = json.load(open(os.path.join(TRY, m), encoding="utf-8"))
        files = [man["dry"]] + [p["wet"] for p in man["presets"]]
        for p in man["presets"]:
            if p.get("pic"):
                files += [f"{p['pic']}-{v}.webp" for v in NAMES.values()]
        for f in files:
            path = os.path.join(TRY, *f.split("/")[2:])
            if os.path.exists(path):
                total += os.path.getsize(path)
            else:
                bad.append(f"{m}: {f}")
    print(f"{total / 1e6:.1f} MB of takes and pictures named by the manifests")
    for b in bad:
        print("missing:", b)
    return not bad


if __name__ == "__main__":
    if len(sys.argv) > 1:
        print(convert(sys.argv[1]), "pictures")
    sys.exit(0 if check() else 1)
