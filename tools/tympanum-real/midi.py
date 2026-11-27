#!/usr/bin/env python3
"""A drum performance's MIDI as the notes `tympanum_real notes` plays (round 42).

    python -I tools/tympanum-real/midi.py <file.mid> <kit info.jsonl> <kit> <out.csv> [--start S --len S]

A minimal Standard MIDI File reader (format 0/1, tempo map, running status), the Groove MIDI
Dataset's Roland TD-11 keys folded into nine classes as its paper does (Gillick et al. 2019,
table 2) and each class sent to the target kit's own key for it (`tympanum_real info`): the
kick, the snare's head / rim / cross-stick, three toms, the hat closed / open / pedal, crash,
ride, the ride's bell. The hat's openness is the TD-11's CC4 (0 open … 127 closed, inverted:
Tympanum's 0 is closed) at the stroke, held at least half open on the open keys and at most a
quarter on the closed ones.
"""
import json
import struct
import sys

# TD-11 key -> class (GMD paper's mapping, with the rim and edge keys kept apart)
GMD = {36: "kick", 38: "snare", 40: "rimshot", 37: "sidestick",
       48: "tom1", 50: "tom1", 45: "tom2", 47: "tom2", 43: "tom3", 58: "tom3",
       46: "hat_open", 26: "hat_open", 42: "hat_closed", 22: "hat_closed", 44: "hat_pedal",
       49: "crash", 55: "crash", 57: "crash", 52: "crash",
       51: "ride", 59: "ride", 53: "bell"}


def vlq(b, i):
    v = 0
    while True:
        c = b[i]
        i += 1
        v = (v << 7) | (c & 0x7F)
        if not c & 0x80:
            return v, i


def read_smf(path):
    b = open(path, "rb").read()
    assert b[:4] == b"MThd"
    _, ntr, div = struct.unpack(">HHH", b[8:14])
    i = 14
    events = []  # (tick, kind, a, b)
    tempos = [(0, 500000)]
    for _ in range(ntr):
        assert b[i:i + 4] == b"MTrk"
        n = struct.unpack(">I", b[i + 4:i + 8])[0]
        j, end, t, status = i + 8, i + 8 + n, 0, 0
        while j < end:
            d, j = vlq(b, j)
            t += d
            c = b[j]
            if c == 0xFF:
                typ, ln = b[j + 1], None
                ln, k = vlq(b, j + 2)
                if typ == 0x51:
                    tempos.append((t, int.from_bytes(b[k:k + 3], "big")))
                j = k + ln
            elif c in (0xF0, 0xF7):
                ln, k = vlq(b, j + 1)
                j = k + ln
            else:
                if c & 0x80:
                    status = c
                    j += 1
                hi = status & 0xF0
                if hi in (0xC0, 0xD0):
                    a1, j = b[j], j + 1
                    events.append((t, hi, a1, 0))
                else:
                    a1, a2 = b[j], b[j + 1]
                    j += 2
                    events.append((t, hi, a1, a2))
        i = end
    tempos.sort()

    def sec(tick):
        s, last_t, us = 0.0, 0, 500000
        for tt, u in tempos:
            if tt > tick:
                break
            s += (tt - last_t) * us / div / 1e6
            last_t, us = tt, u
        return s + (tick - last_t) * us / div / 1e6
    return sorted((sec(t), k, a, c) for t, k, a, c in events)


def kit_keys(info):
    """class -> the kit's key: the piece kind's articulation by id, else its first."""
    pieces = info["pieces"]

    def find(kinds, arts=(), nth=0):
        cands = [p for p in pieces if p["kind"] in kinds]
        if kinds == ("racktom", "floortom"):  # toms high to low by size
            cands.sort(key=lambda p: (p["diameter"] if p["diameter"] > 0 else 99, p["arts"][0]["key"]))
        if not cands:
            return None
        p = cands[min(nth, len(cands) - 1)]
        for a in arts:
            for x in p["arts"]:
                if x["id"] == a:
                    return x["key"]
        return p["arts"][0]["key"] if not arts or arts[-1] != "!" else None
    toms = [p for p in pieces if p["kind"] in ("racktom", "floortom")]
    toms.sort(key=lambda p: (p["diameter"] if p["diameter"] > 0 else 99))
    tk = [p["arts"][0]["key"] for p in toms] or [None]
    pick = lambda i: tk[min(i, len(tk) - 1)] if tk[0] is not None else None  # noqa: E731
    return {
        "kick": find(("kick",), ("center", "closed", "hit")),
        "snare": find(("snare",), ("center", "head", "hit")),
        "rimshot": find(("snare",), ("rimshot", "center")),
        "sidestick": find(("snare",), ("sidestick", "center")),
        "tom1": pick(0), "tom2": pick(len(tk) // 2 if len(tk) > 2 else 1), "tom3": pick(len(tk) - 1),
        "hat_closed": find(("hihat",), ("closed", "tip", "bow")),
        "hat_open": find(("hihat",), ("open", "closed")),
        "hat_pedal": find(("hihat",), ("pedal", "foot", "closed")),
        "crash": find(("crash",), ("bow", "edge")),
        "ride": find(("ride",), ("bow", "tip")),
        "bell": find(("ride",), ("bell", "bow")),
    }


def main():
    mid, infos, kit, out = sys.argv[1:5]
    start = float(sys.argv[sys.argv.index("--start") + 1]) if "--start" in sys.argv else 0.0
    ln = float(sys.argv[sys.argv.index("--len") + 1]) if "--len" in sys.argv else 1e9
    info = next(json.loads(l) for l in open(infos, encoding="utf-8") if json.loads(l)["kit"] == kit)
    keys = kit_keys(info)
    cc4 = 0
    rows = []
    for t, kind, a, b in read_smf(mid):
        if kind == 0xB0 and a == 4:
            cc4 = b
        if kind != 0x90 or b == 0 or a not in GMD:
            continue
        if t < start or t >= start + ln:
            continue
        cls = GMD[a]
        key = keys.get(cls)
        if key is None:
            continue
        open_ = 1.0 - cc4 / 127.0
        if cls == "hat_open":
            open_ = max(open_, 0.5)
        elif cls in ("hat_closed", "hat_pedal"):
            open_ = min(open_, 0.25)
        rows.append(f"{t - start:.6f},{key},{b},{open_:.3f}")
    with open(out, "w", encoding="utf-8") as fh:
        fh.write("seconds,key,velocity,cc4\n" + "\n".join(rows) + "\n")
    print(f"{mid}: {len(rows)} notes -> {out}  keys {keys}")


if __name__ == "__main__":
    main()
