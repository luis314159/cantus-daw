#!/usr/bin/env python3
"""Tympanum against the drums library, feature by feature (round 42, tympanum-real).

    python -I tools/tympanum-real/compare.py <kits.json> <out dir>

`kits.json`: a list of {"id", "label", "genre", "role": "ref" | "before" | "after", "hits":
<measure.py's .jsonl>}. Writes `<out>/summary.json` (every kit's `summary.summarise`), a
table per piece kind (`<out>/table.md`: the references' range and median, each Tympanum,
`dentro`/`fuera`) and the charts (`<out>/fig_<kind>.png`: each feature on the references'
range, `<out>/curves_<kind>.png`: level and brightness against velocity). Outside the range
is ranked by a rough audibility weight (`WEIGHT`), so the list of what to fix comes first.
"""
import json
import math
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import summary as S  # noqa: E402

import matplotlib  # noqa: E402
matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402

# the reference palette (dataviz skill, light): references in neutral ink, Tympanum's two
INK, MUTED, GRID, SURFACE = "#0b0b0b", "#8a8984", "#e4e3df", "#fcfcfb"
REF = "#9b9a94"
COL = {"before": "#2a78d6", "after": "#eb6834"}
MARK = {"before": "o", "after": "D"}

# what each feature is called on the owner's page, and how much it is heard (1-3)
FEAT = {
    "span_db": ("Rango dinámico suave→fuerte (dB)", 3),
    "inversions": ("Pasos de velocidad que bajan de nivel", 3),
    "level_hard": ("Pico del golpe más fuerte (dBFS)", 2),
    "bright_oct_10db": ("Brillo que sube con el nivel (oct/10 dB)", 3),
    "cent_hard": ("Centroide 20–80 ms, fuerte (Hz)", 2),
    "cent_attack_hard": ("Centroide 0–3 ms, fuerte (Hz)", 2),
    "crest_hard_db": ("Factor de cresta 10 ms (dB)", 2),
    "rise_hard_ms": ("Subida del ataque (ms)", 1),
    "stick_hard_db": ("Palo: energía >5 kHz en 5 ms (dB)", 2),
    "t30_125": ("Caída 125 Hz (ms)", 2),
    "t30_500": ("Caída 500 Hz (ms)", 2),
    "t30_2000": ("Caída 2 kHz (ms)", 2),
    "t30_8000": ("Caída 8 kHz (ms)", 2),
    "f0": ("Modo fundamental (Hz)", 1),
    "glide_hard_c": ("Caída de afinación, fuerte (cents)", 2),
    "glide_soft_c": ("Caída de afinación, suave (cents)", 1),
    "upper_db": ("Modos asimétricos / fundamental (dB)", 2),
    "late_hf_db": ("Agudos tardíos 4–10 kHz (resonancia, dB)", 1),
    "wires_db": ("Bordonera: nivel 40–250 ms (dB)", 3),
    "wires_t_ms": ("Bordonera: caída a −20 dB (ms)", 2),
    "wires_flat": ("Bordonera: planitud 3–10 kHz", 1),
    "wires_span_db": ("Bordonera: cuánto más pesa en un golpe suave (dB)", 2),
    "flat_4_12": ("Lavado: planitud 4–12 kHz", 3),
    "bloom_ms": ("Lavado: pico de 4–16 kHz (ms)", 2),
    "wash_drop_oct": ("Lavado: el centroide baja 0,1→1,5 s (oct)", 2),
    "take_var_db": ("Variación entre tomas (dB)", 2),
    "take_level_db": ("Variación de nivel entre tomas (dB)", 1),
    "side_db": ("Anchura: lado/centro 100–600 ms (dB)", 2),
    "side_early_db": ("Anchura: lado/centro 0–20 ms (dB)", 1),
    "corr_late": ("Correlación L/R 100–600 ms", 1),
    "c50_db": ("Sala: energía antes/después de 50 ms (dB)", 2),
}
KIND_ES = {"kick": "Bombo", "snare": "Caja", "tom": "Toms", "hihat": "Charles", "crash": "Crash",
           "ride": "Ride"}
KINDS = ["kick", "snare", "tom", "hihat", "crash", "ride"]


def finite(x):
    return isinstance(x, (int, float)) and math.isfinite(x)


def style(ax):
    ax.set_facecolor(SURFACE)
    for s in ("top", "right", "left"):
        ax.spines[s].set_visible(False)
    ax.spines["bottom"].set_color(GRID)
    ax.tick_params(colors=MUTED, labelsize=8)


