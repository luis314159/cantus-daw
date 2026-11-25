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
        ("head/head-scale-horiz-decr", 0.22),
        ("head/head-scale-vert-incr", 0.05),
        ("forehead/forehead-scale-vert-decr", 0.3),
        *both("cheek/cheek-bones-incr", 0.3),
        *both("cheek/cheek-volume-decr", 0.25),
        *both("cheek/cheek-inner-decr", 0.2),
        *both("eyes/eye-trans-in", 0.22),
        *both("eyes/eye-scale-decr", 0.18),
        *both("eyes/eye-corner1-up", 0.25),
        *both("eyes/eye-height2-decr", 0.15),
        ("chin/chin-width-decr", 0.3),
        ("chin/chin-height-incr", 0.25),
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
    ],
    "male": [
        ("head/head-square", 0.3),
        ("head/head-fat-decr", 0.2),
        *both("cheek/cheek-bones-incr", 0.35),
        *both("cheek/cheek-volume-decr", 0.35),
        ("chin/chin-bones-incr", 0.45),
        ("chin/chin-width-incr", 0.15),
        ("chin/chin-prominent-incr", 0.25),
        ("nose/nose-hump-decr", 0.2),
        ("nose/nose-point-width-decr", 0.2),
        ("nose/nose-scale-horiz-decr", 0.1),
        ("mouth/mouth-lowerlip-volume-incr", 0.15),
        ("mouth/mouth-upperlip-volume-decr", 0.1),
        ("eyebrows/eyebrows-trans-down", 0.2),
        ("eyebrows/eyebrows-angle-down", 0.1),
    ],
}


def rot_x(p, c, a):
    q = p - c
    ca, sa = np.cos(a), np.sin(a)
    return c + np.stack([q[:, 0], q[:, 1] * ca - q[:, 2] * sa, q[:, 1] * sa + q[:, 2] * ca], 1)


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


UPPER_LID = 27.0
LOWER_LID = 6.0


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
HER_LINE = [(0, 31), (20, 32), (38, 27), (55, 19), (70, 12), (86, 9), (102, 5), (120, -14),
            (145, -36), (165, -44), (180, -46)]
HIS_LINE = [(0, 30), (16, 31), (30, 33), (42, 28), (55, 20), (68, 12), (80, 2), (90, 0),
            (100, 8), (115, -2), (135, -20), (160, -27), (180, -28)]


def hair_shell(pos, W, tris, c, line, thick, head_w, band=16.0):
    """A shell over the scalp: the head's skin above ``line`` (cut along it, so the
    hairline is a smooth curve), pushed out along its normal by ``thick(θ, φ)`` tapering
    to nothing at the hairline, so its edge lies on the skin. Returns (positions, weights,
    triangles, strand coordinates) in its own numbering."""
    near = tris[(head_w[tris] > 0.2).all(1)]
    th, ph = angles(pos, c)
    f = ph - hairline(th, line)
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
    t = thick(th2, ph2) * smooth01(f2 / 22.0) + 0.03 + 0.11 * smooth01(f2 / 8.0)
    hp = (p2 + n * t[:, None])[used]
    # Per vertex (sheet v7): the strands' across coordinate — the angle round the head's
    # front-to-back axis, so the strands run as hair drawn back does, from the hairline over
    # the head to the nape (the picture's grooves and sheen follow it) — and how far in
    # from the hairline (0 at it, 1 beyond 2.5 cm: the hair's edge blends into the skin, as
    # real hair thins there).
    q = p2[used] - c
    # (Folded under the head's middle, so no seam runs down the nape.)
    across = np.arctan2(q[:, 0], np.abs(q[:, 1]) + 0.5) * 9.0
    uv = np.stack([across, smooth01(f2[used] / band)], 1)
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
    W = scene_weights(len(pos), wt, m2s)
    tris_all, _ = triangulate(faces)
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
    btris, keep_v = body_parts(pos, faces, tris_all, tri_groups, deleted)
    lum, red = skin_detail(pos, faces, uvs, skin_tex, keep_v & (head_w > 0.5))
    lum = lash_line(pos, btris, lum, J, wt)
    brow = brows(pos, keep_v, J, sex)
    lum = lum * (1.0 - (0.5 if sex == "female" else 0.66) * brow)
    red = red - 0.15 * brow
    # Cheeks, lips, the nose's tip and the ears a little warmer (sheet v7: a face with life).
    red = red + 0.18 * warmth(pos, J, sex)
    lash_p, lash_t = lashes(pos, wt, J)
    # The nails: the body cut along each nail's edge (sheet v7).
    nf = nail_field(pos, W, sk, J)
    pos, W, (lum, red), btris, nf = cut(pos, W, [lum, red], btris, nf)
    head_w = np.array([d.get(BI["head"], 0.0) / max(sum(d.values()), 1e-9) for d in W])
    if sex == "female":
        # Swept back with soft volume at the crown (the owner's reference's styling).
        def thick(th, ph):
            return (0.8 + 0.4 * smooth01((ph - 28) / 30.0)
                    + 0.45 * smooth01((np.abs(th) - 60) / 90.0)
                    + 0.75 * smooth01((ph - 42) / 22.0) * (1.0 - smooth01((np.abs(th) - 140) / 35.0)))
        hp, hw, ht, huv = hair_shell(pos, W, btris, c, HER_LINE, thick, head_w, band=16.0)
        bp, bw, bt, bmat, blum, bred = dress(pos, W, btris, J, lum, red, nf)
        fig.add(bp, bw, bt, bmat, blum, bred)
        fig.add(hp, hw, ht, HAIR, uv=huv)
        # Tied low: the bun at the nape, its strands wound round it.
        bc = c + np.array([0.0, -6.6, -9.6])
        ep, et, euv = ellipsoid(bc, np.eye(3), (4.3, 3.2, 3.0), twist=1.5)
        et = outward(ep, et, bc)
        fig.add(ep, [{BI["head"]: 1.0}] * len(ep), et, HAIR, uv=euv)
        # Small drop earrings: a bead at each lobe, a drop under it.
        for jp, jt in earrings(pos, head_w, c, J):
            fig.add(jp, [{BI["head"]: 1.0}] * len(jp), jt, JEWEL)
    else:
        def thick(th, ph):
            return (0.45 + 0.55 * smooth01((ph - 25) / 30.0)
                    - 0.15 * smooth01((np.abs(th) - 75) / 45.0))
        # His short hair's edge crisper (a groomed hairline; a wide band read as teeth).
        hp, hw, ht, huv = hair_shell(pos, W, btris, c, HIS_LINE, thick, head_w, band=6.0)
        fig.add(pos, W, btris, np.where(nf[btris].mean(1) > 0, NAIL, SKIN), lum, red)
        fig.add(hp, hw, ht, HAIR, uv=huv)
        sp, st, smat = suit(assets, obj, refs, scales, pos)
        fig.add(sp, mh.proxy_weights(refs, W), st, smat)
    fig.add(shp, shw, sht, SHOES)
    for s_ in SIDES:
        ep, et, emat = eyeball(J[f"eye.{s_}____head"])
        fig.add(ep, [{BI["head"]: 1.0}] * len(ep), et, emat)
    fig.add(lash_p, [{BI["head"]: 1.0}] * len(lash_p), lash_t, LASH)
    return compact(fig), J


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
    full = 1.0 if sex == "female" else 1.35
    for s_, sign in (("L", 1.0), ("R", -1.0)):
        e = J[f"eye.{s_}____head"]
        x = (pos[:, 0] - e[0]) * sign
        y = pos[:, 1] - e[1]
        front = (pos[:, 2] > e[2] - 1.5) & keep_v
        x0, x1 = -1.45, 2.55
        t = np.clip((x - x0) / (x1 - x0), 0.0, 1.0)
        rise = 0.42 if sex == "female" else 0.22
        yc = 1.25 + rise * np.sin(np.pi * np.clip(t, 0, 1) ** 0.75) - 0.25 * t
        half = (0.26 - 0.15 * t) * full
        across = 1.0 - smooth01((np.abs(y - yc) - half) / 0.22 + 0.5)
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
        (el + np.array([0.6, -3.0, 0.6]), 2.2, 0.8),
        (er + np.array([-0.6, -3.0, 0.6]), 2.2, 0.8),
        (mid + np.array([0.0, -4.0, 2.6]), 1.0, 0.5),
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
    """An eye under its lowered lid: a ball in the socket, looking down 32° toward the
    instrument, its iris (a grey-green disc 30° across its half) and the white round it."""
    r = 1.18
    gaze = np.array([0.0, -np.sin(np.radians(32.0)), np.cos(np.radians(32.0))])
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
    return pts, tri, np.array(mat)


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


