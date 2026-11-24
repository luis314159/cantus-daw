"""r42:arcus-upper — reading MakeHuman 1.x assets (CC0 1.0: the base mesh, its targets, the
default skeleton and its weights, the clothes and their materials). Only the *assets* are
used; no MakeHuman code is copied (the program is AGPL). Every format read here is a plain
text format, documented by the files themselves:

- ``base.obj``: Wavefront OBJ (``v``, ``vt``, ``f a/ta b/tb …``, ``g group``), decimetres,
  y up, the figure facing +z, its left on +x.
- ``*.target``: lines ``index dx dy dz`` (a morph's offsets, decimetres).
- ``default.mhskel`` (JSON): ``joints`` name → vertex indices (a joint is their mean),
  ``bones`` name → head / tail joint names, parent, rotation plane.
- ``default_weights.mhw`` (JSON): ``weights`` bone → ``[[vertex, weight], …]``.
- ``*.mhclo``: a proxy (clothes, hair) fitted to the base mesh: per proxy vertex either one
  base vertex or three with barycentric weights and an offset scaled by the body's size
  (``x_scale a b d``: the offset's x is scaled by ``|P[a].x - P[b].x| / d``);
  ``delete_verts`` the base vertices it hides (ranges ``a - b``).
"""

import json
import os

import numpy as np


def read_obj(path):
    """(positions N×3, uvs M×2, faces [(vertex ids, uv ids, group, material)])."""
    vs, vts, faces = [], [], []
    group, mtl = "", ""
    with open(path, encoding="utf-8", errors="replace") as f:
        for line in f:
            if line.startswith("v "):
                vs.append([float(x) for x in line.split()[1:4]])
            elif line.startswith("vt "):
                vts.append([float(x) for x in line.split()[1:3]])
            elif line.startswith("g "):
                group = line.split(maxsplit=1)[1].strip()
            elif line.startswith("usemtl "):
                mtl = line.split(maxsplit=1)[1].strip()
            elif line.startswith("f "):
                vi, ti = [], []
                for tok in line.split()[1:]:
                    parts = tok.split("/")
                    vi.append(int(parts[0]) - 1)
                    ti.append(int(parts[1]) - 1 if len(parts) > 1 and parts[1] else -1)
                faces.append((vi, ti, group, mtl))
    return np.array(vs, dtype=np.float64), np.array(vts, dtype=np.float64), faces


def read_target(path):
    idx, d = [], []
    with open(path, encoding="utf-8", errors="replace") as f:
        for line in f:
            if not line.strip() or line.startswith("#"):
                continue
            p = line.split()
            idx.append(int(p[0]))
            d.append([float(p[1]), float(p[2]), float(p[3])])
    return np.array(idx, dtype=np.int64), np.array(d, dtype=np.float64)


def apply_targets(pos, root, targets):
    """``targets``: [(path relative to data/targets, weight)]."""
    out = pos.copy()
    for rel, w in targets:
        if w == 0.0:
            continue
        i, d = read_target(os.path.join(root, "targets", rel))
        if len(i) == 0:
            continue
        out[i] += d * w
    return out


def read_skeleton(root):
    with open(os.path.join(root, "rigs", "default.mhskel"), encoding="utf-8") as f:
        sk = json.load(f)
    with open(os.path.join(root, "rigs", "default_weights.mhw"), encoding="utf-8") as f:
        wt = json.load(f)
    return sk, wt["weights"]


def joint_positions(sk, pos):
    return {name: pos[np.array(ids)].mean(axis=0) for name, ids in sk["joints"].items()}


def read_mhclo(path):
    """(obj path, refs [(ids[3], weights[3], offset[3])], scales {axis: (a, b, d)},
    deleted base vertex ids, material path, uuid/name tags)."""
    refs, scales, deleted = [], {}, []
    obj, mat = None, None
    mode = None
    with open(path, encoding="utf-8", errors="replace") as f:
        for line in f:
            s = line.strip()
            if not s or s.startswith("#"):
                continue
            p = s.split()
            if p[0] == "obj_file":
                obj = p[1]
                mode = None
            elif p[0] == "material":
                mat = p[1]
            elif p[0] in ("x_scale", "y_scale", "z_scale"):
                scales[p[0][0]] = (int(p[1]), int(p[2]), float(p[3]))
            elif p[0] == "verts":
                mode = "verts"
            elif p[0] == "delete_verts":
                mode = "delete"
            elif p[0] in ("weights", "tag", "name", "uuid", "basemesh", "z_depth",
                          "max_pole", "special_pose", "offset", "license", "author",
                          "homepage", "description", "vertexboneweights_file"):
                mode = None if p[0] == "weights" else mode
            elif mode == "verts":
                if len(p) == 1:
                    refs.append(([int(p[0])] * 3, [1.0, 0.0, 0.0], [0.0, 0.0, 0.0]))
                else:
                    refs.append(([int(x) for x in p[0:3]], [float(x) for x in p[3:6]],
                                 [float(x) for x in p[6:9]]))
            elif mode == "delete":
                toks = s.replace("-", " - ").split()
                k = 0
                while k < len(toks):
                    if k + 2 < len(toks) and toks[k + 1] == "-":
                        deleted.extend(range(int(toks[k]), int(toks[k + 2]) + 1))
                        k += 3
                    else:
                        deleted.append(int(toks[k]))
                        k += 1
    return obj, refs, scales, deleted, mat


def fit_proxy(refs, scales, pos):
    """The proxy's vertices on the shaped body ``pos``."""
    sc = np.ones(3)
    for ax, k in (("x", 0), ("y", 1), ("z", 2)):
        if ax in scales:
            a, b, d = scales[ax]
            sc[k] = abs(pos[a][k] - pos[b][k]) / d
    out = np.zeros((len(refs), 3))
    for i, (ids, w, off) in enumerate(refs):
        out[i] = (pos[ids[0]] * w[0] + pos[ids[1]] * w[1] + pos[ids[2]] * w[2]
                  + np.array(off) * sc)
    return out


def proxy_weights(refs, base_w):
    """Each proxy vertex's bone weights from its base vertices': ``base_w`` is a list of
    {bone: weight} per base vertex."""
    out = []
    for ids, w, _ in refs:
        acc = {}
        for vid, ww in zip(ids, w):
            if ww <= 0.0:
                continue
            for b, x in base_w[vid].items():
                acc[b] = acc.get(b, 0.0) + x * ww
        out.append(acc)
    return out
