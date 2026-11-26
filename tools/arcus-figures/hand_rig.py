"""r42:arcus-upper — a Python mirror of the live hand's posing (scene/bowed_human.rs::arm_aims' hand part) for
designing grips quickly: reads a .fig, poses one hand from a HandAim-like set of joint
targets with a planar (hinge) finger IK, skins it, renders it with preview.py."""
import os
import struct
import sys
import numpy as np

sys.path.insert(0, __import__('os').path.dirname(__import__('os').path.abspath(__file__)))
import preview  # noqa

BONES = 53
DEBUG = False


def side(s):
    return 5 + 24 * s


CLAV, UPPER1, UPPER2, FORE1, FORE2, HAND = range(6)


def finger(f, k):
    return 6 + f * 3 + k


def read_fig(path, knuckles=True):
    b = open(path, 'rb').read()
    o = 0

    def take(fmt):
        nonlocal o
        v = struct.unpack_from(fmt, b, o)
        o += struct.calcsize(fmt)
        return v
    assert b[:4] == b'CFIG'
    o = 4
    take('<H')
    nb, = take('<H')
    bones = []
    for _ in range(nb):
        v = take('<h9f')
        bones.append((v[0], np.array(v[1:4]), np.array(v[4:7]), np.array(v[7:10])))
    nm, = take('<H')
    marks = {}
    for _ in range(nm):
        l, = take('<B')
        name = b[o:o + l].decode()
        o += l
        marks[name] = np.array(take('<3f'))
    nv, = take('<I')
    pos = np.zeros((nv, 3))
    ids = np.zeros((nv, 4), int)
    ws = np.zeros((nv, 4))
    det = np.zeros((nv, 2))
    for i in range(nv):
        v = take('<3h4B4BBb2e')
        pos[i] = np.array(v[0:3]) * 0.01
        ids[i] = v[3:7]
        ws[i] = np.array(v[7:11]) / 255.0
        det[i] = [0.35 + v[11] / 255.0, v[12] / 100.0]
    nt, = take('<I')
    tris = np.zeros((nt, 3), int)
    mat = np.zeros(nt, int)
    part = np.zeros(nt, int)
    for i in range(nt):
        v = take('<3HBB')
        tris[i] = v[0:3]
        mat[i] = v[3]
        part[i] = v[4]
    fig = dict(bones=bones, marks=marks, pos=pos, ids=ids, ws=ws, det=det, tris=tris,
               mat=mat, part=part)
    if knuckles:
        name = os.path.basename(path)
        knuckle_fix(fig, HAND_SCALE.get(name, 1.0), PALM_SLIM.get(name, 1.0))
        for s in (0, 1):
            thumb_web(fig, s)
    return fig


# r42:arcus-polish — the first web space and the thenar (``scene/bowed_human.rs::thumb_web``,
# the two alike): the hand bone's skin past the index's metacarpal toward the thumb drawn in
# across and partly handed to the thumb's first bone, so it goes under the neck with the
# thumb instead of standing out as a mitten's thumb pocket.
WEB_IN = 0.45
WEB_TO_THUMB = 0.6
EDGE_IN = 0.5


def _smooth(x):
    x = np.clip(x, 0.0, 1.0)
    return x * x * (3.0 - 2.0 * x)


