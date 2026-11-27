"""r42:arcus-hands, pass 7 — ``arcus_hand_table::the_tables_hand_plays_as_a_violinists_does``
rebuilt in Python on the table's blended poses (``blend_check.Blend``), so the rules can be
measured and solved against without a Rust build: each string at +1, +2, +3, +5, +7, +9, +12,
+14 semitones, the hand at ``hand::target``'s position, the finger ``left_at`` picks, the pose
built as ``neck_hand::row`` and ``place`` build it.

    python tools/arcus-figures/rule_check.py <dump.json> [--only=woman-violin] [--all]

Prints one line a broken rule in the gate's words (``--all``: every case's measures). The
measures are functions of a pose (``measure``) so ``blend_fix.py`` can hold them.
"""
import json
import os
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import blend_check as B  # noqa: E402
import hand_rig as hr  # noqa: E402
import left_hand as L  # noqa: E402

GATE_SEMIS = (1.0, 2.0, 3.0, 5.0, 7.0, 9.0, 12.0, 14.0)
# The gate's bounds (arcus_hand_table.rs).
UPRIGHT = 30.0
WRIST = 16.0
WRIST_HIGH = 30.0
PALM_WIDE = 1.45
PALM_WIDE_REACH = 1.7
SIDEWAYS = 5.0
BACK_WL = (0.45, 0.95)
BACK_WL_HIGH = 1.2
BACK_TO_CAMERA = 0.2
INDEX_OUT = (-0.5, 2.2)
HAND_NECK = (68.0, 92.0)
KNUCKLES_UP = (-1.6, 3.4)
THUMB_HIGH = -3.6
THUMB_UNDER = 0.25
# The neck's hidden test (arcus_draw_hand.rs).
NECK_SLAB = 0.55
NECK_STEPS = 10


def deg(a, b):
    return float(np.degrees(np.arccos(np.clip(hr.norm(a) @ hr.norm(b), -1, 1))))


class Cam:
    """The picture's camera as the dump carries it (perspective about the eye)."""

    def __init__(self, F):
        self.eye = np.array(F["eye"])
        self.f, self.r, self.u = (np.array(F[k]) for k in ("cam_f", "cam_r", "cam_u"))

    def p(self, q):
        d = np.asarray(q) - self.eye
        z = d @ self.f
        return np.stack([d @ self.r, -(d @ self.u)], -1) / z[..., None] if d.ndim > 1 else \
            np.array([d @ self.r, -(d @ self.u)]) / z


class Hidden:
    """``hand::hidden_by_neck`` for many points at once."""

    def __init__(self, F, nk):
        self.nk = nk
        self.y0, self.y1 = F["body"] - 0.5, F["nut_y"] + 0.4
        w0, w1 = nk.hw(self.y0) + 0.1, nk.hw(self.y1) + 0.1
        t0, t1 = nk.top(self.y0) + 0.05, nk.top(self.y1) + 0.05
        self.wk = (w1 - w0) / (self.y1 - self.y0)
        self.w0 = w0 - self.wk * self.y0
        self.tk = (t1 - t0) / (self.y1 - self.y0)
        self.t0 = t0 - self.tk * self.y0
        self.depth = F["depth"]

    def __call__(self, pts, eye):
        a = self.nk.loc(pts)
        e = self.nk.loc(eye)
        v = e - a
        ln = np.linalg.norm(v, axis=1)
        dr = v / np.maximum(ln, 1e-6)[:, None]
        lo = np.full(len(a), 0.2)
        hi = ln.copy()

        def bound(c0, c1):
            nonlocal lo, hi
            z = np.abs(c1) < 1e-7
            hi = np.where(z & (c0 < 0), -1.0, hi)
            with np.errstate(divide="ignore", invalid="ignore"):
                t = -c0 / c1
            lo = np.where(~z & (c1 > 0), np.maximum(lo, t), lo)
            hi = np.where(~z & (c1 < 0), np.minimum(hi, t), hi)
        ya, yd = a[:, 1], dr[:, 1]
        bound(ya - self.y0, yd)
        bound(self.y1 - ya, -yd)
        bound(self.w0 + self.wk * ya - a[:, 0], self.wk * yd - dr[:, 0])
        bound(self.w0 + self.wk * ya + a[:, 0], self.wk * yd + dr[:, 0])
        bound(self.t0 + self.tk * ya - a[:, 2], self.tk * yd - dr[:, 2])
        bound(a[:, 2] - (self.t0 + self.tk * ya - self.depth), dr[:, 2] - self.tk * yd)
        out = np.zeros(len(a), bool)
        ok = lo <= hi
        for k in range(NECK_STEPS):
            t = lo + (hi - lo) * (k + 0.5) / NECK_STEPS
            q = a + dr * t[:, None]
            w = self.w0 + self.wk * q[:, 1]
            top = self.t0 + self.tk * q[:, 1]
            r = np.clip(q[:, 0] / np.maximum(w, 1e-3), -1, 1)
            out |= ok & (q[:, 2] <= top) & (q[:, 2] >= top - np.maximum(NECK_SLAB, self.depth * np.sqrt(1 - r * r)))
        return out


