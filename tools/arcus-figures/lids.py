"""r42:arcus-hand — the realistic figures' closed eyelids, a morph over the built ``.fig``
(the owner, 2026-11-25: he liked the closed eyes — "parecían concentrados o disfrutando de
la música" — and wants the choice: *Eyes* Closed (the default) or Open, Settings ▸
Appearance, ADR 0159).

The ``.fig`` keeps the eyes open (``build_figures.py``: lids 5°/2° over eyes looking down at
the strings). Closing them is a morph made here from the figure itself (no MakeHuman source
needed): about each eye's centre (its mark ``eye.L`` / ``eye.R``), the upper lid's skin is
turned down over the eyeball — by the opening's own angle at the eye's middle (found as the
largest gap in the skin's elevation in front of the eye) plus a little overlap, so its edge
meets the lower lid — fully at its margin, less toward the fold and none on the brow, the
corners and the cheek (smooth falloffs in elevation, across and away from the eye); the lower
lid rises a few degrees; the lashes (the lash line and the lash cards) ride with their lid.
Turning about the eye's centre keeps every lid vertex at its distance from it, so the lid
slides over the ball and never cuts into it. A relaxed, slightly downcast closed eye: the
lids meet a third of the way down the eye, as a player's lowered lids do.

``crates/cantus-ui/assets/figures/<who>_lids.bin`` (little-endian): ``CLID``, ``u16``
version 1, ``u32`` count, then per moved vertex its ``u32`` index and its offset at rest
(three ``i16`` at 0.001 cm). ``scene/bowed_human.rs`` adds the offsets before skinning when
the eyes are closed.

    python tools/arcus-figures/lids.py [--dir=<figures>] [--preview <dir>]
"""
import os
import struct
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import hand_rig as hr  # noqa: E402

FIGS = os.path.join(HERE, "..", "..", "crates", "cantus-ui", "assets", "figures")
# The eyeball's radius (cm; MakeHuman's, as build_figures.py places it).
R = 1.18
# How far past the lower lid's edge the upper lid closes (deg), how far the lower lid rises.
OVERLAP = 3.0
LOWER = 3.0
# Where the upper lid's turn has faded out toward the fold and the brow (deg of elevation).
FOLD = 95.0
SKIN, LIPS, LASH, CARD_LASH = 0, 1, 10, 15


def smooth(e0, e1, x):
    t = np.clip((x - e0) / (e1 - e0), 0, 1)
    return t * t * (3 - 2 * t)


def morph(f, verbose=True):
    """Each vertex's offset at rest closing both eyes (n x 3; zero where none)."""
    pos = f["pos"]
    out = np.zeros_like(pos)
    tri, mat = f["tris"], f["mat"]
    skin = np.zeros(len(pos), bool)
    skin[np.unique(tri[np.isin(mat, [SKIN, LIPS])])] = True
    lash = np.zeros(len(pos), bool)
    lash[np.unique(tri[np.isin(mat, [LASH, CARD_LASH])])] = True
    for s in ("L", "R"):
        c = f["marks"][f"eye.{s}"]
        d = pos - c
        rho = np.hypot(d[:, 1], d[:, 2])
        phi = np.degrees(np.arctan2(d[:, 1], d[:, 2]))
        # The opening at the eye's middle: the largest gap in the skin's elevation.
        m = skin & (np.abs(d[:, 0]) < 0.35) & (d[:, 2] > 0) & (rho < 1.8)
        ph = np.sort(phi[m])
        ph = ph[(ph > -35) & (ph < 22)]
        g = np.diff(ph)
        i = int(np.argmax(g))
        lo, hi = ph[i], ph[i + 1]
        turn = (hi - lo) + OVERLAP
        mid = (lo + hi) * 0.5
        across = 1 - smooth(0.35, 1.05, np.abs(d[:, 0]) / (1.55 * R))
        radial = 1 - smooth(1.15 * R, 1.95 * R, rho)
        near = (d[:, 2] > -0.2) & (np.abs(d[:, 0]) < 1.9 * R)
        wu = (phi >= mid) * (1 - smooth(hi, FOLD, phi)) * across * radial * near * skin
        wl = (phi < mid) * smooth(lo - 35, lo, phi) * across * radial * near * skin
        # The lashes ride with their lid whole.
        lash_near = lash & (np.abs(d[:, 0]) < 2.2 * R) & (d[:, 2] > -0.5)
        wu = np.where(lash_near, (phi >= mid - 4) * across, wu)
        wl = np.where(lash_near, (phi < mid - 4) * across, wl)
        for w, ang in ((wu, -turn), (wl, LOWER)):
            a = -np.radians(ang) * w
            y = d[:, 1] * np.cos(a) - d[:, 2] * np.sin(a)
            z = d[:, 1] * np.sin(a) + d[:, 2] * np.cos(a)
            moved = w > 0
            out[:, 1] += np.where(moved, y - d[:, 1], 0)
            out[:, 2] += np.where(moved, z - d[:, 2], 0)
        if verbose:
            print(f"  eye.{s}: open from {lo:.1f} to {hi:.1f} deg, the upper lid turned "
                  f"{turn:.1f} deg, the lower {LOWER:.1f}")
    return out


