"""The eleventh listen's table edits (round 40, drums-gen 3), on a fit before `tables_blob.py`:
each one aimed at one of the gap analysis's open cues, measured through the engine with
`accept_b10.py` (none that passed may fail).

    python -I trim_b11.py <fit.json> <out.json> studio|jazz [edit …]

Edits (all by default):
- `kick_tail` (cue 13, studio): the recorded kick's fundamental rings on under its first fall
  (66-80 Hz at 0.3-0.6 s 3-15 dB under its 50-ms level, T60 3-6 s); ours fell in 0.31 s,
  so at 250-600 ms its partials were broad lobes whose peak sat between them (71.6 Hz at
  v110, 84.4 at v127 against 76) — a second partial at the layer's own fundamental
  (v40's 67 Hz, v110/v127's 76 Hz; v80's at 86 Hz, where its recording leads), `TAIL`
  under it, ringing `TAIL_T60`.
- `wires` (cue 3, studio): the snare's 2.8-13 kHz noise from 18 to 450 ms raised `WIRES_DB`
  per layer (the whole of 3-10 kHz, so it stays as flat as it was) (the wires: 4-12 kHz re the body −16…−21 dB against DRSKit's −9…−12).
- `tom_t60` (cue 5): the drums' partials at 0.6-8 kHz ring no longer than `T60_CAP` (studio
  toms 0.40 s: DRSKit's 0.35-0.45; jazz toms and kick 0.8 s: Virtuosity's 0.7-0.95) — the
  noise's own cap is applied after the colour's corrections, in `tables_blob.py`
  (`<TABLE>_DECAY` = [lo Hz, hi Hz, T60 s]).
- `tom_air` (cue 14): a soft tom's (v40, v80) partials over 3 kHz left to its noise.
"""
import json
import sys

# per layer: (the tail's frequency, Hz — 0: the layer's own strongest partial at 55-90 Hz —
# and its level re that partial, dB)
TAIL = {40: (0.0, -24.0), 80: (86.2, -24.0), 110: (0.0, -24.0), 127: (0.0, -24.0)}
TAIL_T60 = 2.5
WIRES_DB = {40: 0.0, 80: 5.0, 110: 9.0, 127: 9.0}
AIR_DB = 0.0
# (the measure's band, 1-4 kHz, hears its neighbours an octave out through its skirts)
T60_HZ = (600.0, 8000.0)
T60_CAP = {"studio": (("tom_hi", "tom_mid", "tom_floor"), 0.40),
           "jazz": (("tom_hi", "tom_mid", "tom_floor", "kick"), 0.80)}


def kick_tail(fit):
    for lay in fit["pieces"]["kick"]:
        if int(lay["vel"]) not in TAIL:
            continue
        hz, db = TAIL[int(lay["vel"])]
        lo = [m for m in lay["modes"] if 55 <= m[0] <= 90]
        if not lo:
            continue
        j = lay["modes"].index(max(lo, key=lambda m: m[1]))
        m = lay["modes"][j]
        lay["modes"].append([hz or m[0], m[1] * 10 ** (db / 20), TAIL_T60, 0.0, 1.0])
        for k in ("ph", "ep"):
            if k in lay:
                lay[k].append(lay[k][j])
        if "ea" in lay:
            lay["ea"].append(0.0)


def wires(fit):
    cen, bp = fit["centres"], fit["bp_ms"]
    for lay in fit["pieces"]["snare"]:
        d = WIRES_DB.get(int(lay["vel"]), 0.0)
        if d == 0.0:
            continue
        for b, c in enumerate(cen):
            if 2800 <= c <= 13000:
                lay["env"][b] = [x + d if 18 <= t <= 450 and x > -150 else x
                                 for x, t in zip(lay["env"][b], bp)]


def tom_t60(fit, kit):
    pieces, cap = T60_CAP[kit]
    cen, bp = fit["centres"], fit["bp_ms"]
    for p in pieces:
        for lay in fit["pieces"][p]:
            for m in lay["modes"]:
                if T60_HZ[0] <= m[0] <= T60_HZ[1]:
                    m[2] = min(m[2], cap)
            # and the noise there: past its peak a band falls at least 60 dB in `cap`
            for b, c in enumerate(cen):
                if not T60_HZ[0] <= c <= T60_HZ[1]:
                    continue
                e = lay["env"][b]
                k = max(range(len(e)), key=lambda i: e[i])
                lay["env"][b] = [x if i <= k or x <= -150 else min(x, e[k] - 60.0 * (bp[i] - bp[k]) / 1000.0 / cap)
                                 for i, x in enumerate(e)]


def tom_air(fit):
    """Cue 14: a soft tom's 3-8 kHz is noise, not lines (flatness ≥ 0.40; ours 0.27-0.41):
    its partials over 3 kHz at v40 and v80 are left to its bands (`AIR_DB` would raise those
    from 90 to 450 ms: +4 dB made it less flat, 0.41 → 0.39 — the bands' own noise there is
    combed through the mics, so it stays at 0)."""
    cen, bp = fit["centres"], fit["bp_ms"]
    for p in ("tom_hi", "tom_mid", "tom_floor"):
        for lay in fit["pieces"][p]:
            if int(lay["vel"]) > 80:
                continue
            for j, m in enumerate(lay["modes"]):
                if m[0] > 3000:
                    m[1] = 0.0
                    if "ea" in lay:
                        lay["ea"][j] = 0.0
            for b, c in enumerate(cen):
                if 2800 <= c <= 9000:
                    lay["env"][b] = [x + AIR_DB if 90 <= t <= 450 and x > -150 else x
                                     for x, t in zip(lay["env"][b], bp)]


def main():
    src, out, kit, *edits = sys.argv[1:]
    edits = edits or (["kick_tail", "wires", "tom_t60", "tom_air"] if kit == "studio"
                      else ["tom_t60", "tom_air"])
    fit = json.load(open(src))
    for e in edits:
        if e == "kick_tail":
            kick_tail(fit)
        elif e == "wires":
            wires(fit)
        elif e == "tom_t60":
            tom_t60(fit, kit)
        elif e == "tom_air":
            tom_air(fit)
        else:
            raise SystemExit(f"unknown edit {e}")
    json.dump(fit, open(out, "w"))


if __name__ == "__main__":
    main()
