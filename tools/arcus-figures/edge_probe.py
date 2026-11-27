"""r42:arcus-hands, pass 8 — what turning one table row edge-on costs. The row is solved again
as ``blend_fix.py --rules`` solves it (its own residuals, its blends kept out of the
instrument, the gate's rules on the cases it makes), with the back of the hand's facing to
the app's eye held to ``--facing`` at ``--weight`` (``blend_fix.FACING_W`` is 100), from the
table's own pose and from ``--rib=N`` frames built along the instrument (crop-02): the
knuckle line level and along the neck, the back at the wanted slant, the wrist clear of the
body by 1.5 cm or more, the playing tip on its stop. ``--relax=W`` lets the facing go at
weight W afterwards. ``--thumb``: instead, starts with the thumb of the rows one and two
positions lower and at rest (the thumb tips out on the E side high up).

    python tools/arcus-figures/edge_probe.py <dump.json> <who-inst> <string> <finger> <at>
        [--facing=0.6] [--weight=1000] [--rib=2] [--relax=300] [--thumb] [--look=<dir>]

Reads the caches beside the dump (as ``blend_fix.py``) and writes nothing to them: each
result prints the total, its own residuals, its blends, the gate's hinges, the facing, the
deepest skin at the row and its blends (and the part), the wrist in the neck's frame and the
residuals that cost most; ``--look`` saves ``left_hand.pen_look``'s three views of each.
Set ``CANTUS_CROOK_X_HIGH`` (cm) to let the crook leave the E side past the heel.
"""
import json
import os
import sys
import time

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import blend_check as B  # noqa: E402
import blend_fix as BF  # noqa: E402
import hand_rig as hr  # noqa: E402
import left_hand as L  # noqa: E402
import rule_check as RC  # noqa: E402


