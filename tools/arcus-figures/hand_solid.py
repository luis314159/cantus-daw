"""r42:arcus-hands — the violin's or the viola's neck, fingerboard, strings and body as the
app draws them, as a signed distance (cm, negative inside) the left hand is solved out of.

The dump (``arcus_hand_table -- --ignored dump``) carries ``solid``: the fingerboard's and
the neck's sections (``bowed_head::neck``'s own patches' rows, each ``[y, [[x, z], ...]]``
in the instrument's frame — x the E side, y toward the scroll, z the fingerboard's normal),
the body's outline, the plates' arching and the ribs (``bowed::arch``, ``bowed::rib``). The
test's ``Solid`` measures the same thing for the gate
``the_left_hand_keeps_out_of_the_neck_and_the_body``.

The distance is sampled once on a grid (1 mm round the neck, 2 mm over the upper bout) and
read back trilinearly, so the solver can ask it for a few hundred skin points per step.
"""
import numpy as np
from scipy.ndimage import map_coordinates


def sd_polys(polys, p):
    """Signed distance of points ``p`` (N x 2) to closed polygons ``polys`` (N x M x 2 —
    each point its own polygon — or M x 2 for one shared)."""
    p = np.asarray(p, float)
    if polys.ndim == 2:
        polys = np.broadcast_to(polys, (len(p),) + polys.shape)
    a = polys
    b = np.roll(polys, -1, axis=1)
    d = b - a
    l2 = np.maximum((d * d).sum(-1), 1e-12)
    rel = p[:, None, :] - a
    t = np.clip((rel * d).sum(-1) / l2, 0, 1)
    e = a + d * t[..., None] - p[:, None, :]
    best = np.sqrt((e * e).sum(-1).min(1))
    ay, by_ = a[..., 1], b[..., 1]
    py = p[:, None, 1]
    cross = (ay > py) != (by_ > py)
    xi = a[..., 0] + (py - ay) / np.where(by_ - ay == 0, 1e-12, by_ - ay) * d[..., 0]
    inside = (cross & (p[:, None, 0] < xi)).sum(1) % 2 == 1
    return np.where(inside, -best, best)


def arch(height, d, reach):
    """``bowed::arch``: the plates' arching at ``d`` cm in from the edge."""
    edge = 0.42
    t = np.clip(d / reach, 0, 1)
    rise = np.sin(t * np.pi / 2) ** 1.7
    channel = 0.10 * np.exp(-((d - 0.95) / 0.45) ** 2)
    return edge + (height - edge * 0.5) * rise - channel