def frame(x, y):
    y = hr.norm(y)
    x = x - y * (x @ y)
    x = hr.norm(x)
    return np.stack([x, y, np.cross(x, y)], 1)


def turn(u, a):
    """Rodrigues: ``a`` radians about the unit ``u``."""
    K = np.array([[0, -u[2], u[1]], [u[2], 0, -u[0]], [-u[1], u[0], 0]])
    return np.eye(3) + np.sin(a) * K + (1 - np.cos(a)) * (K @ K)


def skin_app(h, F, W, Rh, Q, P, fore):
    """The kept skin posed as the app's ``repose_arm`` poses it: the hand and the fingers as
    ``Hand.skin``, the forearm's two bones aimed from the elbow with the hand's twist shared
    out along it (0.45, 0.95) and stretched to the arm's length (``Hand.skin`` carries the
    forearm rigidly with the hand: its wrist read 0.3-0.4 cm broader from the eye)."""
    Bn = h.B
    elbow = W + np.asarray(fore) * F["fore"]
    sh = np.array(F["shoulder"])
    u = hr.norm(elbow - sh)
    w = hr.norm(W - elbow)
    zz = w - u * (w @ u)
    bend = hr.norm(np.cross(u, hr.norm(zz)))
    f1b, f2b = Bn[L.O + hr.FORE1], Bn[L.O + hr.FORE2]
    q = Rh @ f2b[3]
    q = q - w * (q @ w)
    twist = np.arctan2(np.cross(bend, q) @ w, bend @ q) if np.linalg.norm(q) > 1e-4 else 0.0
    rest_fo = np.linalg.norm(f1b[2] - f1b[1]) + np.linalg.norm(f2b[2] - f2b[1])
    kf = np.linalg.norm(W - elbow) / max(rest_fo, 1e-3)
    f1 = np.linalg.norm(f1b[2] - f1b[1]) * kf
    R = np.zeros((len(h.bone_ids), 3, 3))
    c = np.zeros((len(h.bone_ids), 3))
    for i, (b, head, share) in enumerate(((f1b, elbow, 0.45), (f2b, elbow + w * f1, 0.95))):
        y0 = hr.norm(b[2] - b[1])
        rot = frame(turn(w, twist * share) @ bend, w) @ frame(b[3], y0).T
        S = np.eye(3) + (kf - 1.0) * np.outer(y0, y0)
        R[i] = rot @ S
        c[i] = head - R[i] @ b[1]
    R[2], c[2] = Rh, W - Rh @ h.wr
    for f in range(5):
        for k in range(3):
            i = 3 + f * 3 + k
            R[i] = Rh @ Q[f, k]
            c[i] = P[f, k] - R[i] @ h.heads[f, k]
    out = np.zeros_like(h.sk_pos)
    for j in range(4):
        Rj = R[h.sk_ids[:, j]]
        out += h.sk_w[:, j, None] * (np.einsum("nij,nj->ni", Rj, h.sk_pos) + c[h.sk_ids[:, j]])
    return out


def outline_hand(h):
    """``h`` with only the palm's, the thumb's and the forearm's skin points."""
    import copy
    keep = np.isin(h.sk_kind, ("palm", "thumb", "fore"))
    o = copy.copy(h)
    for k in ("sk_pos", "sk_kind", "sk_top", "sk_ids", "sk_w"):
        setattr(o, k, getattr(h, k)[keep])
    return o


def target_finger(s, span=B.SPAN):
    at = B.target(s)
    return at, int(np.clip(round((s - at) / (span / 3.0)), 0, 3))


