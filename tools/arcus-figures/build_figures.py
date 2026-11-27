"""r42:arcus-upper — the realistic musicians that may hold an Arcus instrument (the owner,
2026-11-24): a woman in a black velvet long-sleeved concert dress and a man in a dark suit,
built offline from MakeHuman 1.x's **CC0** assets into the compact ``.fig`` files the scene
engine bakes (``crates/cantus-ui/assets/figures/``; ``scene/bowed_human.rs`` reads them).

The faces are original: MakeHuman's generic adult averages (a mix of its three ethnic
macro-targets), nothing fitted to any photograph (see docs/round42/arcus-upper-next.md).

Sources (verified 2026-11-24; every asset CC0 1.0 per its pack's manifest
``packs/makehuman_system_assets.json`` and MakeHuman's LICENSE.md §C):
- github.com/makehumancommunity/makehuman @ a8bc2d54ff0ac92e78ff71431b1023eda42bf482,
  ``makehuman/data``: ``3dobjs/base.obj``, ``targets/macrodetails/**``,
  ``rigs/default.mhskel``, ``rigs/default_weights.mhw``.
- makehuman_system_assets_cc0.zip (files.makehumancommunity.org/asset_packs/
  makehuman_system_assets/, sha256 b542127a…0107): ``clothes/male_elegantsuit01``,
  ``skins/young_caucasian_{female,male}`` (the skin's albedo detail: brows, lips).

Usage:
    python tools/arcus-figures/build_figures.py <makehuman/data> <system assets dir> \
        <out dir> [--preview <dir>] [--only=woman|man]

What it does: shapes the base mesh (female / male, young adult, average muscle and weight,
ideal proportions), closes the eyelids (the eyes lowered, as a player's), keeps the body's
skin, fits the man's suit (its jacket, shirt, tie and shoes told apart by its texture),
dresses the woman's body in velvet to the wrists with a soft scoop neckline, grows hair as a
shell over the scalp (her hair drawn back into a low bun; his short and groomed), samples the
skin's albedo detail per vertex, maps MakeHuman's 163 bones onto the 53 the scene poses, and
writes vertices (int16 at 0.01 cm), four bone weights, the albedo detail, triangles with
their material and the part (body, left arm, right arm) each belongs to.
"""

import os
import struct
import sys

import numpy as np
from PIL import Image

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import hair_maps  # noqa: E402
import mh  # noqa: E402
import thin  # noqa: E402

# The live arms' shortest edge after thinning (cm).
THIN = 0.62

# The scene's bones: (name, MakeHuman bones merged into it, parent).
SIDES = ("L", "R")


def bone_table():
    t = [
        ("hips", ["root", "pelvis.L", "pelvis.R", "spine05"], None),
        ("spine", ["spine04", "spine03"], "hips"),
        ("chest", ["spine02", "spine01", "breast.L", "breast.R"], "spine"),
        ("neck", ["neck01", "neck02", "neck03"], "chest"),
        ("head", ["head"], "neck"),
    ]
    for s in SIDES:
        t += [
            (f"clav.{s}", [f"clavicle.{s}", f"shoulder01.{s}"], "chest"),
            (f"upper1.{s}", [f"upperarm01.{s}"], f"clav.{s}"),
            (f"upper2.{s}", [f"upperarm02.{s}"], f"upper1.{s}"),
            (f"fore1.{s}", [f"lowerarm01.{s}"], f"upper2.{s}"),
            (f"fore2.{s}", [f"lowerarm02.{s}"], f"fore1.{s}"),
            (f"hand.{s}", [f"wrist.{s}"] + [f"metacarpal{k}.{s}" for k in range(1, 5)],
             f"fore2.{s}"),
        ]
        for f in range(1, 6):
            for k in range(1, 4):
                par = f"hand.{s}" if k == 1 else f"f{f}_{k - 1}.{s}"
                t.append((f"f{f}_{k}.{s}", [f"finger{f}-{k}.{s}"], par))
        t += [
            (f"thigh.{s}", [f"upperleg01.{s}", f"upperleg02.{s}"], "hips"),
            (f"shin.{s}", [f"lowerleg01.{s}", f"lowerleg02.{s}"], f"thigh.{s}"),
            (f"foot.{s}", [f"foot.{s}"], f"shin.{s}"),
        ]
    return t


BONES = bone_table()
BI = {b[0]: i for i, b in enumerate(BONES)}
ARM_PREFIX = ("upper1", "upper2", "fore1", "fore2", "hand", "f")

# Materials (the scene's `Role`s, by number; `scene/bowed_human.rs` names them).
SKIN, LIPS, HAIR, VELVET, WOOL, SHIRT, TIE, SHOES, EYE, IRIS, LASH, NAIL, JEWEL = range(13)
# MakeHuman's CC0 hair, brow and lash proxies: alpha-tested cards in the Photographic bake
# (their textures beside the .fig files), left out of the Drawn picture.
CARD_HAIR, CARD_BROW, CARD_LASH = 13, 14, 15
# The hair as a thick shell (the Drawn picture's) and a thin scalp under the cards (the
# Photographic picture's).
SHELL, SCALP = 16, 17


def mh_to_scene(sk):
    """Every MakeHuman bone → its scene bone (an unlisted one: its nearest listed
    ancestor's, so the face's bones are the head's and the toes the foot's)."""
    direct = {}
    for name, members, _ in BONES:
        for m in members:
            direct[m] = name
    out = {}
    for b in sk["bones"]:
        x = b
        while x not in direct:
            x = sk["bones"][x]["parent"]
        out[b] = direct[x]
    return out


def shaped(base, root, sex, mix):
    t = [(f"macrodetails/universal-{sex}-young-averagemuscle-averageweight.target", 1.0)]
    for race, w in mix.items():
        t.append((f"macrodetails/{race}-{sex}-young.target", w))
    t.append((f"macrodetails/proportions/{sex}-young-averagemuscle-averageweight-"
              "idealproportions.target", 0.6))
    t += [(k + ".target", w) for k, w in FACE[sex]]
    return mh.apply_targets(base, root, t) * 10.0  # decimetres → cm


def both(name, w):
    """A face modifier on both sides (MakeHuman's ``l-``/``r-`` pair)."""
    d, f = name.split("/")
    return [(f"{d}/l-{f}", w), (f"{d}/r-{f}", w)]


# r42:arcus-upper — the faces' own features (sheet v7; the owner, 2026-11-24: she must be
# clearly another person than the photographed woman — a real, identifiable person in a CC
# BY-SA photograph — "change the features enough that she is another person"). Chosen by eye
# on the generic average, nothing fitted to or morphed toward any photograph, and in every
# feature away from the reference's: hers a long, narrow oval (not a round, broad face), a
# lower brow line, almond eyes set closer and smaller (not large, wide-set), a straight,
# slim nose with a longer bridge and a level tip (not small and upturned), a smaller mouth
# with natural lips (not full), a defined, longer chin and slim cheeks under soft
# cheekbones, a long neck; the type kept — young, fair, light brows, serene. His a squarer
# jaw, cheekbones, a straight brow, a firm chin.
FACE = {
    "female": [
        ("head/head-oval", 0.55),
        ("head/head-fat-decr", 0.3),
        ("head/head-scale-horiz-decr", 0.12),
        ("head/head-scale-vert-incr", 0.05),
        ("forehead/forehead-scale-vert-decr", 0.3),
        *both("cheek/cheek-bones-incr", 0.3),
        *both("cheek/cheek-volume-decr", 0.15),
        *both("cheek/cheek-inner-decr", 0.2),
        *both("eyes/eye-trans-in", 0.22),
        *both("eyes/eye-scale-decr", 0.18),
        *both("eyes/eye-corner1-up", 0.25),
        *both("eyes/eye-height2-decr", 0.15),
        ("chin/chin-width-decr", 0.3),
        ("chin/chin-height-incr", 0.15),
        ("chin/chin-prominent-incr", 0.25),
        ("chin/chin-bones-incr", 0.2),
        ("nose/nose-scale-vert-incr", 0.12),
        ("nose/nose-scale-horiz-decr", 0.3),
        ("nose/nose-point-width-decr", 0.35),
        ("nose/nose-volume-decr", 0.2),
        ("nose/nose-nostrils-width-decr", 0.25),
        ("nose/nose-point-down", 0.12),
        ("nose/nose-greek-incr", 0.15),
        ("nose/nose-hump-decr", 0.2),
        ("mouth/mouth-scale-horiz-decr", 0.22),
        ("mouth/mouth-upperlip-volume-decr", 0.1),
        ("mouth/mouth-lowerlip-volume-incr", 0.1),
        ("mouth/mouth-cupidsbow-incr", 0.3),
        ("mouth/mouth-angles-up", 0.12),
        ("eyebrows/eyebrows-angle-up", 0.1),
        ("neck/neck-scale-horiz-decr", 0.2),
        ("neck/neck-scale-vert-incr", 0.3),
        # r42:arcus-polish — the ears tilted back as real ears are (~15° from the face's
        # vertical; MakeHuman's stood upright, 2-6°).
        *both("ears/ear-rot-backward", 0.6),
    ],
    # r42:arcus-polish3 — his more masculine (the owner on the concert light's first sheet:
    # "el hombre puede mejorar mucho sus rasgos, hazlos más masculinos"): an angular head, a
    # wider, stronger jaw and a squarer chin with a slight cleft, a heavier brow ridge with
    # the brows lower and straighter, defined cheekbones over hollower cheeks, a slightly
    # wider nose bridge, a thicker neck; a man about thirty, not a boy. Still MakeHuman's
    # generic average shaped by eye, fitted to nobody.
    "male": [
        ("head/head-square", 0.55),
        ("head/head-rectangular", 0.2),
        ("head/head-fat-decr", 0.3),
        ("head/head-age-incr", 0.3),
        *both("cheek/cheek-bones-incr", 0.6),
        *both("cheek/cheek-volume-decr", 0.6),
        *both("cheek/cheek-inner-decr", 0.3),
        ("chin/chin-bones-incr", 0.8),
        ("chin/chin-width-incr", 0.45),
        ("chin/chin-prominent-incr", 0.35),
        ("chin/chin-height-incr", 0.1),
        ("chin/chin-cleft-incr", 0.45),
        ("forehead/forehead-trans-forward", 0.25),
        ("forehead/forehead-temple-decr", 0.2),
        ("nose/nose-hump-decr", 0.1),
        ("nose/nose-scale-horiz-incr", 0.12),
        ("nose/nose-point-width-decr", 0.1),
        ("mouth/mouth-lowerlip-volume-incr", 0.1),
        ("mouth/mouth-upperlip-volume-decr", 0.2),
        ("mouth/mouth-scale-horiz-incr", 0.08),
        ("eyebrows/eyebrows-trans-forward", 0.45),
        ("eyebrows/eyebrows-trans-down", 0.3),
        ("eyebrows/eyebrows-angle-down", 0.15),
        ("neck/neck-scale-horiz-incr", 0.45),
        ("neck/neck-scale-depth-incr", 0.3),
        *both("ears/ear-rot-backward", 0.6),  # r42:arcus-polish
    ],
}


def rot_x(p, c, a):
    q = p - c
    ca, sa = np.cos(a), np.sin(a)
    return c + np.stack([q[:, 0], q[:, 1] * ca - q[:, 2] * sa, q[:, 1] * sa + q[:, 2] * ca], 1)


# How much of a finger's girth is kept (the owner's review, 2026-11-24: the hands read as
# clay — thick, short fingers): drawn in toward each phalanx's own axis, the thumb less.
FINGER_SLIM = 0.86
THUMB_SLIM = 0.92
# r42:arcus-polish — per finger (MakeHuman's numbering: 3 the middle, 4 the ring, 5 the
# little), on top of FINGER_SLIM.
FINGER_EXTRA = {3: 0.9, 4: 0.95, 5: 0.84}


def slim_fingers(pos, wt, J):
    """Each finger's skin drawn in toward its phalanges' axes (by its weight to them), so
    the fingers read slender and long; the bones stay where they are."""
    out = pos.copy()
    for s in SIDES:
        for f in range(1, 6):
            # (r42:arcus-polish: the middle and the little finger a little more — measured
            # from the app at their middle joints her 1.9 and 1.8 cm, an adult woman's 1.7
            # and 1.4; tests/arcus_hand_shape.rs.)
            k_slim = THUMB_SLIM if f == 1 else FINGER_SLIM * FINGER_EXTRA.get(f, 1.0)
            for k in range(1, 4):
                bone = f"finger{f}-{k}.{s}"
                if bone not in wt:
                    continue
                a, b = J[f"{bone}____head"], J[f"{bone}____tail"]
                ab = b - a
                ids = np.array([i for i, _ in wt[bone]])
                w = np.array([x for _, x in wt[bone]])
                q = pos[ids]
                tt = np.clip(((q - a) @ ab) / max(ab @ ab, 1e-9), -0.3, 1.3)
                axis = a + tt[:, None] * ab
                # The fingertip kept round (the owner's review: drawn in to the axis's end
                # the tips read as claws): no slimming over the last phalanx's last half.
                keep = 1.0 - smooth01((tt - 0.5) / 0.5) if k == 3 else np.ones(len(tt))
                out[ids] += (axis - q) * ((1.0 - k_slim) * w * keep)[:, None]
    return out


# r42:arcus-polish — her arms' girth along each arm (the owner, 2026-11-25: "thick, long,
# uniform tubes"; a slim adult woman violinist's): the skin drawn in toward the arm's axis
# by a factor that falls from the deltoid to a narrower elbow and rises again to the
# forearm's fuller top before slimming to the wrist. (u along the upper arm, the factor),
# then (v along the forearm, the factor); the hand is never touched (1 at the wrist joint).
ARM_GIRTH = {
    "female": ([(0.0, 1.0), (0.25, 1.0), (0.5, 0.99), (0.8, 0.94), (1.0, 0.92)],
               [(0.0, 0.93), (0.12, 0.99), (0.3, 1.0), (0.65, 0.98), (0.88, 1.0), (1.0, 1.0)]),
    # His a man's (a muscular forearm, a defined elbow) — drawn in less.
    "male": ([(0.0, 1.0), (0.3, 0.99), (0.6, 0.97), (0.85, 0.92), (1.0, 0.9)],
             [(0.0, 0.91), (0.15, 0.98), (0.35, 0.98), (0.7, 0.95), (0.9, 0.98), (1.0, 1.0)]),
}
# Her shoulders' soft slope: the top of each shoulder lowered toward the acromion (cm, at
# its most), so the line from the neck falls as a trapezius does to a rounded deltoid.
SHOULDER_SLOPE = {"female": 1.5, "male": 0.4}
# And the deltoid's cap drawn in toward the joint (cm, at its most): a slim woman's shoulder
# is rounded, not padded.
SHOULDER_IN = {"female": 0.5, "male": 0.0}


# r42:arcus-polish — the hand's bulk (the owner, 2026-11-25, sheet v10: "la mano aún sale
# algo tosca o gorda"; the coordinator: the thumb's base and the first web space one big
# convex lobe from under the neck to the wrist, "like a mitten's thumb pocket"). Measured on
# her hand at rest (tools/arcus-figures/measure_hand.py): the palm 3.2-3.4 cm thick at the
# middle (an adult woman's 2.5-3.0), and with the thumb's metacarpal and the thenar 4.5-5.6
# cm. The skin round the thumb's metacarpal is drawn in toward its axis (THENAR), and the
# palm's toward its own middle plane (PALM_THICK); the bones stay.
THENAR = 0.7
PALM_THICK = 0.9
# The palm's own side (its pad and heel) drawn in toward the metacarpals.
PALM_VOLAR = 0.45


