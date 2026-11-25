"""r42:arcus-hand — the violin's left hand studied against photographs (the owner, 2026-11-25:
"URGENT LA MANO IZQUIERDA DEL VIOLIN", after three passes still "un desastre").

Renders a solved pose of ``left_hand.py`` (its caches beside the dump) from the app's own
camera (the true eye, a narrow field round the hand), from a camera like the owner's
photograph 5 (the audience's side, square to the neck, a little above) and photograph 2
(square to the neck, level), and from above (photograph 4), with the landmarks marked; and
measures the landmarks ``left_hand_refs.md`` holds for the photographs:

- the forearm's angle to the neck in the picture, the wrist's bend (3-D);
- the knuckle line: each base knuckle's height over the fingerboard's top and its distance
  out from the E edge;
- each finger's arch (its middle joint's height over the top) and its MCP / PIP / DIP;
- the playing tip's contact angle (its last phalanx against the fingerboard's normal);
- the thumb's tip (across the neck, height against the top, along it);
- the gap under the neck (the palm's nearest probe to the neck);
- the hand's roll about the neck (the back's normal in the neck's cross-section: 0 = the
  back faces the E side square, + = turned up toward the board's normal);
- which way the back of the hand faces the app's camera.

    python tools/arcus-figures/hand_study.py <dump.json> <out dir> [who inst string finger at]
    python tools/arcus-figures/hand_study.py <dump.json> <out dir> --table   (every case's numbers)

Study only: the photographs are never in the repository or the app.
"""
import json
import os
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import hand_rig as hr  # noqa: E402
import left_hand as L  # noqa: E402


def pose_of(cache, who, inst, string, finger, at):
    c = cache[f"{who}-{inst}-{string}-{finger}-{at:g}"]
    W, Rh = np.array(c["W"]), np.array(c["Rh"])
    Q = np.array(c["Q"]).reshape(5, 3, 3, 3)
    way = np.array(c["fore"]) if "fore" in c else None
    return W, Rh, Q, c.get("cost", 0.0), way


def cameras(F, nk, c):
    """(name, eye, at, up, fov): the app's camera from its true eye; photo 5's and photo 2's
    (square to the neck from the audience's side, 8 and 0 degrees over the board's plane);
    photo 4's (from over the G side, the player's eye)."""
    eye = np.array(F["eye"])
    d = np.linalg.norm(eye - c)
    out = [("app", eye, c, np.array(F["cam_u"]), 2 * np.arctan(11.0 / d))]
    # The app's camera turned about the room's vertical (``HAND_AZ`` degrees; a camera
    # squarer to the neck, as photograph 5's).
    az = float(os.environ.get("HAND_AZ", "0"))
    el = float(os.environ.get("HAND_EL", "0"))
    if az or el:
        e = c + hr.about(np.array([0.0, 1.0, 0.0]), np.radians(az)) @ (eye - c)
        side = hr.norm(np.cross(np.array([0.0, 1.0, 0.0]), e - c))
        e = c + hr.about(side, np.radians(-el)) @ (e - c)
        out.append((f"app{az:+.0f}", e, c, np.array([0.0, 1.0, 0.0]), 2 * np.arctan(11.0 / d)))
    ey = nk.E[1]
    front = np.array([0.0, 0.0, 1.0])
    sq = hr.norm(front - ey * (front @ ey))
    up = np.array([0.0, 1.0, 0.0])
    for name, el in (("photo5", 8.0), ("photo2", 0.0)):
        side = hr.norm(np.cross(up, sq))
        lift = hr.norm(np.cross(sq, side))
        v = hr.norm(sq * np.cos(np.radians(el)) + lift * np.sin(np.radians(el)))
        out.append((name, c + v * 300.0, c, up, 2 * np.arctan(11.0 / 300.0)))
    kx, z, bw = nk.E[0], nk.E[2], -nk.E[1]
    out.append(("photo4", c - kx * 16.0 + z * 20.0 - bw * 10.0, c, z, 0.9))
    return out


def project(p, eye, at, up, fov, size):
    import preview
    e, r, u, f = preview.look(eye, at, up)
    w, h = size
    rel = np.asarray(p) - e
    zz = rel @ f
    s = h / (2.0 * np.tan(fov * 0.5))
    return np.stack([w * 0.5 + (rel @ r) / zz * s, h * 0.5 - (rel @ u) / zz * s], -1)


def elbow_of(F, W, P, way):
    """The elbow: on the forearm's way when the row keeps one, else on the hand's line."""
    mid = (P[2, 0] + P[3, 0]) * 0.5
    if way is None:
        way = hr.norm(W - mid)
    return W + way * F["fore"]