def lips(pos, tris, J):
    return np.zeros(len(tris), bool)


def bone_share(d, names):
    tot = sum(d.values()) or 1.0
    return sum(w for b, w in d.items() if BONES[b][0] in names) / tot


def dress(pos, W, tris, J, lum, red, nail):
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
        depth = 15.0 * np.clip(1.0 - ax / 9.6, 0.0, 1.0) ** 1.1 + 1.0
        scoop = np.minimum(np.minimum(z + 1.5, y + depth), 8.4 - np.abs(x))
        g_neck = np.maximum(10.0 * (hn - 0.5), scoop)
        g_hand = 10.0 * (np.array([hand_share(d) for d in w]) - 0.55)
        return g_neck, g_hand

    # The uncut body's normals carried through the cuts (a sliver's own normal pushed the
    # velvet out in fins along the neckline).
    n0 = vnormals(pos, tris)
    p, w, ex, t = pos, W, [lum, red, nail, n0[:, 0], n0[:, 1], n0[:, 2]], tris
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
    puff = (0.55 * smooth01((cap - 0.35) / 0.4) * np.clip(nrm[:, 1] * 1.4, 0.0, 1.0)
            * smooth01(-skin_v / 4.0))
    lift = (0.22 + puff) * smooth01(-skin_v / 0.8)
    return p + nrm * lift[:, None], w, t, mats, ex[0], ex[1]


def suit(assets, obj, refs, scales, pos):
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
    # The jacket a little proud of the shirt where they meet (no shirt showing through).
    wool = np.zeros(len(fitted), bool)
    wool[tris[mats == WOOL].ravel()] = True
    fitted = fitted + vnormals(fitted, tris) * (0.18 * wool)[:, None]
    return fitted, tris, mats


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
           JEWEL: (0.85, 0.7, 0.35)}


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
        uv = [fig.uv[:, 0], fig.uv[:, 1]]
        p2, w2, ex, t2, m2, part = thin.thin(fig.pos, fig.w, [fig.lum, fig.red] + uv,
                                              fig.tris, fig.mat, part, THIN, movable)
        fig.pos, fig.w, fig.tris, fig.mat = p2, w2, t2, m2
        fig.lum, fig.red, fig.uv = ex[0], ex[1], np.stack([ex[2], ex[3]], 1)
        print(f"{name}: arms thinned, {n0} -> {len(fig.tris)} triangles")
        rest = bone_rest(sk, J)
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