def slim_hand(pos, wt, J):
    """r42:arcus-polish — the thenar and the first web space slimmed round the thumb's
    metacarpal, and the palm thinned toward its middle plane (``THENAR``, ``PALM_THICK``)."""
    out = pos.copy()
    W = [dict() for _ in range(len(pos))]
    for b, pairs in wt.items():
        for vid, w in pairs:
            if vid < len(pos):
                W[vid][b] = W[vid].get(b, 0.0) + w
    for s in SIDES:
        tb = f"finger1-1.{s}"
        if tb not in wt:
            continue
        a, b = J[f"{tb}____head"], J[f"{tb}____tail"]
        ab = b - a
        share = np.array([d.get(tb, 0.0) / max(sum(d.values()), 1e-9) for d in W])
        ids = np.where(share > 0.02)[0]
        q = out[ids]
        t = np.clip(((q - a) @ ab) / (ab @ ab), 0.0, 1.0)
        axis = a + t[:, None] * ab
        out[ids] = q + (axis - q) * ((1.0 - THENAR) * share[ids])[:, None]
        # The palm: the metacarpals' plane, its normal from the knuckles' line and the wrist.
        wr = J[f"wrist.{s}____head"]
        k2, k5 = J[f"finger2-1.{s}____head"], J[f"finger5-1.{s}____head"]
        along = (k2 + k5) * 0.5 - wr
        along /= np.linalg.norm(along)
        across = k2 - k5
        across -= along * (across @ along)
        across /= np.linalg.norm(across)
        n = np.cross(along, across)
        mids = [J[f"metacarpal{m}.{s}____head"] * 0.5 + J[f"metacarpal{m}.{s}____tail"] * 0.5
                for m in range(1, 5) if f"metacarpal{m}.{s}____head" in J]
        c0 = np.mean(mids, 0) if mids else wr
        names = [f"wrist.{s}"] + [f"metacarpal{m}.{s}" for m in range(1, 5)]
        pshare = np.array([sum(d.get(x, 0.0) for x in names) / max(sum(d.values()), 1e-9)
                           for d in W])
        ids = np.where(pshare > 0.02)[0]
        q = out[ids]
        # Which side is the palm's: the thumb's skin lies on it.
        tshare = np.array([d.get(tb, 0.0) for d in W])
        tv = tshare > 0.5
        if tv.any() and ((out[tv] - c0) @ n).mean() < 0.0:
            n = -n
        off = (q - c0) @ n
        # From just past the wrist's crease (the heel of the hand is the palm's too) to the knuckles;
        # the palm's side drawn in more than the back (seen from below, as the picture's
        # camera sees a violinist's left hand, the palm's pad and heel were the lobe).
        L = max(np.linalg.norm((k2 + k5) * 0.5 - wr), 1e-6)
        # (Not at the wrist's crease itself: thinned there the wrist read pinched under the
        # palm — arcus_hand_table's palm-to-wrist rule.)
        tt = smooth01((((q - wr) @ along) / L - 0.08) / 0.25)
        k = np.where(off > 0.0, 1.0 - PALM_VOLAR, 1.0 - PALM_THICK)
        out[ids] = q - n[None, :] * (off * k * pshare[ids] * tt)[:, None]
    return out


def _profile(t, table):
    xs = np.array([a for a, _ in table])
    ys = np.array([b for _, b in table])
    return np.interp(np.clip(t, 0.0, 1.0), xs, ys)


def shape_arms(pos, wt, J, sex):
    """r42:arcus-polish — each arm's girth tapered along it (``ARM_GIRTH``) and her
    shoulders' top let down to a soft slope (``SHOULDER_SLOPE``); the joints stay where they
    are (``J`` is not recomputed), so the bones and every pose solved on them are kept."""
    out = pos.copy()
    up_t, lo_t = ARM_GIRTH[sex]
    W = [dict() for _ in range(len(pos))]
    for b, pairs in wt.items():
        for vid, w in pairs:
            if vid < len(pos):
                W[vid][b] = W[vid].get(b, 0.0) + w
    for s in SIDES:
        sh, el, wr = (J[f"upperarm01.{s}____head"], J[f"lowerarm01.{s}____head"],
                      J[f"wrist.{s}____head"])
        names = (f"upperarm01.{s}", f"upperarm02.{s}", f"lowerarm01.{s}", f"lowerarm02.{s}")
        share = np.array([sum(d.get(n, 0.0) for n in names) / max(sum(d.values()), 1e-9)
                          for d in W])
        ids = np.where(share > 0.02)[0]
        q = pos[ids]
        res = []
        for a, b, table in ((sh, el, up_t), (el, wr, lo_t)):
            ab = b - a
            t = ((q - a) @ ab) / (ab @ ab)
            tc = np.clip(t, 0.0, 1.0)
            axis = a + tc[:, None] * ab
            res.append((np.linalg.norm(q - axis, axis=1), axis, _profile(t, table)))
        near_up = res[0][0] <= res[1][0]
        axis = np.where(near_up[:, None], res[0][1], res[1][1])
        f = np.where(near_up, res[0][2], res[1][2])
        k = (1.0 - f) * share[ids]
        out[ids] = q + (axis - q) * k[:, None]
        # The shoulder's top: lowered most over the joint, fading to nothing at the neck's
        # side and down the arm (the deltoid stays round).
        drop = SHOULDER_SLOPE[sex]
        if drop > 0.0:
            neck = J["neck01____head"]
            sv = np.where(np.sign(out[:, 0]) == np.sign(sh[0]))[0]
            p = out[sv]
            inner = abs(neck[0]) + 5.5
            across = smooth01((np.abs(p[:, 0]) - inner) / max(abs(sh[0]) - inner, 1e-3))
            fade_out = 1.0 - smooth01((np.abs(p[:, 0]) - abs(sh[0]) - 3.0) / 4.0)
            above = smooth01((p[:, 1] - (sh[1] - 1.0)) / 3.0)
            front_back = np.exp(-((p[:, 2] - sh[2]) / 7.0) ** 2)
            out[sv, 1] -= drop * across * fade_out * above * front_back
        inn = SHOULDER_IN[sex]
        if inn > 0.0:
            # Round the joint, on its outer side: toward the joint's own centre.
            q = out - sh
            r = np.linalg.norm(q, axis=1)
            outer = smooth01((np.sign(sh[0]) * q[:, 0] + 1.0) / 3.0)
            k = inn * (1.0 - smooth01((r - 4.0) / 7.0)) * outer
            k = np.where(r > 1e-6, k / np.maximum(r, 1e-6), 0.0)
            out = out - q * k[:, None]
    return out


# r42:arcus-polish — the neck's and the collarbones' forms (cm of relief): the
# sternocleidomastoid's ridge, the hollow beside it, the collarbone's ridge and the hollow
# under it, the suprasternal notch's dip. Subtle: drawn, they read as a sketch's lines.
# (r42:arcus-polish4: her collarbones and the notch clearer — the coordinator beside ref-04:
# the neckline read as one smooth plane — and the sternum's line down from the notch.)
RELIEF = {"female": (0.38, 0.22, 0.4, 0.3, 0.6), "male": (0.45, 0.25, 0.3, 0.2, 0.5)}
# r42:arcus-polish4 — the sternum's shallow groove between the chest's two sides, from the
# notch down (cm deep, cm wide, cm long), hers.
STERNUM = (0.22, 1.3, 7.0)


def _seg(p, a, b):
    """Each point's distance to the segment a-b and where along it (0…1)."""
    ab = b - a
    t = np.clip(((p - a) @ ab) / (ab @ ab), 0.0, 1.0)
    return np.linalg.norm(p - (a + t[:, None] * ab), axis=1), t


def _landmarks(J):
    el, er = J["eye.L____head"], J["eye.R____head"]
    out = {}
    for s, e, sg in (("L", el, 1.0), ("R", er, -1.0)):
        ear = e + np.array([4.6 * sg, -1.5, -5.8])
        out[f"mastoid.{s}"] = ear + np.array([-0.6 * sg, -2.8, -1.0])
        out[f"clav_in.{s}"] = J[f"clavicle.{s}____head"]
        out[f"clav_out.{s}"] = J[f"clavicle.{s}____tail"]
        out[f"ear.{s}"] = ear
    out["notch"] = ((J["clavicle.L____head"] + J["clavicle.R____head"]) * 0.5
                    + np.array([0.0, 1.0, 0.6]))
    out["eyes"] = (el + er) * 0.5
    return out


def beard_field(pos, J):
    """His beard's place (0…1): the jaw, the chin, under the jaw to the neck's top and over
    the upper lip; its top from the sideburn, level with the ear's middle, down across the
    cheek to the mouth's corner (never over the cheekbone); the lips left bare."""
    eyes = _landmarks(J)["eyes"]
    q = pos - eyes
    ax = np.abs(q[:, 0])
    top = -7.6 + 4.2 * smooth01((ax - 3.0) / 3.2)
    # (r42:arcus-polish3: fading over 2.5-3 cm toward the cheekbones and the neck — the
    # coordinator: a hard border read as sandpaper — and densest on the chin and over the
    # upper lip.)
    lower = smooth01((top - q[:, 1]) / 2.8) * smooth01((q[:, 1] + 14.5) / 3.0)
    face = smooth01((q[:, 2] + 6.5) / 2.5) * smooth01((7.6 - ax) / 1.5)
    lip = np.exp(-(((q[:, 1] + 6.0) / 0.7) ** 2 + (q[:, 0] / 2.0) ** 2)) * (q[:, 2] > 2.0)
    mouth = np.exp(-(((q[:, 1] + 6.8) / 0.5) ** 2 + (q[:, 0] / 2.3) ** 2)) * (q[:, 2] > 3.0)
    chin = 0.55 + 0.45 * np.exp(-((q[:, 0] / 3.5) ** 2))
    return np.clip((lower * face * chin + lip) * (1.0 - mouth), 0.0, 1.0)


# r42:arcus-polish3 — his stubble in the skin's texture: the beard's place drawn into the
# texture's own coordinates, each name's (set by ``make``).
BEARD_UV = {}


def uv_field(val, tris, tuv, uvs, side):
    """A per-vertex value painted into texture space (``side`` px), triangle by triangle."""
    out = np.zeros((side, side))
    for t, tu in zip(tris, tuv):
        v = val[t]
        if v.max() < 0.02:
            continue
        uv = uvs[tu] * side
        uv[:, 1] = side - uv[:, 1]
        x0, y0 = np.floor(uv.min(0)).astype(int)
        x1, y1 = np.ceil(uv.max(0)).astype(int)
        x0, y0 = max(x0, 0), max(y0, 0)
        x1, y1 = min(x1, side - 1), min(y1, side - 1)
        if x1 < x0 or y1 < y0:
            continue
        xs, ys = np.meshgrid(np.arange(x0, x1 + 1) + 0.5, np.arange(y0, y1 + 1) + 0.5)
        a, b, c = uv
        den = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1])
        if abs(den) < 1e-9:
            continue
        l0 = ((b[1] - c[1]) * (xs - c[0]) + (c[0] - b[0]) * (ys - c[1])) / den
        l1 = ((c[1] - a[1]) * (xs - c[0]) + (a[0] - c[0]) * (ys - c[1])) / den
        l2 = 1.0 - l0 - l1
        inside = (l0 >= -0.01) & (l1 >= -0.01) & (l2 >= -0.01)
        f = l0 * v[0] + l1 * v[1] + l2 * v[2]
        sub = out[y0:y1 + 1, x0:x1 + 1]
        sub[inside] = np.maximum(sub[inside], f[inside])
    return out


def relief(pos, tris, J, sex):
    """r42:arcus-polish — the neck's and the neckline's forms on the skin (``RELIEF``):
    each vertex of the neck and the upper chest moved along its normal; the joints stay."""
    scm, scm_hollow, clav, clav_hollow, notch = RELIEF[sex]
    L = _landmarks(J)
    n = vnormals(pos, tris)
    neck_y = J["neck01____head"][1]
    head_y = L["eyes"][1]
    region = (pos[:, 1] > neck_y - 8.0) & (pos[:, 1] < head_y - 5.0) & (np.abs(pos[:, 0]) < 16.0)
    d = np.zeros(len(pos))
    for s in ("L", "R"):
        # The muscle from behind the ear to the breastbone's top, a little in from the
        # collarbone's head.
        ci = L[f"clav_in.{s}"]
        a, b = L[f"mastoid.{s}"], ci + np.array([-0.6 * np.sign(ci[0]), 0.6, 0.8])
        r, t = _seg(pos, a, b)
        along = smooth01(t / 0.2) * (1.0 - 0.4 * smooth01((t - 0.7) / 0.3))
        d += along * (scm * np.exp(-(r / 0.9) ** 2) - scm_hollow * np.exp(-((r - 2.0) / 0.7) ** 2))
        # The collarbone: a ridge, the hollow under it.
        a, b = ci + np.array([0.0, 0.3, 0.6]), L[f"clav_out.{s}"] + np.array([0.0, 0.2, 0.3])
        r, t = _seg(pos, a, b)
        below = pos[:, 1] < (a[1] + (b[1] - a[1]) * t) - 0.3
        d += clav * np.exp(-(r / 0.6) ** 2) - clav_hollow * np.exp(-((r - 1.4) / 0.6) ** 2) * below
    r = np.linalg.norm(pos - L["notch"], axis=1)
    d -= notch * np.exp(-(r / 1.0) ** 2)
    if sex == "female":
        # r42:arcus-polish4 — the sternum's groove down from the notch, on the chest's front.
        depth, width, length = STERNUM
        top = L["notch"] + np.array([0.0, -1.2, 0.3])
        r, t = _seg(pos, top, top + np.array([0.0, -length, 0.6]))
        fade = smooth01(t / 0.15) * (1.0 - smooth01((t - 0.6) / 0.4))
        d -= depth * fade * np.exp(-(r / (0.5 * width)) ** 2) * (n[:, 2] > 0.5)
    if sex == "male":
        # r42:arcus-polish3 — his Adam's apple: a small rise on the neck's front, a third
        # of the way from the chin's underside down to the notch.
        cy = L["eyes"][1] - 14.8
        front = n[:, 2] > 0.45
        d += 0.42 * np.exp(-((pos[:, 0] / 1.0) ** 2 + ((pos[:, 1] - cy) / 1.3) ** 2)) * front
    d = np.where(region, d, 0.0)
    return pos + n * d[:, None]


def ear_mask(data, n):
    """r42:arcus-polish — each base vertex's share of an ear (0…1): the vertices
    MakeHuman's own ear targets move, by how far."""
    m = np.zeros(n)
    for s in ("l", "r"):
        i, d = mh.read_target(os.path.join(data, "targets", "ears", f"{s}-ear-flap-incr.target"))
        k = np.linalg.norm(d, axis=1)
        # (The target moves the side of the head a little too: only the ear itself, the
        # vertices it moves most.)
        lo_, hi_ = np.percentile(k, 55), np.percentile(k, 85)
        k = smooth01((k - lo_) / max(hi_ - lo_, 1e-9))
        ok = i < n
        m[i[ok]] = np.maximum(m[i[ok]], k[ok])
    return m


def tones(pos, J, sex, ear):
    """r42:arcus-polish — the skin's local tones (a luminance factor and a red shift a
    vertex; a negative red is cooler): the neck and the neckline a little cooler and paler
    than the face, the notch and under the jaw shaded, the ears warmer, the jaw cooler, his
    beard's blue-grey shadow on the jaw, the chin and over the lip."""
    L = _landmarks(J)
    eyes = L["eyes"]
    dl = np.ones(len(pos))
    dr = np.zeros(len(pos))
    neck_top = eyes[1] - 9.5
    neck = smooth01((neck_top - pos[:, 1]) / 3.0) * (pos[:, 1] > J["neck01____head"][1] - 14.0)
    dl *= 1.0 + 0.015 * neck
    # (r42:arcus-polish4: hers as warm as her face — ref-04's neck and chest are warm peach,
    # not the cooler, paler skin the first pass gave them; the coordinator: pale-pink rubber.)
    dr -= (0.07 if sex == "male" else -0.02) * neck
    # Under the jaw: the skin that faces down from the jaw's line to the neck.
    jaw_y = eyes[1] - 8.6
    under = (np.exp(-((pos[:, 1] - jaw_y) / 1.2) ** 2) * smooth01((eyes[2] - 1.0 - pos[:, 2]) / 3.0)
             * smooth01((6.5 - np.abs(pos[:, 0])) / 2.0))
    dl *= 1.0 - 0.22 * under
    dr -= 0.05 * under
    r = np.linalg.norm(pos - L["notch"], axis=1)
    dl *= 1.0 - 0.12 * np.exp(-(r / 1.1) ** 2)
    # The jaw a little cooler than the cheeks.
    jaw = (np.exp(-((pos[:, 1] - (eyes[1] - 7.2)) / 1.8) ** 2)
           * smooth01((np.abs(pos[:, 0]) - 2.5) / 2.0) * (np.abs(pos[:, 0]) < 7.5))
    dr -= 0.07 * jaw
    # The ears warmer and redder (light through their thin cartilage). (r42:arcus-polish4:
    # hers redder still, as ref-04's under the stage light.)
    dr += (0.48 if sex == "female" else 0.32) * ear
    dl *= 1.0 - 0.05 * ear
    if sex == "female":
        # r42:arcus-polish4 — the skin over the breastbone and under the collarbones a
        # little flushed (blood near the surface, the chest's thin skin), the collarbones'
        # ridges catching less of it; a soft unevenness of value over the neck and chest
        # (real skin is never one even tone).
        r = np.linalg.norm((pos - (L["notch"] + np.array([0.0, -4.0, 0.5]))) * [0.8, 1.0, 1.0],
                           axis=1)
        dr += 0.09 * (1.0 - smooth01(r / 6.0))
        q = pos * 0.7
        mott = np.sin(q[:, 0] * 1.1 + 0.3) * np.sin(q[:, 1] * 0.9 + 1.2) * np.sin(q[:, 2] * 1.3)
        low = pos[:, 1] < eyes[1] - 8.0
        dl *= 1.0 + 0.035 * mott * low
        dr += 0.03 * mott * low
    if sex == "male":
        # His beard's shadow: the jaw, the chin and over the upper lip, blue-grey
        # (r42:arcus-polish3: a little deeper; the stubble's own hairs are in his skin's
        # texture, `stubble`).
        beard = beard_field(pos, J)
        dl *= 1.0 - 0.15 * beard
        dr -= 0.14 * beard
    return dl, dr


