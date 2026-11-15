"""The overhead pair's L/R correlation for the studio kit's cymbals, the engine's path
rules (direct + six image sources, cardioid, the plate's dipole with its sign behind,
1/r), for white noise: the pair as designed (spaced 56 cm) against near-coincident ones.

    python -I stereo_sim.py
"""
import numpy as np
from scipy import signal

SR = 48000
C = 343.0
ROOM = dict(w=6.0, d=8.0, h=3.4, back=2.7, reflect=0.7, floor=0.75)


def eng(p):  # design (x, y up, z) -> engine (x, y, z up), metres
    return np.array([p[0], p[2], p[1]])


PIECES = {  # name: (design centre, design normal)
    "crash1": ([0.44, 1.38, 0.24], [-0.0671, 0.9397, -0.3354]),
    "crash2": ([-0.36, 1.46, 0.3], [0.0983, 0.9397, -0.3276]),
    "ride": ([-0.78, 1.1, 0.14], [0.1082, 0.9703, -0.2164]),
    "hat": ([0.66, 0.92, -0.04], [0.0, 0.9994, -0.0349]),
}
OH = eng([0.0, 1.96, -0.04])
AIM = np.array([0.0, 0.0, -1.0])


def images(s):
    w, d, h, b = ROOM["w"], ROOM["d"], ROOM["h"], ROOM["back"]
    r, f = ROOM["reflect"], ROOM["floor"]
    return [(s, 1.0), (np.array([s[0], s[1], -s[2]]), f), (np.array([s[0], s[1], 2 * h - s[2]]), r),
            (np.array([w - s[0], s[1], s[2]]), r), (np.array([-w - s[0], s[1], s[2]]), r),
            (np.array([s[0], -2 * b - s[1], s[2]]), r), (np.array([s[0], 2 * (d - b) - s[1], s[2]]), r)]


def paths(src, normal, cap, aim, dipole=0.85):
    out = []
    disp = -normal
    for k, (img, refl) in enumerate(images(src)):
        v = cap - img
        r = max(np.linalg.norm(v), 0.03)
        u = v / r
        pat = 0.5 + 0.5 * (-aim @ u)
        if k == 0:
            c = disp @ u
            rad = np.sign(c if c != 0 else 1) * ((1 - dipole) + dipole * abs(c))
        else:
            rad = 1 - 0.5 * dipole
        out.append((r / C * SR, refl * pat * rad / r))
    return out


def render(x, ps):
    y = np.zeros(len(x) + 4000)
    for dl, g in ps:
        i = int(dl)
        fr = dl - i
        y[i:i + len(x)] += g * (1 - fr) * x
        y[i + 1:i + 1 + len(x)] += g * fr * x
    return y


def pair(kind):
    """(L cap, L aim), (R cap, R aim)."""
    if kind == "spaced56":
        return [(OH + [-0.28, 0, 0], AIM), (OH + [0.28, 0, 0], AIM)]
    if kind in ("ortf", "xy"):
        sp, ang = (0.17, 55) if kind == "ortf" else (0.0, 45)
        a = np.radians(ang)
        # capsules splayed left/right in the x-z plane, pointing down
        la = np.array([-np.sin(a), 0, -np.cos(a)])
        ra = np.array([np.sin(a), 0, -np.cos(a)])
        return [(OH + [-sp / 2, 0, 0], la), (OH + [sp / 2, 0, 0], ra)]
    raise ValueError(kind)


def bands(L, R):
    out = []
    for lo, hi in [(250, 500), (500, 1000), (1000, 2000), (2000, 4000), (4000, 8000), (8000, 16000)]:
        sos = signal.butter(4, [lo, hi], "band", fs=SR, output="sos")
        a, b = signal.sosfilt(sos, L), signal.sosfilt(sos, R)
        c = np.sum(a * b) / np.sqrt(np.sum(a * a) * np.sum(b * b))
        loss = 10 * np.log10(np.sum(((a + b) / 2) ** 2) / ((np.sum(a * a) + np.sum(b * b)) / 2))
        out.append(f"{c:+.2f}/{loss:+.1f}")
    return "  ".join(out)


if __name__ == "__main__":
    rng = np.random.default_rng(1)
    x = rng.standard_normal(SR)
    for kind in ["spaced56", "ortf", "xy"]:
        print(f"--- {kind}: corr/mono-sum dB per octave 250..16k")
        for name, (c, nrm) in PIECES.items():
            src, n = eng(c), eng(nrm)
            (lc, la), (rc, ra) = pair(kind)
            L = render(x, paths(src, n, lc, la))
            R = render(x, paths(src, n, rc, ra))
            print(f"  {name:7s} {bands(L, R)}")
