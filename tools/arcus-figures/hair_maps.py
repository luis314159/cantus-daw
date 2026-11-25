"""r42:arcus-hair — a hair card texture made into one head of hair's maps (the owner's
review of sheet v8: her ponytail cards rendered as camouflage — MakeHuman's texture carries
large baked light and dark regions, and the bake read them as patches of another colour).

From the RGBA texture (grey strands over alpha) it keeps only what belongs to a strand:

- **detail** (R): the grey over its own alpha-weighted blur (σ ``LOW`` texels) — the strand
  to strand variation, never the texture's large-scale light; compressed toward 1 and
  stored as 128 × detail (1.0 = 128).
- **the strands' way** (G, B): from the structure tensor of the grey (its gradients'
  outer product blurred over σ ``WAY`` texels) the way across which the grey changes
  least — along the strands — stored as its doubled angle's cosine and sine in image
  space (x right, y down), 128 + 127 × (cos 2θ, sin 2θ): a line has no sign, and the
  doubled angle interpolates without a seam.
- **coverage** (A): the alpha as it is (the bake takes it to coverage over its samples).

    python tools/arcus-figures/hair_maps.py <in.png> <out.png> [--view <dir>]

``build_figures.py::cards`` calls ``maps`` for each hair card.
"""

import sys

import numpy as np
from PIL import Image

# The large-scale light taken away (texels at 1024): a strand bundle is ~3-8 texels wide.
LOW = 10.0
# The strands' way smoothed over (texels).
WAY = 5.0
# How much of the strand-to-strand variation is kept, and its range.
KEEP = 1.5
LO, HI = 0.55, 1.45


def gblur(img, sigma):
    """A Gaussian blur through the FFT (the texture wraps, as its coordinates do)."""
    h, w = img.shape
    fy = np.fft.fftfreq(h)[:, None]
    fx = np.fft.rfftfreq(w)[None, :]
    k = np.exp(-2.0 * (np.pi * sigma) ** 2 * (fx ** 2 + fy ** 2))
    return np.fft.irfft2(np.fft.rfft2(img) * k, s=img.shape)


def maps(rgba):
    """``rgba`` an (h, w, 4) uint8 array: the four channels described above."""
    a = rgba[..., 3].astype(np.float64) / 255.0
    g = rgba[..., :3].astype(np.float64).mean(2)
    low = gblur(g * a, LOW) / np.maximum(gblur(a, LOW), 1e-3)
    ratio = g / np.maximum(low, 6.0)
    detail = np.clip(1.0 + KEEP * (ratio - 1.0), LO, HI)
    # Where nothing covers, plain 1 (a bilinear look-up at a strand's edge stays even).
    detail = np.where(a > 0.02, detail, 1.0)
    # The strands' way: the grey against black where bare, so a strand's edges count too.
    f = g * a
    gx = (np.roll(f, -1, 1) - np.roll(f, 1, 1)) * 0.5
    gy = (np.roll(f, -1, 0) - np.roll(f, 1, 0)) * 0.5
    jxx, jxy, jyy = gblur(gx * gx, WAY), gblur(gx * gy, WAY), gblur(gy * gy, WAY)
    # The gradient's doubled angle; the strands run square to it (θ + 90°: the doubled
    # angle turned by 180°, so both its cosine and sine change sign).
    c2 = -(jxx - jyy)
    s2 = -2.0 * jxy
    m = np.hypot(c2, s2)
    c2 = np.where(m > 1e-9, c2 / np.maximum(m, 1e-12), 1.0)
    s2 = np.where(m > 1e-9, s2 / np.maximum(m, 1e-12), 0.0)
    out = np.empty(rgba.shape, dtype=np.uint8)
    out[..., 0] = np.clip(np.round(detail * 128.0), 0, 255)
    out[..., 1] = np.clip(np.round(128.0 + 127.0 * c2), 1, 255)
    out[..., 2] = np.clip(np.round(128.0 + 127.0 * s2), 1, 255)
    out[..., 3] = rgba[..., 3]
    return out


def view(out, path):
    """A picture of the maps to look at: the detail over mid grey, the way as hue."""
    d = out[..., 0].astype(np.float64)
    a = out[..., 3:4].astype(np.float64) / 255.0
    ang = np.arctan2(out[..., 2] / 127.0 - 1.0, out[..., 1] / 127.0 - 1.0) * 0.5
    hue = np.stack([0.5 + 0.5 * np.cos(2 * ang), 0.5 + 0.5 * np.cos(2 * ang - 2.1),
                    0.5 + 0.5 * np.cos(2 * ang + 2.1)], 2) * 255.0
    left = np.repeat(((d - 128.0) * 3.0 + 128.0)[..., None], 3, 2) * a + 60 * (1 - a)
    right = hue * a + 60 * (1 - a)
    Image.fromarray(np.clip(np.hstack([left, right]), 0, 255).astype(np.uint8)).save(path)


if __name__ == "__main__":
    src = np.array(Image.open(sys.argv[1]).convert("RGBA"))
    res = maps(src)
    Image.fromarray(res, "RGBA").save(sys.argv[2], optimize=True)
    if "--view" in sys.argv:
        view(res, sys.argv[sys.argv.index("--view") + 1])