def thumb_web(fig, s):
    B = fig["bones"]
    pos, ids, ws = fig["pos"], fig["ids"], fig["ws"]
    o = side(s)
    wr = B[o + HAND][1]
    ix, lt = B[o + finger(1, 0)][1], B[o + finger(4, 0)][1]
    end = (B[o + finger(2, 0)][1] + B[o + finger(3, 0)][1]) * 0.5
    along = norm(end - wr)
    ln = max(np.linalg.norm(end - wr), 1e-3)
    a = ix - lt
    across = norm(a - along * (a @ along))
    line = (ix - wr) @ across
    ulnar = (lt - wr) @ across
    hb, tb = o + HAND, o + finger(0, 0)
    for i in range(len(pos)):
        wh = float(np.sum(np.where(ids[i] == hb, ws[i], 0.0)))
        if wh < 0.4:
            continue
        d = pos[i] - wr
        t = d @ along / ln
        ru = ulnar - d @ across
        if ru > 0.0 and 0.1 <= t < 1.05:
            ramp = _smooth((t - 0.12) / 0.3) * (1.0 - _smooth((t - 0.8) / 0.3))
            pos[i] = pos[i] + across * (ru * EDGE_IN * ramp * wh)
            continue
        r = d @ across - line
        if r <= 0.0 or not (-0.1 <= t < 1.05):
            continue
        ramp = _smooth((t + 0.1) / 0.3) * (1.0 - _smooth((t - 0.75) / 0.3))
        pos[i] = pos[i] - across * (r * WEB_IN * ramp * wh)
        give = wh * WEB_TO_THUMB * ramp * _smooth(r / 1.5)
        if give < 1e-4:
            continue
        w = {int(ids[i, k]): float(ws[i, k]) for k in range(4) if ws[i, k] > 0}
        w[hb] = w.get(hb, 0.0) - give
        w[tb] = w.get(tb, 0.0) + give
        top = sorted(((v, k) for k, v in w.items() if v > 1e-6), reverse=True)[:4]
        tot = sum(v for v, _ in top)
        ids[i] = 0
        ws[i] = 0
        for k, (v, b) in enumerate(top):
            ids[i, k] = b
            ws[i, k] = v / tot


# r42:arcus-hand — MakeHuman's finger bones begin at the webs, about a centimetre and a
# half past the knuckles' joints (the proximal phalanx 2.2-3.0 cm on her, a real one 3.7-4.2):
# bent there, a finger folds in the middle of its first phalanx and the palm runs on into it
# (the owner's "fist", the coordinator's "short fingers on a broad palm"). Each finger's
# base joint is moved back along its metacarpal by this share of the wrist-to-base length
# and the skin between is handed from the hand's bone to the finger's, smoothly, each vertex
# by how near it lies to that finger's line. ``scene/bowed_human.rs::knuckle_fix`` does the
# same on the figure the app reads; the two must stay alike.
KNUCKLE_BACK = 0.12
KNUCKLE_LANE = 0.8
KNUCKLE_RAMP = (-0.5, 0.7)


# r42:arcus-hand — each figure's hands scaled about the wrist (the coordinator's review: his
# hand, wrist crease to fingertip, read longer than his face; a man's is about as long).
# (r42:arcus-hands: hers 1.08 — her hand 0.095 of her height, a woman's ~0.105.)
HAND_SCALE = {"woman.fig": 1.08, "man.fig": 0.93}
# r42:arcus-hand — each figure's palm drawn in across, toward the hand's middle line (the
# owner: her hand "tosca", the palm bulging out of a slim wrist; her palm measured 1.25-1.64
# times her wrist from the picture's camera, the photographs' about 1.25).
# (r42:arcus-hands, pass 6: kept, though 0.86 / 1.0 draw the breadth across the knuckles to
# ANSUR II's 0th / 4th percentile for their statures — hand_anthro.py; 0.99 / 1.04 moved the
# thumb's base out too and broke arcus_hand_table's PALM_WIDE, the owner's "tosca":
# bowed_human.rs::PALM_SLIM says more; the knuckles alone: build_figures.py KNUCKLE_SPREAD.)
PALM_SLIM = {"woman.fig": 0.86, "man.fig": 1.0}


def slim_palm(fig, s, k):
    B = fig["bones"]
    pos, ids, ws = fig["pos"], fig["ids"], fig["ws"]
    o = side(s)
    wr = B[o + HAND][1]
    across = norm(B[o + finger(1, 0)][1] - B[o + finger(4, 0)][1])
    deltas = []
    for f in range(5):
        h0 = B[o + finger(f, 0)][1]
        d = -across * ((h0 - wr) @ across) * (1.0 - k)
        deltas.append(d)
        for kk in range(3):
            b = o + finger(f, kk)
            par, h, t, x = B[b]
            B[b] = (par, h + d, t + d, x)
    move = np.zeros_like(pos)
    for j in range(4):
        w = ws[:, j]
        for f in range(5):
            m = np.isin(ids[:, j], [o + finger(f, kk) for kk in range(3)])
            move[m] += deltas[f] * w[m, None]
        m = ids[:, j] == o + HAND
        off = (pos[m] - wr) @ across
        move[m] += -across[None, :] * (off * (1.0 - k) * w[m])[:, None]
    pos += move