def feature_chart(kind, rows, kits, path):
    """Each feature on its own scale: the references' range as a bar, each reference a grey
    dot, each Tympanum a coloured mark; the axis is the references' min … max (0 … 1)."""
    feats = [f for f in FEAT if f in rows]
    if not feats:
        return
    fig, ax = plt.subplots(figsize=(9.5, 0.42 * len(feats) + 1.3), dpi=130)
    fig.patch.set_facecolor(SURFACE)
    style(ax)
    for i, f in enumerate(feats):
        y = len(feats) - 1 - i
        refs = [v for k, v in rows[f].items() if kits[k]["role"] == "ref" and finite(v)]
        if len(refs) < 2:
            continue
        lo, hi = min(refs), max(refs)
        span = hi - lo if hi > lo else max(abs(hi), 1e-6)
        pos = lambda v: (v - lo) / span  # noqa: E731
        ax.plot([0, 1], [y, y], color=GRID, lw=7, solid_capstyle="round", zorder=1)
        ax.scatter([pos(v) for v in refs], [y] * len(refs), s=22, color=REF, zorder=2,
                   edgecolor=SURFACE, linewidth=1)
        txt = []
        for k, v in rows[f].items():
            role = kits[k]["role"]
            if role == "ref" or not finite(v):
                continue
            p = max(-0.6, min(1.6, pos(v)))
            ax.scatter([p], [y], s=46, marker=MARK[role], color=COL[role], zorder=3,
                       edgecolor=SURFACE, linewidth=1.5)
            txt.append(f"{kits[k]['label']} {v:.3g}")
        ax.text(1.68, y, f"ref {lo:.3g} … {hi:.3g}   " + "  ".join(txt), va="center",
                fontsize=7.5, color=MUTED)
        ax.text(-0.66, y, FEAT[f][0], va="center", ha="right", fontsize=8.5, color=INK)
    ax.set_xlim(-0.65, 1.65)
    ax.set_ylim(-0.8, len(feats) - 0.2)
    ax.set_yticks([])
    ax.set_xticks([0, 1])
    ax.set_xticklabels(["mín. de las referencias", "máx."], fontsize=8, color=MUTED)
    handles = [plt.Line2D([], [], marker="o", ls="", color=REF, label="kits grabados")]
    for role, name in (("before", "Tympanum antes"), ("after", "Tympanum después")):
        if any(kits[k]["role"] == role for k in kits):
            handles.append(plt.Line2D([], [], marker=MARK[role], ls="", color=COL[role], label=name))
    ax.legend(handles=handles, loc="upper left", bbox_to_anchor=(0, 1.0 + 0.9 / len(feats)),
              ncol=3, frameon=False, fontsize=8.5)
    ax.set_title(f"{KIND_ES.get(kind, kind)}: cada medida sobre el rango de los kits grabados",
                 loc="left", fontsize=11, color=INK, pad=22)
    fig.subplots_adjust(left=0.33, right=0.62)
    fig.savefig(path, facecolor=SURFACE, bbox_inches="tight")
    plt.close(fig)


def curves(kind, kits, hits, path):
    fig, axs = plt.subplots(1, 2, figsize=(10, 3.6), dpi=130)
    fig.patch.set_facecolor(SURFACE)
    for ax in axs:
        style(ax)
        ax.grid(axis="y", color=GRID, lw=0.6)
    for k, kit in kits.items():
        hs = S.main_hits(hits[k], kind)
        if not hs:
            continue
        lv = S.by_vel(hs, "rms80_db")
        if not lv:
            continue
        top = max(lv.values())
        v = sorted(lv)
        role = kit["role"]
        c = REF if role == "ref" else COL[role]
        lw = 1.0 if role == "ref" else 2.0
        axs[0].plot(v, [lv[x] - top for x in v], color=c, lw=lw, alpha=0.8 if role == "ref" else 1,
                    zorder=3 if role != "ref" else 2)
        ce = S.by_vel(hs, "cent_c")
        axs[1].plot([lv[x] - top for x in v if x in ce], [ce[x] for x in v if x in ce], color=c,
                    lw=lw, alpha=0.8 if role == "ref" else 1, zorder=3 if role != "ref" else 2)
        if role != "ref":
            axs[0].text(v[-1] + 2, 0, kit["label"], color=INK, fontsize=7.5, va="center")
    axs[0].set_xlabel("velocidad MIDI", fontsize=8.5, color=MUTED)
    axs[0].set_ylabel("nivel 0–80 ms (dB, 0 = el más fuerte)", fontsize=8.5, color=MUTED)
    axs[1].set_xlabel("nivel (dB)", fontsize=8.5, color=MUTED)
    axs[1].set_ylabel("centroide 20–80 ms (Hz)", fontsize=8.5, color=MUTED)
    axs[1].set_yscale("log")
    axs[0].set_title(f"{KIND_ES.get(kind, kind)}: nivel por velocidad", loc="left", fontsize=10, color=INK)
    axs[1].set_title("brillo por nivel", loc="left", fontsize=10, color=INK)
    handles = [plt.Line2D([], [], color=REF, lw=1, label="kits grabados")]
    for role, name in (("before", "Tympanum antes"), ("after", "Tympanum después")):
        if any(kits[k]["role"] == role for k in kits):
            handles.append(plt.Line2D([], [], color=COL[role], lw=2, label=name))
    axs[1].legend(handles=handles, frameon=False, fontsize=8, loc="lower right")
    fig.tight_layout()
    fig.savefig(path, facecolor=SURFACE)
    plt.close(fig)