def write(path, off):
    idx = np.nonzero(np.abs(off).max(1) > 5e-4)[0]
    b = bytearray(b"CLID")
    b += struct.pack("<HI", 1, len(idx))
    for i in idx:
        q = np.clip(np.round(off[i] * 1000), -32767, 32767).astype(int)
        b += struct.pack("<I3h", int(i), *q)
    open(path, "wb").write(bytes(b))
    print("wrote", path, len(idx), "vertices,", len(b), "bytes")


def preview(f, off, out):
    import preview as pv
    from PIL import Image
    col = {SKIN: (0.86, 0.66, 0.55), LIPS: (0.8, 0.5, 0.5), 8: (0.92, 0.9, 0.88),
           9: (0.35, 0.45, 0.55), LASH: (0.1, 0.08, 0.07), 14: (0.4, 0.3, 0.2)}
    ec = (f["marks"]["eye.L"] + f["marks"]["eye.R"]) / 2
    ims = []
    for d in (np.array([0, -0.25, 1.0]), np.array([0.6, 0.1, 1.0])):
        d = d / np.linalg.norm(d)
        for p in (f["pos"], f["pos"] + off):
            keep = np.isin(f["mat"], list(col))
            t = f["tris"][keep]
            cols = np.zeros((len(p), 3)) + 0.5
            for tt, m in zip(t, f["mat"][keep]):
                cols[tt] = col[int(m)]
            t = t[(np.linalg.norm(p - ec, axis=1) < 9)[t].all(1)]
            flat = t.ravel()
            o = out + f"-{len(ims)}.png"
            pv.render(p[flat], np.arange(len(flat)).reshape(-1, 3), cols[flat],
                      cam=(ec + d * 30, ec), size=(420, 300), fov=0.3,
                      nrm=pv.normals(p, t)[flat], out=o, key=(0.2, 0.6, 0.8), bg=(230, 230, 230))
            ims.append(Image.open(o))
    sh = Image.new("RGB", (840, 600))
    for i, im in enumerate(ims):
        sh.paste(im, ((i % 2) * 420, (i // 2) * 300))
    sh.save(out + ".png")


def main():
    pdir = sys.argv[sys.argv.index("--preview") + 1] if "--preview" in sys.argv else None
    # (r42:arcus-polish: `--dir=<folder>` — the figures a build wrote elsewhere, the lids
    # beside them; the assets' folder by default. Run after every build of the figures: the
    # morph names vertices by their index.)
    figs = next((a.split("=", 1)[1] for a in sys.argv if a.startswith("--dir=")), FIGS)
    for who in ("woman", "man"):
        print(who)
        f = hr.read_fig(os.path.join(figs, f"{who}.fig"))
        off = morph(f)
        write(os.path.join(figs, f"{who}_lids.bin"), off)
        if pdir:
            os.makedirs(pdir, exist_ok=True)
            preview(f, off, os.path.join(pdir, f"lids-{who}"))


if __name__ == "__main__":
    main()