def knuckle_fix(fig, scale=1.0, slim=1.0):
    B = fig["bones"]
    pos, ids, ws = fig["pos"], fig["ids"], fig["ws"]
    for s in (0, 1):
        if slim != 1.0:
            slim_palm(fig, s, slim)
        o = side(s)
        wr = B[o + HAND][1]
        if scale != 1.0:
            hand = list(range(o + HAND, o + finger(4, 2) + 1))
            share = np.zeros(len(pos))
            for k in range(4):
                share += np.where(np.isin(ids[:, k], hand), ws[:, k], 0.0)
            f = 1.0 - (1.0 - scale) * np.clip(share, 0, 1)
            pos[:] = wr + (pos - wr) * f[:, None]
            for b in hand:
                par, h, t, x = B[b]
                B[b] = (par, wr + (h - wr) * scale, wr + (t - wr) * scale, x)
        heads, axes = [], []
        for f in range(1, 5):
            b = o + finger(f, 0)
            par, h, t, x = B[b]
            ax = norm(h - wr)
            nh = h - ax * (np.linalg.norm(h - wr) * KNUCKLE_BACK)
            B[b] = (par, nh, t, x)
            heads.append(nh)
            axes.append(ax)
        heads, axes = np.array(heads), np.array(axes)
        hb = o + HAND
        has = (ids == hb) & (ws > 0)
        rows = np.nonzero(has.any(1))[0]
        for i in rows:
            j = int(np.nonzero(has[i])[0][0])
            wh = ws[i, j]
            d = pos[i] - wr
            t = d @ axes.T
            lat = np.linalg.norm(d[None, :] - axes * t[:, None], axis=1)
            c = np.exp(-(lat / KNUCKLE_LANE) ** 2)
            if c.sum() < 1e-6:
                continue
            c = c / c.sum()
            u = np.einsum("ij,ij->i", pos[i][None, :] - heads, axes)
            r = np.clip((u - KNUCKLE_RAMP[0]) / (KNUCKLE_RAMP[1] - KNUCKLE_RAMP[0]), 0, 1)
            r = r * r * (3 - 2 * r)
            move = wh * c * r
            if move.sum() < 1e-6:
                continue
            w = {int(ids[i, k]): float(ws[i, k]) for k in range(4) if ws[i, k] > 0}
            w[hb] = w.get(hb, 0.0) - float(move.sum())
            for f in range(4):
                b = o + finger(f + 1, 0)
                w[b] = w.get(b, 0.0) + float(move[f])
            top = sorted(((v, k) for k, v in w.items() if v > 1e-6), reverse=True)[:4]
            tot = sum(v for v, _ in top)
            ids[i] = 0
            ws[i] = 0
            for k, (v, b) in enumerate(top):
                ids[i, k] = b
                ws[i, k] = v / tot


def norm(v):
    return v / max(np.linalg.norm(v), 1e-9)


def frame(x, y):
    y = norm(y)
    xx = x - y * (x @ y)
    if np.linalg.norm(xx) < 1e-5:
        xx = np.cross(y, [0, 0, 1.0])
    x = norm(xx)
    return np.stack([x, y, np.cross(x, y)], 1)  # columns


def about(u, a):
    u = norm(u)
    K = np.array([[0, -u[2], u[1]], [u[2], 0, -u[0]], [-u[1], u[0], 0]])
    return np.eye(3) + np.sin(a) * K + (1 - np.cos(a)) * K @ K


def rest_frame(bone):
    _, h, t, x = bone
    return frame(x, t - h)