def cavities(pos, tris, J, rays=32, reach=1.6):
    """r42:arcus-polish — the face's and the ears' small cavities darkened (an occlusion
    factor a vertex, 0.55…1): from each vertex of the head near an ear, the nose, the mouth
    or an eye, rays over its hemisphere against the triangles within ``reach`` cm; the
    share that hits, nearer walls more."""
    L = _landmarks(J)
    eyes = L["eyes"]
    near = np.zeros(len(pos), bool)
    for c, r in ((L["ear.L"], 4.2), (L["ear.R"], 4.2), (eyes + np.array([0, -3.8, 2.4]), 2.6),
                 (eyes + np.array([0, -6.6, 1.6]), 3.0), (J["eye.L____head"], 1.9),
                 (J["eye.R____head"], 1.9)):
        near |= np.linalg.norm(pos - c, axis=1) < r
    n = vnormals(pos, tris)
    out = np.ones(len(pos))
    ids = np.where(near)[0]
    if len(ids) == 0:
        return out
    rng = np.random.default_rng(7)
    dirs = rng.normal(size=(rays, 3))
    dirs /= np.linalg.norm(dirs, axis=1, keepdims=True)
    tc = pos[tris].mean(1)
    for i in ids:
        p, nn = pos[i], n[i]
        sel = np.linalg.norm(tc - p, axis=1) < reach + 0.6
        tt = tris[sel & ~(tris == i).any(1)]
        if len(tt) == 0:
            continue
        a, b, c = pos[tt[:, 0]], pos[tt[:, 1]], pos[tt[:, 2]]
        e1, e2 = b - a, c - a
        d = np.where((dirs @ nn)[:, None] < 0.0, -dirs, dirs)
        o = p + nn * 0.02
        hits = 0.0
        for dd in d:
            h = np.cross(dd, e2)
            det = (e1 * h).sum(1)
            ok = np.abs(det) > 1e-9
            f = np.where(ok, 1.0 / np.where(ok, det, 1.0), 0.0)
            s_ = o - a
            u = f * (s_ * h).sum(1)
            q = np.cross(s_, e1)
            v = f * (q @ dd)
            t = f * (q * e2).sum(1)
            hit = ok & (u >= 0) & (v >= 0) & (u + v <= 1) & (t > 0.01) & (t < reach)
            if hit.any():
                hits += (1.0 - t[hit].min() / reach) * (0.5 + 0.5 * (dd @ nn))
        out[i] = 1.0 - min(0.45, 0.85 * hits / rays)
    return out


def finger_tint(pos, wt, J):
    """Per vertex (0…1): how near a finger's joint it lies (its crease, its knuckle) and
    how much of a fingertip or the palm's little-finger edge it is."""
    crease = np.zeros(len(pos))
    tips = np.zeros(len(pos))
    for s in SIDES:
        for f in range(1, 6):
            for k in range(1, 4):
                bone = f"finger{f}-{k}.{s}"
                if bone not in wt:
                    continue
                a, b = J[f"{bone}____head"], J[f"{bone}____tail"]
                ab = (b - a) / max(np.linalg.norm(b - a), 1e-9)
                ids = np.array([i for i, _ in wt[bone]])
                w = np.array([x for _, x in wt[bone]])
                along = (pos[ids] - a) @ ab
                near = np.exp(-(along / 0.35) ** 2) * w
                crease[ids] = np.maximum(crease[ids], near)
                if k == 3:
                    L = np.linalg.norm(b - a)
                    tips[ids] = np.maximum(tips[ids], smooth01((along / max(L, 1e-6) - 0.3) / 0.7) * w)
        bone = f"metacarpal4.{s}"
        if bone in wt:
            ids = np.array([i for i, _ in wt[bone]])
            w = np.array([x for _, x in wt[bone]])
            tips[ids] = np.maximum(tips[ids], 0.5 * w)
    return crease, tips


def close_lids(pos, wt, J):
    """The eyelids lowered over the eyes (a player's eyes on the instrument): the upper
    lid's vertices turned down about the eye's centre, the lower lid's a little up."""
    out = pos.copy()
    for s in SIDES:
        c = J[f"eye.{s}____head"]
        for bone, ang in ((f"orbicularis03.{s}", np.radians(UPPER_LID)),
                          (f"orbicularis04.{s}", -np.radians(LOWER_LID))):
            ids = np.array([i for i, _ in wt[bone]])
            w = np.array([x for _, x in wt[bone]])
            for k in range(len(ids)):
                out[ids[k]] = rot_x(out[ids[k]][None], c, ang * w[k])[0]
    return out


# Half-open, looking down at the strings (the owner's review, 2026-11-24: closed read as
# asleep or a mannequin).
UPPER_LID = 5.0
LOWER_LID = 2.0
# How far the eyes look down (degrees). (r42:arcus-hair: the owner's review of sheet v8 —
# the eyes should read open, on the strings: the lids raised from 14 and 3 degrees and the
# gaze from 32, so the iris and its pupil show under the lid from the picture's camera.)
GAZE_DOWN = 28.0
# And toward the player's left, where the strings are (under a violin's chin, beside a
# cello's neck): the eyes on the instrument, not on the viewer.
GAZE_LEFT = 12.0


def scene_weights(n, wt, m2s):
    """Per vertex {scene bone index: weight} from MakeHuman's weights."""
    acc = [dict() for _ in range(n)]
    for bone, pairs in wt.items():
        sb = BI[m2s[bone]]
        for vid, w in pairs:
            if vid < n:
                acc[vid][sb] = acc[vid].get(sb, 0.0) + w
    return acc


def top4(d):
    items = sorted(d.items(), key=lambda kv: -kv[1])[:4]
    tot = sum(w for _, w in items)
    if tot <= 0:
        return [(BI["hips"], 255)] + [(0, 0)] * 3
    q = [(b, int(round(255 * w / tot))) for b, w in items]
    # The quantised weights sum to exactly 255 (the error on the largest).
    q[0] = (q[0][0], q[0][1] + 255 - sum(w for _, w in q))
    while len(q) < 4:
        q.append((0, 0))
    return q


def arm_share(d, side):
    tot = sum(d.values()) or 1.0
    s = 0.0
    for b, w in d.items():
        name = BONES[b][0]
        if name.endswith("." + side) and name.split(".")[0].startswith(ARM_PREFIX):
            s += w
    return s / tot


def hand_share(d):
    tot = sum(d.values()) or 1.0
    s = 0.0
    for b, w in d.items():
        base = BONES[b][0].split(".")[0]
        if base == "hand" or (base[0] == "f" and base[1].isdigit()):
            s += w
    return s / tot


def vnormals(pos, tris):
    n = np.zeros_like(pos)
    a, b, c = pos[tris[:, 0]], pos[tris[:, 1]], pos[tris[:, 2]]
    fn = np.cross(b - a, c - a)
    for k in range(3):
        np.add.at(n, tris[:, k], fn)
    return n / np.maximum(np.linalg.norm(n, axis=1, keepdims=True), 1e-12)


def triangulate(faces):
    tris, tuv = [], []
    for vi, ti, _, _ in faces:
        for k in range(1, len(vi) - 1):
            tris.append([vi[0], vi[k], vi[k + 1]])
            tuv.append([ti[0], ti[k], ti[k + 1]])
    return np.array(tris, dtype=np.int64), np.array(tuv, dtype=np.int64)


def sample(img, uvs, r=1):
    """Texture colours (0..1) at ``uvs`` (OBJ convention: v up), averaged over a box of
    ``2r+1`` texels (a vertex's footprint is far larger than a texel)."""
    a = np.asarray(img, dtype=np.float64)[..., :3] / 255.0
    h, w = a.shape[:2]
    x = np.clip((uvs[:, 0] % 1.0) * (w - 1), r, w - 1 - r).astype(int)
    y = np.clip((1.0 - uvs[:, 1] % 1.0) * (h - 1), r, h - 1 - r).astype(int)
    acc = np.zeros((len(uvs), 3))
    for dy in range(-r, r + 1):
        for dx in range(-r, r + 1):
            acc += a[y + dy, x + dx]
    return acc / float((2 * r + 1) ** 2)


def skin_detail(pos, faces, uvs, tex, keep):
    """Per base vertex the skin's albedo detail against its median (luminance factor and a
    red shift), from the skin texture through the base mesh's UVs: the brows, the lips, the
    cheeks' warmth, the nails — the tone itself is the user's choice."""
    vs, ts = [], []
    for vi, ti, g, _ in faces:
        if g != "body":
            continue
        for v_, t_ in zip(vi, ti):
            if t_ >= 0:
                vs.append(v_)
                ts.append(t_)
    vs, ts = np.array(vs), np.array(ts)
    col = sample(tex, uvs[ts])
    acc = np.zeros((len(pos), 3))
    cnt = np.zeros(len(pos))
    np.add.at(acc, vs, col)
    np.add.at(cnt, vs, 1)
    c = acc / np.maximum(cnt, 1)[:, None]
    lum = c @ np.array([0.2126, 0.7152, 0.0722])
    med = np.median(lum[keep])
    lf = np.clip(lum / med, 0.35, 1.35)
    red = (c[:, 0] - c[:, 1]) / np.maximum(c[:, 0], 1e-3)
    rf = np.clip((red - np.median(red[keep])) * 2.5, -0.5, 0.6)
    return lf, rf


def smooth01(x):
    t = np.clip(x, 0.0, 1.0)
    return t * t * (3.0 - 2.0 * t)


def head_frame(J):
    eyes = (J["eye.L____head"] + J["eye.R____head"]) * 0.5
    c = eyes - np.array([0.0, 0.0, 6.5])
    return c, eyes


def angles(p, c):
    q = p - c
    th = np.degrees(np.arctan2(q[:, 0], q[:, 2]))
    ph = np.degrees(np.arctan2(q[:, 1], np.hypot(q[:, 0], q[:, 2])))
    return th, ph


def hairline(th, table):
    a = np.abs(th)
    xs, ys = zip(*table)
    return np.interp(a, xs, ys)


# The hairline's elevation (degrees over the eyes' level, seen from the head's centre)
# round the head (degrees from the front), and the hair's thickness (cm) there.
# (Hers rounded over the brow — a straight line across read as a cap, the owner's v7.)
HER_LINE = [(0, 34), (12, 33.5), (25, 31), (38, 26), (55, 19), (70, 12), (86, 9), (102, 5),
            (120, -14), (145, -36), (165, -44), (180, -46)]
HIS_LINE = [(0, 30), (16, 31), (30, 33), (42, 28), (55, 20), (68, 12), (80, 2), (90, 0),
            (100, 8), (115, -2), (135, -20), (160, -27), (180, -28)]


# r42:arcus-hair — where her scalp under the cards begins at the front (degrees from the
# hairline; it was set back 5° — the owner's review of sheet v8: a hard edge at the brow).
SCALP_IN = -2.5

# r42:arcus-polish4 — where his longer top begins (the coordinator's review of pass 3: his
# hair a helmet, one smooth shell with a hard hairline; ref-06): the front's hairline as
# ``HIS_LINE``, the line rising over the temples and the ears, so the sides and the back
# below it are cut short (the scalp's short, darker hair, its edge on the skin), the top
# above it longer, with volume, combed straight back.
HIS_TOP_LINE = [(0, 30), (16, 31), (30, 33), (42, 31), (55, 28), (68, 25), (82, 23),
                (100, 22), (120, 18), (140, 13), (160, 9), (180, 8)]
# How much his top stands off the scalp (cm): most at the front of the crown, combed back.
HIS_VOLUME = 1.7
HIS_SIDE = 0.12


def hair_shell(pos, W, tris, c, line, thick, head_w, band=16.0, rise=22.0, base=None,
               blend=None, wave=None):
    """A shell over the scalp: the head's skin above ``line`` (cut along it, so the
    hairline is a smooth curve), pushed out along its normal by ``thick(θ, φ)`` tapering
    to nothing at the hairline, so its edge lies on the skin. Returns (positions, weights,
    triangles, strand coordinates) in its own numbering."""
    near = tris[(head_w[tris] > 0.2).all(1)]
    th, ph = angles(pos, c)
    f = ph - hairline(th, line)
    if wave is not None:
        # r42:arcus-polish4 — a broken edge (tufts at the temples), cut along it.
        f = f + wave(th)
    p2, w2, _, t2, f2 = cut(pos, W, [], near, f)
    keep = t2[(f2[t2].mean(1) > 0)]
    used = np.unique(keep.ravel())
    remap = -np.ones(len(p2), dtype=np.int64)
    remap[used] = np.arange(len(used))
    n = vnormals(p2, t2)
    th2, ph2 = angles(p2, c)
    # (1.4 mm proud a little in from the hairline: thinner, the scalp showed through the
    # hair in triangles at 200 %, the two surfaces fighting for the same depth; at the
    # hairline itself on the skin, where the hair's colour is the skin's.)
    t = thick(th2, ph2) * smooth01(f2 / rise) + (
        0.03 + 0.11 * smooth01(f2 / 8.0) if base is None else base)
    hp = (p2 + n * t[:, None])[used]
    # Per vertex (sheet v7): the strands' across coordinate — the angle round the head's
    # front-to-back axis, so the strands run as hair drawn back does, from the hairline over
    # the head to the nape (the picture's grooves and sheen follow it) — and how far in
    # from the hairline (0 at it, 1 beyond 2.5 cm: the hair's edge blends into the skin, as
    # real hair thins there).
    q = p2[used] - c
    # (Folded under the head's middle, so no seam runs down the nape.)
    across = np.arctan2(q[:, 0], np.abs(q[:, 1]) + 0.5) * 9.0
    edge = smooth01(f2[used] / band) if blend is None else blend(th2[used], ph2[used], f2[used])
    uv = np.stack([across, edge], 1)
    return hp, [w2[i] for i in used], remap[keep], uv


def ellipsoid(c, axes, radii, rows=14, cols=24, twist=0.0):
    """An ellipsoid (a bun) round ``c``: its points, triangles and strand coordinates
    (strands winding round it, ``twist`` turns from pole to pole)."""
    pts, uv = [], []
    for r in range(rows + 1):
        ph = -np.pi / 2 + np.pi * r / rows
        for k in range(cols):
            a = 2 * np.pi * k / cols
            q = (axes[0] * radii[0] * np.cos(ph) * np.cos(a) + axes[1] * radii[1] * np.sin(ph)
                 + axes[2] * radii[2] * np.cos(ph) * np.sin(a))
            pts.append(c + q)
            uv.append([a * 3.0 + twist * ph * 4.0, 1.0])
    tri = []
    for r in range(rows):
        for k in range(cols):
            a0, a1 = r * cols + k, r * cols + (k + 1) % cols
            b0, b1 = a0 + cols, a1 + cols
            tri += [[a0, b0, b1], [a0, b1, a1]]
    return np.array(pts), np.array(tri), np.array(uv)


class Fig:
    """A figure as it is built: vertices, their weights ({bone: w}), albedo detail,
    triangles with a material each."""

    def __init__(self):
        self.pos = np.zeros((0, 3))
        self.w = []
        self.lum = np.zeros(0)
        self.red = np.zeros(0)
        self.tris = np.zeros((0, 3), dtype=np.int64)
        self.mat = np.zeros(0, dtype=np.int64)
        self.uv = np.zeros((0, 2))

    def add(self, pos, w, tris, mat, lum=None, red=None, uv=None):
        o = len(self.pos)
        self.pos = np.vstack([self.pos, pos])
        self.w += list(w)
        self.lum = np.concatenate([self.lum, np.ones(len(pos)) if lum is None else lum])
        self.red = np.concatenate([self.red, np.zeros(len(pos)) if red is None else red])
        self.uv = np.vstack([self.uv, np.zeros((len(pos), 2)) if uv is None else uv])
        self.tris = np.vstack([self.tris, tris + o])
        m = np.full(len(tris), mat) if np.isscalar(mat) else mat
        self.mat = np.concatenate([self.mat, m])


