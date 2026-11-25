"""r42:arcus-hand — the violin's and the viola's left hand from the app beside photographs
of professional violinists at the same angle (the owner, 2026-11-25: "URGENT LA MANO
IZQUIERDA DEL VIOLIN"; the measures in ``left_hand_refs.md``).

For each figure (woman, man), instrument (violin, viola) and case (first position: the A
string, the first finger; a high position: the E string an octave up), ``shoot_arcus`` draws
the app's own picture with a test camera round the left hand (``--cam``) from three ways:
the app's own camera's, photograph 5's (the audience's side, square to the neck, a little
over it) and photograph 2's (square, level). Each row: the three, then the reference
photographs that match (study material, outside the repository).

    python tools/arcus-figures/hand_sheet_v10.py <shoot_arcus.exe> <dump.json> <out dir>
        [--refs=<dir>] [--look=photo|drawn] [--theme=night|paper] [--only=woman-violin]

Without ``--refs`` the rows hold the app's pictures only (what may go in the repository);
with it, the photographs are added (keep those sheets outside the tree). ``numbers.txt``
beside them: each case's picture measures (``hand_study.picture``) from the three ways.
"""

import json
import os
import subprocess
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import hand_rig as hr  # noqa: E402
import hand_study as hs  # noqa: E402
import left_hand as lh  # noqa: E402

# (tag, string, MIDI note, the table's row: string, finger, position)
CASES = {
    "violin": [("first", 2, 71, (2, 0, 1.0)), ("high", 3, 88, (3, 0, 12.0))],
    "viola": [("first", 2, 64, (2, 0, 1.0)), ("high", 3, 81, (3, 0, 12.0))],
}
# The reference photographs for a case (files in --refs, then the owner's).
REFS = {
    "first": ["crop-11.jpg", "crop-08.jpg", "crop-19.jpg", "owner:5.png"],
    "high": ["crop-02.jpg", "ref-12-frontleft-soloist-hope.jpg", "ref-40-side-section-benedetti.jpg"],
}
OWNER = os.path.join(HERE, "..", "..", "docs", "round42", "arcus-upper", "ref-owner")
H = 420
NL = chr(10)


def shoot(exe, out, who, inst, case, cam, look, theme, tag):
    """One close shot of the app's picture from ``cam`` (eye, at, half) for the case."""
    _, string, note, _ = case
    eye, at, half = cam
    c = ",".join(f"{v:.3f}" for v in list(eye) + list(at) + [half])
    d = os.path.join(out, "shots", tag)
    os.makedirs(d, exist_ok=True)
    only = f"{inst}-play+{who}+{look}-{theme}+200"
    subprocess.run([exe, d, f"--hold={who}", f"--note={string},{note}", f"--cam={c}",
                    f"--only={only}", "--scales=2"], check=True, capture_output=True)
    pngs = [os.path.join(d, f) for f in os.listdir(d) if f.endswith(".png")]
    return max(pngs, key=os.path.getmtime) if pngs else None


def picture_box(path):
    """The Arcus picture's box in an editor shot (as hand_sheet.py's)."""
    from PIL import Image
    im = Image.open(path).convert("RGB")
    w, h = im.size
    box = (8 / 1040, 62 / 680, 600 / 1040, 490 / 680)
    return im.crop((int(box[0] * w), int(box[1] * h), int(box[2] * w), int(box[3] * h)))


def label(im, text):
    from PIL import ImageDraw
    d = ImageDraw.Draw(im)
    d.rectangle([0, 0, im.width, 18], fill=(16, 16, 20))
    d.text((6, 3), text, fill=(235, 235, 235))
    return im


def full_shots(exe, out, who, inst, case, look, theme):
    """The whole picture from the new camera and from the earlier one (``--view=4,7``)."""
    from PIL import Image
    _, string, note, _ = case
    tiles = []
    for name, extra in (("earlier camera", ["--view=4,7"]), ("new camera", [])):
        d = os.path.join(out, "shots", f"full-{who}-{inst}-{case[0]}-{name.split()[0]}-{look}-{theme}")
        os.makedirs(d, exist_ok=True)
        subprocess.run([exe, d, f"--hold={who}", f"--note={string},{note}", "--scales=1",
                        f"--only={inst}-play+{who}+{look}-{theme}+100"] + extra,
                       check=True, capture_output=True)
        pngs = [os.path.join(d, f) for f in os.listdir(d) if f.endswith(".png")]
        if pngs:
            im = picture_box(max(pngs, key=os.path.getmtime))
            im = im.resize((int(im.width * H / im.height), H))
            tiles.append(label(im, f"{who}, {inst}: the {name} (100 %)"))
    return tiles