class Rules:
    """The gate's measures of one pose."""

    def __init__(self, F, h, nk):
        self.F, self.nk, self.h = F, nk, h
        self.cam = Cam(F)
        self.hidden = Hidden(F, nk)
        self.rows = {(r["string"], r["finger"], r["at"]): r for r in F["rows"]}

    def widths(self, W, Rh, Q, P, fore):
        """The hand's outline from the eye at the wrist's crease and at its middle (cm)."""
        sk = skin_app(self.h, self.F, W, Rh, Q, P, fore)
        sk = sk[~self.hidden(sk, self.cam.eye)]
        mid = (P[2, 0] + P[3, 0]) * 0.5
        ax = hr.norm(mid - W)
        ln = np.linalg.norm(mid - W)
        out = []
        for t in (0.12, 0.5):
            c = W + ax * (ln * t)
            across = hr.norm(np.cross(hr.norm(self.cam.eye - c), ax))
            near = np.abs((sk - c) @ ax) < 0.35
            w = (sk[near] - c) @ across
            out.append(float(w.max() - w.min()) if near.any() else 0.0)
        return out

    def measure(self, string, semis, at, f, W, Rh, Q, P, fore, row):
        """Every rule's measure: a dict of name -> (value, lo, hi)."""
        nk, cam = self.nk, self.cam
        m = {}
        w0, w1 = self.widths(W, Rh, Q, P, fore)
        wide = PALM_WIDE if semis <= 3.0 else PALM_WIDE_REACH
        m["palm_wide"] = (w1 / max(w0, 1e-3), 0.0, wide, f"the palm {w1:.1f} cm wide at its middle, the wrist {w0:.1f}")
        z = np.array(row["z"])
        fj = P[f + 1]
        mcp, pip, dip = deg(fj[0] - W, fj[1] - fj[0]), deg(fj[1] - fj[0], fj[2] - fj[1]), deg(fj[2] - fj[1], fj[3] - fj[2])
        up = deg(fj[3] - fj[2], -z)
        m["mcp"] = (mcp, 15.0, 80.0, None)
        m["pip"] = (pip, 50.0, 110.0, None)
        m["dip"] = (dip, 0.0, 45.0, None)
        m["upright"] = (up, 0.0, UPRIGHT, f"MCP {mcp:.0f}° PIP {pip:.0f}° DIP {dip:.0f}°, {up:.0f}° off upright")
        loc = nk.loc
        ix, iy, _ = loc(P[1, 0])
        ihw = nk.hw(iy)
        index_in = -1.0 if semis > 5.0 else INDEX_OUT[0]
        m["index_out"] = (ix - ihw, index_in, INDEX_OUT[1], f"the index's base {ix - ihw:.1f} cm out from the E edge")
        for k in range(4):
            kx, ky, kz = loc(P[k + 1, 0])
            m[f"knuckle{k}"] = (kz - nk.top(ky), KNUCKLES_UP[0], KNUCKLES_UP[1],
                                f"knuckle {k} at {kx - nk.hw(ky):.1f} cm out, {kz - nk.top(ky):.1f} cm over the top")
            m[f"knuckle_out{k}"] = (kx - nk.hw(ky), index_in, 99.0, None)
        mid = (P[2, 0] + P[3, 0]) * 0.5
        back = -(Rh @ self.h.n)
        m["facing"] = (float(back @ hr.norm(cam.eye - mid)), BACK_TO_CAMERA, 9.0, None)
        wl = np.linalg.norm(cam.p(P[1, 0]) - cam.p(P[4, 0])) / max(np.linalg.norm(cam.p(mid) - cam.p(W)), 1e-6)
        m["back_wl"] = (wl, BACK_WL[0], BACK_WL_HIGH if semis > 8.0 else BACK_WL[1], f"the back of the hand {wl:.2f} wide over long")
        if semis <= 9.0:
            a_, bw = np.array(row["a"]), np.array(row["bw"])
            line = cam.p(mid) - cam.p(W)
            edge = cam.p(a_) - cam.p(a_ + bw * 5.0)
            a = np.arctan2(line[1], line[0]) - np.arctan2(edge[1], edge[0])
            a = abs(np.degrees(np.arctan2(np.sin(a), np.cos(a))))
            a = 180.0 - a if a > 90.0 else a
            m["hand_neck"] = (a, HAND_NECK[0], HAND_NECK[1], f"the hand {a:.0f}° to the neck in the picture")
        wr = deg(-fore, mid - W)
        m["wrist"] = (wr, 0.0, WRIST_HIGH if semis >= 9.0 else WRIST, f"the wrist bent {wr:.0f}°")
        tx, ty, tz = loc(P[0, 3])
        top = nk.top(ty)
        hw = row["hw"]
        m["thumb_index"] = (ix - (hw + index_in), 0.0, 99.0, None)
        # (r42:arcus-hands, pass 7: from the 12th semitone the thumb in the heel's curve under
        # the neck — THUMB_HIGH, THUMB_UNDER.)
        high = semis >= 12.0
        m["thumb_x"] = (tx, -99.0, hw * THUMB_UNDER if high else -hw * 0.5, None)
        m["thumb_z"] = (tz - top, THUMB_HIGH if high else -1.6, 1.2, f"the index's base at x {ix:.1f}, the thumb's tip at x {tx:.1f}, {tz - top:.1f} cm from the top")
        return m


