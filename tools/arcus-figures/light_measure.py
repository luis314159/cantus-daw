"""r42:arcus-polish3 — the concert light measured: the photographs the owner chose (ref-04,
ref-06, ref-10, study only, never committed) against the app's own pictures.

    python tools/arcus-figures/light_measure.py refs <refs dir>
    python tools/arcus-figures/light_measure.py shot <png> <who>   (a shoot_arcus window shot)
    python tools/arcus-figures/light_measure.py scene <png> <who>  (a --scene picture)

For each picture: the face's luminance (linear, the mean of its skin), its warmth (R/B and
R/G of the mean), the background's luminance (its mean and its 90th percentile: the hall's
bokeh), the face over the background, the face's two sides (lit third over shadow third:
the modelling), the hair's edge and the shoulders' (the rim), the picture's contrast (p99
over p5). Regions are fractions of the picture (x0, y0, x1, y1), read off gridded copies.
The bands in tests/arcus_light_measure.rs come from these numbers.
"""

import sys

import numpy as np
from PIL import Image

REFS = {
    "ref-04-front-soloist-hahn.jpg": {
        "face": (0.40, 0.27, 0.53, 0.45),
        "bg": [(0.0, 0.0, 0.30, 0.20), (0.75, 0.0, 1.0, 0.30)],
        "hair": (0.31, 0.22, 0.38, 0.44),
        "shoulder": (0.22, 0.52, 0.30, 0.60),
    },
    "ref-06-front-soloist-khoury.jpg": {
        "face": (0.43, 0.24, 0.56, 0.42),
        "bg": [(0.0, 0.0, 0.35, 0.30), (0.80, 0.0, 1.0, 0.40)],
        "hair": (0.41, 0.10, 0.57, 0.20),
        "shoulder": (0.26, 0.28, 0.40, 0.40),
    },
    "ref-10-front-soloist-raychen-orch.jpg": {
        "face": (0.47, 0.18, 0.57, 0.33),
        "bg": [(0.0, 0.0, 0.45, 0.18), (0.60, 0.0, 0.85, 0.20)],
        "hair": (0.46, 0.11, 0.58, 0.18),
        "shoulder": (0.36, 0.38, 0.47, 0.50),
    },
}

# Our pictures: fractions of the picture in a window shot, for each figure.
SCENE = {
    "woman": {
        "face": (0.30, 0.17, 0.43, 0.36),
        "bg": [(0.0, 0.0, 0.18, 0.38), (0.72, 0.0, 1.0, 0.35)],
        "hair": (0.22, 0.07, 0.44, 0.20),
        "shoulder": (0.10, 0.47, 0.22, 0.58),
    },
    "man": {
        "face": (0.37, 0.17, 0.45, 0.37),
        "bg": [(0.0, 0.0, 0.18, 0.38), (0.72, 0.0, 1.0, 0.35)],
        "hair": (0.26, 0.05, 0.45, 0.18),
        "shoulder": (0.05, 0.37, 0.20, 0.47),
    },
}
# A window shot's picture (Arcus's Play at 100 %: points of a 1040 x 680 window).
PICTURE = (8 / 1040, 62 / 680, 600 / 1040, 490 / 680)


def lin(a):
    a = a / 255.0
    return np.where(a <= 0.04045, a / 12.92, ((a + 0.055) / 1.055) ** 2.4)


def lum(c):
    return 0.2126 * c[..., 0] + 0.7152 * c[..., 1] + 0.0722 * c[..., 2]


def region(img, box):
    h, w = img.shape[:2]
    x0, y0, x1, y1 = box
    return img[int(y0 * h) : int(y1 * h), int(x0 * w) : int(x1 * w)]


def skin_mask(c):
    """Skin-like pixels in linear `c`: red over green over blue, not too dark."""
    r, g, b = c[..., 0], c[..., 1], c[..., 2]
    y = lum(c)
    return (r > g * 1.05) & (g > b * 0.95) & (y > 0.01) & (r < 1.0)


def measure(path, regions, crop=None):
    im = np.asarray(Image.open(path).convert("RGB")).astype(np.float64)
    if crop:
        h, w = im.shape[:2]
        im = im[int(crop[1] * h) : int(crop[3] * h), int(crop[0] * w) : int(crop[2] * w)]
    c = lin(im)
    out = {}
    face = region(c, regions["face"])
    m = skin_mask(face)
    fs = face[m] if m.sum() > 50 else face.reshape(-1, 3)
    fy = lum(fs)
    out["face_y"] = float(np.mean(fy))
    mean = fs.mean(axis=0)
    out["face_r_b"] = float(mean[0] / max(mean[2], 1e-6))
    out["face_r_g"] = float(mean[0] / max(mean[1], 1e-6))
    # The face's modelling: its skin's 85th percentile over its 15th.
    out["face_p85_p15"] = float(np.percentile(fy, 85) / max(np.percentile(fy, 15), 1e-5))
    # Its two sides: the skin's columns in thirds.
    cols = np.where(m.any(axis=0))[0]
    if len(cols) > 9:
        third = (cols[-1] - cols[0]) // 3
        ys = []
        for k in range(3):
            a = cols[0] + k * third
            sub = face[:, a : a + third]
            sm = m[:, a : a + third]
            ys.append(float(lum(sub[sm]).mean()) if sm.sum() > 10 else 0.0)
        lo, hi = min(ys[0], ys[2]), max(ys[0], ys[2])
        out["face_sides"] = hi / max(lo, 1e-5)
    bg = np.concatenate([lum(region(c, b)).ravel() for b in regions["bg"]])
    out["bg_y"] = float(np.mean(bg))
    out["bg_p90"] = float(np.percentile(bg, 90))
    out["face_over_bg"] = out["face_y"] / max(out["bg_y"], 1e-5)
    hair = lum(region(c, regions["hair"]))
    out["hair_p95"] = float(np.percentile(hair, 95))
    out["hair_p95_over_bg"] = out["hair_p95"] / max(out["bg_y"], 1e-5)
    sh = lum(region(c, regions["shoulder"]))
    out["shoulder_p95_over_p50"] = float(np.percentile(sh, 95) / max(np.percentile(sh, 50), 1e-5))
    y = lum(c)
    out["p5"] = float(np.percentile(y, 5))
    out["p50"] = float(np.percentile(y, 50))
    out["p99"] = float(np.percentile(y, 99))
    out["contrast_p99_p5"] = out["p99"] / max(out["p5"], 1e-5)
    return out


def show(name, d):
    print(name)
    for k, v in d.items():
        print(f"  {k:24s} {v:8.4f}")


def main():
    mode = sys.argv[1]
    if mode == "refs":
        import os

        for f, reg in REFS.items():
            show(f, measure(os.path.join(sys.argv[2], f), reg))
    elif mode == "scene":
        show(sys.argv[2], measure(sys.argv[2], SCENE[sys.argv[3]]))
    elif mode == "shot":
        show(sys.argv[2], measure(sys.argv[2], SCENE[sys.argv[3]], crop=PICTURE))


if __name__ == "__main__":
    main()
