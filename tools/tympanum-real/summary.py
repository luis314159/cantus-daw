"""A kit's measured hits (`measure.py`'s JSON lines) summed up per piece kind (round 42).

`load(path)` reads one kit's hits; `summarise(hits)` gives, per kind (kick, snare, racktom,
floortom, hihat, crash, ride …), the numbers the comparison ranks — each a property of the
instrument as it is played across its velocity range, so kits whose velocities mean different
levels still compare:

- `span_db` the level (first 80 ms) from the softest to the hardest velocity, `inversions`
  how many steps up in velocity got quieter, `level_hard` the hardest hit's peak (dBFS);
- `bright_oct_10db` how far the centroid (20–80 ms) climbs, in octaves, per 10 dB of level
  (regression over every hit), `cent_hard` the hardest hit's centroid;
- `glide_hard_c` the pitch glide at the hardest velocity and `glide_soft_c` at the softest
  (cents), `upper_db` the asymmetric modes' share at mid velocity (strike position);
- `t30_<band>` decays at the hardest velocity; `crest_hard_db`, `rise_hard_ms`,
  `stick_hard_db` the attack;
- snare: `wires_db`, `wires_t_ms`, `wires_flat` at mid velocity, `wires_span_db` the wires'
  share from soft to hard (a ghost note is mostly wires);
- cymbals: `flat_4_12`, `bloom_ms` at the hardest, `wash_drop_oct` the centroid from 0.1 to
  1.5 s;
- `take_var_db` the third-octave difference between takes of one velocity (first 200 ms,
  RMS over 63 Hz–16 kHz), `take_level_db` their level spread;
- `side_db`, `corr_late`, `c50_db`, `pan_db` the image and the room.
"""
import json
import math
from collections import defaultdict

import numpy as np

KIND_ALIAS = {"racktom": "tom", "floortom": "tom"}
# the main articulation of each kind: what "the piece" is measured on
MAIN = {"kick": None, "snare": ("center", "head", "hit", "snare"), "tom": None,
        "hihat": ("closed", "tip"), "crash": ("bow", "edge", "crash"), "ride": ("bow", "tip", "ride")}


def load(path):
    out = []
    for line in open(path, encoding="utf-8"):
        r = json.loads(line)
        out.append(r)
    return out


def finite(x):
    return x is not None and isinstance(x, (int, float)) and math.isfinite(x)


def med(v):
    v = [x for x in v if finite(x)]
    return float(np.median(v)) if v else float("nan")


def main_hits(hits, kind):
    """The hits of a kind's main articulation (its first listed piece of that kind)."""
    hs = [h for h in hits if KIND_ALIAS.get(h["kind"], h["kind"]) == kind]
    if not hs:
        return []
    want = MAIN.get(kind)
    arts = list(dict.fromkeys(h["art"] for h in hs))
    art = arts[0]
    if want:
        for w in want:
            if w in arts:
                art = w
                break
        else:
            # an open hat's art is not the closed one: the least open
            if kind == "hihat":
                art = min(arts, key=lambda a: min(h["open"] for h in hs if h["art"] == a))
    cand = [h for h in hs if h["art"] == art]
    # the same piece in every kit: a rack tom on General MIDI's 45 (else 47, 48, 50, else
    # the first rack tom, else a floor tom); the kick on 36; else the first
    pref = {"tom": (45, 47, 48, 50, 43, 41), "kick": (36, 35)}.get(kind, ())
    for k in pref:
        if any(h["key"] == k and h["kind"] != "floortom" for h in cand) or (
                k in (43, 41) and any(h["key"] == k for h in cand)):
            cand = [h for h in cand if h["key"] == k]
            break
    pieces = list(dict.fromkeys(h["piece"] for h in cand))
    return [h for h in cand if h["piece"] == pieces[0]]


def by_vel(hs, key):
    d = defaultdict(list)
    for h in hs:
        if finite(h.get(key)):
            d[h["vel"]].append(h[key])
    return {v: float(np.mean(x)) for v, x in sorted(d.items())}


