"""cnsm.py - test the harmonic-peak body estimate against measured violin admittances (CNSM set).

Usage:  venv\\Scripts\\python.exe -I tools\\cnsm.py
Data: %CANTUS_STUDY_DOWNLOADS%\\study\\cnsm-violin\\ (Zenodo 18696786, CC BY 4.0; Pauget
Ballesteros, Lalitte, Lostanlen, Fritz, arXiv 2605.18305): three violins (Klimke, Levaggi,
Stoppani) with hammer/vibrometer bridge admittances and scale recordings by several players.
For each violin:
  1. |Y(f)| in dB, the median over its measurements, on body.py's 1/12-octave grid;
  2. the harmonic-peak estimate (body.body_estimate) from every stable note (>= 150 ms) of its
     scale recordings (all players);
  3. their agreement: Pearson r of the two curves (dB, each mean removed) over 250-6000 Hz, at
     1/12- and 1/3-octave smoothing, and both curves' A0 / bridge-hill marks.
Writes refdb/cnsm_validation.json and refdb/plots/cnsm_violins.png. The admittance is
velocity/force at the bridge, the recordings are pressure: the radiation efficiency (rising with
frequency to the coincidence region) separates them, so r measures how much of the body's
shape the method recovers, not identity.
"""
import os
import json
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
from body import body_estimate, body_marks  # noqa: E402
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "sound-refs"))
import cantus_paths as cp  # noqa: E402  ($CANTUS_REFS, $CANTUS_STUDY_REFDB: tools/sound-refs)
from bowlib_core import load, yin_track, refine_track, phase_of, project, rms_db, median_filter  # noqa: E402

C = (Path(os.environ.get("CANTUS_STUDY_DOWNLOADS") or cp.refs())) / "study" / "cnsm-violin"
R = cp.env_or("CANTUS_STUDY_REFDB", Path(__file__).resolve().parents[3] / "research" / "instruments-next" / "bowed-refdb")


def admittance(violin):
    curves, fgrid = [], None
    for p in sorted((C / "admittances").glob(f"Phase */{violin}/measurement_*.csv")):
        A = np.genfromtxt(p, delimiter=",", names=True)
        f = A["f"]
        H = np.abs(A["H_real"] + 1j * A["H_imag"])
        curves.append((f, H))
    return curves


def to_grid(f, H, fc):
    out = []
    for c in fc:
        s = (f >= c * 2 ** (-1 / 24)) & (f < c * 2 ** (1 / 24))
        out.append(20 * np.log10(np.mean(H[s] ** 2) ** 0.5) if s.any() else np.nan)
    return np.array(out)


def notes_of(path):
    x, sr = load(path)
    t, f, ap = yin_track(x, sr, 180, 3400, hop_s=0.005)
    if len(t) < 50:
        return []
    et, e = rms_db(x, sr, 0.02, 0.005)
    lv = np.interp(t, et, e)
    voiced = (ap < 0.2) & (lv > np.percentile(lv, 99) - 30)
    st = median_filter(12 * np.log2(np.maximum(f, 1) / 440) + 69, 7)
    rows, i, n = [], 0, len(t)
    while i < n:
        if not voiced[i]:
            i += 1
            continue
        j = i
        while j + 1 < n and voiced[j + 1] and abs(st[j + 1] - st[i]) < 0.5:
            j += 1
        if t[j] - t[i] >= 0.15:
            a, b = t[i] + 0.2 * (t[j] - t[i]), t[j] - 0.2 * (t[j] - t[i])
            s0, s1 = int(a * sr), int(b * sr)
            fm = float(np.median(f[i:j + 1]))
            seg = x[s0:s1]
            if len(seg) > 8 * sr / fm:
                tt = t[i:j + 1] - a
                ff = refine_track(seg, sr, tt, f[i:j + 1], fm)
                phi, _ = phase_of(tt, ff, len(seg), sr)
                nh = int(12000 / fm)
                _, c = project(seg, phi, sr, fm, nh, periods=6.0)
                if c.shape[1] >= 2:
                    amp = 20 * np.log10(np.median(np.abs(c), axis=1) + 1e-12)
                    rows.append({"f0_hz": fm, "h_db_all_re_max": (amp - amp.max()).tolist()})
        i = j + 1
    return rows