def palm_normal(fig, s):
    """Out of the palm at rest."""
    o = side(s)
    B = fig['bones']
    kn = lambda f: B[o + finger(f, 0)][1]
    wr = B[o + HAND][1]
    nb = norm(np.cross(kn(1) - kn(4), (kn(2) + kn(3)) * 0.5 - wr))
    sign = 1 if s == 0 else -1
    if nb[0] * sign < 0:
        nb = -nb
    return -nb


def hand_turn(fig, s, fingers, wrist):
    o = side(s)
    B = fig['bones']
    kn = lambda f: B[o + finger(f, 0)][1]
    rest_mid = (kn(2) + kn(3)) * 0.5
    wr = B[o + HAND][1]
    ry = norm(rest_mid - wr)
    rx = kn(1) - kn(4)
    mid = (fingers[1][0] + fingers[2][0]) * 0.5
    py = norm(mid - wrist)
    px = fingers[0][0] - fingers[3][0]
    return frame(px, py) @ frame(rx, ry).T


def planar(l, target2, couple=0.7, lo=-0.35, hi_pip=1.9):
    """Angles (MCP, PIP, DIP): DIP = couple*PIP; PIP found by bisection so the chain's
    reach equals the target's distance, MCP turns the chain onto it."""
    def chain(t2):
        a2, a3 = t2, t2 + couple * t2
        p = np.array([l[0], 0.0]) + l[1] * np.array([np.cos(a2), np.sin(a2)]) + l[2] * np.array([np.cos(a3), np.sin(a3)])
        return p
    dist = np.linalg.norm(target2)
    if dist >= np.linalg.norm(chain(0.0)):
        t2 = 0.0
    elif dist <= np.linalg.norm(chain(hi_pip)):
        t2 = hi_pip
    else:
        a, b = 0.0, hi_pip
        for _ in range(30):
            m = 0.5 * (a + b)
            if np.linalg.norm(chain(m)) > dist:
                a = m
            else:
                b = m
        t2 = 0.5 * (a + b)
    c = chain(t2)
    t1 = np.arctan2(target2[1], target2[0]) - np.arctan2(c[1], c[0])
    got = np.array([np.cos(t1) * c[0] - np.sin(t1) * c[1], np.sin(t1) * c[0] + np.cos(t1) * c[1]])
    return np.array([t1, t2, couple * t2]), np.linalg.norm(target2 - got)


def natural(l, dist, mcp=0.55, couple=0.7):
    """A relaxed finger's curl reaching ``dist`` from its knuckle: MCP = mcp*c, PIP = c,
    DIP = couple*c."""
    def reach(c):
        a1, a2, a3 = mcp * c, mcp * c + c, mcp * c + c + couple * c
        p = l[0] * np.array([np.cos(a1), np.sin(a1)]) + l[1] * np.array([np.cos(a2), np.sin(a2)]) + l[2] * np.array([np.cos(a3), np.sin(a3)])
        return np.linalg.norm(p)
    a, b = 0.0, 1.6
    if dist >= reach(0):
        return np.array([0.0, 0.0, 0.0])
    for _ in range(30):
        m = 0.5 * (a + b)
        if reach(m) > dist:
            a = m
        else:
            b = m
    c = 0.5 * (a + b)
    return np.array([mcp * c, c, couple * c])


EXACT = [False] * 5
EXACT_F = -1
# r42:arcus-hands — (scene/bowed_human.rs::RELAX, the two alike; was 0.7.)
RELAX = (0.55, 0.86)
# r42:arcus-hands — the last joint's bend for the middle one's (scene/figure/fingers.rs).
COUPLE = 0.7
CURL_MOST = 2.0


def _reach(l, b):
    c = COUPLE * b
    return (l[0] + l[1] * np.cos(b) + l[2] * np.cos(b + c),
            -l[1] * np.sin(b) - l[2] * np.sin(b + c))