def main():
    spec = json.load(open(sys.argv[1], encoding="utf-8"))
    out = Path(sys.argv[2])
    out.mkdir(parents=True, exist_ok=True)
    kits = {k["id"]: k for k in spec}
    hits = {k: S.load(kits[k]["hits"]) for k in kits}
    summ = {k: S.summarise(hits[k]) for k in kits}
    json.dump(summ, open(out / "summary.json", "w", encoding="utf-8"), indent=1, allow_nan=True)
    md = ["# Tympanum frente a la biblioteca de baterías\n"]
    ranked = []
    for kind in KINDS:
        rows = {}
        for f in FEAT:
            vals = {k: summ[k].get(kind, {}).get(f) for k in kits}
            if sum(1 for k, v in vals.items() if kits[k]["role"] == "ref" and finite(v)) >= 2:
                rows[f] = vals
        if not rows:
            continue
        feature_chart(kind, rows, kits, out / f"fig_{kind}.png")
        curves(kind, kits, hits, out / f"curves_{kind}.png")
        md.append(f"\n## {KIND_ES[kind]}\n\n| medida | referencias (mín … mediana … máx) | "
                  + " | ".join(kits[k]["label"] for k in kits if kits[k]["role"] != "ref") + " |")
        md.append("|---|---|" + "---|" * sum(1 for k in kits if kits[k]["role"] != "ref"))
        for f, vals in rows.items():
            refs = [v for k, v in vals.items() if kits[k]["role"] == "ref" and finite(v)]
            lo, hi, me = min(refs), max(refs), float(np.median(refs))
            span = max(hi - lo, 1e-9)
            cells = []
            for k in kits:
                if kits[k]["role"] == "ref":
                    continue
                v = vals[k]
                if not finite(v):
                    cells.append("—")
                    continue
                d = 0.0 if lo <= v <= hi else (lo - v if v < lo else v - hi) / span
                cells.append(f"{v:.3g} {'dentro' if d == 0 else f'**fuera** ({d:.2f})'}")
                if d > 0:
                    ranked.append((FEAT[f][1] * min(d, 3.0), kind, f, kits[k]["label"], v, lo, hi))
            md.append(f"| {FEAT[f][0]} | {lo:.3g} … {me:.3g} … {hi:.3g} | " + " | ".join(cells) + " |")
    ranked.sort(reverse=True)
    md.append("\n## Lo que está fuera, de lo más audible a lo menos\n")
    md.append("| peso | pieza | medida | kit | valor | rango |\n|---|---|---|---|---|---|")
    for w, kind, f, lab, v, lo, hi in ranked:
        md.append(f"| {w:.2f} | {KIND_ES[kind]} | {FEAT[f][0]} | {lab} | {v:.3g} | {lo:.3g} … {hi:.3g} |")
    (out / "table.md").write_text("\n".join(md) + "\n", encoding="utf-8")
    json.dump([dict(zip(("w", "kind", "feature", "kit", "value", "lo", "hi"), r)) for r in ranked],
              open(out / "ranked.json", "w", encoding="utf-8"), indent=1)
    print(f"{len(ranked)} outside; {out / 'table.md'}")


if __name__ == "__main__":
    main()
