"""fiddle_truth.py - the notes a Cantus project asked for, in seconds (the fiddle study's ground
truth for our own renders: it checks the onset and double-stop detectors and gives our side's
exact grid, swing and velocities).

    python -I tools/arcus/fiddle/fiddle_truth.py <project.cantus>      # prints a summary

notes(path) -> {track name: [(t_on, t_off, pitch, velocity, beat_pos, art), ...]}, where
beat_pos is the note's position in its beat (0 = on the beat, 0.5 = the straight off-beat).
The project's tempo is constant in these demos (a tempo map would need the tempo track).
"""
import json
import sys

PPQ = 960


def notes(path):
    d = json.load(open(path, encoding="utf-8"))["project"]
    bpm = float(d["tempo_bpm"])
    sec = 60.0 / bpm / PPQ
    out = {}
    for tr in d["tracks"]:
        rows = []
        for c in tr.get("clips", []):
            if c.get("muted"):
                continue
            base = c["start"] - c.get("offset", 0)
            for n in c.get("notes", []):
                if n.get("muted"):
                    continue
                if n["start"] < c.get("offset", 0) or n["start"] - c.get("offset", 0) >= c["length"]:
                    continue
                tick = base + n["start"]
                rows.append((tick * sec, (tick + n["length"]) * sec, n["pitch"], n["velocity"],
                             (tick % PPQ) / PPQ, n.get("art", {})))
        rows.sort(key=lambda r: r[0])
        out[tr["name"]] = rows
    return out, bpm


if __name__ == "__main__":
    ns, bpm = notes(sys.argv[1])
    print("tempo", bpm)
    for k, v in ns.items():
        if not v:
            continue
        iois = [b[0] - a[0] for a, b in zip(v, v[1:]) if b[0] > a[0]]
        iois.sort()
        med = iois[len(iois) // 2] if iois else 0
        pos = {}
        for r in v:
            pos[round(r[4], 3)] = pos.get(round(r[4], 3), 0) + 1
        print(f"{k}: {len(v)} notes, median IOI {med:.3f} s, beat positions {sorted(pos.items())[:8]}")