# r42:arcus-hands, pass 7 — what ``blend_fix.py --rules`` holds of the gate's measures:
# the bounds with a margin (this reproduction reads the outline's ratio up to 0.07 under the
# gate's, the rest within 0.1) and each hinge's weight (the instrument's walls weigh 30 a cm).
HOLD = {
    "palm_wide": (40.0, None, -0.15),
    "upright": (0.6, None, -1.5),
    "mcp": (0.3, 2.0, -2.0),
    "pip": (0.3, 2.0, -2.0),
    "dip": (0.3, 1.0, -2.0),
    "index_out": (40.0, 0.1, -0.2),
    "knuckle0": (8.0, 0.2, -0.2),
    "knuckle1": (8.0, 0.2, -0.2),
    "knuckle2": (8.0, 0.2, -0.2),
    "knuckle3": (8.0, 0.2, -0.2),
    "knuckle_out0": (8.0, 0.1, None),
    "knuckle_out1": (8.0, 0.1, None),
    "knuckle_out2": (8.0, 0.1, None),
    "knuckle_out3": (8.0, 0.1, None),
    "facing": (20.0, 0.05, None),
    "back_wl": (20.0, 0.02, -0.03),
    "hand_neck": (3.0, 1.5, -1.5),
    "wrist": (0.6, None, -1.0),
    "thumb_index": (10.0, 0.1, None),
    "thumb_x": (10.0, None, -0.1),
    "thumb_z": (10.0, 0.15, -0.15),
}


def hold(m, only=None):
    """The hinges over the gate's bounds (with HOLD's margins) for the measures ``m``."""
    out = []
    for k, (w, dlo, dhi) in HOLD.items():
        if k not in m or (only is not None and k not in only):
            continue
        v, lo, hi, _ = m[k]
        r = 0.0
        if dlo is not None:
            r += max(0.0, lo + dlo - v)
        if dhi is not None:
            r += max(0.0, v - (hi + dhi))
        out.append(w * r)
    return out


def broken(m):
    """The gate's messages for the measures ``m``."""
    out = []
    for k, (v, lo, hi, msg) in m.items():
        if k in ("mcp", "pip", "dip", "thumb_index", "thumb_x", "knuckle_out0", "knuckle_out1",
                 "knuckle_out2", "knuckle_out3"):
            continue
        if k == "upright":
            bad = any(not (m[j][1] <= m[j][0] <= m[j][2]) for j in ("mcp", "pip", "dip", "upright"))
        elif k == "thumb_z":
            bad = any(not (m[j][1] <= m[j][0] <= m[j][2]) for j in ("thumb_index", "thumb_x", "thumb_z"))
        elif k.startswith("knuckle"):
            o = m["knuckle_out" + k[-1]]
            bad = not (lo <= v <= hi) or o[0] <= o[1]
        elif k == "palm_wide":
            bad = v > hi
        else:
            bad = not (lo <= v <= hi)
        if bad:
            out.append(msg or k)
    return out


def cases(blend, rules, string, x_over=None, at_rows=None):
    """The gate's cases on one string: (semis, at, finger, the measures). With ``at_rows``,
    only the cases whose blend uses one of those rows."""
    out = []
    for s in GATE_SEMIS:
        at, f = target_finger(s)
        lo = max(a for a in blend.ats if a <= at)
        hi = min((a for a in blend.ats if a >= at), default=lo)
        if at_rows is not None and not ({lo, hi} & at_rows):
            continue
        W, Rh, Q, P, fore = blend.pose(string, f, at, s, x_over=x_over)
        row = rules.rows[(string, f, lo)]
        out.append((s, at, f, rules.measure(string, s, at, f, W, Rh, Q, P, fore, row)))
    return out


def main():
    dump = sys.argv[1]
    D = json.load(open(dump))
    L.dense()
    here = os.path.dirname(os.path.abspath(dump))
    only = next((a.split("=", 1)[1] for a in sys.argv if a.startswith("--only=")), None)
    every = "--all" in sys.argv
    total = 0
    count = {}
    for F in D["figs"]:
        key = f'{F["who"]}-{F["inst"]}'
        if only and key != only:
            continue
        h = L.Hand(F["who"])
        b = B.Blend(F, h, B.cache_of(here, key))
        r = Rules(F, h, L.Neck(F))
        who = F["who"].capitalize()
        inst = F["inst"].capitalize()
        for string in range(4):
            for s, at, f, m in cases(b, r, string):
                tag = f"{who} {inst} string {string} +{s:g}"
                if every:
                    print(tag, f"(at {at:g} f{f})", " ".join(f"{k}={v[0]:.2f}" for k, v in m.items()))
                for msg in broken(m):
                    total += 1
                    kind = msg.split(" ")[1] if msg.startswith("the ") else msg.split(" ")[0]
                    count[kind] = count.get(kind, 0) + 1
                    print(f"{tag}: {msg}")
    print("broken", total, count)


if __name__ == "__main__":
    main()