def curl(k0, tip, pole, l):
    """``scene/figure/fingers.rs::curl``: a finger's joints from its knuckle ``k0`` to
    ``tip``, its middle and last joints bent together (the last COUPLE of the middle), bent
    away from ``pole``; out of reach it straightens and stretches."""
    d = tip - k0
    dist = np.linalg.norm(d)
    e = d / dist if dist > 1e-5 else np.array([0.0, 1.0, 0.0])
    pr = pole - (k0 + tip) * 0.5
    y = pr - e * (pr @ e)
    if np.linalg.norm(y) < 1e-5:
        y = np.cross(e, [1.0, 0.0, 0.0])
    y = norm(y)
    l = list(l)
    full = sum(l)
    if dist >= full * 0.999:
        k = dist / max(full, 1e-5)
        l, b = [x * k for x in l], 0.0
    else:
        ln = lambda b: np.hypot(*_reach(l, b))
        if ln(CURL_MOST) >= dist:
            b = CURL_MOST
        else:
            lo, hi = 0.0, CURL_MOST
            for _ in range(24):
                m = (lo + hi) * 0.5
                if ln(m) > dist:
                    lo = m
                else:
                    hi = m
            b = (lo + hi) * 0.5
    rx, ry = _reach(l, b)
    phi = -np.arctan2(ry, rx)
    way = lambda a: e * np.cos(a) + y * np.sin(a)
    p1 = k0 + way(phi) * l[0]
    p2 = p1 + way(phi - b) * l[1]
    p3 = p2 + way(phi - b - COUPLE * b) * l[2]
    return [k0, p1, p2, p3]


def pose_hand(fig, s, wrist, fingers, thumb, couple=0.7):
    """Per bone of the hand (HAND and the fingers): (rot 3x3, at) with p -> at + rot(p - head)."""
    o = side(s)
    B = fig['bones']
    R = hand_turn(fig, s, fingers, wrist)
    xf = {}
    wr = B[o + HAND][1]
    xf[o + HAND] = (R, wrist, wr)
    npalm = R @ palm_normal(fig, s)
    for f in range(5):
        tgt = thumb if f == 0 else fingers[f - 1]
        b0, b1, b2 = (B[o + finger(f, k)] for k in range(3))
        k0 = wrist + R @ (b0[1] - wr)
        ls = [np.linalg.norm(b[2] - b[1]) for b in (b0, b1, b2)]
        d = norm(R @ (b0[2] - b0[1]))
        tip = tgt[3]
        if f == 0:
            # The thumb bends in the plane holding its way and its target.
            fl = tip - k0
            fl = norm(fl - d * (fl @ d))
            if fl @ (tgt[1] - k0) < 0:
                pass
        else:
            # Abduct toward the target's side, then flex toward the palm.
            q = tip - k0
            qp = q - npalm * (q @ npalm)
            ab = np.arctan2(np.cross(d, norm(qp)) @ npalm, d @ norm(qp))
            ab = np.clip(ab, -0.45, 0.45)
            d = about(npalm, ab) @ d
            fl = npalm - d * (npalm @ d)
            fl = norm(fl)
        x2 = (tip - k0) @ d
        y2 = (tip - k0) @ fl
        ang, miss = planar(ls, np.array([x2, y2]), couple)
        if not EXACT[f] and f > 0:
            ang = natural(ls, np.hypot(x2, y2))
            miss = -1
        if DEBUG: print('f',f,'x2 %.2f y2 %.2f'%(x2,y2),'ang',np.round(np.degrees(ang)),'miss %.2f'%miss, 'len %.2f'%sum(ls))
        axis = norm(np.cross(d, fl))
        # The chain: each bone's turn = about(axis, cumulative) ∘ (the rest bone way -> d).
        rest_d0 = norm(R @ (b0[2] - b0[1]))
        base = about(np.cross(rest_d0, d) if np.linalg.norm(np.cross(rest_d0, d)) > 1e-6 else axis,
                     np.arccos(np.clip(rest_d0 @ d, -1, 1))) @ R
        at = k0
        cum = 0.0
        for k, b in enumerate((b0, b1, b2)):
            cum += ang[k]
            Rk = about(axis, cum) @ base
            # Rk maps rest bone directions; keep each bone's own rest way relative.
            xf[o + finger(f, k)] = (Rk, at, b[1])
            at = at + Rk @ (b[2] - b[1])
    return xf


