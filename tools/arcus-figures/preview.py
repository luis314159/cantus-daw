"""r42:arcus-upper — a small z-buffer rasteriser for looking at a figure while it is built
(``build_figures.py --preview``): smooth normals, one key light, a colour per role, PNG out.
Not the product's renderer — only the converter's eyes."""

import numpy as np
from PIL import Image


def normals(pos, tris):
    n = np.zeros_like(pos)
    a, b, c = pos[tris[:, 0]], pos[tris[:, 1]], pos[tris[:, 2]]
    fn = np.cross(b - a, c - a)
    for k in range(3):
        np.add.at(n, tris[:, k], fn)
    ln = np.linalg.norm(n, axis=1, keepdims=True)
    return n / np.maximum(ln, 1e-12)


def look(eye, at, up=(0.0, 1.0, 0.0)):
    eye, at, up = np.array(eye, float), np.array(at, float), np.array(up, float)
    f = at - eye
    f /= np.linalg.norm(f)
    r = np.cross(f, up)
    r /= np.linalg.norm(r)
    u = np.cross(r, f)
    return eye, r, u, f


def render(pos, tris, cols, cam, size=(600, 800), fov=1.0, key=(-0.4, 0.7, 0.6),
           out="preview.png", bg=(40, 40, 46), nrm=None):
    """``cols``: per-vertex RGB 0..1 (N×3). ``cam``: (eye, at)."""
    w, h = size
    eye, r, u, f = look(*cam)
    if nrm is None:
        nrm = normals(pos, tris)
    rel = pos - eye
    z = rel @ f
    s = h / (2.0 * np.tan(fov * 0.5))
    sx = w * 0.5 + (rel @ r) / np.maximum(z, 1e-3) * s
    sy = h * 0.5 - (rel @ u) / np.maximum(z, 1e-3) * s
    keyv = np.array(key, float)
    keyv /= np.linalg.norm(keyv)
    lam = np.clip(nrm @ keyv, 0.0, 1.0)
    wrap = np.clip((nrm @ keyv + 0.35) / 1.35, 0.0, 1.0)
    shade = 0.25 + 0.75 * (0.6 * lam + 0.4 * wrap)
    vc = np.clip(cols * shade[:, None], 0, 1)
    img = np.zeros((h, w, 3)) + np.array(bg) / 255.0
    zb = np.full((h, w), np.inf)
    for t in tris:
        xs, ys, zs = sx[t], sy[t], z[t]
        if np.any(zs <= 1e-3):
            continue
        x0, x1 = int(max(np.floor(xs.min()), 0)), int(min(np.ceil(xs.max()), w - 1))
        y0, y1 = int(max(np.floor(ys.min()), 0)), int(min(np.ceil(ys.max()), h - 1))
        if x0 > x1 or y0 > y1:
            continue
        gx, gy = np.meshgrid(np.arange(x0, x1 + 1) + 0.5, np.arange(y0, y1 + 1) + 0.5)
        (ax, bx, cx), (ay, by, cy) = xs, ys
        den = (by - cy) * (ax - cx) + (cx - bx) * (ay - cy)
        if abs(den) < 1e-9:
            continue
        l0 = ((by - cy) * (gx - cx) + (cx - bx) * (gy - cy)) / den
        l1 = ((cy - ay) * (gx - cx) + (ax - cx) * (gy - cy)) / den
        l2 = 1.0 - l0 - l1
        m = (l0 >= 0) & (l1 >= 0) & (l2 >= 0)
        if not m.any():
            continue
        zz = l0 * zs[0] + l1 * zs[1] + l2 * zs[2]
        sub = zb[y0:y1 + 1, x0:x1 + 1]
        m &= zz < sub
        if not m.any():
            continue
        sub[m] = zz[m]
        c = (l0[..., None] * vc[t[0]] + l1[..., None] * vc[t[1]] + l2[..., None] * vc[t[2]])
        img[y0:y1 + 1, x0:x1 + 1][m] = c[m]
    Image.fromarray((np.clip(img, 0, 1) ** (1 / 1.0) * 255).astype(np.uint8)).save(out)