def split_seams(pos, W, head_w, tris, tuv, uvs):
    """Each vertex copied for every further texture coordinate its corners have (a seam
    of the skin's texture), the copies appended: (positions, weights, head share,
    triangles, each vertex's source, each vertex's texture coordinates)."""
    n = len(pos)
    first = -np.ones(n, dtype=np.int64)
    extra = {}
    src = list(range(n))
    out = tris.copy()
    for k in range(len(tris)):
        for j in range(3):
            v, t_ = int(tris[k, j]), int(max(tuv[k, j], 0))
            if first[v] < 0:
                first[v] = t_
            elif first[v] != t_:
                key = (v, t_)
                if key not in extra:
                    extra[key] = len(src)
                    src.append(v)
                out[k, j] = extra[key]
    src = np.array(src)
    vt_ = np.maximum(first, 0)
    vuv = np.zeros((len(src), 2))
    vuv[:n] = uvs[vt_] if len(uvs) else 0.0
    for (v, t_), i in extra.items():
        vuv[i] = uvs[t_]
    return pos[src], [W[i] for i in src], head_w[src], out, src, vuv


# r42:arcus-hair — the eyes' irises: MakeHuman's CC0 eye textures (the system assets'
# ``eyes/materials/<colour>_eye.png``; CC0 per its .mhmat header and the pack's manifest):
# hers blue-green, his light brown. In the texture the eye is seen from the front, its
# pupil at ``EYE_PUPIL`` px (of 1024) and its iris ``EYE_IRIS`` px across the radius; the
# crop keeps ``EYE_CROP`` px round the pupil, written at ``EYE_SIDE`` px.
EYE_TEX = {"woman": "bluegreen_eye.png", "man": "brownlight_eye.png"}
EYE_PUPIL = (723, 303)
EYE_IRIS = 120.0
EYE_CROP = 168
EYE_SIDE = 128
# The iris's half-angle on our eyeball (its rings to 26°, the white from 30°).
IRIS_DEG = 28.0


def eye_texture(assets, out, name):
    """The iris and the white round it for ``name``'s eyes, sRGB, ``<name>_eye.png``."""
    im = Image.open(os.path.join(assets, "eyes", "materials", EYE_TEX[name])).convert("RGB")
    x, y = EYE_PUPIL
    im = im.crop((x - EYE_CROP, y - EYE_CROP, x + EYE_CROP, y + EYE_CROP))
    im.resize((EYE_SIDE, EYE_SIDE), Image.LANCZOS).save(
        os.path.join(out, f"{name}_eye.png"), optimize=True)


def eye_uv(pts, c, gaze, up, side, sex):
    """Each eyeball point's place in its eye texture (0…1, v up; his 2…3 in u, as the skin's):
    the eye seen down its gaze, the iris's edge at ``IRIS_DEG``."""
    q = (pts - c) / np.linalg.norm(pts - c, axis=1)[:, None]
    k = EYE_IRIS / np.sin(np.radians(IRIS_DEG)) / (2.0 * EYE_CROP)
    u = 0.5 + q @ side * k
    v = 0.5 + q @ up * k
    return np.stack([u + (0.0 if sex == "female" else 2.0), v], 1)


def skin_texture(assets, out, name, sex):
    """The skin's fine detail for the bake (pores, the brows' hairs, the lips' lines): the
    MakeHuman skin's luminance over a blur of itself (128 the mean), 1024 px."""
    im = Image.open(os.path.join(
        assets, "skins", f"young_caucasian_{sex}",
        "young_lightskinned_female_diffuse.png" if sex == "female"
        else "young_lightskinned_male_diffuse.png")).convert("RGB").resize((1024, 1024),
                                                                           Image.LANCZOS)
    from PIL import ImageFilter
    lum = np.asarray(im.convert("L"), dtype=np.float64)
    blur = np.asarray(im.convert("L").filter(ImageFilter.GaussianBlur(6)), dtype=np.float64)
    hp = np.clip(lum / np.maximum(blur, 1.0), 0.6, 1.4)
    beard = BEARD_UV.get(name)
    if beard is not None:
        # r42:arcus-polish3 — his stubble: short dark hairs a texel or two across, dense
        # where the beard grows, thinning out at its edge (no flat paint).
        # (At twice the texture's side: hairs a texel across read as sand at 200 %.)
        big = (hp.shape[0] * 2, hp.shape[1] * 2)
        hp = np.asarray(Image.fromarray(np.clip(hp * 128.0, 0, 255).astype(np.uint8)).resize(
            big[::-1], Image.BILINEAR), dtype=np.float64) / 128.0
        rng = np.random.default_rng(73)
        b = np.asarray(Image.fromarray((np.clip(beard, 0, 1) * 255).astype(np.uint8)).resize(
            big[::-1], Image.BILINEAR).filter(ImageFilter.GaussianBlur(6)),
            dtype=np.float64) / 255.0
        # Each texel a hair with the beard's density as its chance: sparse at the edges,
        # dense on the chin; the shadow between them a smooth darkening of its own.
        hairs = (rng.random(big) < (0.25 + 0.6 * b)).astype(np.float64)
        hp = hp * (1.0 - b * (0.22 + 0.35 * hairs))
    Image.fromarray(np.clip(hp * 128.0, 0, 255).astype(np.uint8), "L").save(
        os.path.join(out, f"{name}_skin.png"), optimize=True)


def body_parts(pos, faces, tris_all, tri_groups, deleted):
    """The body's triangles (its skin, not the helpers) and which vertices remain."""
    keep_t = np.array([g == "body" for g in tri_groups])
    t = tris_all[keep_t]
    if deleted is not None:
        dm = np.zeros(len(pos), bool)
        dm[list(deleted)] = True
        t = t[~dm[t].any(1)]
    keep_v = np.zeros(len(pos), bool)
    keep_v[t.ravel()] = True
    return t, keep_v


def outward(pts, tris, c):
    """Triangles wound so their normals point away from ``c``."""
    a, b, d = pts[tris[:, 0]], pts[tris[:, 1]], pts[tris[:, 2]]
    fn = np.cross(b - a, d - a)
    flip = ((a + b + d) / 3.0 - c).__mul__(fn).sum(1) < 0
    out = tris.copy()
    out[flip] = out[flip][:, [0, 2, 1]]
    return out


def shoes(assets, pos, W):
    """MakeHuman's shoes04 (CC0): black leather dress shoes, fitted, the feet under them
    taken away."""
    obj, refs, scales, deleted, _ = mh.read_mhclo(os.path.join(
        assets, "clothes", "shoes04", "shoes04.mhclo"))
    sp, _, sfaces = mh.read_obj(os.path.join(assets, "clothes", "shoes04", obj))
    tris, _ = triangulate(sfaces)
    return (mh.fit_proxy(refs, scales, pos / 10.0) * 10.0, mh.proxy_weights(refs, W), tris,
            deleted)


def name_of(sex):
    return "woman" if sex == "female" else "man"


def make(sex, data, assets):
    base, uvs, faces = BASE
    sk, wt = mh.read_skeleton(data)
    m2s = mh_to_scene(sk)
    mix = ({"caucasian": 0.5, "asian": 0.3, "african": 0.2} if sex == "female"
           else {"caucasian": 0.45, "african": 0.3, "asian": 0.25})
    pos = shaped(base, data, sex, mix)
    J = mh.joint_positions(sk, pos)
    pos = close_lids(pos, wt, J)
    pos = slim_fingers(pos, wt, J)
    pos = shape_arms(pos, wt, J, sex)  # r42:arcus-polish
    pos = slim_hand(pos, wt, J)  # r42:arcus-polish
    W = scene_weights(len(pos), wt, m2s)
    tris_all, tuv_all = triangulate(faces)
    pos = relief(pos, tris_all, J, sex)  # r42:arcus-polish
    ear_v = ear_mask(data, len(pos))  # r42:arcus-polish
    tri_groups = [f[2] for f in faces for _ in range(len(f[0]) - 2)]
    fig = Fig()
    c, _ = head_frame(J)
    head_w = np.array([d.get(BI["head"], 0.0) / max(sum(d.values()), 1e-9) for d in W])
    skin_tex = Image.open(os.path.join(
        assets, "skins", f"young_caucasian_{sex}",
        "young_lightskinned_female_diffuse.png" if sex == "female"
        else "young_lightskinned_male_diffuse.png"))
    shp, shw, sht, shdel = shoes(assets, pos, W)
    deleted = set(shdel)
    if sex == "male":
        obj, refs, scales, sdel, _ = mh.read_mhclo(os.path.join(
            assets, "clothes", "male_elegantsuit01", "male_elegantsuit01.mhclo"))
        deleted |= set(sdel)
    # The skin's own texture per pixel (the owner's review, 2026-11-24: porcelain): each
    # vertex split where its corners' texture coordinates differ, the copies after the
    # originals (so the proxies' references still name the base mesh's vertices).
    if sex == "male":
        # r42:arcus-polish3 — his beard's place in the skin texture's coordinates.
        BEARD_UV[name_of(sex)] = uv_field(beard_field(pos, J), tris_all, tuv_all, uvs, 1024)
    n0 = len(pos)
    pos, W, head_w, tris_all, src, vuv = split_seams(pos, W, head_w, tris_all, tuv_all, uvs)
    deleted = set(np.where(np.isin(src, list(deleted)))[0]) if deleted else deleted
    btris, keep_v = body_parts(pos, faces, tris_all, tri_groups, deleted)
    lum, red = skin_detail(pos[:n0], faces, uvs, skin_tex, (keep_v & (head_w > 0.5))[:n0])
    lum, red = lum[src], red[src]
    lum = lash_line(pos, btris, lum, J, wt)
    brow = brows(pos, keep_v, J, sex)
    # (The owner's review, 2026-11-24: brows that read — hers a light brown, darker than
    # her blonde; his darker.)
    lum = lum * (1.0 - (0.84 if sex == "female" else 0.9) * brow)
    red = red - 0.15 * brow
    # Cheeks, lips, the nose's tip and the ears warmer (a face with life, not porcelain),
    # the lips a natural rose, and the skin not one even tone: a little redness here and
    # there, as real skin has.
    red = red + 0.28 * warmth(pos, J, sex)
    lip = lip_spot(pos, J)
    # (r42:arcus-hair: a touch more natural rose — the coordinator's review: still pale.)
    red = red + 0.45 * lip
    lum = lum * (1.0 - 0.14 * lip)
    q = pos * 0.45
    blotch = (np.sin(q[:, 0] * 1.3 + 0.7) * np.sin(q[:, 1] * 1.1 + 1.9) * np.sin(q[:, 2] * 1.7))
    red = red + 0.05 * blotch
    # The fingers' joints a little darker in their creases, the knuckles shaded, the
    # fingertips and the palm's edge warmer (the owner's review: hands of clay).
    crease, tips = finger_tint(pos[:n0], wt, J)
    crease, tips = crease[src], tips[src]
    lum = lum * (1.0 - 0.16 * crease)
    # (r42:arcus-polish4: the knuckles' and the joints' skin redder, 0.06 — the
    # coordinator: the redness under the skin at the knuckles; colour only.)
    red = red + 0.12 * crease + 0.16 * tips
    if sex == "male":
        # His skin less pale: a little more colour all over.
        red = red + 0.08
        lum = lum * 0.95
    # r42:arcus-polish — the skin's local tones (the neck cooler and paler than the face, the
    # notch, under the jaw, the ears warmer, his beard's shadow) and the cavities' occlusion
    # (the ears' conchae, the nostrils, the mouth's corners, the eyes' inner corners).
    dl, dr = tones(pos[:n0], J, sex, ear_v)
    occ = cavities(pos[:n0], tris_all[(tris_all < n0).all(1)], J)
    lum = lum * (dl * occ)[src]
    red = red + dr[src]
    lash_p, lash_t = lashes(pos, wt, J)
    # The nails: the body cut along each nail's edge (sheet v7).
    nf = nail_field(pos, W, sk, J)
    pos, W, (lum, red, su, sv), btris, nf = cut(pos, W, [lum, red, vuv[:, 0], vuv[:, 1]],
                                                btris, nf)
    # Her or his texture told apart by u (his 2 … 3): the bake picks the texture by it.
    vuv = np.stack([su + (0.0 if sex == "female" else 2.0), sv], 1)
    head_w = np.array([d.get(BI["head"], 0.0) / max(sum(d.values()), 1e-9) for d in W])
    if sex == "female":
        # Swept back with soft volume at the crown (the owner's reference's styling).
        # Strand groups (the owner's review: a shell read as a striped helmet): the hair's
        # depth rises and falls across the head in soft locks drawn back, uneven.
        def thick(th, ph):
            a = np.radians(th)
            locks = 0.5 + 0.5 * np.sin(a * 11.0 + 0.8 * np.sin(a * 27.0 + 1.3))
            return (0.8 + 0.4 * smooth01((ph - 28) / 30.0)
                    + 0.45 * smooth01((np.abs(th) - 60) / 90.0)
                    + 0.75 * smooth01((ph - 42) / 22.0) * (1.0 - smooth01((np.abs(th) - 140) / 35.0))
                    + 0.28 * locks * smooth01((ph - 24) / 18.0))
        # (Rising slowly from the hairline: swept back, lying flat at the brow — a quick
        # rise read as a cap's edge across the forehead.)
        hp, hw, ht, huv = hair_shell(pos, W, btris, c, HER_LINE, thick, head_w, band=16.0,
                                     rise=40.0)
        bp, bw, bt, bmat, blum, bred, buv = dress(pos, W, btris, J, lum, red, nf, vuv)
        # r42:arcus-polish4 — the sleeves finer (a wrinkle a couple of centimetres long
        # needs more than the base mesh's centimetre), then wrinkled.
        bp, bw, bt, bmat, (blum, bred, buv) = subdivide(
            bp, bw, bt, bmat, [blum, bred, buv], arm_cloth(bw, bt, bmat, VELVET))
        bp = sleeve_wrinkles(bp, bw, bt, bmat, J, VELVET)
        bp, bw, bt, bmat, (blum, bred, buv) = hem(bp, bw, bt, bmat, [blum, bred, buv], VELVET,
                                                  wrists(bw) > 0.5)
        buv = cloth_folds(bp, bw, bt, bmat, buv, J)  # r42:arcus-polish
        fig.add(bp, bw, bt, bmat, blum, bred, uv=buv)
        fig.add(hp, hw, ht, SHELL, uv=huv)
        # (The scalp a few degrees in from the hairline: the cards' own strands make the
        # hairline, soft, not the scalp's edge — the owner's review: a hard band.)
        # (r42:arcus-hair: now reaching a little past the hairline, its colour the skin's
        # at its edge and the hair's a few degrees in — ``uv[1]`` the degrees in from the
        # hairline, the bake's blend — so the gaps between the feathered strands at the
        # front show hair over skin, not a band of scalp.)
        scalp_line = [(a_, e_ + SCALP_IN * (a_ < 100)) for a_, e_ in HER_LINE]
        sp_, sw_, st_, suv_ = hair_shell(pos, W, btris, c, scalp_line, scalp_thick, head_w,
                                         band=16.0, rise=40.0, base=SCALP_BASE)
        th_, ph_ = angles(sp_, c)
        suv_[:, 1] = ph_ - hairline(th_, HER_LINE)
        fig.add(sp_, sw_, st_, SCALP, uv=suv_)
        # Tied low: a chignon at the nape — the hair twisted into a ring, its strands wound
        # round it — over a soft core that fills its middle.
        bc = c + np.array([0.0, -6.6, -9.6])
        ep, et, euv = ellipsoid(bc, np.eye(3), (3.0, 2.3, 2.2), twist=1.5)
        et = outward(ep, et, bc)
        fig.add(ep, [{BI["head"]: 1.0}] * len(ep), et, HAIR, uv=euv)
        tp, tt, tuv = torus(bc + np.array([0.0, 0.0, -0.6]), np.array([0.0, -0.25, -1.0]),
                            2.6, 1.35, rows=16, cols=72, rope=0.12, turns=3, dense=9.0)
        fig.add(tp, [{BI["head"]: 1.0}] * len(tp), tt, HAIR, uv=tuv)
        # Small drop earrings: a bead at each lobe, a drop under it.
        for jp, jt in earrings(pos, head_w, c, J):
            fig.add(jp, [{BI["head"]: 1.0}] * len(jp), jt, JEWEL)
    else:
        def thick(th, ph):
            # (r42:arcus-polish4: the sides short — a short even layer up to where the top
            # begins (``HIS_TOP_LINE``) — and the top long, standing off the scalp most at
            # the front of the crown and less toward the back, as hair combed straight back
            # does; the coordinator's review: a helmet.)
            top = smooth01((ph - hairline(th, HIS_TOP_LINE)) / 9.0)
            front = 1.0 - 0.5 * smooth01((np.abs(th) - 25.0) / 120.0)
            rise = 0.55 + 0.45 * smooth01((ph - 30.0) / 25.0)
            return HIS_SIDE + HIS_VOLUME * top * front * rise

        def blend(th, ph, f):
            # Its colour along the strands: the short sides deeper (more of the roots and
            # the skin under them, a short cut's look), the top the hair's own.
            ft = ph - hairline(th, HIS_TOP_LINE)
            return 0.05 * smooth01(f / 5.0) + 0.95 * smooth01((ft + 2.0) / 10.0)
        # His short hair's edge crisper (a groomed hairline; a wide band read as teeth).
        def tufts(th):
            # His temples' and sideburns' edge broken into short tufts (the coordinator: the
            # Drawn shell's hard edge), the forehead's line kept.
            a = np.radians(th)
            at = smooth01((np.abs(th) - 30.0) / 20.0) * (1.0 - smooth01((np.abs(th) - 150.0) / 20.0))
            return at * (1.6 * np.sin(a * 23.0) + 1.0 * np.sin(a * 41.0 + 1.3))
        hp, hw, ht, huv = hair_shell(pos, W, btris, c, HIS_LINE, thick, head_w, band=6.0,
                                     rise=6.0, blend=blend, wave=tufts)
        fig.add(pos, W, btris, np.where(nf[btris].mean(1) > 0, NAIL, SKIN), lum, red, uv=vuv)
        fig.add(hp, hw, ht, SHELL, uv=huv)
        sp_, sw_, st_, suv_ = hair_shell(pos, W, btris, c, HIS_LINE, scalp_thick, head_w,
                                         band=6.0, base=SCALP_BASE)
        th_, ph_ = angles(sp_, c)
        suv_[:, 1] = ph_ - hairline(th_, HIS_LINE)
        fig.add(sp_, sw_, st_, SCALP, uv=suv_)
        sp, st, smat = suit(assets, obj, refs, scales, pos, J)
        sw = mh.proxy_weights(refs, W)
        # r42:arcus-polish4 — the jacket's sleeves finer, then wrinkled. (Finer, they keep
        # the suit's own modelled cuff buttons, which the arms' thinning had flattened.)
        sp, sw, st, smat, _ = subdivide(sp, sw, st, smat, [], arm_cloth(sw, st, smat, WOOL))
        sp = soften(sp, st, smat, arm_cloth(sw, st, smat, WOOL), SOFTEN)
        sp = sleeve_wrinkles(sp, sw, st, smat, J, WOOL)
        sp = seams(sp, sw, st, smat, J, WOOL)  # r42:arcus-polish5
        # r42:arcus-polish4 — the clothes' edges with a thickness (the coordinator: his
        # lapel's edge, a modelled collar, the cuffs' thickness): the jacket's edge over the
        # shirt (the lapels, the sleeves' ends), the shirt collar's edge over the jacket.
        neck_y = J["neck01____head"][1]
        collar = np.array([q[1] > neck_y - 9.0 for q in sp])
        sp, sw, st, smat, _ = hem(sp, sw, st, smat, [], WOOL, ~collar | (wrists(sw) > 0.5),
                                  HEM_JACKET, against=(SHIRT,))
        collar = np.array([q[1] > neck_y - 9.0 for q in sp])
        sp, sw, st, smat, _ = hem(sp, sw, st, smat, [], SHIRT, collar & (wrists(sw) < 0.2),
                                  HEM_COLLAR, against=(WOOL,))
        fig.add(sp, sw, st, smat)
    fig.add(shp, shw, sht, SHOES)
    for s_ in SIDES:
        ec = J[f"eye.{s_}____head"]
        ep, et, emat, (gz, up_, sd_) = eyeball(ec)
        # (r42:arcus-hair: each point's place in the eye's texture.)
        fig.add(ep, [{BI["head"]: 1.0}] * len(ep), et, emat,
                uv=eye_uv(ep, ec, gz, up_, sd_, sex))
    fig.add(lash_p, [{BI["head"]: 1.0}] * len(lash_p), lash_t, LASH)
    # The proxies' cards (the owner's review, 2026-11-24: hair, brows and lashes that read
    # as hair, not paint): MakeHuman's own, fitted to this body, each card's texture
    # coordinates kept (``uv``).
    hair_name = "ponytail01" if sex == "female" else HIS_HAIR
    brow_name = "eyebrow001" if sex == "female" else "eyebrow010"
    for kind, folder, name, mat in (("hair", "hair", hair_name, CARD_HAIR),
                                     ("brow", "eyebrows", brow_name, CARD_BROW),
                                     ("lash", "eyelashes", "eyelashes01", CARD_LASH)):
        cp, cw, ct, cuv = card_proxy(os.path.join(assets, folder, name, f"{name}.mhclo"), pos, W)
        if kind == "hair" and sex == "female":
            # The ponytail's tail is the chignon's: only the cards over the head kept.
            cen = cp[ct].mean(1)
            ct = ct[~((cen[:, 1] < c[1] - 4.0) & (cen[:, 2] < c[2] - 3.0))]
        lum = None
        if kind == "hair":
            if sex == "female":
                cp = reach_line(cp, c, HER_LINE)
            cp = lift_cards(cp, pos, btris, head_w)
            # (r42:arcus-hair: how far in from the hairline each vertex lies — the bake
            # feathers the cards' coverage at the front of the hair, so the strands thin
            # out onto the forehead instead of ending in a line.)
            if sex == "male":
                cp = his_volume(cp, c)
            th_, ph_ = angles(cp, c)
            lum = line_lum(ph_ - hairline(th_, HER_LINE if sex == "female" else HIS_TOP_LINE))
        fig.add(cp, cw, ct, mat, lum=lum, uv=cuv)
    return compact(fig), J


