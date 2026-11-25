import sys, glob, re, numpy as np, soundfile as sf
from collections import defaultdict
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "sound-refs")); import cantus_paths as cp  # noqa: E402
root = str(cp.refs("study", "zenodo-lampis-bridge-force-attack", "waveforms"))  # $CANTUS_REFS
P = 50000 / 98.0
res = defaultdict(list)
files = sorted(glob.glob(root + r"\model_*_2\*\*.wav"))
for p in files:
    m = re.search(r"Fb_([\d.]+)_a_([\d.]+)\.wav", p)
    F, a = float(m.group(1)), float(m.group(2))
    x, sr = sf.read(p); x = x * 10.0
    if x.ndim > 1: x = x[:, 0]
    d = np.diff(x)
    # the flyback: the sawtooth's steep edge; its sign is the larger tail of d
    s = 1.0 if np.percentile(d, 99.9) > -np.percentile(d, 0.1) else -1.0
    d = s * d
    thr = 0.3 * np.percentile(d, 99.5)
    pk = []
    i = 0
    while i < len(d):
        if d[i] > thr:
            j = i + int(np.argmax(d[i:i + 40])); pk.append(j); i = j + int(0.5 * P)
        else:
            i += 1
    pk = np.array(pk)
    # the bow's start: the force's envelope's first rise over 2 % of its max
    env = np.convolve(np.abs(x - np.median(x[:200])), np.ones(250) / 250, "same")
    t0 = int(np.argmax(env > 0.02 * env.max()))
    th = None
    for k in range(len(pk) - 6):
        g = np.diff(pk[k:k + 6]) / P
        if np.all(np.abs(g - 1) < 0.04):
            th = (pk[k] - t0) / sr; break
    # the level's rise: the envelope reaching -3 dB of its last 30 ms
    tail = env[-int(0.03 * sr):].mean()
    tr = (int(np.argmax(env > 0.707 * tail)) - t0) / sr
    model = re.search(r"model_(\w)_2", p).group(1)
    res[(round(F, 1), round(a, 1))].append((th, tr, model, len(x) / sr))
Fs = sorted({k[0] for k in res}); As = sorted({k[1] for k in res})
print("time to Helmholtz (ms, median over files; '-' none) rows F (N), cols a (m/s^2)")
print("      " + " ".join("%5.1f" % a for a in As))
for F in Fs:
    row = []
    for a in As:
        v = [r[0] for r in res.get((F, a), [])]
        if not v: row.append("     "); continue
        ok = [t for t in v if t is not None]
        row.append("%5.0f" % (1000 * np.median(ok)) if len(ok) * 2 >= len(v) else "    -")
    print("%5.1f " % F + " ".join(row))
print("level rise to -3 dB of the end (ms)")
for F in Fs:
    print("%5.1f " % F + " ".join(("%5.0f" % (1000 * np.median([r[1] for r in res[(F, a)]]))) if (F, a) in res else "     " for a in As))
print("durations", sorted({round(r[3], 2) for v in res.values() for r in v})[:10])
