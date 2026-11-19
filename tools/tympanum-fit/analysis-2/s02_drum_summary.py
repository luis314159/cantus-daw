"""Compact per-drum scalar metrics: gen vs rec per velocity, + five recorded kits at v110."""
import sys, json
sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
from feats import *

DRUMS = ["bombo", "caja", "tom-medio", "tom-de-piso"]
F0RANGE = {"bombo": (35, 120), "caja": (130, 330), "tom-medio": (60, 200), "tom-de-piso": (40, 140)}


def ridge_glide(m, on, lo, hi):
    """Track the fundamental ridge backwards from 300-600 ms to 8 ms; return f(8ms), f(late), cents."""
    nper = int(0.03 * SR); hop = int(0.004 * SR)
    n = 1 << 16
    f = np.fft.rfftfreq(n, 1 / SR)
    late = m[on + int(0.25 * SR):on + int(0.6 * SR)]
    X = np.abs(np.fft.rfft(late * np.hanning(len(late)), n))
    k = (f >= lo) & (f <= hi)
    f_late = f[k][np.argmax(X[k])]
    cur = f_late
    track = []
    for t0 in np.arange(int(0.25 * SR), int(0.004 * SR), -hop):
        y = m[on + t0:on + t0 + nper]
        X = np.abs(np.fft.rfft(y * np.hanning(len(y)), n))
        kk = (f >= cur * 0.97) & (f <= cur * 1.12)  # pitch only rises going back in time
        i = np.nonzero(kk)[0]
        j = i[np.argmax(X[i])]
        cur = f[j]
        track.append((t0 / SR * 1000, cur))
    f_early = track[-1][1]
    return f_early, f_late, 1200 * np.log2(f_early / f_late), track


def metrics(m, on, el):
    r = {}
    r["att"] = attack(m, on)[0]
    r["c3"] = first_ms_centroid(m, on, 3) / 1000
    e = lambda lo, hi, a, b: 10 * np.log10(np.mean(bandpass(m, lo, hi)[on + int(a * SR / 1000):on + int(b * SR / 1000)] ** 2) + 1e-30)
    r["click"] = e(2000, 8000, 0, 8) - e(40, 400, 0, 8)          # beater/stick click vs body, first 8 ms
    r["air"] = e(8000, 16000, 0, 30) - e(40, 400, 0, 30)          # top air vs body
    r["mid1040"] = e(1000, 4000, 10, 50) - e(40, 400, 10, 50)     # crack / slap / wires
    r["hi50"] = e(4000, 12000, 50, 300) - e(40, 400, 50, 300)     # late highs vs body (wires, room)
    lv = level_at(m, on, [300, 600, 1000])
    r["L300"], r["L600"], r["L1000"] = lv
    lo, hi = F0RANGE[el]
    t = t60_bands(m, on, ((lo * 0.7, hi * 1.3), (1000, 4000), (4000, 16000)))
    r["T_f0_edt"], r["T_f0"] = t[0]
    r["T_1_4k"] = t[1][1]; r["T_4_16k"] = t[2][1]
    fe, fl, c, _ = ridge_glide(m, on, lo, hi)
    r["f0"] = fl; r["glide"] = c
    r["flat3_10"] = flat_band(m, on, 20, 200, 3000, 10000)
    if el == "caja":
        eh = band_env(m, 150, 400, 0.002)[on:on + int(0.6 * SR)]
        ew = band_env(m, 4000, 10000, 0.002)[on:on + int(0.6 * SR)]
        r["wire_lag"] = (np.argmax(ew) - np.argmax(eh)) / SR * 1000
        dw = 20 * np.log10(ew / ew.max())
        pk = np.argmax(ew)
        r["wire_t20"] = (np.argmax(dw[pk:] < -20)) / SR * 1000
        r["wire_t40"] = (np.argmax(dw[pk:] < -40)) / SR * 1000
    return r


KEYS_ALL = ["att", "c3", "click", "air", "mid1040", "hi50", "L300", "L600", "L1000", "T_f0_edt", "T_f0",
            "T_1_4k", "T_4_16k", "f0", "glide", "flat3_10"]

if __name__ == "__main__":
    out = {}
    for kit in KITS:
        for el in DRUMS:
            H = {v: element_hits(kit, el, v) for v in ("gen", "rec", "prev")}
            keys = KEYS_ALL + (["wire_lag", "wire_t20", "wire_t40"] if el == "caja" else [])
            res = {v: [metrics(*H[v][i], el) for i in range(4)] for v in H}
            kits = {}
            if kit == "studio":  # cross-kit at v80 and v110 once
                for pk in PACKS:
                    kits[pk] = [metrics(*pack_hit(pk, EL2P[el], vel), el) for vel in (80, 110)]
                out[el + "/kits"] = kits
            out[f"{kit}/{el}"] = res
            print(f"\n===== {kit} {el}   (gen / rec per velocity 40 80 110 127)")
            for k in keys:
                s = "  ".join(f"{res['gen'][i][k]:7.2f}/{res['rec'][i][k]:7.2f}" for i in range(4))
                print(f"  {k:9s} {s}   prev v110 {res['prev'][2][k]:7.2f}")
    # kits table
    print("\n===== recorded kits (v80 / v110): DRS Crocell Muldjord BigRusty Virtuosity")
    for el in DRUMS:
        keys = KEYS_ALL + (["wire_lag", "wire_t20", "wire_t40"] if el == "caja" else [])
        print(f"-- {el}")
        for k in keys:
            print(f"  {k:9s} " + "  ".join(f"{out[el + '/kits'][pk][0][k]:7.2f}/{out[el + '/kits'][pk][1][k]:7.2f}" for pk in PACKS))
    json.dump(out, open(OUT + "scripts/s02_drums.json", "w"), default=float)
