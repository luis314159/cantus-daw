import json, numpy as np
J = json.load(open(__import__("os").path.join(__import__("os").environ.get("TYMP_ANALYSIS_OUT", "analysis-2-out"), "scripts", "s01_drums.json")))
OCT = [31.5, 63, 125, 250, 500, 1000, 2000, 4000, 8000, 16000]
print("gen-rec octave levels per window (each hit's table re its own loudest cell); cols 250 500 1k 2k 4k 8k 16k")
for k, v in J.items():
    g = np.array(v["gen"]["win"]); r = np.array(v["rec"]["win"])
    d = g - r
    print(f"{k:24s} 0-3ms " + " ".join(f"{x:4.0f}" for x in d[0, 3:]) + " | 3-10 " + " ".join(f"{x:4.0f}" for x in d[1, 3:]) + " | 10-30 " + " ".join(f"{x:4.0f}" for x in d[2, 3:]) + " | 100-300 " + " ".join(f"{x:4.0f}" for x in d[4, 3:]))
