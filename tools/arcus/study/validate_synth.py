"""validate_synth.py - the feature extractor measured on synthetic notes with known values.

Usage:  venv\\Scripts\\python.exe -I tools\\validate_synth.py [out.json]
Writes refdb/validation.json (default) with each feature's true value, measured value and error,
for a bowed note at 41.2 Hz (the bass's E1), 98 Hz (the cello's G2), 196 Hz (the violin's G3),
440 Hz and 880 Hz, and plucked notes. Every synthetic note has: a known harmonic spectrum
(1/n with a known ripple), vibrato (5.8 Hz, +-18 cents) with a known AM per partial, coloured
noise at a known level, an attack whose harmonics arrive in a known order, a release at a
known decay rate.
"""
import json
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
from bowlib_core import band_powers  # noqa: E402
from bowlib_note import analyse_arco, analyse_pizz  # noqa: E402

SR = 44100


def t_rise_true(ns, a_db, tau):
    """When the harmonics' summed power reaches -3 dB of steady (their delays included)."""
    tt = np.arange(0, 0.5, 1e-4)
    p = np.zeros_like(tt)
    for i, k in enumerate(ns):
        d = 0.004 * (k - 1) if k <= 8 else 0.028
        p += 10 ** (a_db[i] / 10) * np.clip(1 - np.exp(-np.maximum(tt - d, 0) / tau), 0, 1) ** 2
    return float(tt[np.argmax(p > p[-1] * 10 ** -0.3)])


def synth_arco(f0, rng, dur=3.6, vib_rate=5.8, vib_c=18.0, noise_db=-30.0, rel_db_s=40.0):
    n = int(dur * SR)
    t = np.arange(n) / SR
    t_on, t_off = 0.3, 2.9
    nh = int(15000 / f0)
    ns = np.arange(1, nh + 1)
    a_db = -20 * np.log10(ns) + 6 * np.sin(ns / 3.0)
    vib = vib_c * np.sin(2 * np.pi * vib_rate * (t - t_on)) * np.clip((t - t_on) / 0.3, 0, 1)
    f = f0 * 2 ** (vib / 1200)
    phi = np.cumsum(f) / SR
    am_db = 1.0 + (ns % 4)  # +-dB at the vibrato rate, sign alternating with n
    am_sign = np.where(ns % 2 == 0, 1.0, -1.0)
    tau = 0.03
    harm = np.zeros(n)
    for i, k in enumerate(ns):
        delay = 0.004 * (k - 1) if k <= 8 else 0.028
        env = np.clip(1 - np.exp(-np.maximum(t - t_on - delay, 0) / tau), 0, 1)
        rel = np.where(t > t_off, 10 ** (-rel_db_s * (t - t_off) / 20), 1.0)
        am = 10 ** (am_sign[i] * am_db[i] * np.sin(2 * np.pi * vib_rate * (t - t_on)) * np.clip((t - t_on) / 0.3, 0, 1) / 20)
        harm += 10 ** (a_db[i] / 20) * env * rel * am * np.sin(2 * np.pi * k * phi + rng.uniform(0, 6.28))
    w = rng.standard_normal(n)
    from scipy.signal import lfilter
    w = lfilter([1.0], [1.0, -0.7], w)  # a low-tilted noise
    st = (t > 1.0) & (t < 2.5)
    p_h = np.mean(harm[st] ** 2)
    w *= np.sqrt(p_h * 10 ** (noise_db / 10) / np.mean(w[st] ** 2))
    gate = np.where(t > t_on, 1.0, 0.0) * np.where(t > t_off, 10 ** (-rel_db_s * (t - t_off) / 20), 1.0)
    noise = w * gate
    floor = 1e-6 * rng.standard_normal(n)
    x = harm + noise + floor
    truth = {"f0": f0, "a_db_re_f1": (a_db - a_db[0])[:20].tolist(), "vib_rate": vib_rate,
             "vib_c": vib_c, "am_db": am_db[:20].tolist(), "noise_db": noise_db,
             "rel_db_s": rel_db_s, "t_rise": t_rise_true(ns, a_db, tau),
             "build_order": "rising with n (rho = +1)"}
    p = 10 ** (a_db / 10)
    lim = ns * f0 <= 10000
    truth["centroid_hz"] = float(np.sum((ns * f0 * p)[lim]) / np.sum(p[lim]))
    _, pb_n = band_powers(noise[st], SR, 3)
    _, pb_h = band_powers(harm[st], SR, 3)
    truth["hnr_band_db"] = (10 * np.log10(pb_h / pb_n)).tolist()
    fcb, _ = band_powers(noise[st], SR, 3)
    vb = (fcb >= 300) & (fcb >= 5 * f0) & (fcb <= 0.42 * SR)
    truth["noise_valid_db"] = float(10 * np.log10(pb_n[vb].sum() / pb_h.sum()))
    return x, truth