def measures(h, nk, F, row, W, Rh, Q, way=None):
    """The landmarks of one pose (dict of numbers; cm and degrees)."""
    P = h.fk(W, Rh, Q)
    ang = h.angles(W, Rh, P)
    loc = nk.loc
    m = {}
    hw = lambda y: nk.hw(y)  # noqa: E731
    top = lambda y: nk.top(y)  # noqa: E731
    m["knuckles"] = []
    for f in range(1, 5):
        q = loc(P[f, 0])
        m["knuckles"].append((round(q[0] - hw(q[1]), 2), round(q[2] - top(q[1]), 2)))
    m["arch"] = []
    for f in range(1, 5):
        q = loc(P[f, 1])
        m["arch"].append(round(q[2] - top(q[1]), 2))
    m["angles"] = np.round(ang[1:], 0).astype(int).tolist()
    p = row["playing"]
    if p >= 0:
        d = hr.norm(P[p + 1, 3] - P[p + 1, 2])
        m["tip_angle"] = round(float(np.degrees(np.arccos(np.clip(d @ (-nk.E[2]), -1, 1)))), 1)
    qt = loc(P[0, 3])
    m["thumb"] = (round(qt[0] + hw(qt[1]), 2), round(qt[2] - top(qt[1]), 2),
                  round(qt[1] - loc(P[1, 3])[1], 2))
    # The wrist: the forearm (elbow -> wrist, carried on the hand's line in the solve) and
    # the hand's own line (wrist -> the middle knuckles).
    mid = (P[2, 0] + P[3, 0]) * 0.5
    hand_dir = hr.norm(mid - W)
    sh = np.array(F["shoulder"])
    el = elbow_of(F, W, P, way)
    fa = hr.norm(W - el)
    m["wrist_dev"] = round(float(np.degrees(np.arccos(np.clip(fa @ hand_dir, -1, 1)))), 1)
    m["elbow_loc"] = np.round(loc(el), 1).tolist()
    m["upper_len_err"] = round(float(np.linalg.norm(el - sh) - F["upper"]), 2)
    npalm = Rh @ h.n
    back = -npalm
    bl = nk.E @ back
    m["roll"] = round(float(np.degrees(np.arctan2(bl[2], bl[0]))), 1)
    m["back_yaw"] = round(float(np.degrees(np.arctan2(-bl[1], bl[0]))), 1)
    eye = np.array(F["eye"])
    m["back_to_cam"] = round(float(back @ hr.norm(eye - mid)), 2)
    # The gap: the palm's middle (wrist -> knuckles at 0.45 / 0.8) to the neck.
    gaps = []
    for f in (1, 2, 3, 4):
        for t in (0.45, 0.8):
            q = W + (P[f, 0] - W) * t + npalm * 1.1
            gaps.append(float(nk.outside(q[None])[0]))
    m["gap"] = round(min(gaps), 2)
    # The forearm against the neck, in the app's picture.
    pr = lambda a: project(a, eye, mid, np.array(F["cam_u"]), 0.1, (1000, 1000))  # noqa
    a2 = pr(el) - pr(W)
    n2 = pr(nk.world([0, nk.nut_y, 0])) - pr(nk.world([0, nk.body, 0]))
    cosv = (a2 @ n2) / np.linalg.norm(a2) / np.linalg.norm(n2)
    m["forearm_vs_neck_app"] = round(float(np.degrees(np.arccos(np.clip(cosv, -1, 1)))), 1)
    m["forearm_up_app"] = round(float(np.degrees(np.arctan2(-(-a2[1]), -a2[0]))), 1)
    return m, P


def picture(h, nk, F, W, P, cam, way=None):
    """The landmarks as a photograph shows them (``left_hand_refs.md``'s picture measures):
    from camera ``cam`` (name, eye, at, up, fov), the forearm's and the hand's angles to the
    neck's line (deg), the wrist's bend in the picture, the base knuckles' and the arch's
    height over the fingerboard's top edge in neck thicknesses (the E edge's top to the
    neck's underside, both as the picture shows them), the back of the hand's width over its
    length."""
    name, eye, at, up, fov = cam
    pr = lambda q: project(np.asarray(q), eye, at, up, fov, (1000, 1000))  # noqa: E731
    mid = (P[2, 0] + P[3, 0]) * 0.5
    el = elbow_of(F, W, P, way)
    y = nk.loc(mid)[1]
    a, b = pr(nk.world([nk.hw(y - 3), y - 3, nk.top(y - 3)])), pr(nk.world([nk.hw(y + 3), y + 3, nk.top(y + 3)]))
    under = pr(nk.world([nk.hw(y) * 0.6, y, nk.top(y) - nk.depth]))
    t = (b - a) / np.linalg.norm(b - a)
    nrm = np.array([-t[1], t[0]])
    top_at = pr(nk.world([nk.hw(y), y, nk.top(y)]))
    if (under - top_at) @ nrm > 0:
        nrm = -nrm  # "up" away from the neck's underside
    thick = abs((under - top_at) @ nrm)
    over = lambda q: float((pr(q) - top_at) @ nrm / thick)  # noqa: E731
    ang = lambda u: float(np.degrees(np.arccos(abs(u @ t) / np.linalg.norm(u))))  # noqa: E731
    fa = pr(W) - pr(el)
    ha = pr(mid) - pr(W)
    return {
        "forearm_vs_neck": round(ang(fa), 0),
        "hand_vs_neck": round(ang(ha), 0),
        "wrist_bend": round(float(np.degrees(np.arccos(np.clip(fa @ ha / np.linalg.norm(fa) / np.linalg.norm(ha), -1, 1)))), 0),
        "knuckles_over": [round(over(P[f, 0]), 2) for f in range(1, 5)],
        "arch_over": round(max(over(P[f, k]) for f in range(1, 5) for k in (1, 2)), 2),
        "back_w_over_l": round(float(np.linalg.norm(pr(P[1, 0]) - pr(P[4, 0])) / np.linalg.norm(ha)), 2),
    }


