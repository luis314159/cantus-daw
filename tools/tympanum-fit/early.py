"""A drum's onset, measured: each mode's phase at the strike and its early part (round 40,
drums-gen).

A struck head's modes start together — the head is pushed one way at once — and the
first milliseconds are the pulse that sum makes (the kick's thud, the tom's push, the
snare's crack below the wires). Random phases (model 5 as first built) scatter that pulse
into a smear. Here, per layer, a least-squares fit of the recording's first 80 ms to the
layer's own modes (their frequencies, T60s and the piece's glide, as tabled) gives:

- each mode's **phase** at the strike (its amplitude stays the table's: the tail is
  calibrated already);
- each mode's **early part**: a second, short-lived component (time constant `ets`, the
  best of a few) — a head loses much of its energy in the first milliseconds to the air,
  then rings on (the two-stage decay of a membrane).

The bands give up the early part's power, so the layer's energy stays the recording's.
Only the pulse below ~1.5 kHz is what the phases are judged on; above it a strike's
phases differ hit to hit (the engine scatters them, the more the higher).
"""
import numpy as np
from scipy import signal
import sys
sys.path.insert(0, __file__.rsplit("/", 1)[0].rsplit("\\", 1)[0])
from metrics import SR

ETS = [0.003, 0.005, 0.008, 0.012, 0.02]
WIN = 0.08
LP = 1500.0


def onset1(m, on):
    """The strike's sample: the first one over 1 % of the hit's peak (the 5 % detector
    lands a fraction of a millisecond late, and a phase is judged on that)."""
    a = max(0, on - int(0.004 * SR))
    pk = np.abs(m[on:on + int(0.05 * SR)]).max()
    k = np.nonzero(np.abs(m[a:on + 1]) > 0.01 * pk)[0]
    return a + int(k[0]) if len(k) else on


def trajectories(hz, glide, n):
    t = np.arange(n) / SR
    g, tau = glide
    ph = [2 * np.pi * f * (t + g * tau * (1 - np.exp(-t / max(tau, 1e-3)))) for f in hz]
    return t, ph


def _ls(B, y):
    lam = 1e-3 * np.trace(B.T @ B) / max(B.shape[1], 1)
    return np.linalg.solve(B.T @ B + lam * np.eye(B.shape[1]), B.T @ y)


def fit_early(m, on, lay, hz=None):
    """Adds `ph` (rad), `ea`, `ep` (rad) per mode and `ets` (s) to a drum's layer, and
    takes the early part's power out of its bands. Returns the low-passed residual share
    of the first 40 ms (phases as fitted against random ones), for the log."""
    modes = lay["modes"]
    if not modes:
        return None
    on = onset1(m, on)
    n = int(WIN * SR)
    y = m[on:on + n]
    # (`hz`: each mode's frequency as the engine's table will hold it — the union of the
    # piece's layers — so the phase is the one the engine's mode will have)
    hz = hz or [md[0] for md in modes]
    t, ph = trajectories(hz, lay.get("glide", [0.0, 0.05]), n)
    long_e = [np.exp(-6.9078 * t / md[2]) for md in modes]
    # 1: the long parts' phases (free amplitudes, with a short part beside them).
    sos = signal.butter(4, LP, fs=SR, output="sos")
    yl = signal.sosfilt(sos, y)
    k40 = int(0.04 * SR)
    best = None
    for ets in ETS:
        es = np.exp(-t / ets)
        cols = []
        for e, p in zip(long_e, ph):
            cols += [e * np.cos(p), -e * np.sin(p), es * np.cos(p), -es * np.sin(p)]
        c = _ls(np.array(cols).T, y)
        lp = np.arctan2(c[1::4], c[0::4])
        amp = np.array([md[1] for md in modes])
        long_sig = sum(a * e * np.cos(p + q) for a, e, p, q in zip(amp, long_e, ph, lp))
        # 2: the short parts again, the long ones held at the table's amplitude.
        sc = []
        for p in ph:
            sc += [es * np.cos(p), -es * np.sin(p)]
        S = np.array(sc).T
        d = _ls(S, y - long_sig)
        z = long_sig + S @ d
        r = np.sum((yl[:k40] - signal.sosfilt(sos, z)[:k40]) ** 2) / np.sum(yl[:k40] ** 2)
        if best is None or r < best[0]:
            best = (r, ets, lp, np.hypot(d[0::2], d[1::2]), np.arctan2(d[1::2], d[0::2]))
    r, ets, lp, ea, ep = best
    # The short parts may not carry more than the recording had below LP in the window.
    lay["ph"] = [round(float(x), 4) for x in lp]
    lay["ea"] = [round(float(x), 7) for x in ea]
    lay["ep"] = [round(float(x), 4) for x in ep]
    lay["ets"] = float(ets)
    take_from_bands(lay)
    return r


