"""r42:arcus-upper — design sheet v7 (the realistic figures, every instrument): shoots the
editor with the woman and the man holding the cello, the bass, the violin and the viola, in
both looks and both themes at 100 % and 200 %, then the side-by-sides with the owner's
reference photograph (posture, dress and light only — the faces are original) and the
hands at 200 % beside free photographs of real hands.

    python tools/arcus-figures/sheet_v7.py <shoot_arcus.exe> <out dir> <refs dir>

``refs dir`` holds the photographs (not in the repository's tools; the owner's reference
is docs/round42/arcus-upper/ref-owner/6-cellist-woman.png): ``Bow_hand_Violoncello.jpg``
(Wikimedia Commons, CC BY-SA 2.5), ``Right_hand_position_on_bow.jpg`` (Martin
Morgenstern, Wikimedia Commons, CC BY-SA 3.0).
"""

import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
OWNER = os.path.join(ROOT, "docs", "round42", "arcus-upper", "ref-owner")

# Hands at 200 % (boxes in the 2080 × 1360 shot): (shot, box, photograph, its box, label).
HANDS = [
    ("arcus-cello-play-woman-photo-night-en-200.png", (490, 360, 620, 490),
     "Bow_hand_Violoncello.jpg", None,
     "Bow hand, cello (Commons 'Bow hand Violoncello.jpg', CC BY-SA 2.5)"),
    ("arcus-violin-play-man-photo-night-en-200.png", (330, 570, 570, 800),
     "Right_hand_position_on_bow.jpg", None,
     "Bow hand, violin (Commons 'Right hand position on bow.jpg', M. Morgenstern, CC BY-SA 3.0)"),
    ("arcus-violin-play-woman-photo-night-en-200.png", (752, 325, 992, 525),
     "owner:3.png", None, "Left hand, violin (the owner's reference 3)"),
    ("arcus-cello-play-woman-photo-night-en-200.png", (590, 240, 700, 350),
     "owner:6-cellist-woman.png", (230, 130, 330, 260),
     "Left hand, cello (the owner's reference; hands only)"),
]


def run(*a):
    subprocess.run([sys.executable, *a], check=True)


def main():
    exe, out, refs = sys.argv[1:4]
    os.makedirs(out, exist_ok=True)
    if "--no-shoot" not in sys.argv:
        subprocess.run([exe, out, "--hold=woman,man", "--only=play", "--scales=1,2"],
                       check=True, stdout=subprocess.DEVNULL)
    sheet = os.path.join(HERE, "sheet.py")
    ref6 = os.path.join(OWNER, "6-cellist-woman.png")
    credit = ("Reference for posture, dress and light only: 'Woman playing cello during a "
              "concert in Moscow', Tatiana El-Bakri, CC BY-SA 2.0")
    for look in ("photo", "drawn"):
        for theme in ("night", "paper"):
            shot = os.path.join(out, f"arcus-cello-play-woman-{look}-{theme}-en-200.png")
            run(sheet, shot, ref6, os.path.join(out, f"side-cello-woman-{look}-{theme}-vs-reference.png"),
                f"Cantus: the realistic woman ({look}, {theme}, 200 %) - an original face",
                credit)
    names = ["bow-cello-woman", "bow-violin-man", "left-violin-woman", "left-cello-woman"]
    for (shot, box, ref, rbox, label), nm in zip(HANDS, names):
        path = os.path.join(OWNER, ref[6:]) if ref.startswith("owner:") else os.path.join(refs, ref)
        name = f"hand-{nm}-200-vs-photo.png"
        args = [sheet, os.path.join(out, shot), path, os.path.join(out, name),
                "Cantus at 200 % (crop)", label, "--crop=" + ",".join(map(str, box)),
                "--height=420"]
        if rbox:
            args.append("--ref-crop=" + ",".join(map(str, rbox)))
        run(*args)


if __name__ == "__main__":
    main()