class Probe:
    def __init__(self, dump, only, string, finger, at, facing):
        D = json.load(open(dump))
        self.D, self.dir = D, os.path.dirname(os.path.abspath(dump))
        F = next(F for F in D["figs"] if f'{F["who"]}-{F["inst"]}' == only)
        self.F, self.only = F, only
        self.string, self.finger, self.at = string, finger, at
        thin = L.Hand(F["who"])
        L.dense()
        self.h = h = L.Hand(F["who"])
        self.cache = B.cache_of(self.dir, only)
        self.b = b = B.Blend(F, h, self.cache)
        self.neck = L.Neck(F)
        rules = RC.Rules(F, h, self.neck)
        R0, _ = L.start(h, L.rows_of(F, 0, 1)[0])
        row = b.rows[(string, finger, at)]
        hs = BF.near(b, thin, string, finger, at, BF.Row.mids_of(b, at))
        self.pose = L.Pose(hs, self.neck, F, row, R0)
        i = b.ats.index(at)
        lo = self.cache.get(f"{only}-{string}-{finger}-{b.ats[i - 1]:g}") if i > 0 else None
        self.prev = np.array(lo["x"]) if lo is not None else None
        self.row = BF.Row(b, self.pose, string, finger, at).gate(rules)
        if row["semis"] > 3.0 and finger < 4:
            self.pose.palm_free = BF.PALM_FREE_REACH
        BF.FACING_MOST = facing
        self.fun = self.row.fun(self.prev)
        self.x0 = np.array(self.cache[f"{only}-{string}-{finger}-{at:g}"]["x"])
        self.eye = np.array(F["eye"])

    def facing(self, x):
        W, Rh, Q = self.pose.unpack(x)
        P = self.pose.h.fk(W, Rh, Q)
        mid = (P[2, 0] + P[3, 0]) * 0.5
        return float(-(Rh @ self.h.n) @ hr.norm(self.eye - mid)), W, Rh, P

    def report(self, tag, x):
        pose, prev = self.pose, self.prev
        pose.pen_w = L.PEN_W
        fc, W, _, _ = self.facing(x)
        r = self.fun(x)
        own = pose.residuals(x, prev)
        pose.want_names = True
        names = pose.residuals(x, prev)
        pose.want_names = False
        by = {}
        for n, v in zip(names, own):
            by[n] = by.get(n, 0.0) + v * v
        top = sorted(by.items(), key=lambda kv: -kv[1])[:6]
        nb = len(own) + len(self.row.blends(x))
        bl, he = r[len(own):nb], r[nb:]
        me = BF.me_of(pose, x)
        deep, part = 0.0, ""
        for a in [self.at] + BF.Row.mids_of(self.b, self.at):
            Wb, Rb, Qb, Pb, fb = self.b.pose(self.string, self.finger, a, None, x_over={self.at: me})
            dp, dr, top_, _ = self.b.depth(Wb, Rb, Qb, Pb, fb, self.finger)
            d = max(dr, dp - B.PAD_PRESS)
            if d > deep:
                deep, part = d, f"{B.name(top_)} at {a:.2f}"
        print(f"{tag}: total {r @ r:.0f} own {own @ own:.0f} blends {bl @ bl:.0f} held {he @ he:.0f} "
              f"facing {fc:.2f} deepest {deep * 10:.1f} mm ({part}) wrist {np.round(self.neck.loc(W), 1)}  "
              + " ".join(f"{n}={v:.0f}" for n, v in top), flush=True)

    def built(self, back, axis, xs):
        """``xs`` with the hand turned so its back faces ``back`` and its length runs ``axis``
        (world), the fingers as they were, the wrist moved so the playing tip is on its stop."""
        h, pose = self.h, self.pose
        ry = hr.norm((h.heads[2, 0] + h.heads[3, 0]) * 0.5 - h.wr)
        nb = hr.norm(-h.n - ry * (-h.n @ ry))
        Mr = np.stack([ry, nb, np.cross(ry, nb)], 1)
        a = hr.norm(axis)
        bb = hr.norm(back - a * (back @ a))
        Mt = np.stack([a, bb, np.cross(a, bb)], 1)
        x = xs.copy()
        x[3:6] = L.rotlog(Mt @ Mr.T @ pose.R0.T)
        x[:3] = 0.0
        W, Rh, Q = pose.unpack(x)
        P = h.fk(W, Rh, Q)
        x[:3] = pose.tips[self.finger] - P[self.finger + 1, 3]
        return x

    def rib_starts(self, n, want):
        """Frames along the instrument: the knuckle line level (the playing finger's last
        phalanx can come down onto its string) and running toward the scroll from the little
        finger to the index, the back at ``want`` to the eye, the wrist clear of the body."""
        _, W, _, P = self.facing(self.x0)
        mid = (P[2, 0] + P[3, 0]) * 0.5
        E = self.neck.E
        e = E @ hr.norm(self.eye - mid)
        length = np.linalg.norm(mid - W)
        up = np.array([0.0, 0.0, 1.0])
        got = []
        for th in np.radians(np.arange(0, 180, 7.5)):
            for ph in np.radians(np.arange(0, 360, 7.5)):
                bn = np.array([np.sin(th) * np.cos(ph), np.sin(th) * np.sin(ph), np.cos(th)])
                fc = bn @ e
                r = np.cross(bn, up)
                if not (want - 0.15 <= fc <= want + 0.02) or np.linalg.norm(r) < 0.2:
                    continue
                for kr in (hr.norm(r), -hr.norm(r)):
                    if kr[1] < 0.4:
                        continue
                    a = np.cross(kr, bn)
                    if np.cross(a, kr) @ bn > 0:  # (a left hand: index on the right of the back)
                        a = -a
                    if a[2] < 0.3:
                        continue
                    wn = self.neck.loc(mid) - length * a
                    d = self.b.solid.local_sd(self.b.solid.loc(self.neck.world(wn)[None]))[0]
                    if d >= 1.5:
                        got.append((d, bn, a))
        got.sort(key=lambda t: -t[0])
        pick = []
        for g in got:
            if len(pick) < n and all(np.linalg.norm(g[1] - p[1]) > 0.35 for p in pick):
                pick.append(g)
        return [(f"rib{k} (wrist clear {d:.1f} cm, back {np.round(bn, 2)})",
                 self.built(E.T @ bn, E.T @ a, self.x0)) for k, (d, bn, a) in enumerate(pick)]

    def thumb_starts(self):
        b, i = self.b, self.b.ats.index(self.at)
        out = []
        for j in (i - 1, i - 2):
            if j < 0:
                continue
            xn = np.array(self.cache[f"{self.only}-{self.string}-{self.finger}-{b.ats[j]:g}"]["x"])
            xs = self.x0.copy()
            xs[22:27] = xn[22:27]
            out.append((f"the thumb of {b.ats[j]:g}", xs))
        xs = self.x0.copy()
        xs[22:25] = 0.0
        xs[25:27] = np.radians(15.0)
        out.append(("the thumb at rest", xs))
        return out

    def look(self, x, out, tag):
        os.makedirs(out, exist_ok=True)
        key = f"{self.only}-{self.string}-{self.finger}-{self.at:g}"
        c = dict(self.cache)
        c[key] = L.entry(self.pose, x, 0.0)
        p = L.pen_look(self.D, out, self.F["who"], self.F["inst"], self.string, self.finger,
                       self.at, c)
        os.replace(p, os.path.join(out, f"probe-{key}-{tag}.png"))


def main():
    a = [x for x in sys.argv[1:] if not x.startswith("--")]
    arg = {x.split("=", 1)[0]: x.split("=", 1)[1] for x in sys.argv[1:] if "=" in x}
    facing = float(arg.get("--facing", "0.6"))
    p = Probe(a[0], a[1], int(a[2]), int(a[3]), float(a[4]), facing)
    BF.FACING_W = float(arg.get("--weight", "1000"))
    p.report("the table's row", p.x0)
    starts = [("the table's row", p.x0)]
    if "--thumb" in sys.argv:
        BF.FACING_W = 0.0
        starts += p.thumb_starts()
    else:
        starts += p.rib_starts(int(arg.get("--rib", "2")), facing)
    for k, (tag, xs) in enumerate(starts):
        t0 = time.time()
        if k:
            p.report("  start " + tag, xs)
        x, _ = L.settle(p.pose, p.fun, xs, 30)
        p.report(f"  -> from {tag} ({time.time() - t0:.0f} s)", x)
        if "--relax" in arg:
            w = BF.FACING_W
            BF.FACING_W = float(arg["--relax"])
            x, _ = L.lm(p.fun, x, iters=30)
            p.report(f"     the facing let go at weight {BF.FACING_W:g}", x)
            BF.FACING_W = w
        if "--look" in arg:
            p.look(x, arg["--look"], f"{facing:g}-{k}")


if __name__ == "__main__":
    main()
