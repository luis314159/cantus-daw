"""Groove: LTAS, short-term loudness, stereo per band, HF texture over time, between-hit floor, figure."""
import sys
sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
from feats import *
from scipy import ndimage
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt


def groove(kit, ver):
    return load(f"{BASE}{kit}-groove-{VERS[ver]}.flac")


def ltas(x):
    m = x.mean(1)
    c, L = thirdoct_spec(m)
    return c, L


def st_loud(x, win=0.1):
    m = x.mean(1)
    n = int(win * SR)
    k = len(m) // n
    e = 10 * np.log10(np.mean(m[:k * n].reshape(k, n) ** 2, axis=1) + 1e-20)
    return e


def stereo(x):
    out = []
    for lo, hi in ((60, 250), (250, 1000), (1000, 4000), (4000, 16000)):
        l = bandpass(x[:, 0], lo, hi); r = bandpass(x[:, 1], lo, hi)
        c = np.sum(l * r) / np.sqrt(np.sum(l * l) * np.sum(r * r))
        side = 10 * np.log10(np.sum((l - r) ** 2) / np.sum((l + r) ** 2) + 1e-20)
        out.append((c, side))
    return out


def hf_texture(x, t0=0, t1=None):
    """6-16 kHz flatness and 4-16 kHz stability in 0.5 s windows, plus 'effective components'/kHz."""
    m = x.mean(1)[int(t0 * SR):None if t1 is None else int(t1 * SR)]
    f, t, S = signal.stft(m, SR, nperseg=8192, noverlap=4096)
    P = np.abs(S) ** 2
    k = (f >= 6000) & (f < 16000)
    fl = flatness(P[k], axis=0)
    p = P[k] / P[k].sum(0)
    eff = 1 / np.sum(p ** 2, 0) / 10  # components per kHz (10 kHz span)
    return float(np.median(fl)), float(np.median(eff))


def onsets(x):
    m = x.mean(1)
    e = env_db(m, 2)
    d = np.diff(e[::48])  # 1-ms frames
    pk, _ = signal.find_peaks(d, height=1.5, distance=40)
    return pk * 48


if __name__ == "__main__":
    for kit in KITS:
        G = {v: groove(kit, v) for v in ("prev", "gen", "rec")}
        print(f"\n===== {kit} groove")
        c, Lr = ltas(G["rec"])
        for v in ("prev", "gen"):
            _, L = ltas(G[v])
            d = L - Lr
            print(f"  LTAS {v}-rec per 1/3 oct (31.5..20k): " + " ".join(f"{x:4.0f}" for x in d))
        print("  centres: " + " ".join(f"{int(x):4d}" if x < 10000 else f"{int(x/1000):3d}k" for x in c))
        for v in ("prev", "gen", "rec"):
            e = st_loud(G[v])
            act = e[e > e.max() - 60]
            print(f"  {v:4s} 100ms loudness p95-p5 {np.percentile(e,95)-np.percentile(e,5):5.1f} dB, p50 re max {np.median(e)-e.max():5.1f}"
                  f" | stereo corr/side(dB) 60-250,250-1k,1-4k,4-16k: " + " ".join(f"{a:+.2f}/{b:5.1f}" for a, b in stereo(G[v]))
                  + f" | HF flat/eff {hf_texture(G[v])}")
        # figure
        fig, axs = plt.subplots(3, 1, figsize=(16, 11))
        for ax, v in zip(axs, ("prev", "gen", "rec")):
            m = G[v].mean(1)
            f, t, S = signal.stft(m, SR, nperseg=2048, noverlap=2048 - 256)
            P = 20 * np.log10(np.abs(S) + 1e-9); P -= P.max()
            ax.pcolormesh(t, f / 1000, P, vmin=-100, vmax=0, shading="auto", cmap="magma")
            ax.set_yscale("symlog", linthresh=1); ax.set_ylim(0.03, 22); ax.set_title(f"{kit} groove {v}")
        fig.tight_layout(); fig.savefig(FIG + f"groove_{kit}.png", dpi=60); plt.close(fig)
        on = onsets(G["rec"])
        print(f"  rec onsets ({len(on)}): " + " ".join(f"{o/SR:.3f}" for o in on[:60]))
