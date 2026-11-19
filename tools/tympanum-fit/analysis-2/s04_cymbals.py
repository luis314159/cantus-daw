"""Cymbals and hats: scalar battery, gen vs rec per velocity, + recorded kits at v80/v110."""
import sys, json
sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
from feats import *
from scipy import ndimage

CYM = ["hihat-cerrado", "hihat-abierto", "ride", "crash"]
WIN = {"hihat-cerrado": (0.01, 0.12), "hihat-abierto": (0.1, 0.6), "ride": (0.15, 0.65), "crash": (0.15, 0.65)}


def tonal(m, on, el):
    a, b = WIN[el]
    w = m[on + int(a * SR):on + int(b * SR)]
    n = len(w); nfft = 1 << int(np.ceil(np.log2(n)) + 1)
    P = np.abs(np.fft.rfft(w * np.hanning(n), nfft)) ** 2
    f = np.fft.rfftfreq(nfft, 1 / SR); df = f[1]
    Pdb = 10 * np.log10(P + 1e-30)
    med = ndimage.median_filter(Pdb, size=int(300 / df) | 1)
    s = (f >= 4000) & (f < 12000)
    exc = Pdb[s] - med[s]
    pk, _ = signal.find_peaks(exc, height=12, distance=max(1, int(20 / df)))
    hi = f >= 1000
    pk2, _ = signal.find_peaks(Pdb[hi], distance=max(1, int(20 / df)))
    e = np.sort(P[hi][pk2])[::-1]
    # effective components per kHz 6-16 kHz: participation ratio of the spectrum
    s2 = (f >= 6000) & (f < 16000)
    p = P[s2] / P[s2].sum()
    eff = 1 / np.sum(p ** 2) * df / 1000 / 10  # equivalent bins -> 'components' per kHz scale
    return dict(flat=float(flatness(P[s])), peaks=len(pk) / 8.0, top3=float(e[:3].sum() / P[hi].sum() * 100),
                p995=float(np.percentile(exc, 99.5)), eff=float(eff))


def stability(m, on, el):
    nfft = 4096; f = np.fft.rfftfreq(nfft, 1 / SR); s = (f >= 4000) & (f < 16000)
    a, b = (0.03, 0.25) if el == "hihat-cerrado" else (0.2, 1.2)
    cs = []; t = a
    while t + 0.05 + nfft / SR < b:
        def fs(tt):
            i = on + int(tt * SR)
            X = 10 * np.log10(np.abs(np.fft.rfft(m[i:i + nfft] * np.hanning(nfft))) ** 2 + 1e-30)[s]
            return X - ndimage.uniform_filter1d(X, 41)
        cs.append(np.corrcoef(fs(t), fs(t + 0.05))[0, 1]); t += 0.05
    return float(np.mean(cs)) if cs else np.nan


def fluct(m, on, el, lo, hi):
    a, b = (0.02, 0.2) if el == "hihat-cerrado" else (0.1, 1.5)
    e = band_env(m, lo, hi, win=0.002)
    seg = 20 * np.log10(e[on + int(a * SR):on + int(b * SR)] + 1e-12)[::48]
    t = np.arange(len(seg)); p = np.polyfit(t, seg, 3)
    return float(np.std(seg - np.polyval(p, t)))


def metrics(m, on, el):
    r = {}
    m = np.pad(m, (0, int(4 * SR)))
    r["c3"] = first_ms_centroid(m, on, 3) / 1000
    e = lambda lo, hi, a, b: 10 * np.log10(np.mean(bandpass(m, lo, hi)[on + int(a * SR / 1000):on + int(b * SR / 1000)] ** 2) + 1e-30)
    r["hf3"] = e(6000, 20000, 0, 3) - e(200, 1500, 0, 3)
    eh = band_env(m, 8000, 16000, 0.005)[on:on + int(0.6 * SR)]
    r["pk816"] = np.argmax(eh) / SR * 1000
    r["at400"] = 20 * np.log10(eh[int(0.4 * SR)] / eh.max())
    r.update({k: v for k, v in tonal(m, on, el).items()})
    r["stab"] = stability(m, on, el)
    r["fl48"] = fluct(m, on, el, 4000, 8000); r["fl816"] = fluct(m, on, el, 8000, 16000)
    for lo, hi, k in ((500, 1000, "T05_1"), (2000, 4000, "T2_4"), (8000, 16000, "T8_16")):
        r[k] = schroeder_t60(bandpass(m, lo, hi)[on:on + int(3.5 * SR)], lo_db=-5, hi_db=-25)
    tot = e(30, 20000, 0, 2000); r["low450"] = e(30, 450, 0, 2000) - tot
    r["cen0_300"] = centroid(m[on:on + int(0.3 * SR)]) / 1000
    r["cen1_2"] = centroid(m[on + int(1.0 * SR):on + int(2.0 * SR)]) / 1000
    r["peak"] = float(db(np.abs(m).max()))
    if el == "hihat-cerrado":
        ee = env_db(m[on:], 5)
        r["dur40"] = float(np.argmax(ee < ee.max() - 40) / SR * 1000)
        r["siz50"] = 20 * np.log10(band_env(m, 6000, 16000, 0.005)[on + int(0.05 * SR)] / band_env(m, 6000, 16000, 0.005)[on:on + int(0.05 * SR)].max())
    return r


if __name__ == "__main__":
    out = {}
    for el in CYM:
        kits = {pk: [metrics(*pack_hit(pk, EL2P[el], v), el) for v in (80, 110)] for pk in PACKS}
        out[el + "/kits"] = kits
        for kit in KITS:
            H = {v: element_hits(kit, el, v) for v in ("gen", "rec", "prev")}
            res = {v: [metrics(*H[v][i], el) for i in range(4)] for v in H}
            out[f"{kit}/{el}"] = res
            print(f"\n===== {kit} {el}   (gen / rec per velocity 40 80 110 127) | prev v110")
            for k in res["gen"][0]:
                s = "  ".join(f"{res['gen'][i][k]:7.2f}/{res['rec'][i][k]:7.2f}" for i in range(4))
                print(f"  {k:8s} {s}  | {res['prev'][2][k]:7.2f}")
        print(f"\n-- {el} recorded kits v80/v110: DRS Crocell Muldjord BigRusty Virtuosity")
        for k in kits["DRS"][0]:
            print(f"  {k:8s} " + "  ".join(f"{kits[pk][0][k]:7.2f}/{kits[pk][1][k]:7.2f}" for pk in PACKS))
    json.dump(out, open(OUT + "scripts/s04_cym.json", "w"), default=float)
