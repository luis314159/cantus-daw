import numpy as np, soundfile as sf, json
D = r"F:\projects\daw-r42\downloads\study\zenodo-lampis-bridge-force-static-dynamic"
A = np.genfromtxt(D + r"\features.csv", delimiter=",", names=True)
out = []
for r in A:
    if r["Fb"] <= 0: continue
    x, sr = sf.read(D + rf"\DATA\processed\{int(r['n_ref'])}.wav")
    s = x[:, 1] / 0.0922 * np.sqrt(2)
    s = s - s.mean()
    n = len(s); w = np.hanning(n); f0 = r["f0"]
    t = np.arange(n) / sr
    hs = []
    for h in range(1, 41):
        best = 0
        for k in np.linspace(-0.01, 0.01, 9):
            f = h * f0 * (1 + k)
            best = max(best, abs(np.sum(s * w * np.exp(-2j * np.pi * f * t))) ** 2)
        hs.append(best)
    hs = np.array(hs); nn = np.arange(1, 41)
    cen = float((nn * hs).sum() / hs.sum())
    db = 10 * np.log10(hs / hs.max())
    out.append({"beta": float(r["beta"]), "vb": float(r["vb"]), "Fb": float(r["Fb"]), "f0": float(f0), "cen_harm": cen, "h_db": [round(float(v), 1) for v in db]})
json.dump(out, open(r"lampis_harmonics.json", "w"))
X = np.array([[1, np.log2(o["Fb"]), np.log2(o["beta"]), np.log2(o["vb"])] for o in out]); y = np.log2([o["cen_harm"] for o in out])
c = np.linalg.lstsq(X, y, rcond=None)[0]
print("real cello A string, harmonic power centroid (harmonic numbers): median %.2f, p10..p90 %s" % (np.median(2**y), np.percentile(2**y, [10, 90]).round(2)))
print("  oct per doubling: F %.3f beta %.3f v %.3f" % tuple(c[1:]))
for o in out[::8]:
    print("  b %.3f v %.2f F %.2f: cen %.2f h2..10 %s" % (o["beta"], o["vb"], o["Fb"], o["cen_harm"], o["h_db"][1:10]))
