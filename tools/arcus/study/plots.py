"""plots.py - the reference database's figures (Arcus study).

Usage:  venv\\Scripts\\python.exe -I tools\\plots.py
Reads refdb/<family>.json (aggregate.py); writes refdb/plots/<family>_<what>.png and
refdb/plots/families_body.png. Instruments take categorical colours in a fixed order (the
same instrument keeps its colour in every figure); dynamics take one hue, light to dark.
"""
import json
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402
import numpy as np  # noqa: E402

R = Path(__file__).resolve().parents[3] / "research" / "instruments-next" / "bowed-refdb"
P = R / "plots"
CAT = ["#2a78d6", "#eb6834", "#1baf7a", "#eda100", "#e87ba4", "#008300", "#6250d6", "#e34948"]
SEQ = {"soft": "#9ec5f0", "medium": "#2a78d6", "loud": "#0f3f80"}
INK, INK2, GRID, SURF = "#0b0b0b", "#52514e", "#e4e3df", "#fcfcfb"
FAMS = ["cello", "bass", "viola", "violin"]


def style(ax, title, xl, yl, logx=True):
    ax.set_facecolor(SURF)
    if logx:
        ax.set_xscale("log")
    ax.grid(True, which="major", color=GRID, lw=0.6)
    ax.set_axisbelow(True)
    for s in ("top", "right"):
        ax.spines[s].set_visible(False)
    for s in ("left", "bottom"):
        ax.spines[s].set_color(INK2)
    ax.tick_params(colors=INK2, labelsize=8)
    ax.set_title(title, color=INK, fontsize=10, loc="left")
    ax.set_xlabel(xl, color=INK2, fontsize=8)
    ax.set_ylabel(yl, color=INK2, fontsize=8)


def arr(v):
    return np.array([np.nan if x is None else x for x in v], float)


def fig(n=1, w=9, h=4):
    f, ax = plt.subplots(1, n, figsize=(w, h), dpi=110, facecolor=SURF, squeeze=False)
    return f, ax[0]


def body(d, fam):
    f, (ax,) = fig()
    insts = sorted(d["body"])
    curves = []
    for k, i in enumerate(insts):
        b = d["body"][i]
        fc, H = arr(b["fc_hz"]), arr(b["H_db"])
        curves.append(H)
        ax.plot(fc, H, color=CAT[k % 8], lw=1.0, alpha=0.9, label=i)
    if curves:
        M = np.array(curves)
        with np.errstate(all="ignore"):
            ax.fill_between(fc, np.nanmin(M, 0), np.nanmax(M, 0), color=INK2, alpha=0.08, lw=0)
            ax.plot(fc, np.nanmedian(M, 0), color=INK, lw=2.0, label="median")
    style(ax, f"{fam}: body response by the harmonic-peak method (body x mic x source roll-off)",
          "frequency (Hz)", "dB re mean 200-4000 Hz")
    ax.set_ylim(-40, 30)
    ax.legend(fontsize=7, frameon=False, ncol=2)
    f.tight_layout()
    f.savefig(P / f"{fam}_body.png")
    plt.close(f)


def harmonics(d, fam):
    keys = sorted({k.split("_")[0] for k in d["harmonics_db_re_max"]})
    f, axs = fig(len(keys), 3.2 * len(keys), 3.6)
    for ax, o in zip(axs, keys):
        for dc in ("soft", "medium", "loud"):
            v = d["harmonics_db_re_max"].get(f"{o}_{dc}")
            if not v:
                continue
            n = np.arange(1, 41)
            ax.plot(n, arr(v["median"]), color=SEQ[dc], lw=1.6, label=f"{dc} ({v['n_inst']} inst.)")
        v = d["harmonics_db_re_max"].get(f"{o}_all")
        if v:
            ax.fill_between(np.arange(1, 41), arr(v["min"]), arr(v["max"]), color=INK2, alpha=0.10, lw=0)
            lo = v["midi_lo"]
        style(ax, f"{fam} MIDI {lo}-{lo + 11}", "harmonic number", "dB re strongest", logx=False)
        ax.set_ylim(-70, 3)
        ax.legend(fontsize=6, frameon=False)
    f.tight_layout()
    f.savefig(P / f"{fam}_harmonics.png")
    plt.close(f)