def synth_pizz(f0, B, rng, dur=4.0):
    n = int(dur * SR)
    t = np.arange(n) / SR
    t_on = 0.2
    x = np.zeros(n)
    t60 = []
    for k in range(1, 21):
        fk = k * f0 * np.sqrt(1 + B * k * k)
        if fk > 10000:
            break
        T = 2.5 / (1 + 0.15 * (k - 1))
        t60.append(T)
        env = np.where(t >= t_on, 10 ** (-60 * (t - t_on) / T / 20), 0.0)
        x += (1.0 / k) * env * np.sin(2 * np.pi * fk * (t - t_on))
    x += 1e-6 * rng.standard_normal(n)
    return x, {"f0": f0, "B": B, "t60": t60}


def main():
    out_path = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parents[3] / "research" / "instruments-next" / "bowed-refdb" / "validation.json"
    rng = np.random.default_rng(42)
    rep = {"arco": [], "pizz": []}
    for f0, fmin, fmax in [(41.2, 30, 500), (98.0, 55, 1100), (196.0, 170, 3000), (440.0, 170, 3000), (880.0, 170, 3000)]:
        x, tr = synth_arco(f0, rng)
        r = analyse_arco(x, SR, {"fmin": fmin, "fmax": fmax})
        k = min(20, len(r["h_db_re_f1"]), len(tr["a_db_re_f1"]))
        m = np.array(r["h_db_re_f1"][:k])
        tt = np.array(tr["a_db_re_f1"][:k])
        hn = np.array(r["noise"]["hnr_band_db"])
        th = np.array(tr["hnr_band_db"])
        fc = np.array(r["noise"]["bands_hz"])
        sel = np.array(r["noise"]["band_valid"]) & (fc < 12000) & np.isfinite(th)
        v = r["vibrato"] or {}
        am = np.array((v.get("am_db") or [np.nan])[:10])
        e = {"f0_true": f0,
             "f0_err_cents": 1200 * np.log2(r["f0_hz"] / f0),
             "harm_db_err_max_n2_20": float(np.max(np.abs(m[1:] - tt[1:]))),
             "harm_db_err_median_n2_20": float(np.median(np.abs(m[1:] - tt[1:]))),
             "centroid_err_pct": 100 * (r["centroid_hz"] / tr["centroid_hz"] - 1),
             "vib_rate_err_hz": v.get("rate_hz", np.nan) - tr["vib_rate"],
             "vib_extent_err_cents": v.get("extent_cents", np.nan) - tr["vib_c"],
             "am_db_err_median_n1_10": float(np.nanmedian(np.abs(am - np.array(tr["am_db"][: len(am)])))),
             "noise_total_err_db": r["noise"]["res_total_db_re_harm"] - tr["noise_db"],
             "noise_valid_bands_err_db": r["noise"]["res_valid_db_re_harm"] - tr["noise_valid_db"],
             "hnr_band_err_db_median": float(np.median(np.abs(hn[sel] - th[sel]))),
             "hnr_band_err_db_max": float(np.max(np.abs(hn[sel] - th[sel]))),
             "t_rise_err_ms": 1000 * ((r["attack"] or {}).get("t_rise_3db_s") or np.nan) - 1000 * tr["t_rise"],
             "build_order_rho": (r["attack"] or {}).get("build_order_rho"),
             "t_steady_s": (r["attack"] or {}).get("t_steady_s"),
             "noise_burst_db_true0": (r["attack"] or {}).get("noise_burst_db_over_steady"),
             "release_decay_err_db_s": ((r["release"] or {}).get("decay_db_s") or np.nan) - tr["rel_db_s"],
             "partial_dev_cents_max": float(np.nanmax(np.abs(np.array([v if v is not None else np.nan for v in r["partial_dev_cents"]], float))))}
        rep["arco"].append({k: (float(v2) if isinstance(v2, (float, np.floating)) else v2) for k, v2 in e.items()})
        print(json.dumps(rep["arco"][-1]))
    for f0, B, fmin, fmax in [(41.2, 4e-5, 30, 500), (65.4, 1.2e-4, 55, 1100), (196.0, 6e-5, 170, 3000)]:
        x, tr = synth_pizz(f0, B, rng)
        r = analyse_pizz(x, SR, {"fmin": fmin, "fmax": fmax})
        t60m = np.array([v if v else np.nan for v in r["t60_s"]], float)
        t60t = np.array([tr["t60"][n - 1] for n in r["partials"]])
        e = {"f0_true": f0, "f0_err_cents": float(1200 * np.log2(r["f0_hz"] / f0)),
             "B_true": B, "B_err_pct": float(100 * (r["B"] / B - 1)),
             "t60_err_pct_median": float(np.nanmedian(np.abs(t60m / t60t - 1)) * 100),
             "t60_err_pct_max": float(np.nanmax(np.abs(t60m / t60t - 1)) * 100),
             "n_partials": len(r["partials"])}
        rep["pizz"].append(e)
        print(json.dumps(e))
    out_path.write_text(json.dumps(rep, indent=1))


if __name__ == "__main__":
    main()