# r42:arcus-polish4 — his hair: MakeHuman's CC0 ``short04`` (combed straight back, longer on
# top than at the sides, a groomed texture whose strands run one way — the coordinator's
# review: ``short02``'s curls read as a helmet), ``short02`` before.
HIS_HAIR = "short04"


# The cards' textures as written beside the .fig files: (folder, asset, texture, side px).
CARD_TEX = {
    "woman": [("hair", "ponytail01", "ponytail01_diffuse.png", 1024),
              ("eyebrows", "eyebrow001", "eyebrow001.png", 256),
              ("eyelashes", "eyelashes01", "eyelashes01.png", 256)],
    "man": [("hair", HIS_HAIR, f"{HIS_HAIR}_diffuse.png", 1024),
            ("eyebrows", "eyebrow010", "eyebrow010.png", 256),
            ("eyelashes", "eyelashes01", "eyelashes01.png", 256)],
}


def cards(assets, out, name):
    """The cards' textures for ``name`` (woman, man): each MakeHuman texture scaled down and
    kept as grey and alpha (the strands' light and coverage; the colour is the figure's),
    written as ``<name>_<hair|brow|lash>.png``. A hair card's as its maps (r42:arcus-hair,
    ``hair_maps.py``: the strands' detail without the texture's large-scale light, the way
    they run, the coverage) in RGBA."""
    for (folder, asset, tex, side), kind in zip(CARD_TEX[name], ("hair", "brow", "lash")):
        im = Image.open(os.path.join(assets, folder, asset, tex)).convert("RGBA")
        im = im.resize((side, side), Image.LANCZOS)
        path = os.path.join(out, f"{name}_{kind}.png")
        if kind == "hair":
            Image.fromarray(hair_maps.maps(np.array(im)), "RGBA").save(path, optimize=True)
        else:
            im.convert("LA").save(path, optimize=True)


def line_lum(f):
    """r42:arcus-hair — degrees in from the hairline (negative: out over the skin), carried
    in a hair card vertex's luminance byte (unused on a card): 128 + 2 × f, so half a degree
    a step over −64 … +63.5 (``bowed_human::parse`` reads it back)."""
    return 0.35 + np.clip(np.round(np.asarray(f) * 2.0 + 128.0), 0, 255) / 255.0


def scalp_thick(th, ph):
    """The scalp under the cards: a thin even layer of hair's colour (cm). (r42:arcus-hair:
    0.25 cm stood over a quarter of her cards — MakeHuman's ponytail hugs the skull — and
    the scalp showing through them in patches was the owner's "camouflage": now it lies
    just over the skin, ``SCALP_BASE``, and the cards are lifted clear of it.)"""
    return np.zeros(np.shape(th))


# r42:arcus-hair — the scalp's height over the skin, and the least a hair card stands over
# the skin (cm).
SCALP_BASE = 0.04
CARD_CLEAR = 0.12


# r42:arcus-hair — how far her front cards are drawn down to the hairline (degrees; the
# ponytail's cards end about a centimetre short of it, and the scalp showed there as a band).
REACH = 8.0


def reach_line(cp, c, line):
    """The hair cards' front drawn down over the head toward the hairline: each vertex within
    15° of it turned down (about the head's centre, keeping its distance) by up to ``REACH``
    — the strands, combed back, run a little longer; nothing past the temples moves."""
    q = cp - c
    r = np.linalg.norm(q, axis=1)
    th, ph = angles(cp, c)
    f = ph - hairline(th, line)
    front = 1.0 - smooth01((np.abs(th) - 55.0) / 45.0)
    d = REACH * front * (1.0 - smooth01((f - 4.0) / 14.0))
    ph2 = np.radians(ph - d)
    h = np.cos(ph2) * r
    a = np.radians(th)
    return c + np.stack([np.sin(a) * h, np.sin(ph2) * r, np.cos(a) * h], 1)


def his_volume(cp, c):
    """r42:arcus-polish4 — his cards (``short04``) lifted off the head as the Drawn shell's top
    is (``HIS_VOLUME``): each vertex pushed out from the head's centre by the top's volume
    where it lies, so the combed-back top has a body over the crown instead of lying on the
    skull as a cap; the sides, below ``HIS_TOP_LINE``, untouched (they are feathered away
    there: the scalp's short hair shows)."""
    q = cp - c
    r = np.linalg.norm(q, axis=1)
    th, ph = angles(cp, c)
    top = smooth01((ph - hairline(th, HIS_TOP_LINE)) / 9.0)
    front = 1.0 - 0.5 * smooth01((np.abs(th) - 25.0) / 120.0)
    rise = 0.55 + 0.45 * smooth01((ph - 30.0) / 25.0)
    push = 0.85 * HIS_VOLUME * top * front * rise
    return c + q * ((r + push) / np.maximum(r, 1e-6))[:, None]


def lift_cards(cp, pos, tris, head_w):
    """Each hair card vertex lifted along the head's skin normal under it until it stands
    ``CARD_CLEAR`` over the skin (a card under the scalp or the skin is hidden: patches)."""
    head = tris[(head_w[tris] > 0.2).all(1)]
    n = vnormals(pos, head)
    hv = np.unique(head.ravel())
    out = cp.copy()
    for a in range(0, len(cp), 512):
        q = cp[a:a + 512]
        d = np.linalg.norm(q[:, None, :] - pos[hv][None, :, :], axis=2)
        j = hv[d.argmin(1)]
        h = ((q - pos[j]) * n[j]).sum(1)
        near = d.min(1) < 3.0
        lift = np.where(near, np.maximum(CARD_CLEAR - h, 0.0), 0.0)
        out[a:a + 512] = q + n[j] * lift[:, None]
    return out


def card_proxy(path, pos, W):
    """A MakeHuman proxy (hair, brows, lashes) fitted to the body ``pos``: its vertices,
    weights, triangles and texture coordinates (a vertex split where its corners' texture
    coordinates differ)."""
    obj, refs, scales, _, _ = mh.read_mhclo(path)
    sp, suv, sfaces = mh.read_obj(os.path.join(os.path.dirname(path), obj))
    fitted = mh.fit_proxy(refs, scales, pos / 10.0) * 10.0
    w = mh.proxy_weights(refs, W)
    tris, tuv = triangulate(sfaces)
    pairs = np.stack([tris.ravel(), np.maximum(tuv.ravel(), 0)], 1)
    uniq, inv = np.unique(pairs, axis=0, return_inverse=True)
    return (fitted[uniq[:, 0]], [w[i] for i in uniq[:, 0]], inv.reshape(-1, 3),
            suv[uniq[:, 1]] if len(suv) else np.zeros((len(uniq), 2)))


def torus(c, axis, R, r, rows=12, cols=36, twist=2.5, rope=0.0, turns=3, dense=4.0):
    """A ring of hair round ``axis`` at ``c`` (major radius ``R``, the twist's ``r``), its
    strands winding round the twist: points, triangles, strand coordinates. (r42:arcus-
    polish4: ``rope`` > 0 twists it as a rope of two locks — its section swelling and
    narrowing ``turns`` times round the ring — the coordinator: her bun read as a carved
    shell; ``dense`` how finely the strands' coordinate runs round it.)"""
    a = axis / np.linalg.norm(axis)
    u = np.cross(a, [0.0, 1.0, 0.0])
    u = u / max(np.linalg.norm(u), 1e-9)
    v = np.cross(a, u)
    pts, uv = [], []
    for i in range(cols):
        th = 2 * np.pi * i / cols
        ring = u * np.cos(th) + v * np.sin(th)
        for j in range(rows):
            ph = 2 * np.pi * j / rows
            rr = r * (1.0 + rope * np.cos(2.0 * (ph - turns * th)))
            pts.append(c + ring * (R + rr * np.cos(ph)) + a * (rr * np.sin(ph)))
            uv.append([th * dense + ph * twist, 1.0])
    tris = []
    for i in range(cols):
        for j in range(rows):
            p0 = i * rows + j
            p1 = ((i + 1) % cols) * rows + j
            p2 = ((i + 1) % cols) * rows + (j + 1) % rows
            p3 = i * rows + (j + 1) % rows
            tris += [[p0, p1, p2], [p0, p2, p3]]
    pts, tris = np.array(pts), np.array(tris)
    # Wound outward from the ring's own core.
    a_, b_, d_ = pts[tris[:, 0]], pts[tris[:, 1]], pts[tris[:, 2]]
    cen = (a_ + b_ + d_) / 3.0
    rel = cen - c
    core = c + (rel - np.outer(rel @ a, a)) / np.maximum(
        np.linalg.norm(rel - np.outer(rel @ a, a), axis=1, keepdims=True), 1e-9) * R
    fn = np.cross(b_ - a_, d_ - a_)
    flip = ((cen - core) * fn).sum(1) < 0
    tris[flip] = tris[flip][:, [0, 2, 1]]
    return pts, tris, np.array(uv)


DEBUG_EARS = False


def earrings(pos, head_w, c, J):
    """Each earlobe found on the mesh (the lowest point of the ear's outer rim) and a small
    drop earring hung from it: a bead and, under it, a teardrop."""
    eyes = (J["eye.L____head"] + J["eye.R____head"]) * 0.5
    out = []
    for sgn in (1.0, -1.0):
        q = pos - c
        side = q[:, 0] * sgn
        band = (head_w > 0.5) & (q[:, 1] < eyes[1] - c[1] - 1.5) & (q[:, 1] > eyes[1] - c[1] - 4.5)             & (np.abs(q[:, 2] + 0.5) < 3.0)
        if not band.any():
            continue
        far = side[band].max()
        ear = band & (side > far - 1.0)
        lobe = pos[ear][np.argmin(pos[ear][:, 1])]
        if DEBUG_EARS:
            print("lobe", sgn, np.round(lobe - eyes, 2), "far", round(far, 2))
        # Through the lobe, a little in from its rim.
        bead = lobe + np.array([-0.25 * sgn, -0.2, 0.0])
        bp, bt, _ = ellipsoid(bead, np.eye(3), (0.2, 0.2, 0.2), rows=6, cols=10)
        dp, dt, _ = ellipsoid(bead + np.array([0.0, -0.62, 0.0]), np.eye(3), (0.22, 0.42, 0.22),
                              rows=8, cols=10)
        bt = outward(bp, bt, bead)
        dt = outward(dp, dt, bead + np.array([0.0, -0.62, 0.0]))
        out.append((np.vstack([bp, dp]), np.vstack([bt, dt + len(bp)])))
    return out


