"""A fit with one part of some pieces changed — to find, through the engine, which part of a
piece makes a measure (drums-gen 3: the toms' sub-sonic thump, cue 4).

    python -I variant.py <fit.json> <out.json> <piece,piece…> <op> [<op> …]

Ops: `knock=0` (the stick's knock off), `burst=0`, `early=0` (a drum's early part off),
`modes<HZ` / `modes>HZ` (the modes under / over HZ off), `bands<HZ` (the noise bands under HZ off), `modes@LO-HI` / `bands@LO-HI` (those in a range off),
`glide=0`, `modes=0`, `bands=0`.
"""
import json
import sys


def main():
    src, out, pieces, *ops = sys.argv[1:]
    t = json.load(open(src))
    cen = t["centres"]
    for p in pieces.split(","):
        for lay in t["pieces"][p]:
            for op in ops:
                if op == "knock=0":
                    lay["stick"][1] = 0.0
                elif op == "burst=0":
                    lay["stick"][0] = 0.0
                elif op == "early=0":
                    if "ea" in lay:
                        lay["ea"] = [0.0] * len(lay["ea"])
                elif op == "glide=0":
                    if "glide" in lay:
                        lay["glide"] = [0.0, lay["glide"][1]]
                elif op == "modes=0":
                    lay["modes"] = [[m[0], 0.0] + m[2:] for m in lay["modes"]]
                elif op == "bands=0":
                    lay["env"] = [[-200.0] * len(e) for e in lay["env"]]
                elif op.startswith("modes<"):
                    hz = float(op[6:])
                    lay["modes"] = [[m[0], 0.0 if m[0] < hz else m[1]] + m[2:] for m in lay["modes"]]
                    if "ea" in lay:
                        lay["ea"] = [0.0 if m[0] < hz else e for m, e in zip(lay["modes"], lay["ea"])]
                elif op.startswith("modes>"):
                    hz = float(op[6:])
                    lay["modes"] = [[m[0], 0.0 if m[0] > hz else m[1]] + m[2:] for m in lay["modes"]]
                    if "ea" in lay:
                        lay["ea"] = [0.0 if m[0] > hz else e for m, e in zip(lay["modes"], lay["ea"])]
                elif op.startswith("modes@"):
                    lo, hi = (float(x) for x in op[6:].split("-"))
                    lay["modes"] = [[m[0], 0.0 if lo <= m[0] <= hi else m[1]] + m[2:] for m in lay["modes"]]
                    if "ea" in lay:
                        lay["ea"] = [0.0 if lo <= m[0] <= hi else e for m, e in zip(lay["modes"], lay["ea"])]
                elif op.startswith("bands@"):
                    lo, hi = (float(x) for x in op[6:].split("-"))
                    lay["env"] = [[-200.0] * len(e) if lo <= c <= hi else e for c, e in zip(cen, lay["env"])]
                elif op.startswith("bands<"):
                    hz = float(op[6:])
                    lay["env"] = [[-200.0] * len(e) if c < hz else e for c, e in zip(cen, lay["env"])]
                else:
                    raise SystemExit(f"unknown op {op}")
    json.dump(t, open(out, "w"))


if __name__ == "__main__":
    main()