def render_case(dump, cache, outdir, who, inst, string, finger, at, tag=None, size=(560, 480)):
    from PIL import Image, ImageDraw
    F = next(F for F in dump["figs"] if F["who"] == who and F["inst"] == inst)
    h = L.Hand(who)
    nk = L.Neck(F)
    row = next(r for r in F["rows"] if r["string"] == string and r["finger"] == finger
               and abs(r["at"] - at) < 1e-3)
    W, Rh, Q, cost, way = pose_of(cache, who, inst, string, finger, at)
    m, P = measures(h, nk, F, row, W, Rh, Q, way)
    hp, ht, hc = L.hand_mesh(h, W, Rh, Q, P, way)
    npos, ntri, ncol = L.neck_mesh(nk, F)
    pos = np.vstack([hp, npos])
    tris = np.vstack([ht, ntri + len(hp)])
    cols = np.vstack([hc, ncol])
    c = (P[1, 0] + P[4, 0] + W) / 3.0
    tag = tag or f"{who}-{inst}-s{string}-f{finger}-at{at:g}"
    for cam in cameras(F, nk, c):
        if cam[0] == "photo4":
            continue
        m["pic_" + cam[0]] = picture(h, nk, F, W, P, cam, way)
    ims = []
    for name, eye, tgt, up, fov in cameras(F, nk, c):
        o = os.path.join(outdir, f"study-{tag}-{name}.png")
        L.render(pos, tris, cols, eye, tgt, up, size, o, fov=fov)
        im = Image.open(o).convert("RGB")
        d = ImageDraw.Draw(im)
        for f in range(5):
            pts = project(P[f], eye, tgt, up, fov, size)
            col = (220, 40, 40) if f == 0 else (40, 90, 220)
            for x, y in pts:
                d.ellipse([x - 2, y - 2, x + 2, y + 2], outline=col)
        d.text((6, 4), name, fill=(0, 0, 0))
        ims.append(im)
    sheet = Image.new("RGB", (size[0] * len(ims), size[1] + 16), (255, 255, 255))
    for i, im in enumerate(ims):
        sheet.paste(im, (size[0] * i, 16))
    ImageDraw.Draw(sheet).text((4, 2), f"{tag}  cost {cost:.1f}  {m}", fill=(0, 0, 0))
    out = os.path.join(outdir, f"study-{tag}.png")
    sheet.save(out)
    return out, m


def main():
    dump_path, outdir = sys.argv[1], sys.argv[2]
    dump = json.load(open(dump_path))
    here = os.path.dirname(os.path.abspath(dump_path))
    os.makedirs(outdir, exist_ok=True)

    def cache_of(key):
        return json.load(open(os.path.join(here, f"left_hand_cache-{key}.json")))

    if "--table" in sys.argv:
        for F in dump["figs"]:
            key = f'{F["who"]}-{F["inst"]}'
            cache = cache_of(key)
            h = L.Hand(F["who"])
            nk = L.Neck(F)
            for row in F["rows"]:
                if row["string"] not in (0, 3) or row["finger"] not in (0, 4):
                    continue
                W, Rh, Q, cost, way = pose_of(cache, F["who"], F["inst"], row["string"],
                                              row["finger"], row["at"])
                m, _ = measures(h, nk, F, row, W, Rh, Q, way)
                print(key, row["string"], row["finger"], row["at"], round(cost, 1), m)
        return
    a = [x for x in sys.argv[3:] if not x.startswith("--")]
    who, inst = (a + ["woman", "violin"])[:2]
    string = int(a[2]) if len(a) > 2 else 2
    finger = int(a[3]) if len(a) > 3 else 0
    at = float(a[4]) if len(a) > 4 else 1.0
    out, m = render_case(dump, cache_of(f"{who}-{inst}"), outdir, who, inst, string, finger, at)
    print(out)
    for k, v in m.items():
        print(f"  {k}: {v}")


if __name__ == "__main__":
    main()
