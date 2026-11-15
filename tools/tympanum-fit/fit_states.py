"""The hi-hat's half-open states: the open hat of hits files rendered with the recorded
kit's pedal part open (TYMP_HAT_OPEN), fitted as `hat_oNN` and merged into a fit.

    python -I fit_states.py <fit.json> <NN>=<hits file> [<NN>=<hits file> ...]
"""
import json
import sys
sys.path.insert(0, __file__.rsplit("/", 1)[0].rsplit("\\", 1)[0])
import fit

if __name__ == "__main__":
    path = sys.argv[1]
    out = json.load(open(path))
    for arg in sys.argv[2:]:
        nn, f = arg.split("=", 1)
        part = fit.fit(f, ["hat_open"])
        out["pieces"][f"hat_o{nn}"] = part["pieces"]["hat_open"]
        print("fitted", nn, f)
    json.dump(out, open(path, "w"))
