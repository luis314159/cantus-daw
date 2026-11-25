"""r42:arcus-upper — thinning the live arms' meshes (sheet v7, the frame budget): the arms
are skinned, lit and painted every frame while the figure plays, and MakeHuman's hands are
far denser than a hand on screen needs (a fingertip's hundred triangles are a few pixels).
Short edges inside an arm's own part (not the band it shares with the body, not where two
stuffs meet: a nail's outline, a cuff) are collapsed to their middles, shortest first,
never folding a triangle over, until none is shorter than ``limit`` cm.
"""

import numpy as np


def thin(pos, w, extra, tris, mat, part, limit, movable):
    """``pos`` (n×3), ``w`` (a weight dict per vertex), ``extra`` (arrays per vertex),
    ``tris`` (m×3), ``mat``/``part`` per triangle; ``movable`` a bool per triangle (the
    ones whose vertices may move). Returns the same, thinned (vertices compacted)."""
    pos = pos.copy()
    w = list(w)
    extra = [e.copy() for e in extra]
    tris = [list(t) for t in tris]
    alive = [True] * len(tris)
    vt = [set() for _ in range(len(pos))]
    for k, t in enumerate(tris):
        for v in t:
            vt[v].add(k)

    def free(v):
        ks = vt[v]
        if not ks:
            return False
        m0 = None
        for k in ks:
            if not movable[k]:
                return False
            if m0 is None:
                m0 = mat[k]
            elif mat[k] != m0:
                return False
        return True

    def nbrs(v):
        out = set()
        for k in vt[v]:
            out.update(tris[k])
        out.discard(v)
        return out

    def nrm(t, p):
        a, b, c = p[t[0]], p[t[1]], p[t[2]]
        return np.cross(b - a, c - a)

    for _ in range(12):
        edges = set()
        for k, t in enumerate(tris):
            if not alive[k] or not movable[k]:
                continue
            for a, b in ((t[0], t[1]), (t[1], t[2]), (t[2], t[0])):
                edges.add((min(a, b), max(a, b)))
        cand = []
        for a, b in edges:
            ln = np.linalg.norm(pos[a] - pos[b])
            if ln < limit and free(a) and free(b):
                cand.append((ln, a, b))
        cand.sort()
        locked = set()
        done = 0
        for _, a, b in cand:
            if a in locked or b in locked:
                continue
            na, nb = nbrs(a), nbrs(b)
            if len(na & nb) != 2:
                continue
            mid = (pos[a] + pos[b]) * 0.5
            shared = vt[a] & vt[b]
            ok = True
            for k in (vt[a] | vt[b]) - shared:
                t = tris[k]
                before = nrm(t, pos)
                p2 = {v: pos[v] for v in t}
                for v in t:
                    if v in (a, b):
                        p2[v] = mid
                after = np.cross(p2[t[1]] - p2[t[0]], p2[t[2]] - p2[t[0]])
                la, lb = np.linalg.norm(before), np.linalg.norm(after)
                if lb < 1e-9 or before @ after < 0.3 * la * lb:
                    ok = False
                    break
            if not ok:
                continue
            pos[a] = mid
            wa, wb = w[a], w[b]
            w[a] = {kk: 0.5 * wa.get(kk, 0.0) + 0.5 * wb.get(kk, 0.0) for kk in set(wa) | set(wb)}
            for e in extra:
                e[a] = 0.5 * (e[a] + e[b])
            for k in shared:
                alive[k] = False
                for v in tris[k]:
                    vt[v].discard(k)
            for k in list(vt[b]):
                tris[k] = [a if v == b else v for v in tris[k]]
                vt[a].add(k)
            vt[b] = set()
            locked |= na | nb | {a, b}
            done += 1
        if done == 0:
            break
    keep = [k for k in range(len(tris)) if alive[k]]
    t = np.array([tris[k] for k in keep], dtype=np.int64)
    used = np.unique(t.ravel())
    remap = -np.ones(len(pos), dtype=np.int64)
    remap[used] = np.arange(len(used))
    return (pos[used], [w[i] for i in used], [e[used] for e in extra], remap[t],
            np.asarray(mat)[keep], np.asarray(part)[keep])