def main():
    from PIL import Image
    pos = [a for a in sys.argv[1:] if not a.startswith("--")]
    exe, dump_path, out = pos[:3]
    opt = {a.split("=", 1)[0][2:]: a.split("=", 1)[1] for a in sys.argv if a.startswith("--") and "=" in a}
    refs, look, theme = opt.get("refs"), opt.get("look", "photo"), opt.get("theme", "night")
    only = opt.get("only")
    old_dump = json.load(open(opt["old-dump"])) if "old-dump" in opt else None
    os.makedirs(out, exist_ok=True)
    dump = json.load(open(dump_path))
    here = os.path.dirname(os.path.abspath(dump_path))
    lines = []
    for who in ("woman", "man"):
        h = lh.Hand(who)
        for inst in ("violin", "viola"):
            if only and only != f"{who}-{inst}":
                continue
            F = next(F for F in dump["figs"] if F["who"] == who and F["inst"] == inst)
            F_old = next((G for G in old_dump["figs"] if G["who"] == who and G["inst"] == inst),
                         None) if old_dump else None
            nk = lh.Neck(F)
            cache = json.load(open(os.path.join(here, f"left_hand_cache-{who}-{inst}.json")))
            full = full_shots(exe, out, who, inst, CASES[inst][0], look, theme)
            if full:
                sheet = Image.new("RGB", (sum(t.width for t in full), H), (16, 16, 20))
                x = 0
                for t in full:
                    sheet.paste(t, (x, 0))
                    x += t.width
                sheet.save(os.path.join(out, f"full-{who}-{inst}-{look}-{theme}.png"))
            for case in CASES[inst]:
                name, _, _, (s, f, at) = case
                W, Rh, Q, _, way = hs.pose_of(cache, who, inst, s, f, at)
                P = h.fk(W, Rh, Q)
                c = (P[1, 0] + P[4, 0] + W) / 3.0
                cams = hs.cameras(F, nk, c)
                views = []
                if F_old is not None:
                    views.append(("earlier camera", np.array(F_old["eye"]), cams[0][2], cams[0][3], cams[0][4]))
                views.append(("new camera",) + tuple(cams[0][1:]))
                views.append(("photo 5's angle",) + tuple(next(x for x in cams if x[0] == "photo5")[1:]))
                # The hand and the sleeve together from the new camera (the owner's review:
                # the sleeve got lost against the dress).
                el = hs.elbow_of(F, W, P, way)
                views.append(("new camera, hand and sleeve", cams[0][1], (c * 0.6 + el * 0.4),
                              cams[0][3], cams[0][4]))
                tiles = []
                for cname, eye, tgt, up, fov in views:
                    eye2 = tgt + hr.norm(eye - tgt) * 120.0
                    half = 17.0 if "sleeve" in cname else 10.0
                    tag = f"{who}-{inst}-{name}-{cname.replace(' ', '_').replace(',', '')}-{look}-{theme}"
                    shot = shoot(exe, out, who, inst, case, (eye2, tgt, half), look, theme, tag)
                    if shot is None:
                        continue
                    im = picture_box(shot)
                    im = im.resize((int(im.width * H / im.height), H))
                    m = hs.picture(h, nk, F, W, P, (cname, eye, tgt, up, fov), way)
                    tiles.append(label(im, f"Cantus (app renderer), {name} position, from the {cname}: "
                                           f"back w/l {m['back_w_over_l']:.2f}, wrist {m['wrist_bend']:.0f}"))
                    lines.append(f"{who} {inst} {name} {cname}: {m}")
                if refs:
                    for r in REFS[name]:
                        path = os.path.join(OWNER, r[6:]) if r.startswith("owner:") else os.path.join(refs, r)
                        if os.path.exists(path):
                            im = Image.open(path).convert("RGB")
                            im = im.resize((int(im.width * H / im.height), H))
                            tiles.append(label(im, f"reference {os.path.basename(path)}"))
                if not tiles:
                    continue
                sheet = Image.new("RGB", (sum(t.width for t in tiles), H), (16, 16, 20))
                x = 0
                for t in tiles:
                    sheet.paste(t, (x, 0))
                    x += t.width
                suffix = "-vs-refs" if refs else ""
                sheet.save(os.path.join(out, f"hand-{who}-{inst}-{name}-{look}-{theme}{suffix}.png"))
    open(os.path.join(out, "numbers.txt"), "w").write(NL.join(lines) + NL)
    print(NL.join(lines))


if __name__ == "__main__":
    main()