def ltas_noise(d, fam):
    f, (a1, a2) = fig(2, 11, 4)
    lt = d.get("ltas_sixth_octave")
    if lt:
        fc = arr(lt["fc_hz"])
        for dc in ("soft", "medium", "loud"):
            if lt.get(dc):
                a1.plot(fc, arr(lt[dc]["median"]), color=SEQ[dc], lw=1.6, label=dc)
        if lt.get("all"):
            a1.fill_between(fc, arr(lt["all"]["min"]), arr(lt["all"]["max"]), color=INK2, alpha=0.1, lw=0)
    style(a1, f"{fam}: long-term average spectrum, 1/6 octave", "frequency (Hz)", "dB re total")
    a1.set_ylim(-60, 0)
    a1.legend(fontsize=7, frameon=False)
    nb = d.get("noise_bands")
    if nb:
        fc = arr(nb["fc_hz"])
        for dc in ("soft", "medium", "loud"):
            v = nb.get("hnr_" + dc)
            if v:
                a2.plot(fc, arr(v["median"]), color=SEQ[dc], lw=1.6, label=dc)
        v = nb.get("hnr_all")
        if v:
            a2.fill_between(fc, arr(v["min"]), arr(v["max"]), color=INK2, alpha=0.1, lw=0)
    style(a2, f"{fam}: harmonics-to-noise ratio per 1/3 octave (bow noise)", "frequency (Hz)", "dB")
    a2.legend(fontsize=7, frameon=False)
    f.tight_layout()
    f.savefig(P / f"{fam}_ltas_noise.png")
    plt.close(f)


def pizz(d, fam):
    t = d.get("pizz_t60_by_freq")
    if not t:
        return
    f, (ax,) = fig(1, 6, 3.6)
    insts = sorted({i for v in t.values() if v for i in v["per_inst"]})
    for k, i in enumerate(insts):
        xs = [float(fq) for fq, v in t.items() if v and i in v["per_inst"]]
        ys = [v["per_inst"][i] for fq, v in t.items() if v and i in v["per_inst"]]
        ax.plot(xs, ys, "o-", color=CAT[k % 8], ms=4, lw=1, label=i)
    style(ax, f"{fam}: pizzicato T60 per partial", "partial frequency (Hz)", "T60 (s)")
    ax.set_yscale("log")
    ax.legend(fontsize=7, frameon=False)
    f.tight_layout()
    f.savefig(P / f"{fam}_pizz_t60.png")
    plt.close(f)


def families(ds):
    f, (ax,) = fig(1, 9, 4)
    for k, fam in enumerate(FAMS):
        d = ds.get(fam)
        if not d or not d["body"]:
            continue
        M = np.array([arr(b["H_db"]) for b in d["body"].values()])
        fc = arr(next(iter(d["body"].values()))["fc_hz"])
        with np.errstate(all="ignore"):
            ax.plot(fc, np.nanmedian(M, 0), color=CAT[k], lw=1.8, label=f"{fam} ({len(M)} inst.)")
    style(ax, "the four families' median body responses (harmonic-peak method)", "frequency (Hz)", "dB re mean 200-4000 Hz")
    ax.set_ylim(-40, 30)
    ax.legend(fontsize=8, frameon=False)
    f.tight_layout()
    f.savefig(P / "families_body.png")
    plt.close(f)


def main():
    P.mkdir(exist_ok=True)
    ds = {}
    for fam in FAMS:
        p = R / f"{fam}.json"
        if not p.exists():
            continue
        d = json.loads(p.read_text())
        ds[fam] = d
        if not d["notes"]["arco"]:
            continue
        body(d, fam)
        harmonics(d, fam)
        ltas_noise(d, fam)
        pizz(d, fam)
    families(ds)
    print(sorted(p.name for p in P.glob("*.png")))


if __name__ == "__main__":
    main()