def skin_hand(fig, s, xf, wrist_rot=None):
    o = side(s)
    pos, ids, ws = fig['pos'], fig['ids'], fig['ws']
    out = np.zeros_like(pos)
    tot = np.zeros(len(pos))
    for b, (Rk, at, head) in xf.items():
        for j in range(4):
            m = ids[:, j] == b
            if not m.any():
                continue
            w = ws[m, j]
            out[m] += (at + (pos[m] - head) @ Rk.T) * w[:, None]
            tot[m] += w
    return out, tot


def hand_tris(fig, s, min_share=0.6):
    o = side(s)
    ids, ws = fig['ids'], fig['ws']
    hb = set([o + HAND] + [o + finger(f, k) for f in range(5) for k in range(3)])
    share = np.zeros(len(ids))
    for j in range(4):
        share += np.where(np.isin(ids[:, j], list(hb)), ws[:, j], 0)
    t = fig['tris']
    return t[share[t].min(1) > min_share]


def between(a, b):
    a, b = norm(a), norm(b)
    ax = np.cross(a, b)
    c = np.clip(a @ b, -1, 1)
    if np.linalg.norm(ax) < 1e-7:
        return np.eye(3)
    return about(ax, np.arccos(c))


def knee(hip, ankle, a, b, pole):
    dv = ankle - hip
    dist = np.clip(np.linalg.norm(dv), 1.0, a + b - 0.05)
    u = norm(dv)
    x = (a * a - b * b + dist * dist) / (2 * dist)
    h = np.sqrt(max(a * a - x * x, 0))
    side = pole - u * (pole @ u)
    side = norm(side) if np.linalg.norm(side) > 1e-6 else norm(np.cross(u, [0, 0, 1.0]))
    return hip + u * x + side * h


def pose_joints(fig, s, wrist, fingers, thumb):
    """The Rust way (arm_aims): each finger from its own knuckle through a two-bone reach to
    the target tip, bent toward the pose's middle joint; each bone turned from the hand's
    turn by the shortest way to its target way (no twist)."""
    o = side(s)
    B = fig['bones']
    R = hand_turn(fig, s, fingers, wrist)
    xf = {}
    wr = B[o + HAND][1]
    xf[o + HAND] = (R, wrist, wr)
    for f in range(5):
        j = thumb if f == 0 else fingers[f - 1]
        bs = [B[o + finger(f, k)] for k in range(3)]
        l0, l1, l2 = [np.linalg.norm(b[2] - b[1]) for b in bs]
        k0 = wrist + R @ (bs[0][1] - wr)
        tip = j[3]
        L = l0 + l1 + l2
        if f != EXACT_F:
            d_ = tip - k0
            dl = np.clip(np.linalg.norm(d_), RELAX[0] * L, RELAX[1] * L)
            tip = k0 + norm(d_) * dl
        if f > 0:
            # r42:arcus-hands — the three joints together (curl).
            k0, pip, dip, tip = curl(k0, tip, j[1], [l0, l1, l2])
        else:
            reach = np.linalg.norm(tip - k0)
            sc = max(reach / ((l0 + l1 + l2) * 0.97), 1.0)
            l0, l1, l2 = l0 * sc, l1 * sc, l2 * sc
            pole = j[1] - (k0 + tip) * 0.5
            pip = knee(k0, tip, l0, l1 + l2, pole)
            along = norm(tip - pip)
            bend = norm(pole - along * (pole @ along))
            dip = pip + along * (np.linalg.norm(tip - pip) * l1 / (l1 + l2)) + bend * (0.12 * sc)
        at = k0
        for k, (a_, b_) in enumerate([(k0, pip), (pip, dip), (dip, tip)]):
            b = bs[k]
            rest_y = R @ norm(b[2] - b[1])
            Rk = between(rest_y, b_ - a_) @ R
            xf[o + finger(f, k)] = (Rk, a_, b[1])
    return xf