def brows(pos, keep_v, J, sex):
    """The brows painted on the skin as a soft arc over each eye (seen from the front: from
    over the eye's inner corner, rising to its highest two thirds of the way out, tapering
    to the outer end), so the brow is the skin's own colour darkened — no card that could
    read as a stuck-on strip. A man's a little fuller and straighter."""
    out = np.zeros(len(pos))
    full = 1.0 if sex == "female" else 1.6  # r42:arcus-polish3: his thicker
    for s_, sign in (("L", 1.0), ("R", -1.0)):
        e = J[f"eye.{s_}____head"]
        x = (pos[:, 0] - e[0]) * sign
        y = pos[:, 1] - e[1]
        front = (pos[:, 2] > e[2] - 1.5) & keep_v
        x0, x1 = -1.45, 2.55
        t = np.clip((x - x0) / (x1 - x0), 0.0, 1.0)
        rise = 0.42 if sex == "female" else 0.1  # r42:arcus-polish3: his straighter
        yc = 1.25 + rise * np.sin(np.pi * np.clip(t, 0, 1) ** 0.75) - 0.25 * t
        # (Wider than a real brow's hairs: the forehead's vertices are half a centimetre
        # apart, and a thinner arc vanished between them — the owner's review: no brows.)
        half = (0.42 - 0.2 * t) * full
        across = 1.0 - smooth01((np.abs(y - yc) - half) / 0.3 + 0.5)
        ends = smooth01((x - x0) / 0.35) * (1.0 - smooth01((x - x1) / 0.35 + 1.0))
        amt = np.where(front, across * ends * (1.0 - 0.35 * t), 0.0)
        out = np.maximum(out, amt)
    return out


def lash_line(pos, tris, lum, J, wt):
    """The lowered lids' lash line: each upper lid's edge (``lid_margin``) a dark line, the
    vertices round it a shade darker."""
    rim = set()
    for s_ in SIDES:
        rim |= set(lid_margin(pos, wt, J, s_))
    out = lum.copy()
    near = set()
    for t in tris:
        if rim & set(t.tolist()):
            near |= set(t.tolist())
    for i in near:
        out[i] = min(out[i], 0.78)
    for i in rim:
        out[i] = 0.42
    return out


def warmth(pos, J, sex):
    """Where a face is warmer (0…1): the cheeks' apples, the lips, the nose's tip, the
    ears' rims — soft blobs round points taken from the eyes' places."""
    el, er = J["eye.L____head"], J["eye.R____head"]
    mid = (el + er) * 0.5
    out = np.zeros(len(pos))
    spots = [
        (el + np.array([0.6, -3.0, 0.6]), 2.4, 1.0),
        (er + np.array([-0.6, -3.0, 0.6]), 2.4, 1.0),
        (mid + np.array([0.0, -4.0, 2.6]), 1.2, 0.9),  # (polish4: the nose's tip 0.7)
        (mid + np.array([0.0, -6.6, 1.3]), 1.4, 0.9),
        (el + np.array([4.6, -1.5, -5.8]), 2.0, 0.6),
        (er + np.array([-4.6, -1.5, -5.8]), 2.0, 0.6),
    ]
    for c, r, k in spots:
        d = np.linalg.norm(pos - c, axis=1)
        out = np.maximum(out, k * (1.0 - smooth01(d / r)))
    return out * (1.0 if sex == "female" else 0.6)


def nail_field(pos, W, sk, J):
    """Per vertex: inside a nail (> 0) or outside it, cm-ish — on each fingertip's back,
    from 42 % of its last phalanx to its end, as wide as 0.6 of the finger, its base
    rounded (a cuticle's curve). The fingers' backs face the hand's back (its knuckles'
    plane); the thumb's its own, turned 20° about it."""
    out = np.full(len(pos), -9.0)
    for s_ in SIDES:
        def kn(f):
            return J[sk["bones"][f"finger{f}-1.{s_}"]["head"]]
        wr = J[sk["bones"][f"wrist.{s_}"]["head"]]
        nb = np.cross(kn(2) - kn(5), (kn(3) + kn(4)) * 0.5 - wr)
        nb = nb / np.linalg.norm(nb)
        # At rest (MakeHuman's A pose) the back of each hand faces out, away from the body.
        if nb[0] * (1 if s_ == "L" else -1) < 0:
            nb = -nb
        for f in range(1, 6):
            b = sk["bones"][f"finger{f}-3.{s_}"]
            h, tl = J[b["head"]], J[b["tail"]]
            L = np.linalg.norm(tl - h)
            a = (tl - h) / L
            d = nb - a * (nb @ a)
            d = d / np.linalg.norm(d)
            if f == 1:
                ang = np.radians(20.0 if s_ == "L" else -20.0)
                d = d * np.cos(ang) + np.cross(a, d) * np.sin(ang)
            side = np.cross(a, d)
            q = pos - h
            al = q @ a
            r = q - np.outer(al, a)
            rad = np.maximum(np.linalg.norm(r, axis=1), 1e-6)
            x, y = r @ side, r @ d
            sb = BI[f"f{f}_3.{s_}"]
            share = np.array([dd.get(sb, 0.0) / max(sum(dd.values()), 1e-9) for dd in W])
            w = 0.36 * L * (0.62 if f > 1 else 0.7)
            fx = w - np.abs(x)
            fa = (al - 0.42 * L) - 0.6 * (np.abs(x) / w) ** 2 * w
            fe = 0.96 * L - al
            fy = (y / rad - 0.42) * 1.5
            fl = np.minimum(np.minimum(fx, fa), np.minimum(fy, fe))
            out = np.maximum(out, np.where(share > 0.3, fl, -9.0))
    return out


def eyeball(c):
    """An eye under its lowered lid: a ball in the socket, looking down ``GAZE_DOWN`` toward the
    instrument, its iris (a grey-green disc 30° across its half) and the white round it."""
    r = 1.18
    dn, lf = np.radians(GAZE_DOWN), np.radians(GAZE_LEFT)
    gaze = np.array([np.sin(lf) * np.cos(dn), -np.sin(dn), np.cos(lf) * np.cos(dn)])
    up = np.cross(np.cross(gaze, np.array([0.0, 1.0, 0.0])), gaze)
    up = up / np.linalg.norm(up)
    side = np.cross(up, gaze)
    rings = [0, 6, 12, 19, 26, 30, 36, 48, 64, 82, 100, 125, 150, 180]
    cols = 20
    pts = [c + gaze * r]
    for th in rings[1:-1]:
        t = np.radians(th)
        for k in range(cols):
            a = 2 * np.pi * k / cols
            pts.append(c + (gaze * np.cos(t) + (up * np.cos(a) + side * np.sin(a)) * np.sin(t)) * r)
    pts.append(c - gaze * r)
    pts = np.array(pts)
    tri, mat = [], []
    n_r = len(rings) - 2
    for k in range(cols):
        tri.append([0, 1 + k, 1 + (k + 1) % cols])
        mat.append(IRIS)
    for i in range(n_r - 1):
        for k in range(cols):
            a0, a1 = 1 + i * cols + k, 1 + i * cols + (k + 1) % cols
            b0, b1 = a0 + cols, a1 + cols
            m = IRIS if rings[i + 1] < 30 else EYE
            tri += [[a0, b0, b1], [a0, b1, a1]]
            mat += [m, m]
    last = len(pts) - 1
    for k in range(cols):
        a0, a1 = 1 + (n_r - 1) * cols + k, 1 + (n_r - 1) * cols + (k + 1) % cols
        tri.append([a0, last, a1])
        mat.append(EYE)
    tri = outward(pts, np.array(tri), c)
    return pts, tri, np.array(mat), (gaze, up, side)


def lid_margin(pos, wt, J, s_):
    """The upper lid's edge over eye ``s_`` (the base mesh is closed round the eyes, so the
    edge is found, not read): the upper lid's lowest vertex in front of the eye in each
    3 mm column across it, ordered from the inner corner out."""
    e = J[f"eye.{s_}____head"]
    ids = np.array([i for i, w in wt[f"orbicularis03.{s_}"] if w > 0.5])
    q = pos[ids] - e
    r = np.linalg.norm(q, axis=1)
    ids, q = ids[(r < 1.75) & (q[:, 2] > 0.8)], q[(r < 1.75) & (q[:, 2] > 0.8)]
    cols = {}
    for i, qq in zip(ids, q):
        k = int(np.round(qq[0] / 0.3))
        if k not in cols or qq[1] < pos[cols[k]][1] - e[1]:
            cols[k] = i
    return [cols[k] for k in sorted(cols)]


def lashes(pos, wt, J):
    """The upper lids' lashes: a short dark fringe along each lid's edge, curving out and
    down over the lowered lid (both windings, so it is seen from either side)."""
    pts, tri = [], []
    for s_ in SIDES:
        e = J[f"eye.{s_}____head"]
        m = lid_margin(pos, wt, J, s_)
        for a, b in zip(m, m[1:]):
            if np.linalg.norm(pos[a] - pos[b]) > 0.7:
                continue
            o = len(pts)
            for v_ in (a, b):
                p = pos[v_]
                out = p - e
                out = out / np.linalg.norm(out)
                # Longer in the middle of the lid than at its corners.
                mid = 1.0 - min(abs(p[0] - e[0]) / 1.3, 1.0) ** 2
                d = out * 0.55 + np.array([0.0, -0.7, 0.3])
                d = d / np.linalg.norm(d) * (0.14 + 0.16 * mid)
                pts += [p + out * 0.02, p + d]
            tri += [[o, o + 1, o + 3], [o, o + 3, o + 2], [o, o + 3, o + 1], [o, o + 2, o + 3]]
    return np.array(pts).reshape(-1, 3), np.array(tri, dtype=np.int64).reshape(-1, 3)


def lip_spot(pos, J):
    """The lips (0…1): a soft blob at the mouth, wider than tall."""
    el, er = J["eye.L____head"], J["eye.R____head"]
    c = (el + er) * 0.5 + np.array([0.0, -6.7, 1.3])
    q = (pos - c) / np.array([2.3, 0.8, 1.4])
    return 1.0 - smooth01(np.linalg.norm(q, axis=1))


def lips(pos, tris, J):
    return np.zeros(len(tris), bool)


def bone_share(d, names):
    tot = sum(d.values()) or 1.0
    return sum(w for b, w in d.items() if BONES[b][0] in names) / tot


# r42:arcus-polish — the velvet's folds: how deep at the waist (where a seated figure's
# bodice gathers as she leans to the instrument), and the band they lie in (cm round the
# waist's height, its soft edge).
WAIST_FOLDS = (0.8, 5.0, 3.0)


def cloth_folds(p, w, t, mats, uv, J):
    """r42:arcus-polish — each velvet-only vertex's folds, carried as its texture
    coordinates ``[10 + depth, height]`` (``scene/figure/cloth.rs::fold``: a run of soft
    folds across the height): the bodice gathered at the waist, front and sides, fading to
    nothing at the back, the neckline and the arms. A vertex a skin triangle shares keeps
    the skin's coordinates (no fold at the dress's edge)."""
    uv = uv.copy()
    vel = np.zeros(len(p), bool)
    vel[t[mats == VELVET].ravel()] = True
    skin = np.zeros(len(p), bool)
    skin[t[mats != VELVET].ravel()] = True
    only = vel & ~skin
    depth, half, soft = WAIST_FOLDS
    waist = J["spine03____head"]
    trunk = np.array([bone_share(d, ("spine", "hips", "chest")) for d in w])
    band = 1.0 - smooth01((np.abs(p[:, 1] - waist[1]) - half) / soft)
    # Front and sides (the back hangs straight from the shoulders).
    front = smooth01((p[:, 2] - waist[2] + 6.0) / 8.0)
    amp = depth * band * front * smooth01((trunk - 0.4) / 0.4)
    uv[only, 0] = 10.0 + amp[only]
    uv[only, 1] = p[only, 1]
    return uv


# r42:arcus-polish4 — the sleeves' wrinkles in the cloth itself (the coordinator: the
# black cloth read as smooth plastic; ref-06's and ref-10's sleeves): (how deep, how far
# apart, cm) at the elbow's crook and the armpit's drag folds; the crook's
# folds run round the arm a little on the slant, as a bent sleeve's spiral.
WRINKLE_ELBOW = (0.45, 2.6)
WRINKLE_PIT = (0.25, 2.6)


def subdivide(p, w, t, mats, extra, sel):
    """r42:arcus-polish4 — the triangles ``sel`` (a mask) split in four at their edges'
    middles, and each neighbour that shares a split edge split to match (in two or three:
    no crack): (positions, weights, triangles, materials, extra arrays). New vertices
    interpolate the positions, the weights and every array in ``extra``."""
    p = list(p)
    w = list(w)
    extra = [list(e) for e in extra]
    mid = {}
    for tr in t[sel]:
        for a, b in ((tr[0], tr[1]), (tr[1], tr[2]), (tr[2], tr[0])):
            k = (min(a, b), max(a, b))
            if k in mid:
                continue
            p.append((np.asarray(p[a]) + np.asarray(p[b])) * 0.5)
            wa, wb = w[a], w[b]
            w.append({x: 0.5 * wa.get(x, 0.0) + 0.5 * wb.get(x, 0.0) for x in set(wa) | set(wb)})
            for e in extra:
                e.append((np.asarray(e[a]) + np.asarray(e[b])) * 0.5)
            mid[k] = len(p) - 1
    out, om = [], []
    for tr, m in zip(t, mats):
        a, b, c = (int(x) for x in tr)
        ab, bc, ca = (mid.get((min(x, y), max(x, y))) for x, y in ((a, b), (b, c), (c, a)))
        n = sum(x is not None for x in (ab, bc, ca))
        if n == 0:
            tris = [[a, b, c]]
        elif n == 3:
            tris = [[a, ab, ca], [ab, b, bc], [ca, bc, c], [ab, bc, ca]]
        elif n == 1:
            if ab is not None:
                tris = [[a, ab, c], [ab, b, c]]
            elif bc is not None:
                tris = [[a, b, bc], [a, bc, c]]
            else:
                tris = [[a, b, ca], [ca, b, c]]
        else:
            # Two split edges: the corner between them cut off, the rest in two.
            if ab is None:
                tris = [[c, ca, bc], [ca, a, b], [ca, b, bc]]
            elif bc is None:
                tris = [[a, ab, ca], [ab, b, c], [ab, c, ca]]
            else:
                tris = [[b, bc, ab], [ab, bc, c], [ab, c, a]]
        out += tris
        om += [m] * len(tris)
    return (np.array(p), w, np.array(out, dtype=np.int64), np.array(om),
            [np.array(e) for e in extra])


# r42:arcus-polish4 — his sleeves' smoothing passes after they are made finer: the suit's
# cuff buttons kept as soft domes (sharp-topped, each caught the back light on one pixel:
# `arcus_figure_specks`' row of dots along his cuff).
SOFTEN = 10


def soften(p, t, mats, sel, passes):
    """The vertices of the triangles ``sel`` that no other triangle shares drawn toward their
    neighbours' middle ``passes`` times (half way each): a surface's small sharp forms
    rounded, its edges where it meets other triangles kept."""
    tt = t[sel]
    inner = np.zeros(len(p), bool)
    inner[tt.ravel()] = True
    inner[t[~sel].ravel()] = False
    e = np.concatenate([tt[:, [0, 1]], tt[:, [1, 2]], tt[:, [2, 0]]])
    deg = np.zeros(len(p))
    np.add.at(deg, e[:, 0], 1.0)
    np.add.at(deg, e[:, 1], 1.0)
    q = p.copy()
    for _ in range(passes):
        acc = np.zeros_like(q)
        np.add.at(acc, e[:, 0], q[e[:, 1]])
        np.add.at(acc, e[:, 1], q[e[:, 0]])
        avg = acc / np.maximum(deg, 1.0)[:, None]
        q = np.where(inner[:, None], q + (avg - q) * 0.5, q)
    return q


def arm_cloth(w, t, mats, cloth, least=0.3):
    """r42:arcus-polish4 — which triangles are ``cloth`` on an arm (their corners' arm
    share over ``least``): the sleeves."""
    share = np.array([max(arm_share(d, "L"), arm_share(d, "R")) for d in w])
    return (mats == cloth) & (share[t].mean(1) > least)


# r42:arcus-polish4 — her sleeves' ends as a finished hem (the owner: her sleeve's end must
# not "lose itself" into the body; it thinned to nothing on the skin, as paint): the cloth's
# edge stands this far off the wrist (cm), and a little more flared, with the hem's own face
# between it and the skin.
HEM = (0.3, 0.12)


# His clothes' edges (cm): the jacket's over the shirt (the lapels' edge, the sleeves' hems
# over the shirt's cuffs) and the shirt collar's over the jacket and the neck.
HEM_JACKET = (0.22, 0.06)
HEM_COLLAR = (0.18, 0.05)


def wrists(w):
    """Each vertex's share of a forearm or a hand (either side)."""
    return np.array([max(bone_share(d, ("fore1.L", "fore2.L", "hand.L")),
                         bone_share(d, ("fore1.R", "fore2.R", "hand.R"))) for d in w])


