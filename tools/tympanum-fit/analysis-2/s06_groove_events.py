"""Groove per event: matched onsets gen vs rec — attack spectra, the floor between hits per band,
the sub-bass excess, and hit-to-hit variation of repeated strokes (round-robin realism)."""
import sys
sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
from feats import *
from s05_groove import groove, onsets

BANDS = [(30, 60), (60, 120), (120, 400), (400, 1500), (1500, 5000), (5000, 16000)]


def band_db(m, lo, hi, a, b):
    y = bandpass(m, lo, hi)[a:b]
    return 10 * np.log10(np.mean(y * y) + 1e-20)


if __name__ == "__main__":
    for kit in KITS:
        G = {v: groove(kit, v).mean(1) for v in ("gen", "rec")}
        Bf = {v: {bd: bandpass(G[v], *bd) for bd in BANDS} for v in G}
        on_r = onsets(groove(kit, "rec")); on_g = onsets(groove(kit, "gen"))
        pairs = []
        for o in on_r:
            j = np.argmin(np.abs(on_g - o))
            if abs(on_g[j] - o) < 0.008 * SR:
                pairs.append((on_g[j], o))
        print(f"\n===== {kit} groove: {len(on_r)} rec onsets, {len(on_g)} gen onsets, {len(pairs)} matched")
        # floor before each onset (10 ms before) re the following 30 ms, per band; and attack 0-30 ms gen-rec per band
        fl = {v: [] for v in G}; att = []
        for og, orr in pairs:
            for v, o in (("gen", og), ("rec", orr)):
                row = []
                for bd in BANDS:
                    y = Bf[v][bd]
                    pre = 10 * np.log10(np.mean(y[o - int(0.012 * SR):o - int(0.002 * SR)] ** 2) + 1e-20)
                    post = 10 * np.log10(np.mean(y[o:o + int(0.03 * SR)] ** 2) + 1e-20)
                    row.append(pre - post)
                fl[v].append(row)
            att.append([10 * np.log10(np.mean(Bf["gen"][bd][og:og + int(0.03 * SR)] ** 2) + 1e-20)
                        - 10 * np.log10(np.mean(Bf["rec"][bd][orr:orr + int(0.03 * SR)] ** 2) + 1e-20) for bd in BANDS])
        fg = np.median(fl["gen"], 0); fr = np.median(fl["rec"], 0)
        print("  bands: " + " ".join(f"{lo}-{hi}" for lo, hi in BANDS))
        print("  floor before onsets re next 30 ms (median dB) gen: " + " ".join(f"{x:6.1f}" for x in fg))
        print("                                              rec: " + " ".join(f"{x:6.1f}" for x in fr))
        a = np.array(att)
        print("  attack 0-30 ms gen-rec (median, IQR):  " + " ".join(f"{np.median(a[:,i]):5.1f}({np.percentile(a[:,i],25):4.0f},{np.percentile(a[:,i],75):4.0f})" for i in range(len(BANDS))))
        # sub-bass over time: 30-60 Hz level per second
        for v in G:
            y = Bf[v][(30, 60)]; z = Bf[v][(60, 120)]
            s = [10 * np.log10(np.mean(y[int(t * SR):int((t + 1) * SR)] ** 2) + 1e-20) for t in range(14)]
            s2 = [10 * np.log10(np.mean(z[int(t * SR):int((t + 1) * SR)] ** 2) + 1e-20) for t in range(14)]
            print(f"  {v} 30-60 Hz per second (dB): " + " ".join(f"{x:5.0f}" for x in s))
            print(f"  {v} 60-120 Hz per second (dB): " + " ".join(f"{x:5.0f}" for x in s2))
        # strongest low lines in the last 4 s (ring-out)
        for v in G:
            y = G[v][int(10 * SR):int(14 * SR)]
            n = 1 << 19
            X = 20 * np.log10(np.abs(np.fft.rfft(y * np.hanning(len(y)), n)) + 1e-12)
            f = np.fft.rfftfreq(n, 1 / SR); k = (f > 25) & (f < 400)
            pk, _ = signal.find_peaks(X[k], prominence=10, distance=200)
            o = np.argsort(X[k][pk])[::-1][:6]
            print(f"  {v} ring-out 10-14 s low lines: " + ", ".join(f"{f[k][pk][i]:.1f}Hz {X[k][pk][i]-X[k].max():.0f}" for i in sorted(o, key=lambda i: f[k][pk][i])))
        # hit-to-hit variation: events grouped by spectral shape (1/3-oct, 0-50 ms); within a group, the
        # dB difference between successive members (std over bands) and their level spread.
        for v in G:
            idx = [p[0] if v == "gen" else p[1] for p in pairs]
            S = []
            for o in idx:
                c, L = thirdoct_spec(G[v][o:o + int(0.05 * SR)])
                S.append(L)
            S = np.array(S)
            Sr = S - S.max(1, keepdims=True)
            # groups from the REC shape so that both versions use the same membership
            if v == "rec":
                pass
            print(f"  {v}: computed {len(S)} event spectra")
            np.save(OUT + f"scripts/ev_{kit}_{v}.npy", S)
        Sg = np.load(OUT + f"scripts/ev_{kit}_gen.npy"); Sr_ = np.load(OUT + f"scripts/ev_{kit}_rec.npy")
        # cluster on rec shapes (normalised) with a simple greedy threshold
        shp = Sr_ - Sr_.max(1, keepdims=True)
        lab = -np.ones(len(shp), int); cents = []
        for i in range(len(shp)):
            for ci, cc in enumerate(cents):
                if np.sqrt(np.mean((shp[i] - cc) ** 2)) < 4.0:
                    lab[i] = ci; break
            if lab[i] < 0:
                cents.append(shp[i]); lab[i] = len(cents) - 1
        for ci in range(len(cents)):
            mem = np.nonzero(lab == ci)[0]
            if len(mem) < 6:
                continue
            out = []
            for S in (Sg, Sr_):
                X = S[mem]
                lv = 10 * np.log10(np.sum(10 ** (X / 10), 1))
                shape = X - lv[:, None]
                # band-wise spread of the shape (dB std across members), A-relevant bands 200 Hz-16 kHz
                sel = (c >= 200)
                spread = np.median(np.std(shape[:, sel], 0))
                d = np.diff(shape[:, sel], axis=0)
                succ = np.median(np.sqrt(np.mean(d ** 2, 1)))
                out.append((np.std(lv), spread, succ))
            print(f"  group {ci} n={len(mem):3d} peak band {int(c[np.argmax(cents[ci])])} Hz | level sd gen {out[0][0]:4.1f} rec {out[1][0]:4.1f} dB"
                  f" | shape sd/band gen {out[0][1]:4.1f} rec {out[1][1]:4.1f} | successive rms diff gen {out[0][2]:4.1f} rec {out[1][2]:4.1f} dB")
