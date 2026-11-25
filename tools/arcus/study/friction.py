"""friction.py - measured rosin friction on cello G strings (Lampis 2025, CC BY 4.0) vs the curves.

Usage:  venv\\Scripts\\python.exe -I tools\\friction.py
Reads the reconstructed bow-point friction force and string velocity of
%CANTUS_STUDY_DOWNLOADS%\\study\\zenodo-lampis-friction-force\\ (doi:10.5281/zenodo.15879128:
`Fr_rec` N, `V_rec` m/s at the bow, with `bowing_parameters_<n>_0.csv`: beta, Fb N, v0 m/s).
For each steady-state recording: mu = Fr/Fb against the sliding speed dv = V - v_bow (v_bow
taken as the string's median velocity while sticking, which should equal +-v0).
Writes refdb/friction.json:
  - per file: beta, Fb, v0, the peak mu while sticking (mu_s_eff), mu over sliding-speed bins,
    the loop's hysteresis (mu while slip accelerates minus while it decelerates, at equal |dv|);
  - pooled: the best-fitting hyperbolic curve mu = mu_d + (mu_s - mu_d) * v0c / (v0c + |dv|)
    (MSW / Schoonderwaldt form) and the residual a velocity-only curve cannot explain (the
    hysteresis the thermal models add); plus refdb/plots/friction.png.
"""
import os
import json
import re
from pathlib import Path

import numpy as np
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "sound-refs"))
import cantus_paths as cp  # noqa: E402  ($CANTUS_REFS, $CANTUS_STUDY_REFDB: tools/sound-refs)

R = cp.env_or("CANTUS_STUDY_REFDB", Path(__file__).resolve().parents[3] / "research" / "instruments-next" / "bowed-refdb")
D = (Path(os.environ.get("CANTUS_STUDY_DOWNLOADS") or cp.refs())) / "study" / "zenodo-lampis-friction-force" / "output_measurements" / "output_measurements" / "reconstructions"