class Solid:
    def __init__(self, F):
        S = F["solid"]
        self.F = F
        self.o = np.array(F["o"])
        self.E = np.array([F["ex"], F["ey"], F["ez"]])

        def stack(rows):
            ys = np.array([r[0] for r in rows])
            polys = np.array([r[1] for r in rows], float)
            return ys, polys

        self.board = stack(S["board"])
        self.shaft = stack(S["shaft"])
        self.outline = np.array(S["outline"], float)
        self.arch_h, self.arch_b, self.reach = S["arch"], S["arch_back"], S["reach"]
        self.flat_back = S["flat_back"]
        ribs = np.array(S["ribs"])
        self.rib = lambda y: np.interp(y, ribs[:, 0], ribs[:, 1])
        self.body_len = F["body"]
        # The strings in the instrument's frame (bridge, nut) and their radii.
        self.strings = [(self.loc(np.array(s["bridge"])), self.loc(np.array(s["nut"])),
                         s["gauge"] * 0.5) for s in F["strings"]]
        self._grids()

    def loc(self, p):
        return (np.asarray(p) - self.o) @ self.E.T

    # ---- exact distances (local points) ----

    @staticmethod
    def _stack(st, q):
        ys, polys = st
        y = np.clip(q[:, 1], ys[0], ys[-1])
        i = np.clip(np.searchsorted(ys, y), 1, len(ys) - 1)
        t = np.clip((y - ys[i - 1]) / np.maximum(ys[i] - ys[i - 1], 1e-6), 0, 1)
        poly = polys[i - 1] * (1 - t)[:, None, None] + polys[i] * t[:, None, None]
        d2 = sd_polys(poly, q[:, [0, 2]])
        e = np.maximum(ys[0] - q[:, 1], q[:, 1] - ys[-1])
        return np.where(e <= 0, np.maximum(d2, e),
                        np.where(d2 > 0, np.sqrt(d2 * d2 + e * e), e))

    def _body(self, q):
        out = self.outline
        sd = sd_polys(out, q[:, :2])
        dd = np.abs(sd)
        inside = sd < 0
        yb = np.clip(q[:, 1], 0, self.body_len)
        zb_arch = arch(self.arch_b, dd, self.reach)
        if self.flat_back:
            zb_arch = 0.42 + (zb_arch - 0.42) * 0.4
        zt = np.where(inside, arch(self.arch_h, dd, self.reach), 0.42)
        zb = np.where(inside, -self.rib(yb) - zb_arch, -self.rib(yb) - 0.42)
        dz = np.maximum(q[:, 2] - zt, zb - q[:, 2])
        dxy = sd
        return np.where((dxy > 0) & (dz > 0), np.sqrt(dxy ** 2 + dz ** 2), np.maximum(dxy, dz))

    def exact(self, q):
        """The neck's, the fingerboard's and the body's signed distance at local points."""
        q = np.atleast_2d(q)
        return np.minimum(np.minimum(self._stack(self.board, q), self._stack(self.shaft, q)),
                          self._body(q))

    def strings_d(self, q):
        """The distance from local points to the nearest string's surface."""
        best = np.full(len(q), 1e9)
        for a, b, r in self.strings:
            d = b - a
            t = np.clip(((q - a) @ d) / (d @ d), 0, 1)
            e = a + t[:, None] * d - q
            best = np.minimum(best, np.sqrt((e * e).sum(-1)) - r)
        return best

    # ---- the grids ----

    def _grids(self):
        nb, nn = self.body_len, self.F["nut_y"]
        top = max(self.board[1][..., 1].max(), 3.0)
        self.grids = []
        for lo, hi, step in (
                # Round the neck and the fingerboard, from the upper bout to the nut.
                (np.array([-4.0, nb - 12.0, -5.0]), np.array([4.0, nn + 1.0, top + 1.5]), 0.1),
                # The whole body and round it, where the hand and the forearm come high up
                # and the forearm passes under the back.
                (np.array([-16.0, -2.0, -9.0]), np.array([16.0, nb + 3.0, 6.0]), 0.25)):
            n = np.ceil((hi - lo) / step).astype(int) + 1
            xs, ys, zs = [lo[i] + step * np.arange(n[i]) for i in range(3)]
            self.grids.append((lo, step, n, self._sampled(xs, ys, zs)))

    def _sampled(self, xs, ys, zs):
        """``exact`` on the grid xs × ys × zs, slice by slice: each y's sections once over
        the (x, z) plane, the body's outline once over the (x, y) plane."""
        X, Z = np.meshgrid(xs, zs, indexing="ij")
        xz = np.stack([X.ravel(), Z.ravel()], -1)
        out = np.empty((len(xs), len(ys), len(zs)))
        for j, y in enumerate(ys):
            d = np.full(len(xz), np.inf)
            for st in (self.board, self.shaft):
                sy, polys = st
                yc = np.clip(y, sy[0], sy[-1])
                i = int(np.clip(np.searchsorted(sy, yc), 1, len(sy) - 1))
                t = np.clip((yc - sy[i - 1]) / max(sy[i] - sy[i - 1], 1e-6), 0, 1)
                d2 = sd_polys(polys[i - 1] * (1 - t) + polys[i] * t, xz)
                e = max(sy[0] - y, y - sy[-1])
                if e <= 0:
                    dd = np.maximum(d2, e)
                else:
                    dd = np.where(d2 > 0, np.sqrt(d2 * d2 + e * e), e)
                d = np.minimum(d, dd)
            out[:, j, :] = d.reshape(len(xs), len(zs))
        # The body: its outline's distance over (x, y), the plates' heights from it.
        Xb, Yb = np.meshgrid(xs, ys, indexing="ij")
        xy = np.stack([Xb.ravel(), Yb.ravel()], -1)
        sd = sd_polys(self.outline, xy).reshape(len(xs), len(ys))
        dd = np.abs(sd)
        inside = sd < 0
        yb = np.clip(Yb, 0, self.body_len)
        zb_arch = arch(self.arch_b, dd, self.reach)
        if self.flat_back:
            zb_arch = 0.42 + (zb_arch - 0.42) * 0.4
        zt = np.where(inside, arch(self.arch_h, dd, self.reach), 0.42)[..., None]
        zb = np.where(inside, -self.rib(yb) - zb_arch, -self.rib(yb) - 0.42)[..., None]
        z = zs[None, None, :]
        dz = np.maximum(z - zt, zb - z)
        dxy = sd[..., None]
        body = np.where((dxy > 0) & (dz > 0), np.sqrt(dxy ** 2 + dz ** 2), np.maximum(dxy, dz))
        return np.minimum(out, body)

    def local_sd(self, q):
        """The signed distance at local points: the finer grid where it reaches, else the
        coarser, else exact; the strings' surfaces too."""
        q = np.atleast_2d(q)
        out = np.full(len(q), np.nan)
        for lo, step, n, g in self.grids:
            idx = (q - lo) / step
            ok = np.isnan(out) & (idx >= 0).all(1) & (idx <= n - 1).all(1)
            if ok.any():
                out[ok] = map_coordinates(g, idx[ok].T, order=1, mode="nearest")
        miss = np.isnan(out)
        if miss.any():
            # Outside both: the coarser grid at the nearest point of its box, plus the way
            # out to the point (what the grid holds lies well inside its box).
            lo, step, n, g = self.grids[-1]
            hi = lo + step * (n - 1)
            c = np.clip(q[miss], lo, hi)
            out[miss] = (map_coordinates(g, ((c - lo) / step).T, order=1, mode="nearest")
                         + np.linalg.norm(q[miss] - c, axis=1))
        return out

    def sd(self, p, strings=True):
        """The signed distance at world points (the strings included unless told not)."""
        q = self.loc(p)
        d = self.local_sd(q)
        if strings:
            d = np.minimum(d, self.strings_d(q))
        return d