def corr(a, b, fc, lo=250, hi=6000):
    s = (fc >= lo) & (fc <= hi) & np.isfinite(a) & np.isfinite(b)
    x, y = a[s] - a[s].mean(), b[s] - b[s].mean()
    return float(np.sum(x * y) / np.sqrt(np.sum(x * x) * np.sum(y * y))), int(s.sum())


def third(v):
    return np.array([np.nanmean(v[max(i - 2, 0):i + 3]) if np.isfinite(v[max(i - 2, 0):i + 3]).any() else np.nan for i in range(len(v))])


def main():
    out = {}
    curves = {}
    for v in ("Klimke", "Levaggi", "Stoppani"):
        adm = admittance(v)
        rows = []
        for p in sorted((C / "recordings").glob(f"Session */{v}/*/*.wav")):
            rows += notes_of(p)
        if not adm or len(rows) < 10:
            continue
        fc, H, Hu, cnt = body_estimate(rows)
        fc = np.array(fc)
        Y = np.nanmedian(np.array([to_grid(f, h, fc) for f, h in adm]), axis=0)
        Y = Y - np.nanmean(Y[(fc >= 200) & (fc <= 4000)])
        Hn = np.array([np.nan if q is None else q for q in H], float)
        r12, n12 = corr(Hn, Y, fc)
        r3, n3 = corr(third(Hn), third(Y), fc)
        # the radiation efficiency's tilt: fit a line in log f to (H - Y) and remove it
        s = (fc >= 250) & (fc <= 6000) & np.isfinite(Hn) & np.isfinite(Y)
        k = np.polyfit(np.log2(fc[s]), (Hn - Y)[s], 1)
        Yt = Y + np.polyval(k, np.log2(fc))
        r3t, _ = corr(third(Hn), third(Yt), fc)
        out[v] = {"notes": len(rows), "measurements": len(adm), "r_1_12oct": r12, "r_1_3oct": r3,
                  "tilt_db_per_oct_H_minus_Y": float(k[0]), "r_1_3oct_after_tilt": r3t, "bins": n12,
                  "marks_recordings": body_marks(fc.tolist(), H, "violin"),
                  "marks_admittance": body_marks(fc.tolist(), [None if not np.isfinite(q) else float(q) for q in Y], "violin")}
        curves[v] = (fc, Hn, Y)
        print(v, json.dumps({k2: out[v][k2] for k2 in ("notes", "r_1_12oct", "r_1_3oct", "tilt_db_per_oct_H_minus_Y", "r_1_3oct_after_tilt")}))
    # how much do two different violins' admittances agree (the yardstick for r)?
    names = list(curves)
    cross = {}
    for i in range(len(names)):
        for j in range(i + 1, len(names)):
            fc = curves[names[i]][0]
            cross[f"{names[i]}-{names[j]}_admittance_r_1_3oct"] = corr(third(curves[names[i]][2]), third(curves[names[j]][2]), fc)[0]
            cross[f"{names[i]}-{names[j]}_recordings_r_1_3oct"] = corr(third(curves[names[i]][1]), third(curves[names[j]][1]), fc)[0]
    out["between_violins"] = cross
    (R / "cnsm_validation.json").write_text(json.dumps(out, indent=1))
    print(json.dumps(cross, indent=1))
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    f, axs = plt.subplots(1, len(curves), figsize=(4 * len(curves), 3.4), dpi=110, facecolor="#fcfcfb", squeeze=False)
    for ax, (v, (fc, Hn, Y)) in zip(axs[0], curves.items()):
        ax.plot(fc, Y, color="#52514e", lw=1.2, label="bridge admittance |Y| (measured)")
        ax.plot(fc, Hn, color="#2a78d6", lw=1.6, label="harmonic-peak estimate (recordings)")
        ax.set_xscale("log")
        ax.set_xlim(150, 12000)
        ax.set_ylim(-40, 20)
        ax.grid(True, color="#e4e3df", lw=0.6)
        ax.set_title(f"{v}: r(1/3 oct) {out[v]['r_1_3oct']:.2f}", fontsize=9, loc="left")
        ax.legend(fontsize=6, frameon=False)
        ax.set_xlabel("Hz", fontsize=8)
    f.tight_layout()
    f.savefig(R / "plots" / "cnsm_violins.png")


if __name__ == "__main__":
    main()
