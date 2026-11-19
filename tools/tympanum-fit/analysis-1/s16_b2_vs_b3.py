"""Did the cymbal measures move between the second (b2) and third (b3) listens of the modelled kit?
Reuses measures() from s14 (its definitions only)."""
import sys
sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
src = open(__import__("os").path.join(__import__("os").path.dirname(__import__("os").path.abspath(__file__)), "s14_all_kits.py")).read().split("rows = {}")[0]
exec(src)
for lab, fn in (("b2", __import__("os").environ.get("TYMP_LISTEN_B2", "listen-b2") + "/golpes-modelado-studio.flac"),
                ("b3", __import__("os").environ.get("TYMP_LISTEN", "listen") + "/golpes-modelado-studio.flac")):
    x, _ = sf.read(fn, always_2d=True)
    for p in ["hat_closed", "hat_open", "crash", "ride"]:
        seg, on = hit(x.astype(np.float64), p, 110)
        r = measures(seg.mean(1), on, p)
        print(lab, f"{p:10s}", " ".join(f"{k}={r[k]:.3g}" for k in ["flat4_12", "eff_per_khz", "top3_share", "fine_stability", "cen_0_3ms"] + (["T60_ratio_hf_lf", "t_peak_8_16k_ms", "share_below_450_dB"] if "T60_ratio_hf_lf" in r else [])))
