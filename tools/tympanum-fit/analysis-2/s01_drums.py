"""Drums (kick, snare, toms): the battery, gen vs rec (+ prev, + other recorded kits)."""
import sys, json
sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
from feats import *

DRUMS = ["bombo", "caja", "tom-medio", "tom-de-piso"]
PITCH = {"bombo": (35, 150), "caja": (120, 400), "tom-medio": (60, 250), "tom-de-piso": (45, 160)}


def battery(m, on, el):
    f = {}
    f["peak"] = float(db(np.abs(m).max()))
    f["rms300"] = float(db(np.sqrt(np.mean(m[on:on + int(0.3 * SR)] ** 2))))
    f["att_pk_ms"], f["rise_ms"] = attack(m, on)
    f["c3"] = first_ms_centroid(m, on, 3)
    f["c10_30"] = centroid(m[on + int(0.01 * SR):on + int(0.03 * SR)])
    f["c30_200"] = centroid(m[on + int(0.03 * SR):on + int(0.2 * SR)])
    f["lvl"] = level_at(m, on, [50, 100, 200, 300, 600, 1000, 1400])
    f["t60"] = t60_bands(m, on)
    lo, hi = PITCH[el]
    f["pitch"] = pitch_track(m, on, lo, hi)
    f["partials"] = low_partials(m, on)
    f["flat_3_10k_20_200"] = flat_band(m, on, 20, 200, 3000, 10000)
    f["flat_1_4k_20_200"] = flat_band(m, on, 20, 200, 1000, 4000)
    f["win"] = window_table(m, on).round(1).tolist()
    f["floor"] = noise_floor(m, on)
    return f


def fmt(f):
    p = f["pitch"]
    gl = 1200 * np.log2(p[0] / p[-2]) if p[0] and p[-2] else np.nan
    return (f"pk{f['peak']:6.1f} rms{f['rms300']:6.1f} att{f['att_pk_ms']:5.2f}/{f['rise_ms']:4.2f}ms "
            f"c3 {f['c3']:6.0f} c10-30 {f['c10_30']:6.0f} c30-200 {f['c30_200']:5.0f} | "
            f"lvl@100/300/600/1000 {f['lvl'][1]:5.1f} {f['lvl'][3]:5.1f} {f['lvl'][4]:5.1f} {f['lvl'][5]:5.1f} | "
            f"pitch {p[0]:5.1f}->{p[3]:5.1f}->{p[4]:5.1f} glide {gl:5.0f}c | fl3-10k {f['flat_3_10k_20_200']:.2f} "
            f"fl1-4k {f['flat_1_4k_20_200']:.2f}")


if __name__ == "__main__":
    allres = {}
    for kit in KITS:
        for el in DRUMS:
            print(f"\n===== {kit} {el} =====")
            H = {v: element_hits(kit, el, v) for v in ("prev", "gen", "rec")}
            for vi, vel in enumerate(VELS):
                print(f"-- v{vel}")
                res = {}
                for v in ("prev", "gen", "rec"):
                    m, on = H[v][vi]
                    res[v] = battery(m, on, el)
                    print(f"  {v:4s} {fmt(res[v])}")
                for pk in PACKS:
                    m, on = pack_hit(pk, EL2P[el], vel)
                    r = battery(m, on, el)
                    res[pk] = r
                    print(f"  {pk[:4]:4s} {fmt(r)}")
                for v in ("gen", "rec"):
                    t = res[v]["t60"]
                    print(f"  T60 {v:4s} " + " ".join(f"{a:5.2f}/{b:5.2f}" for a, b in t) +
                          "   (EDT/late; 40-100,100-250,250-1k,1-4k,4-16k)")
                print(f"  partials gen {res['gen']['partials']}")
                print(f"  partials rec {res['rec']['partials']}")
                g = np.array(res["gen"]["win"]); r = np.array(res["rec"]["win"])
                print("  win gen-rec (rows 0-3,3-10,10-30,30-100,100-300,300-1000 ms; cols 31..16k):")
                for row, w in zip(g - r, WINS):
                    print(f"    {w[0]:4d}-{w[1]:<4d} " + " ".join(f"{x:5.0f}" for x in row))
                print("  rec win: ")
                for row, w in zip(r, WINS):
                    print(f"    {w[0]:4d}-{w[1]:<4d} " + " ".join(f"{x:5.0f}" for x in row))
                mg, og = H["gen"][vi]; mr, orr = H["rec"][vi]
                pdiff = proxy_diff(mg, og, mr, orr)
                print("  proxy |d| 0-10/10-50/50-200/200-800: " + " ; ".join(f"{a:4.1f} {b}" for a, b in pdiff))
                print(f"  floor (pre, last50ms) gen {res['gen']['floor']} rec {res['rec']['floor']}")
                allres[f"{kit}/{el}/{vel}"] = {k: v for k, v in res.items()}
    json.dump(allres, open(OUT + "scripts/s01_drums.json", "w"), default=float, indent=0)
