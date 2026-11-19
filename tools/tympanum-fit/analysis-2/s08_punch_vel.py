"""Punch (first push vs what follows), velocity response (level/brightness), and groove hit-to-hit variation."""
import sys
sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
from feats import *
from s05_groove import groove, onsets
from scipy.cluster.vq import kmeans2


def punch(m, on):
    m = np.pad(m, (0, SR))
    lp = signal.sosfiltfilt(signal.butter(4, 1500, fs=SR, output="sos"), m)
    a = np.abs(lp[on:on + int(0.006 * SR)]).max(); b = np.abs(lp[on + int(0.006 * SR):on + int(0.06 * SR)]).max()
    y = m[on:on + int(0.1 * SR)]
    crest = db(np.abs(y).max() / np.sqrt(np.mean(y ** 2)))
    return db(a / b), crest


def velresp(kit, el, v):
    out = []
    for m, on in element_hits(kit, el, v):
        m = np.pad(m, (0, SR))
        out.append((db(np.sqrt(np.mean(m[on:on + int(0.3 * SR)] ** 2))), centroid(m[on:on + int(0.1 * SR)]) / 1000))
    return np.array(out)


if __name__ == "__main__":
    print("== punch: first-6-ms low-passed peak re 6-60 ms peak (dB), crest 0-100 ms (dB); v40 v80 v110 v127")
    for el in ("bombo", "caja", "tom-medio", "tom-de-piso"):
        for kit in KITS:
            g = [punch(*element_hits(kit, el, "gen")[i]) for i in range(4)]
            r = [punch(*element_hits(kit, el, "rec")[i]) for i in range(4)]
            print(f"  {kit:6s} {el:12s} punch gen " + " ".join(f"{x[0]:5.1f}" for x in g) + " | rec " + " ".join(f"{x[0]:5.1f}" for x in r)
                  + "   crest gen " + " ".join(f"{x[1]:5.1f}" for x in g) + " | rec " + " ".join(f"{x[1]:5.1f}" for x in r))
        print("     kits v80/v110 punch: " + " ".join(f"{pk} {punch(*pack_hit(pk, EL2P[el], 80))[0]:5.1f}/{punch(*pack_hit(pk, EL2P[el], 110))[0]:5.1f}" for pk in PACKS))
    print("\n== velocity response: rms 0-300 ms re v127 (dB) and centroid 0-100 ms (kHz)")
    for el in ELEMENTS:
        for kit in KITS:
            g = velresp(kit, el, "gen"); r = velresp(kit, el, "rec")
            print(f"  {kit:6s} {el:14s} level gen " + " ".join(f"{x:5.1f}" for x in g[:, 0] - g[3, 0]) + " | rec " + " ".join(f"{x:5.1f}" for x in r[:, 0] - r[3, 0])
                  + "   centroid gen " + " ".join(f"{x:5.2f}" for x in g[:, 1]) + " | rec " + " ".join(f"{x:5.2f}" for x in r[:, 1]))
    print("\n== groove: hit-to-hit variation within clusters of repeated strokes (rec-defined clusters)")
    np.random.seed(1)
    for kit in KITS:
        G = {v: groove(kit, v).mean(1) for v in ("gen", "rec")}
        on_r = onsets(groove(kit, "rec")); on_g = onsets(groove(kit, "gen"))
        pairs = [(on_g[np.argmin(np.abs(on_g - o))], o) for o in on_r if np.min(np.abs(on_g - o)) < 0.008 * SR]
        S = {v: [] for v in G}; W = {v: [] for v in G}
        for og, orr in pairs:
            for v, o in (("gen", og), ("rec", orr)):
                c, L = thirdoct_spec(G[v][o:o + int(0.04 * SR)])
                S[v].append(L)
                W[v].append(G[v][o:o + int(0.04 * SR)])
        Sr = np.array(S["rec"]); Sg = np.array(S["gen"])
        sel = c >= 150
        Sr = np.nan_to_num(Sr, nan=-200.0); Sg = np.nan_to_num(Sg, nan=-200.0); Sr = np.maximum(Sr, Sr.max(1, keepdims=True) - 80); Sg = np.maximum(Sg, Sg.max(1, keepdims=True) - 80)
        shp = Sr[:, sel] - Sr[:, sel].max(1, keepdims=True)
        cen, lab = kmeans2(shp, 6, seed=3, minit="++")
        for ci in range(6):
            mem = np.nonzero(lab == ci)[0]
            if len(mem) < 5:
                continue
            row = []
            for X, Wv in ((Sg, W["gen"]), (Sr, W["rec"])):
                Y = X[mem][:, sel]
                lv = 10 * np.log10(np.sum(10 ** (Y / 10), 1))
                shape = Y - lv[:, None]
                sd = np.median(np.std(shape, 0))
                # waveform similarity of members (max normalised xcorr within +-0.5 ms) on 2-16 kHz
                hp = [bandpass(Wv[i], 2000, 16000) for i in mem]
                cc = []
                for i in range(len(hp) - 1):
                    a, b = hp[i], hp[i + 1]
                    x = signal.correlate(a, b, mode="full")[len(a) - 1 - 24:len(a) + 24]
                    cc.append(np.abs(x).max() / np.sqrt(np.sum(a * a) * np.sum(b * b)))
                row.append((np.std(lv), sd, np.median(cc)))
            print(f"  {kit:6s} cluster n={len(mem):3d} peak {int(c[sel][np.argmax(cen[ci])]):5d} Hz | level sd gen {row[0][0]:4.1f} rec {row[1][0]:4.1f} dB"
                  f" | shape sd/band gen {row[0][1]:4.1f} rec {row[1][1]:4.1f} dB | 2-16k waveform xcorr gen {row[0][2]:.2f} rec {row[1][2]:.2f}")
