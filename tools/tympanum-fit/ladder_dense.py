"""The ladder's dense-spectrum probe (round 40, ladder): a cymbal's wash read as stable fine
structure, not noise. R3's residual at 4096/1024 (11.7-Hz bins), held as a table per bin x
40 log-spaced times, resynthesised two ways:

- R3b: random phase every frame (noise at bin resolution);
- R3c: each bin's phase drawn once and advanced coherently frame to frame (a stationary
  component at every bin centre — ~2000 stable partials for the price of one FFT a hop).

    python -I ladder_dense.py <out dir> [kit:art:vel ...]   (the probe, R3b and R3c side by side)
    python -I ladder_dense.py <out dir> all                 (R3b for every hit, into measures.json)
"""
import json
import os
import sys
import numpy as np
import soundfile as sf

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import ladder as L  # noqa: E402
import ladder_measure as M  # noqa: E402
from early import onset1  # noqa: E402

N, H = 4096, 1024
W = np.hanning(N + 1)[:N]
KEYS = ["flat", "peaks", "eff", "stab", "fl816", "T8_16", "tail_flat", "tail_eff", "at400"]


def stft(x):
    xp = np.pad(x, (N // 2, N))
    nf = len(x) // H + 1
    idx = np.arange(N)[None, :] + H * np.arange(nf)[:, None]
    return np.fft.rfft(xp[idx] * W, axis=1)


def istft(X, n):
    fr = np.fft.irfft(X, N, axis=1) * W
    y = np.zeros(len(X) * H + N)
    w2 = np.zeros_like(y)
    for i in range(len(X)):
        y[i * H:i * H + N] += fr[i]
        w2[i * H:i * H + N] += W ** 2
    return (y / np.maximum(w2, 1e-3))[N // 2:N // 2 + n]


def table(P, a0):
    ft = (np.arange(len(P)) * H - a0) / L.SR
    T = np.empty((len(L.TT), P.shape[1]))
    for j, t in enumerate(L.TT):
        k = np.abs(ft - t) <= max(H / L.SR, 0.3 * t) / 2
        if not k.any():
            k = np.argmin(np.abs(ft - t)) == np.arange(len(ft))
        T[j] = P[k].mean(0)
    T = 10 * np.log10(T + 1e-30)
    fc = np.clip(ft, 0, None)
    lt = np.log(L.TT + 0.0005)
    B = np.array([np.interp(np.log(fc + 0.0005), lt, T[:, b]) for b in range(T.shape[1])]).T
    B[ft < -0.005] = -300
    return 10 ** (B / 10)


def synth(P, n, rng, coherent):
    nf, nb = P.shape
    if coherent:
        ph0 = rng.uniform(0, 2 * np.pi, nb)
        ph = np.exp(1j * (ph0[None, :] + 2 * np.pi * np.arange(nb)[None, :] * H / N * np.arange(nf)[:, None]))
    else:
        ph = np.exp(2j * np.pi * rng.uniform(0, 1, P.shape))
    A = np.sqrt(P)
    for _ in range(3):
        Q = np.abs(stft(istft(A * ph, n))[:nf]) ** 2
        g = np.convolve(P.sum(1), np.ones(3) / 3, "same") / np.maximum(np.convolve(Q.sum(1), np.ones(3) / 3, "same"), 1e-30)
        A = A * np.sqrt(np.clip(g, 0.1, 10))[:, None]
    return istft(A * ph, n)


def rung(out, key):
    """R3b for one hit of the ladder (written next to its rungs) and its measures."""
    kit, art, v = key.split("-")
    vel = int(v[1:])
    m, on = L.recorded(kit, art, vel)
    n = len(m)
    tab, lay = L.layer(kit, art, vel)
    a0 = onset1(m, on) - int(0.0005 * L.SR)
    s0 = L.seed(kit, art, vel)
    rng = lambda k: np.random.default_rng(s0 + k)
    bands = L.place(L.render_bands(tab, lay, n - on, rng(1)), n, on)
    modes = L.place(L.render_modes(lay, n - on, rng(2)), n, on)
    phased = lay.get("ets", 0.0) > 0 and len(lay.get("ph", [])) == len(lay["modes"])
    gate = L.late_gate(n, a0)
    att = L.attack(m, a0, bands * gate, modes) if phased else L.attack(m, a0, modes + bands * gate)
    P = np.abs(stft(m)) ** 2
    Pres = np.maximum(P - np.abs(stft(modes)) ** 2 - np.abs(stft(att)) ** 2, 0.03 * P)
    y = modes + att + synth(table(Pres, a0), n, rng(11), False) * L.onset_gate(n, a0)
    g, s = L.clip_window(n, a0)
    gain = json.load(open(os.path.join(out, "ladder.json")))[key]["gain"]
    y = (y * g)[s:] * gain
    sf.write(os.path.join(out, f"{key}-R3b.wav"), y, L.SR, subtype="PCM_24")
    return key, M.measures(y, on - s, a0 - s, art)


def all_hits(out):
    """R3b for every hit of the ladder, its measures added to measures.json."""
    from multiprocessing import Pool
    keys = sorted(json.load(open(os.path.join(out, "ladder.json"))))
    with Pool(int(os.environ.get("LADDER_JOBS", "6"))) as pool:
        res = pool.starmap(rung, [(out, k) for k in keys], chunksize=1)
    path = os.path.join(out, "measures.json")
    ms = json.load(open(path))
    for k, r in res:
        ms[k]["R3b"] = r
    json.dump(ms, open(path, "w"), indent=1, default=float)


def main():
    out = sys.argv[1]
    if sys.argv[2:] == ["all"]:
        return all_hits(out)
    items = [a.split(":") for a in sys.argv[2:]] or [["drs", "crash", "127"], ["virt", "crash", "127"],
                                                     ["drs", "ride", "127"], ["virt", "ride", "80"],
                                                     ["drs", "hat_open", "127"], ["drs", "hat_half", "80"]]
    for kit, art, vel in items:
        vel = int(vel)
        m, on = L.recorded(kit, art, vel)
        n = len(m)
        tab, lay = L.layer(kit, art, vel)
        a0 = onset1(m, on) - int(0.0005 * L.SR)
        s0 = L.seed(kit, art, vel)
        rng = lambda k: np.random.default_rng(s0 + k)
        bands = L.place(L.render_bands(tab, lay, n - on, rng(1)), n, on)
        modes = L.place(L.render_modes(lay, n - on, rng(2)), n, on)
        att = L.attack(m, a0, modes + bands * L.late_gate(n, a0))
        P = np.abs(stft(m)) ** 2
        Pres = np.maximum(P - np.abs(stft(modes)) ** 2 - np.abs(stft(att)) ** 2, 0.03 * P)
        Pt = table(Pres, a0)
        gate = L.onset_gate(n, a0)
        g, s = L.clip_window(n, a0)
        cut = lambda y: (y * g)[s:]
        r3, _ = sf.read(os.path.join(out, f"{kit}-{art}-v{vel}-R3.wav"))
        rows = {"R0": cut(m), "R3": r3,
                "R3b": cut(modes + att + synth(Pt, n, rng(11), False) * gate),
                "R3c": cut(modes + att + synth(Pt, n, rng(12), True) * gate)}
        for k in ("R3b", "R3c"):
            sf.write(os.path.join(out, f"{kit}-{art}-v{vel}-{k}.wav"), rows[k], L.SR, subtype="PCM_24")
        o, f = on - s, a0 - s
        ref = M.measures(rows["R0"], o, f, art)
        print(f"\n{kit} {art} v{vel}   " + " ".join(f"{k:>9s}" for k in KEYS) + "   met")
        for name, y in rows.items():
            r = ref if name == "R0" else M.measures(y, o, f, art)
            ks = [q for q in KEYS if q in ref]
            met = sum(bool(M.meets(q, r[q], ref[q])) for q in ks)
            print(f"  {name:4s}" + " ".join(f"{r.get(q, np.nan):9.3f}" for q in KEYS) + f"   {met}/{len(ks)}", flush=True)


if __name__ == "__main__":
    main()