def main():
    out, pool = [], []
    for f in sorted(D.glob("friction_velocity_reconstruction_*.csv"), key=lambda p: int(re.findall(r"\d+", p.stem)[0])):
        n = int(re.findall(r"\d+", f.stem)[0])
        bp = D / f"bowing_parameters_{n}_0.csv"
        if not bp.exists():
            continue
        A = np.genfromtxt(f, delimiter=",", names=True)
        P = np.genfromtxt(bp, delimiter=",", names=True)
        beta, Fb, v0 = float(P["beta"]), float(P["Fb"]), float(P["v0"])
        if Fb <= 0.02:
            continue
        Fr, V = A["Fr_rec"], A["V_rec"]
        # the stick velocity: the mode of V (the string moves with the bow most of the period)
        h, e = np.histogram(V, bins=200)
        vs = 0.5 * (e[np.argmax(h)] + e[np.argmax(h) + 1])
        dv = V - vs
        mu = Fr / Fb * np.sign(vs if vs != 0 else 1.0)
        slip = np.abs(dv) > 0.02
        stick = ~slip
        if slip.sum() < 50 or stick.sum() < 50:
            continue
        bins = np.array([0.02, 0.05, 0.1, 0.2, 0.4, 0.8, 1.6])
        ad = np.abs(dv)
        mb = []
        for a, b in zip(bins[:-1], bins[1:]):
            s = slip & (ad >= a) & (ad < b)
            mb.append(float(np.median(np.abs(mu[s]))) if s.sum() > 10 else None)
        acc = np.gradient(ad) > 0
        hy = []
        for a, b in zip(bins[:-1], bins[1:]):
            s1 = slip & (ad >= a) & (ad < b) & acc
            s2 = slip & (ad >= a) & (ad < b) & ~acc
            hy.append(float(np.median(np.abs(mu[s1])) - np.median(np.abs(mu[s2]))) if s1.sum() > 10 and s2.sum() > 10 else None)
        rec = {"n": n, "beta": beta, "Fb_N": Fb, "v0_ms": v0, "v_stick_ms": float(vs),
               "mu_s_eff": float(np.percentile(np.abs(mu[stick]), 99)), "mu_stick_median": float(np.median(np.abs(mu[stick]))),
               "slip_share": float(slip.mean()), "mu_by_dv_bins": mb, "dv_bins_ms": bins.tolist(), "hysteresis_acc_minus_dec": hy}
        out.append(rec)
        pool.append(np.stack([ad[slip], np.abs(mu[slip]), np.full(slip.sum(), n), acc[slip].astype(float)], 1))
    P = np.concatenate(pool)
    # fit the hyperbolic curve on the pooled slip samples (grid search: robust, deterministic)
    best = None
    for mud in np.arange(0.15, 0.6, 0.01):
        for mus in np.arange(0.6, 1.6, 0.02):
            for v0c in (0.005, 0.01, 0.02, 0.03, 0.05, 0.08, 0.12, 0.2):
                pred = mud + (mus - mud) * v0c / (v0c + P[:, 0])
                err = np.median(np.abs(P[:, 1] - pred))
                if best is None or err < best[0]:
                    best = (err, mud, mus, v0c)
    err, mud, mus, v0c = best
    pred = mud + (mus - mud) * v0c / (v0c + P[:, 0])
    res_acc = float(np.median((P[:, 1] - pred)[P[:, 3] > 0]))
    res_dec = float(np.median((P[:, 1] - pred)[P[:, 3] == 0]))
    summ = {"files": len(out), "hyperbolic_fit": {"mu_d": round(float(mud), 3), "mu_s": round(float(mus), 3),
            "v0_ms": v0c, "median_abs_err": round(float(err), 4)},
            "residual_while_slip_accelerates": round(res_acc, 4), "residual_while_slip_decelerates": round(res_dec, 4),
            "mu_s_eff_p10_50_90": [round(float(np.percentile([r["mu_s_eff"] for r in out], q)), 3) for q in (10, 50, 90)],
            "Fb_range_N": [min(r["Fb_N"] for r in out), max(r["Fb_N"] for r in out)],
            "beta_range": [min(r["beta"] for r in out), max(r["beta"] for r in out)],
            "v0_values_ms": sorted({r["v0_ms"] for r in out})}
    (R / "friction.json").write_text(json.dumps({"summary": summ, "files": out}, indent=1))
    print(json.dumps(summ, indent=1))
    try:
        import matplotlib
        matplotlib.use("Agg")
        import matplotlib.pyplot as plt
        f, ax = plt.subplots(figsize=(6.5, 4), dpi=110, facecolor="#fcfcfb")
        idx = np.random.default_rng(0).choice(len(P), min(len(P), 20000), replace=False)
        ax.scatter(P[idx, 0], P[idx, 1], s=2, c=np.where(P[idx, 3] > 0, "#2a78d6", "#eb6834"), alpha=0.25, lw=0)
        xs = np.logspace(-2, 0.3, 100)
        ax.plot(xs, mud + (mus - mud) * v0c / (v0c + xs), color="#0b0b0b", lw=2, label=f"hyperbolic fit μd {mud:.2f} μs {mus:.2f} v0 {v0c}")
        ax.plot(xs, 0.4 * np.exp(-xs / 0.01) + 0.45 * np.exp(-xs / 0.1) + 0.35, color="#52514e", lw=1.2, ls="--", label="Desvages & Bilbao curve")
        ax.set_xscale("log")
        ax.set_xlabel("sliding speed |Δv| (m/s)")
        ax.set_ylabel("μ = friction / bow force")
        ax.set_title("cello G strings, measured (Lampis 2025): blue slip accelerating, orange decelerating", fontsize=8, loc="left")
        ax.set_ylim(0, 2)
        ax.legend(fontsize=7, frameon=False)
        f.tight_layout()
        (R / "plots").mkdir(exist_ok=True)
        f.savefig(R / "plots" / "friction.png")
    except Exception as e:  # noqa: BLE001
        print("plot failed", e)


if __name__ == "__main__":
    main()
