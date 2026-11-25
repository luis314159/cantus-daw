"""r42:arcus-upper — the violin's left hand beside the owner's photographs (the owner,
2026-11-24: "the violin's left hand is a disaster"): a 4-up of photograph 4 (a close-up of a
hand on a violin's neck), the app's own picture from photograph 4's angle (a test camera,
``shoot_arcus --cam``), the app's picture from its own camera at 200 % (a crop) and
photograph 3 (the hand from the G side).

    python tools/arcus-figures/hand_sheet.py <shoot_arcus.exe> <dump.json> <out dir>
        [woman|man] [photo|drawn] [night|paper] [--quality=light]

The hand's place comes from the table's row the shot plays (the A string, E5: the hand in
second position, the little finger down), worked out as ``left_hand.py`` poses it.
"""

import json
import os
import subprocess
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import left_hand as lh  # noqa: E402

OWNER = os.path.join(HERE, "..", "..", "docs", "round42", "arcus-upper", "ref-owner")
# The picture's box in the editor's shot (fractions), as sheet.py's.
BOX = (8 / 1040, 62 / 680, 600 / 1040, 490 / 680)


def picture(path):
    from PIL import Image
    im = Image.open(path).convert("RGB")
    w, h = im.size
    return im.crop((int(BOX[0] * w), int(BOX[1] * h), int(BOX[2] * w), int(BOX[3] * h)))


def main():
    from PIL import Image
    pos = [a for a in sys.argv[1:] if not a.startswith("--")]
    exe, dump_path, out = pos[:3]
    who = pos[3] if len(pos) > 3 else "woman"
    look = pos[4] if len(pos) > 4 else "photo"
    theme = pos[5] if len(pos) > 5 else "night"
    light = "--quality=light" in sys.argv
    os.makedirs(out, exist_ok=True)
    dump = json.load(open(dump_path))
    F = next(F for F in dump["figs"] if F["who"] == who and F["inst"] == "violin")
    h = lh.Hand(who)
    # The table's hand for the shot's note (string 2, the little finger, position 1).
    row = next(r for r in F["rows"] if r["string"] == 2 and r["finger"] == 3 and r["at"] == 1.0)
    cache = json.load(open(os.path.join(os.path.dirname(os.path.abspath(dump_path)),
                                        f"left_hand_cache-{who}-violin.json")))
    c = cache[f"{who}-violin-2-3-1"]
    W, Rh = np.array(c["W"]), np.array(c["Rh"])
    Q = np.array(c["Q"]).reshape(5, 3, 3, 3)
    P = h.fk(W, Rh, Q)
    kx, z, bw = (np.array(row[k]) for k in ("kx", "z", "bw"))
    t = (P[1, 1] + P[2, 1] + P[0, 2]) / 3.0
    # Photograph 4: from over the player's left shoulder, onto the G side and the top.
    eye = t - kx * 16.0 + z * 20.0 - bw * 10.0
    cam = ",".join(f"{v:.2f}" for v in [*eye, *t, 7.0])
    tag = f"violin-play-{who}{'-light' if light else ''}-{look}-{theme}-en-200"
    common = [exe, "", f"--hold={who}", f"--only=violin-play+{look}+{theme}", "--scales=2"]
    if light:
        common.append("--quality=light")
    near = os.path.join(out, "near")
    run = common.copy()
    run[1] = near
    subprocess.run(run + [f"--cam={cam}"], check=True, stdout=subprocess.DEVNULL)
    run = common.copy()
    run[1] = out
    subprocess.run(run, check=True, stdout=subprocess.DEVNULL)
    ours_near = picture(os.path.join(near, f"arcus-{tag}.png"))
    shot = Image.open(os.path.join(out, f"arcus-{tag}.png")).convert("RGB")
    # The hand in the picture at 200 % (where the table puts it).
    crop = shot.crop((700, 260, 1040, 560))
    p4 = Image.open(os.path.join(OWNER, "4.webp")).convert("RGB")
    p3 = Image.open(os.path.join(OWNER, "3.png")).convert("RGB")
    p3 = p3.crop((60, 20, 330, 230))
    paths = []
    for name, im in (("p4", p4), ("near", ours_near), ("crop", crop), ("p3", p3)):
        p = os.path.join(out, f"_{name}.png")
        im.save(p)
        paths.append(p)
    lvl = " (Light)" if light else ""
    sheet = os.path.join(out, f"hand-left-violin-{who}-{look}-{theme}{'-light' if light else ''}-4up.png")
    subprocess.run([sys.executable, os.path.join(HERE, "nup.py"), sheet, "460",
                    f"{paths[0]}=Photograph 4 (the owner's reference)",
                    f"{paths[1]}=Cantus{lvl}, from photograph 4's angle (test camera)",
                    f"{paths[2]}=Cantus{lvl}, the picture's camera, 200 %",
                    f"{paths[3]}=Photograph 3 (the owner's reference)"], check=True)
    for p in paths:
        os.remove(p)
    print(sheet)


if __name__ == "__main__":
    main()
