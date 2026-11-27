"""r42 arcus-classical pass 2 — Variation 2's notes more than 30 dB under its median (the dropouts). python -I paganini_drops.py <W dir> <render.wav in W/work>"""
import sys, json, numpy as np, soundfile as sf
W = sys.argv[1]; f = sys.argv[2]
d = json.load(open(f"{W}/work/notes.json"))
x, sr = sf.read(f"{W}/work/{f}"); x = x.mean(axis=1) if x.ndim > 1 else x
j = d["join"] if not isinstance(d["join"], list) else 0
lv = []
for n in d["notes"]:
    if n["section"] != "var2": continue
    a, b = int(n["p_on"] * sr), int(n["p_off"] * sr)
    seg = x[a:b]
    lv.append((n["p_on"], n["key"], 10*np.log10(np.mean(seg**2)+1e-20) if len(seg) else -200))
med = np.median([l for *_, l in lv])
bad = [(round(t,2), k, round(l-med,1)) for t, k, l in lv if l < med - 30]
print(f, "var2 notes", len(lv), "median", round(med,1), "dropouts (<-30 dB re median):", len(bad), bad[:10])