def hem(p, w, t, mats, extra, cloth, pick, size=HEM, against=None):
    """r42:arcus-polish4 — the edge where ``cloth`` meets another stuff given a thickness,
    where ``pick`` (a mask over the vertices) allows: the cloth's edge vertices copied (the
    cloth's triangles take the copies, the other stuff keeps the originals), the copies
    lifted along the cloth's normal (``size``: the thickness and a little flare) and a band
    of ``cloth`` between each copied edge and the original — the hem's face. Returns
    (positions, weights, triangles, materials, extra). ``against``: only edges with these
    stuffs (all others by default)."""
    cl = mats == cloth
    other = ~cl if against is None else np.isin(mats, against)
    on = np.zeros(len(p), bool)
    on[t[cl].ravel()] = True
    sk = np.zeros(len(p), bool)
    sk[t[other].ravel()] = True
    ids = np.where(on & sk & pick)[0]
    if len(ids) == 0:
        return p, w, t, mats, extra
    n = vnormals(p, t[cl])
    thick, flare = size
    copy = {}
    p = list(p)
    w = list(w)
    extra = [list(e) for e in extra]
    for i in ids:
        copy[int(i)] = len(p)
        p.append(p[i] + n[i] * (thick + flare))
        w.append(dict(w[i]))
        for e in extra:
            e.append(e[i])
    t = t.copy()
    for k in np.where(cl)[0]:
        t[k] = [copy.get(int(x), int(x)) for x in t[k]]
    # The hem's face: each cloth triangle's edge whose two ends were copied, if a skin
    # triangle has that edge too.
    skin_e = set()
    for tr in t[other]:
        for a, b in ((tr[0], tr[1]), (tr[1], tr[2]), (tr[2], tr[0])):
            skin_e.add((min(int(a), int(b)), max(int(a), int(b))))
    inv = {v: k for k, v in copy.items()}
    band = []
    for tr in t[cl]:
        for a, b in ((tr[0], tr[1]), (tr[1], tr[2]), (tr[2], tr[0])):
            a0, b0 = inv.get(int(a)), inv.get(int(b))
            if a0 is None or b0 is None or (min(a0, b0), max(a0, b0)) not in skin_e:
                continue
            band += [[int(b), int(a), a0], [int(b), a0, b0]]
    t = np.vstack([t, np.array(band, dtype=np.int64).reshape(-1, 3)])
    mats = np.concatenate([mats, np.full(len(band), cloth)])
    return np.array(p), w, t, mats, [np.array(e) for e in extra]


def sleeve_wrinkles(p, w, t, mats, J, cloth):
    """r42:arcus-polish4 — the cloth ``cloth`` on each arm moved along its normal into soft
    wrinkles (``WRINKLE_*``): rings round the arm near the elbow, densest and deepest on the
    crook's side, their phase turning round the arm so they slant; drag folds under the arm
    from the armpit. Only vertices every
    triangle round which is ``cloth`` move (a cuff's or a neckline's edge stays put), so the
    cloth's edges and the skin keep their places."""
    on = np.zeros(len(p), bool)
    on[t[mats == cloth].ravel()] = True
    off = np.zeros(len(p), bool)
    off[t[mats != cloth].ravel()] = True
    move = on & ~off
    n = vnormals(p, t[mats == cloth])
    d = np.zeros(len(p))
    rng = np.random.default_rng(11)
    for s_ in SIDES:
        sh, el, wr = (J[f"upperarm01.{s_}____head"], J[f"lowerarm01.{s_}____head"],
                      J[f"wrist.{s_}____head"])
        share = np.array([bone_share(x, (f"upper1.{s_}", f"upper2.{s_}", f"fore1.{s_}",
                                         f"fore2.{s_}")) for x in w])
        ids = np.where(move & (share > 0.25))[0]
        if len(ids) == 0:
            continue
        q = p[ids]
        # Each point's place along the arm (cm from the elbow: − up the arm, + down the
        # forearm) and its angle round it (0 on the crook's side, the front).
        up, fo = sh - el, wr - el
        lu, lf = np.linalg.norm(up), np.linalg.norm(fo)
        tu = ((q - el) @ up) / lu
        tf = ((q - el) @ fo) / lf
        along = np.where(tu > 0.0, -tu, tf)
        ax = np.where((tu > 0.0)[:, None], (up / lu)[None, :], (fo / lf)[None, :])
        rad = (q - el) - ax * ((q - el) * ax).sum(1)[:, None]
        rad /= np.maximum(np.linalg.norm(rad, axis=1, keepdims=True), 1e-6)
        front = np.array([0.0, 0.0, 1.0])
        side_ = np.cross(ax, front)
        phi = np.arctan2((rad * side_).sum(1), rad @ front)
        jitter = rng.uniform(-0.6, 0.6, size=5)
        wave = lambda lam, slant: np.sin(2 * np.pi * along / lam + slant * phi
                                         + jitter[0] * np.sin(phi * 2.0 + jitter[1]))
        crook = 0.35 + 0.65 * np.clip(np.cos(phi), 0.0, 1.0)
        a, lam = WRINKLE_ELBOW
        # (Creases cut in from the sleeve's surface, never standing out of it: the sleeve's
        # silhouette stays the arm's — `arcus_figure_shape`'s elbow.)
        d[ids] += a * np.exp(-(along / 8.0) ** 2) * crook * 0.5 * (wave(lam, 0.8) - 1.0)
        # Under the arm, from the armpit: drag folds along the arm's first quarter on its
        # body side.
        a, lam = WRINKLE_PIT
        inner = np.clip(-(rad[:, 0] * np.sign(sh[0])), 0.0, 1.0)
        pit = smooth01((-along - lu * 0.6) / (lu * 0.2)) * inner
        d[ids] += a * pit * 0.5 * (np.sin(2 * np.pi * phi / 1.1 + jitter[3] + along / lam) - 1.0)
    return p + n * d[:, None]


# r42:arcus-polish5 — the set-in sleeve's seam and the shoulder seam (the clothing study's
# lacking item, ref-10's jacket: the set-in seam reads as a soft ridge): (the ridge's
# height, its half-width, how far toward the sleeve it lies off the seam's line, the
# groove's depth at the line; cm; ``CANTUS_SEAM=h,half,lean,groove`` while studied). The
# cloth's mesh is a centimetre or so a vertex: a ridge it can draw, not a stitch (0.4 cm
# read as a lump in the shoulder's outline, 0.13 not at all). His jacket's only: her velvet
# gown's seams hide in its pile, and on her raised shoulder the ridge rippled its outline.
SEAM = tuple(float(x) for x in os.environ.get("CANTUS_SEAM", "0.22,0.8,0.45,0.08").split(","))
# Where the sleeve is set in: the iso-line of the arm's share of the weights (MakeHuman's
# shoulder blend, about the acromion's line round the shoulder).
SEAM_SHARE = 0.5


def seams(p, w, t, mats, J, cloth):
    """r42:arcus-polish5 — ``cloth`` moved along its normal into two soft ridges: round each
    armhole (where the sleeve's cap is sewn to the body: the line where the arm's share of
    the weights crosses ``SEAM_SHARE``, the ridge on the sleeve's side — the seam allowance
    pressed into the cap) and along each shoulder's top from the neck's side to the armhole
    (the front panel's meeting with the back's, a little behind the shoulder's crest). Only
    vertices every triangle round which is ``cloth`` move (its edges stay put)."""
    on = np.zeros(len(p), bool)
    on[t[mats == cloth].ravel()] = True
    off = np.zeros(len(p), bool)
    off[t[mats != cloth].ravel()] = True
    move = on & ~off
    n = vnormals(p, t[mats == cloth])
    nn = n / np.maximum(np.linalg.norm(n, axis=1, keepdims=True), 1e-9)
    h, half, lean, groove = SEAM
    d = np.zeros(len(p))
    tc = t[mats == cloth]
    for s_ in SIDES:
        share = np.array([arm_share(x, s_) for x in w]) - SEAM_SHARE
        sh = J[f"upperarm01.{s_}____head"]
        # The armhole's line: where each cloth edge crosses the share's middle.
        e = np.concatenate([tc[:, [0, 1]], tc[:, [1, 2]], tc[:, [2, 0]]])
        fa, fb = share[e[:, 0]], share[e[:, 1]]
        x = (fa > 0) != (fb > 0)
        if not x.any():
            continue
        f = (fa[x] / (fa[x] - fb[x]))[:, None]
        line = p[e[x, 0]] + (p[e[x, 1]] - p[e[x, 0]]) * f
        # (Near the shoulder only: the share's middle elsewhere is no seam.)
        line = line[np.linalg.norm(line - sh, axis=1) < 14.0]
        ids = np.where(move & (np.linalg.norm(p - sh, axis=1) < 16.0))[0]
        if len(line) == 0 or len(ids) == 0:
            continue
        dist = np.min(np.linalg.norm(p[ids][:, None, :] - line[None, :, :], axis=2), axis=1)
        sgn = np.sign(share[ids])
        sd = dist * sgn
        d[ids] += h * np.exp(-((sd - lean) / half) ** 2) - groove * np.exp(-(sd / 0.35) ** 2)
        # The shoulder seam: over the shoulder's top, from the neck's side to the armhole,
        # a little behind its crest.
        nk = J["neck01____head"]
        ax = sh - nk
        ax[1] = 0.0
        L = np.linalg.norm(ax)
        ax /= max(L, 1e-6)
        q = p[ids] - nk
        along = q @ ax
        z_line = nk[2] + (sh[2] - nk[2]) * np.clip(along / L, 0.0, 1.0) - 0.6
        top = np.clip((nn[ids, 1] - 0.45) / 0.35, 0.0, 1.0)
        span = (smooth01((along - 3.0) / 2.0) * (1.0 - smooth01((along - (L - 1.0)) / 2.0))
                * (share[ids] < 0))
        dz = p[ids, 2] - z_line
        d[ids] += h * 0.8 * top * span * np.exp(-((dz - 0.3) / half) ** 2)
    return p + n * (d / np.maximum(np.linalg.norm(n, axis=1), 1e-9))[:, None]


def dress(pos, W, tris, J, lum, red, nail, vuv=None):
    """The woman's black velvet concert dress on her body: long sleeves to the wrists, a
    soft scoop neckline in front (the collarbones' line showing, the shoulders covered);
    the skirt is draped by the scene round the seated legs. The body is cut along each
    edge of the dress so it is a smooth curve; the velvet stands a little proud of the
    skin (the cloth's thickness, a soft roll at the cuffs and the neckline). Returns
    (positions, weights, triangles, materials, albedo detail)."""
    neck = J["neck01____head"]

    def fields(p, w):
        x, y, z = p[:, 0] - neck[0], p[:, 1] - neck[1], p[:, 2] - neck[2]
        hn = np.array([bone_share(d, ("head", "neck")) for d in w])
        # A V neckline (sheet v7, the reference's dress): from beside the neck down to the
        # breastbone's middle, its point softly rounded.
        ax = np.sqrt(x * x + 0.9 ** 2)
        # (r42:arcus-polish4: its point higher — `V_DEPTH`, 15 before: the bow forearm crosses
        # her chest, and the V's point showed under it as a skin diamond that read as the arm
        # through the sleeve; and modesty.)
        depth = V_DEPTH * np.clip(1.0 - ax / 9.6, 0.0, 1.0) ** 1.1 + 1.0
        scoop = np.minimum(np.minimum(z + 1.5, y + depth), 8.4 - np.abs(x))
        g_neck = np.maximum(10.0 * (hn - 0.5), scoop)
        g_hand = 10.0 * (np.array([hand_share(d) for d in w]) - 0.55)
        return g_neck, g_hand

    # The uncut body's normals carried through the cuts (a sliver's own normal pushed the
    # velvet out in fins along the neckline).
    n0 = vnormals(pos, tris)
    if vuv is None:
        vuv = np.zeros((len(pos), 2))
    p, w, ex, t = pos, W, [lum, red, nail, n0[:, 0], n0[:, 1], n0[:, 2], vuv[:, 0], vuv[:, 1]], tris
    for k in range(2):
        f = fields(p, w)[k]
        p, w, ex, t, _ = cut(p, w, ex, t, f)
    g_neck, g_hand = fields(p, w)
    # The shins under the long skirt are never seen (the scene drapes it to the floor):
    # left out, so none shows through its folds.
    shin = np.array([bone_share(d, ("shin.L", "shin.R", "foot.L", "foot.R")) for d in w])
    t = t[shin[t].mean(1) < 0.5]
    skin_v = np.maximum(g_neck, g_hand)
    mats = np.where(skin_v[t].mean(1) > 0, SKIN, VELVET)
    mats = np.where((mats == SKIN) & (ex[2][t].mean(1) > 0), NAIL, mats)
    nrm = np.stack(ex[3:6], 1)
    nrm = nrm / np.maximum(np.linalg.norm(nrm, axis=1, keepdims=True), 1e-9)
    # The velvet stands proud of the skin, more over the shoulder's top (the reference's
    # dress: its sleeves gathered a little at the shoulder).
    cap = np.array([bone_share(d, ("upper1.L", "upper1.R", "clav.L", "clav.R")) for d in w])
    # (r42:arcus-polish: hardly any — the owner, 2026-11-25: broad, padded shoulders.)
    puff = (SHOULDER_PUFF * smooth01((cap - 0.35) / 0.4) * np.clip(nrm[:, 1] * 1.4, 0.0, 1.0)
            * smooth01(-skin_v / 4.0))
    lift = (0.22 + puff) * smooth01(-skin_v / 0.8)
    q = p + nrm * lift[:, None]
    # A draped bodice (the owner, 2026-11-24: modest and elegant — the dress must never
    # read as paint on the body): over the trunk the cloth is smoothed out of the body's
    # contour (each vertex drawn toward its neighbours' middle, many times, weighted from
    # nothing at the neckline and the cuffs to full well inside the dress) and stands off it
    # by a loose fit's ease, more in front; the sleeves a little fuller too.
    trunk = np.array([bone_share(d, ("spine", "chest", "hips", "breast.L", "breast.R"))
                      for d in w])
    arm = np.array([bone_share(d, ("upper1.L", "upper1.R", "upper2.L", "upper2.R",
                                    "fore1.L", "fore1.R", "fore2.L", "fore2.R")) for d in w])
    inner = smooth01(-skin_v / 3.0)
    vel = mats == VELVET
    on = np.zeros(len(q), bool)
    on[t[vel].ravel()] = True
    # (Never across the armpit: smoothed there the sleeve and the side webbed together.)
    m = np.clip(trunk, 0.0, 1.0) * inner * (1.0 - smooth01(arm / 0.3))
    m = np.where(on, m, 0.0)
    # The front falls as cloth does from the shoulders: the depth of the trunk's front half
    # blurred across (a 5 cm Gaussian over its own points), so it lies over the body's forms
    # instead of following them.
    zc = np.median(q[on & (trunk > 0.4), 2]) if (on & (trunk > 0.4)).any() else 0.0
    fr = np.where(on & (trunk > 0.2) & (q[:, 2] > zc - 4.0))[0]
    if len(fr):
        xy = q[fr][:, :2]
        d2 = ((xy[:, None, :] - xy[None, :, :]) ** 2).sum(-1)
        g = np.exp(-d2 / (2.0 * 5.0 ** 2))
        # (Weighted in softly from the trunk's middle depth and its sides: a hard edge to
        # the blurred part left a ridge.)
        k = (inner[fr] * smooth01((trunk[fr] - 0.2) / 0.6)
             * smooth01((q[fr, 2] - zc + 4.0) / 8.0))
        for _ in range(2):
            zb = (g @ q[fr][:, 2]) / g.sum(1)
            q[fr, 2] = q[fr, 2] + (zb - q[fr, 2]) * k
    # Welded for the smoothing: the base mesh repeats a vertex where its pieces meet (an
    # unwelded piece over the bust stood out of the smoothed cloth as a point).
    keys = np.round(q * 50.0).astype(np.int64)
    _, rep, inv = np.unique(keys, axis=0, return_index=True, return_inverse=True)
    rep = rep[inv.ravel()]
    e = np.concatenate([t[vel][:, [0, 1]], t[vel][:, [1, 2]], t[vel][:, [2, 0]]])
    e = rep[e]
    e = e[e[:, 0] != e[:, 1]]
    deg = np.zeros(len(q))
    np.add.at(deg, e[:, 0], 1.0)
    np.add.at(deg, e[:, 1], 1.0)
    for _ in range(BODICE_SMOOTH):
        acc = np.zeros_like(q)
        np.add.at(acc, e[:, 0], q[e[:, 1]])
        np.add.at(acc, e[:, 1], q[e[:, 0]])
        avg = acc / np.maximum(deg, 1.0)[:, None]
        q = q + (avg - q) * (0.5 * m)[:, None]
        q = q[rep]
    # r42:arcus-polish4 — the bodice's front over the bust smoothed further (modesty, the
    # owner's standing direction: the Drawn picture's sheen still drew a small crescent of the
    # bust's form through the cloth): more passes, weighted to the chest's front at its height.
    chest = J["spine01____head"]
    bust = (np.exp(-((q[:, 1] - chest[1]) / 7.0) ** 2) * smooth01((q[:, 2] - chest[2]) / 4.0)
            * np.where(on, m, 0.0))
    for _ in range(BUST_SMOOTH):
        acc = np.zeros_like(q)
        np.add.at(acc, e[:, 0], q[e[:, 1]])
        np.add.at(acc, e[:, 1], q[e[:, 0]])
        avg = acc / np.maximum(deg, 1.0)[:, None]
        q = q + (avg - q) * (0.5 * bust)[:, None]
        q = q[rep]
    nq = vnormals(q, t[vel])
    nq = np.where(on[:, None], nq, nrm)
    front = np.clip(nq[:, 2], 0.0, 1.0)
    ease = m * (BODICE_EASE + 0.6 * front) + np.where(on, arm, 0.0) * inner * SLEEVE_EASE
    return q + nq * ease[:, None], w, t, mats, ex[0], ex[1], np.stack([ex[6], ex[7]], 1)