def take_from_bands(lay):
    """No band's modes, long and early, hold more than the recording's power in the band
    at any breakpoint from 1 to 60 ms (its noise plus its long modes, as fitted): its
    early parts are scaled down if they would; then the band's noise gives up their power
    (never below 3 % of what it had): the layer's energy stays the recording's."""
    from fit import CENTRES, BP_MS, edges, modal_power
    t = np.array(BP_MS) / 1000
    env = np.array(lay["env"], dtype=float)
    ets = lay["ets"]
    ea = list(lay["ea"])
    judge = (t >= 0.001) & (t <= 0.06)
    for k, c in enumerate(CENTRES):
        lo, hi = edges(c)
        idx = [i for i, md in enumerate(lay["modes"]) if lo <= md[0] < hi]
        if not idx:
            continue
        p = 10 ** (env[k] / 10)
        room = p + modal_power(lay["modes"], lo, hi, t)
        p_e = sum(0.5 * (ea[i] * np.exp(-t / ets)) ** 2 for i in idx)
        ratio = float(np.max(p_e[judge] / (room[judge] + 1e-30)))
        if ratio > 1.0:
            g = np.sqrt(1.0 / ratio)
            for i in idx:
                ea[i] *= g
            p_e = p_e * g * g
        q = np.maximum(p - p_e, 0.03 * p)
        env[k] = 10 * np.log10(q + 1e-24)
    lay["ea"] = [round(float(a), 7) for a in ea]
    lay["env"] = env.round(2).tolist()


def drop_early(lay):
    """A layer whose onset the modes cannot carry keeps random phases and no early part."""
    for k in ("ph", "ea", "ep"):
        lay.pop(k, None)
    lay["ets"] = 0.0


# (the sidestick is a click its few modes cannot carry)
DRUMS = ["kick", "snare", "rimshot", "tom_hi", "tom_mid", "tom_floor"]

if __name__ == "__main__":
    # early.py <fit.json> <reference hits> <out.json> [piece ...]: a fit's drums with
    # their onsets measured.
    import json
    import sys
    from metrics import load, hit, VELS
    from tables import TABLES, union
    fit = json.load(open(sys.argv[1]))
    x = load(sys.argv[2])
    want = sys.argv[4:] or DRUMS
    for tid, name, parts in TABLES:
        parts = [q for q in parts if q[0] in fit["pieces"]]
        layers, where = [], []
        for p, _, _ in parts:
            for v, lay in zip(VELS, fit["pieces"][p]):
                layers.append(lay)
                where.append((p, v))
        if not any(p in want for p, _ in where):
            continue
        hz_of = {}
        for r in union(layers):
            for li, j in r[9]:
                hz_of[(li, j)] = r[0]
        for li, (lay, (p, v)) in enumerate(zip(layers, where)):
            if p not in want:
                continue
            m, on = hit(x, p, v)
            env0 = list(lay["env"])
            hz = [hz_of.get((li, j), md[0]) for j, md in enumerate(lay["modes"])]
            r = fit_early(m, on, lay, hz)
            if r > 0.5:
                drop_early(lay)
                lay["env"] = env0
                print(f"{p:10s} {v:3d}: onset residual {r:.2f} - dropped")
                continue
            # the stick's burst and knock again: the onset carries some of what they did
            from fit import fit_stick
            old = lay["stick"]
            lay["stick"] = fit_stick(m, on, lay, v)
            print(f"{p:10s} {v:3d}: stick {old[0]:.4f} {old[1]:.4f} -> "
                  f"{lay['stick'][0]:.4f} {lay['stick'][1]:.4f}")
            ea2 = sum(a * a for a in lay["ea"])
            la2 = max(sum(md[1] ** 2 for md in lay["modes"]), 1e-30)
            print(f"{p:10s} {v:3d}: onset residual {r:.2f}, early {lay['ets'] * 1000:.0f} ms, "
                  f"early/long {10 * np.log10(ea2 / la2 + 1e-30):+.1f} dB")
    json.dump(fit, open(sys.argv[3], "w"))