def summarise_kind(hs, kind):
    s = {}
    if not hs:
        return s
    lv = by_vel(hs, "rms80_db")
    vels = sorted(lv)
    if len(vels) >= 2:
        s["span_db"] = lv[vels[-1]] - lv[vels[0]]
        s["inversions"] = sum(1 for a, b in zip(vels, vels[1:]) if lv[b] < lv[a] - 0.5)
        s["step_min_db"] = min(lv[b] - lv[a] for a, b in zip(vels, vels[1:]))
    hard = [h for h in hs if h["vel"] == vels[-1]]
    soft = [h for h in hs if h["vel"] == vels[0]]
    mid = [h for h in hs if h["vel"] == vels[len(vels) // 2]]
    s["level_hard"] = med([h["peak_db"] for h in hard])
    pts = [(h["rms80_db"], math.log2(h["cent_c"])) for h in hs if finite(h.get("cent_c")) and h["cent_c"] > 0]
    if len(pts) >= 3:
        x, y = np.array(pts).T
        if np.ptp(x) > 3:
            s["bright_oct_10db"] = float(np.polyfit(x, y, 1)[0] * 10.0)
    s["cent_hard"] = med([h.get("cent_c") for h in hard])
    s["cent_attack_hard"] = med([h.get("cent_a") for h in hard])
    s["crest_hard_db"] = med([h["crest10_db"] for h in hard])
    s["rise_hard_ms"] = med([h["rise_ms"] for h in hard])
    s["stick_hard_db"] = med([h["stick_hf"] for h in hard])
    for b in (125, 500, 2000, 8000):
        s[f"t30_{b}"] = med([h.get(f"t30_{b}") for h in hard])
    if kind in ("kick", "snare", "tom"):
        s["f0"] = med([h.get("f0") for h in mid])
        s["glide_hard_c"] = med([h.get("glide_c") for h in hard])
        s["glide_soft_c"] = med([h.get("glide_c") for h in soft])
        s["upper_db"] = med([h.get("upper_db") for h in mid])
        s["late_hf_db"] = med([h.get("late_hf_db") for h in mid])
    if kind == "snare":
        s["wires_db"] = med([h.get("wires_db") for h in mid])
        s["wires_t_ms"] = med([h.get("wires_t_ms") for h in mid])
        s["wires_flat"] = med([h.get("wires_flat") for h in mid])
        s["wires_span_db"] = med([h.get("wires_db") for h in soft]) - med([h.get("wires_db") for h in hard])
    if kind in ("hihat", "crash", "ride"):
        s["flat_4_12"] = med([h.get("flat_4_12") for h in hard])
        s["bloom_ms"] = med([h.get("bloom_ms") for h in hard])
        a, b = med([h.get("cent_t0.1") for h in hard]), med([h.get("cent_t1.5") for h in hard])
        if finite(a) and finite(b) and a > 0 and b > 0:
            s["wash_drop_oct"] = math.log2(a / b)
    # take-to-take variation
    groups = defaultdict(list)
    for h in hs:
        groups[h["vel"]].append(h)
    dv, dl = [], []
    for g in groups.values():
        if len(g) < 2:
            continue
        sp = np.array([h["spec3"] for h in g])[:, 4:29]  # 63 Hz … 16 kHz
        sp = sp - sp.max(axis=1, keepdims=True)
        sp = np.maximum(sp, -60)
        for i in range(len(g)):
            for j in range(i + 1, len(g)):
                dv.append(float(np.sqrt(np.mean((sp[i] - sp[j]) ** 2))))
        dl.append(float(np.std([h["rms80_db"] for h in g])))
    if dv:
        s["take_var_db"] = float(np.median(dv))
        s["take_level_db"] = float(np.median(dl))
    s["side_db"] = med([h["side_late_db"] for h in mid])
    s["side_early_db"] = med([h["side_early_db"] for h in mid])
    s["corr_late"] = med([h["corr_late"] for h in mid])
    s["c50_db"] = med([h["c50_db"] for h in mid])
    s["pan_db"] = med([h["pan_db"] for h in mid])
    return s


def summarise(hits):
    kinds = sorted({KIND_ALIAS.get(h["kind"], h["kind"]) for h in hits})
    return {k: summarise_kind(main_hits(hits, k), k) for k in kinds}