# The bodice's smoothing (passes) and ease (cm), the sleeves' ease.
BODICE_SMOOTH = 160
# r42:arcus-polish4 — the V neckline's depth (cm under the neck's base at its middle).
V_DEPTH = float(os.environ.get("CANTUS_V_DEPTH", "11.0"))
# r42:arcus-polish4 — the further passes over the bust (``dress``).
BUST_SMOOTH = 200
BODICE_EASE = 0.7
SLEEVE_EASE = 0.18  # r42:arcus-polish (0.35: the sleeves follow the arm)
# r42:arcus-polish — the cloth's rise over the shoulder's top (0.55: padded).
SHOULDER_PUFF = 0.1


def suit(assets, obj, refs, scales, pos, J=None):
    """The man's suit (MakeHuman's male_elegantsuit01, CC0): fitted to his body, each
    face's material told by its texture — the jacket's and the trousers' wool, the
    shirt's collar and cuffs, the tie, the shoes."""
    d = os.path.join(assets, "clothes", "male_elegantsuit01")
    sp, suv, sfaces = mh.read_obj(os.path.join(d, obj))
    fitted = mh.fit_proxy(refs, scales, pos / 10.0) * 10.0
    tex = Image.open(os.path.join(d, "male_elegantsuit01_diffuse.png"))
    tris, tuv = triangulate(sfaces)
    cuv = suv[tuv].mean(axis=1)
    col = sample(tex, cuv)
    lum = col @ np.array([0.2126, 0.7152, 0.0722])
    u, v = cuv[:, 0] % 1.0, 1.0 - cuv[:, 1] % 1.0
    # The tie's stripes averaged away: blue over a wide box is the tie.
    wide = sample(tex, cuv, 14)
    tie = (wide[:, 2] > wide[:, 0] + 0.04) & (wide[:, 2] > 0.18)
    mats = np.where(lum > 0.5, SHIRT, WOOL)
    mats = np.where(tie, TIE, mats)
    in_shoe = (u > 0.72) & (v > 0.46) & (v < 0.7)
    mats = np.where(in_shoe, SHOES, mats)
    if J is not None:
        mats = cuffs(fitted, tris, mats, J)
    # The jacket a little proud of the shirt where they meet (no shirt showing through).
    wool = np.zeros(len(fitted), bool)
    wool[tris[mats == WOOL].ravel()] = True
    fitted = fitted + vnormals(fitted, tris) * (0.18 * wool)[:, None]
    return fitted, tris, mats


# The shirt's cuff showing below the jacket's sleeve, cm.
CUFF = 1.3


def cuffs(p, tris, mats, J):
    """Each sleeve's end made a tidy band (the owner's review: a white/black sawtooth at
    the wrists, where the texture told shirt from wool triangle by triangle): over the
    forearm's last centimetres the cloth is the shirt's cuff within ``CUFF`` of the wrist,
    the jacket's wool above it — rings round the forearm, not the texture's zigzag."""
    out = mats.copy()
    cen = p[tris].mean(1)
    for s in SIDES:
        wr = J[f"wrist.{s}____head"]
        el = J[f"lowerarm01.{s}____head"]
        ax = (wr - el) / np.linalg.norm(wr - el)
        along = (cen - wr) @ ax
        lo = ((p[tris] - wr) @ ax).min(1)
        radial = np.linalg.norm((cen - wr) - np.outer(along, ax), axis=1)
        near = (along > -10.0) & (along < 3.0) & (radial < 6.0) & np.isin(mats, (SHIRT, WOOL))
        # Shirt only where the whole triangle lies within the cuff (no white point up
        # into the sleeve).
        out = np.where(near & (lo > -CUFF), SHIRT, out)
        out = np.where(near & (lo <= -CUFF), WOOL, out)
    return out


def compact(fig):
    used = np.unique(fig.tris.ravel())
    remap = -np.ones(len(fig.pos), dtype=np.int64)
    remap[used] = np.arange(len(used))
    out = Fig()
    out.pos = fig.pos[used]
    out.w = [fig.w[i] for i in used]
    out.lum, out.red, out.uv = fig.lum[used], fig.red[used], fig.uv[used]
    out.tris = remap[fig.tris]
    out.mat = fig.mat
    return out


def parts(fig):
    """Each triangle's parts, a bit each: 1 the body (baked), 2 the left arm, 4 the right
    (live). A triangle goes to an arm when its corners lean on the arm's bones; a band
    round the shoulder is in both (the live arm drawn over the baked body there, so no
    seam opens when the arm moves)."""
    al = np.array([arm_share(d, "L") for d in fig.w])
    ar = np.array([arm_share(d, "R") for d in fig.w])
    ml, mr = al[fig.tris].mean(1), ar[fig.tris].mean(1)
    p = np.ones(len(fig.tris), dtype=np.int64)
    for m, bit in ((ml, 2), (mr, 4)):
        p[m > ARM_BAND[0]] |= bit
        p[m > ARM_BAND[1]] &= ~1
    return p


ARM_BAND = (0.15, 0.4)


def marks(J, fig):
    """Named points of the rest figure the scene poses by (cm): the eyes' centres, the
    chin's tip (the face's lowest point in front), the jaw's left side under the ear (where
    a violin's chinrest takes it), the shoes' soles' height."""
    m = {}
    for s_ in SIDES:
        m[f"eye.{s_}"] = J[f"eye.{s_}____head"]
    p = fig.pos
    head = np.array([bone_share(d, ("head",)) for d in fig.w]) > 0.8
    skin = np.zeros(len(p), bool)
    skin[fig.tris[fig.mat == SKIN].ravel()] = True
    mid = head & skin & (np.abs(p[:, 0]) < 0.6) & (p[:, 1] < J["eye.L____head"][1] - 5.0)
    chin = p[mid][np.argmin(p[mid][:, 1] - 0.3 * p[mid][:, 2])]
    m["chin"] = chin
    m["jaw.L"] = chin + np.array([4.6, 1.2, -5.0])
    shoe = fig.tris[fig.mat == SHOES].ravel()
    m["sole"] = np.array([0.0, p[shoe, 1].min(), 0.0])
    return m


def frame_x(y):
    """A bone's rest frame's x: across it, level, toward the figure's left for a bone
    along the body, toward the front for one across it."""
    for ref in (np.array([0.0, 0.0, 1.0]), np.array([0.0, 1.0, 0.0])):
        x = np.cross(y, ref)
        if np.linalg.norm(x) > 0.3:
            return x / np.linalg.norm(x)
    return np.array([1.0, 0.0, 0.0])


def bone_rest(sk, J):
    out = []
    for name, members, parent in BONES:
        b0 = sk["bones"][members[0]]
        last = members[0]
        # A merged chain's tail is its last link's (spine04+03, the neck's three).
        for m in members:
            if sk["bones"][m]["parent"] == last or m == members[0]:
                last = m
        if name in ("hips",):
            head, tail = J["pelvis.L____head"] * 0.5 + J["pelvis.R____head"] * 0.5, J["spine05____tail"]
        elif name.startswith("hand"):
            head, tail = J[b0["head"]], J[sk["bones"][f"metacarpal2.{name[-1]}"]["tail"]]
        else:
            head, tail = J[b0["head"]], J[sk["bones"][last]["tail"]]
        y = tail - head
        y = y / max(np.linalg.norm(y), 1e-9)
        out.append((BI[parent] if parent else -1, head, tail, frame_x(y)))
    return out


def write_fig(path, fig, rest, mk, part):
    b = bytearray(b"CFIG")
    b += struct.pack("<H", 1)
    b += struct.pack("<H", len(rest))
    for par, h, t, x in rest:
        b += struct.pack("<h9f", par, *h, *t, *x)
    b += struct.pack("<H", len(mk))
    for k, p in mk.items():
        kb = k.encode()
        b += struct.pack("<B", len(kb)) + kb + struct.pack("<3f", *p)
    n = len(fig.pos)
    assert n < 65536
    b += struct.pack("<I", n)
    q = np.round(fig.pos * 100.0).astype(np.int64)
    assert np.abs(q).max() < 32767
    for i in range(n):
        w = top4(fig.w[i])
        lum = int(np.clip(round((fig.lum[i] - 0.35) * 255.0), 0, 255))
        red = int(np.clip(round(fig.red[i] * 100.0), -127, 127))
        b += struct.pack("<3h4B4BBb2e", *q[i], *[x[0] for x in w], *[x[1] for x in w],
                         lum, red, *fig.uv[i])
    b += struct.pack("<I", len(fig.tris))
    for t, m, p in zip(fig.tris, fig.mat, part):
        b += struct.pack("<3HBB", *t, int(m), int(p))
    with open(path, "wb") as f:
        f.write(bytes(b))
    return len(b)


MAT_COL = {SKIN: (0.86, 0.68, 0.58), LIPS: (0.75, 0.45, 0.45), HAIR: (0.78, 0.64, 0.42),
           VELVET: (0.08, 0.08, 0.09), WOOL: (0.13, 0.13, 0.15), SHIRT: (0.92, 0.92, 0.9),
           TIE: (0.15, 0.2, 0.42), SHOES: (0.04, 0.04, 0.04), EYE: (0.8, 0.78, 0.75),
           IRIS: (0.3, 0.34, 0.33), LASH: (0.12, 0.09, 0.08), NAIL: (0.95, 0.78, 0.74),
           JEWEL: (0.85, 0.7, 0.35), CARD_HAIR: (0.8, 0.68, 0.45), CARD_BROW: (0.4, 0.3, 0.2),
           CARD_LASH: (0.1, 0.08, 0.06), SHELL: (0.78, 0.64, 0.42), SCALP: (0.6, 0.5, 0.35)}


def look_at(fig, name, out, part=None):
    import preview
    sel = fig.tris if part is None else fig.tris[part]
    mats = fig.mat if part is None else fig.mat[part]
    flat = sel.ravel()
    detail = np.clip(fig.lum, 0.3, 1.4)[:, None] * (1 + fig.red[:, None] * np.array([0.25, -0.15, -0.2]))
    cols = np.repeat(np.array([MAT_COL[int(m)] for m in mats]), 3, axis=0)
    cols = np.clip(cols * detail[flat], 0, 1)
    # Smooth normals from the indexed mesh, carried to the unshared corners.
    nrm = vnormals(fig.pos, fig.tris)[flat]
    ppos = fig.pos[flat]
    tris = np.arange(len(flat)).reshape(-1, 3)
    top = fig.pos[:, 1].max()
    preview.render(ppos, tris, cols, nrm=nrm, cam=((0, top * 0.3, 260), (0, top * 0.3 - 20, 0)),
                   out=os.path.join(out, f"{name}-front.png"))
    preview.render(ppos, tris, cols, nrm=nrm, cam=((0, top - 12, 48), (0, top - 13, 0)), size=(500, 600),
                   out=os.path.join(out, f"{name}-face.png"))
    preview.render(ppos, tris, cols, nrm=nrm, cam=((-40, top - 12, 30), (0, top - 13, -2)), size=(500, 600),
                   out=os.path.join(out, f"{name}-face34.png"))
    preview.render(ppos, tris, cols, nrm=nrm, cam=((0, top - 12, -48), (0, top - 13, 0)), size=(500, 600),
                   out=os.path.join(out, f"{name}-back.png"))


def main():
    global BASE
    data, assets, out = sys.argv[1:4]
    prev = sys.argv[sys.argv.index("--preview") + 1] if "--preview" in sys.argv else None
    BASE = mh.read_obj(os.path.join(data, "3dobjs", "base.obj"))
    sk, _ = mh.read_skeleton(data)
    only = next((a.split("=", 1)[1] for a in sys.argv if a.startswith("--only=")), None)
    for sex, name in (("female", "woman"), ("male", "man")):
        if only and name != only:
            continue
        fig, J = make(sex, data, assets)
        part = parts(fig)
        # The live arms thinned (sheet v7's frame budget): short edges collapsed inside
        # each arm's own part, so a hand is not three thousand triangles.
        n0 = len(fig.tris)
        movable = ((part & 1) == 0) & ((part & 6) != 0)
        # (Never the fingertips: thinned, a fingertip's round pad folded to a point — the
        # owner's review: claws.)
        tipv = np.array([bone_share(d, tuple(f"f{f}_3.{s_}" for f in range(1, 6)
                                             for s_ in SIDES)) for d in fig.w])
        movable &= tipv[fig.tris].max(1) < 0.3
        # (r42:arcus-polish4: nor the sleeves — made finer for their wrinkles.)
        movable &= ~np.isin(fig.mat, (VELVET, WOOL))
        uv = [fig.uv[:, 0], fig.uv[:, 1]]
        p2, w2, ex, t2, m2, part = thin.thin(fig.pos, fig.w, [fig.lum, fig.red] + uv,
                                              fig.tris, fig.mat, part, THIN, movable)
        fig.pos, fig.w, fig.tris, fig.mat = p2, w2, t2, m2
        fig.lum, fig.red, fig.uv = ex[0], ex[1], np.stack([ex[2], ex[3]], 1)
        print(f"{name}: arms thinned, {n0} -> {len(fig.tris)} triangles")
        rest = bone_rest(sk, J)
        cards(assets, out, name)
        skin_texture(assets, out, name, sex)
        eye_texture(assets, out, name)
        size = write_fig(os.path.join(out, f"{name}.fig"), fig, rest, marks(J, fig), part)
        print(f"{name}: {len(fig.pos)} vertices, {len(fig.tris)} triangles, {size} bytes")
        if prev:
            look_at(fig, name, prev)



def cut(pos, w, extra, tris, f):
    """Split the triangles that ``f`` (a value per vertex) changes sign across, along its
    zero line, so a material's edge (a neckline, a cuff, a hairline) follows a smooth curve
    instead of the mesh's staircase. New vertices interpolate the positions, the weights
    and every array in ``extra``. Returns (pos, w, extra, tris, f)."""
    pos = list(pos)
    w = list(w)
    extra = [list(e) for e in extra]
    f = list(f)
    made = {}

    def mid(a, b):
        key = (min(a, b), max(a, b))
        if key in made:
            return made[key]
        t = f[a] / (f[a] - f[b])
        pos.append(pos[a] + (pos[b] - pos[a]) * t)
        wa, wb = w[a], w[b]
        ww = {}
        for k in set(wa) | set(wb):
            ww[k] = wa.get(k, 0.0) * (1 - t) + wb.get(k, 0.0) * t
        w.append(ww)
        for e in extra:
            e.append(e[a] * (1 - t) + e[b] * t)
        f.append(0.0)
        made[key] = len(pos) - 1
        return made[key]

    out = []
    for tr in tris:
        s = [f[i] > 0 for i in tr]
        if all(s) or not any(s) or any(abs(f[i]) < 1e-9 for i in tr):
            out.append(list(tr))
            continue
        # The lone vertex (the one on its own side) first, the order kept.
        k = [i for i in range(3) if s[i] != s[(i + 1) % 3] and s[i] != s[(i + 2) % 3]][0]
        a, b, c = tr[k], tr[(k + 1) % 3], tr[(k + 2) % 3]
        ab, ac = mid(a, b), mid(a, c)
        out += [[a, ab, ac], [ab, b, c], [ab, c, ac]]
    return (np.array(pos), w, [np.array(e) for e in extra], np.array(out, dtype=np.int64),
            np.array(f))


if __name__ == "__main__":
    main()
