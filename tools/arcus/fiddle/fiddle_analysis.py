"""fiddle_analysis.py - r42 fiddle: the owner's full comparison of the fiddle against the
recordings (Part 1 of $CANTUS_WORK/listen/arcus-analisis/index.html), before and after the
scratch fix (`docs/round42/fiddle.md` §6).

    python -I tools/arcus/fiddle/fiddle_analysis.py [<out dir>]

Reads, all under $CANTUS_WORK:
- the references' per-file measures (`arcus/fiddle/files/*.json`, `fiddle_measure.py run`) and
  the scratch table (`arcus/fiddle/scratch_refs.json`, `fiddle_scratch.py refs`);
- our takes: `listen/fiddle/raspado-antes/` (the scratchy one the owner heard) and
  `listen/fiddle/ahora/` (the fix), each song's fiddle stem with its .cantus as the notes'
  truth, and the lab's single lines (`arcus/fiddle/scratch/lab/*-ahora.wav` before,
  `listen/fiddle/lab/*-ahora.wav` after).

Writes `<out>/fiddle/*.png`, `<out>/fiddle/parte1.json` and the numbers `parte1_html` turns
into the page's Spanish section. Each feature's reference spread is the union of the chosen
reference groups' [p25, p75] (events pooled per group), or across files for the spectrum.
"""
import glob
import json
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import fiddle_groups as fg  # noqa: E402
import fiddle_measure as fm  # noqa: E402
import fiddle_report as fr  # noqa: E402
import fiddle_scratch as fs  # noqa: E402
import fiddle_truth as ft  # noqa: E402

W = fg.cp.work
STEM = "01 Violín country.wav"
SONGS = ("tren", "hoedown")
SETS = {
    "antes": {"label": "Antes (raspado)", "dir": W("listen", "fiddle", "raspado-antes"),
              "lab": W("arcus", "fiddle", "scratch", "lab")},
    "ahora": {"label": "Ahora (mordida)", "dir": W("listen", "fiddle", "ahora"),
              "lab": W("listen", "fiddle", "lab")},
}
LAB = ("rapidas", "agudas")
REF_STYLE = ["fiddle-bluegrass", "fiddle-tunes"]
REF_DRY = ["classical-scales", "classical-music"]
REF_DS = ["fiddle-bluegrass", "fiddle-78s-dstops", "classical-dstops"]

# (key, Spanish name, unit, reference groups, source: "pool" (the songs' stems) or "lab"
# (the single lines' scratch measures) and a note)
FEATURES = [
    ("Espectro y brillo", None),
    ("timbre.centroid_over_f0", "Centroide / f0", "", REF_STYLE + REF_DRY, "pool", ""),
    ("timbre.share_above_2k_db", "Energía sobre 2 kHz (re total)", "dB", REF_STYLE + REF_DRY, "pool", ""),
    ("Ruido entre armónicos (líneas sueltas)", None),
    ("n2-4", "Ruido 2–4 kHz (re el nivel)", "dB", ["dry-classical", "bluegrass"], "lab", ""),
    ("n4-8", "Ruido 4–8 kHz (re el nivel)", "dB", ["dry-classical", "bluegrass"], "lab", ""),
    ("n8-12", "Ruido 8–12 kHz (re el nivel)", "dB", ["dry-classical"], "lab", "las vistas previas MP3 cortan arriba de 10 kHz"),
    ("act4-8 p90", "Ruido 4–8 kHz en ataques y cruces (p90)", "dB", ["dry-classical", "bluegrass"], "lab", ""),
    ("Periodicidad", None),
    ("ap", "Aperiodicidad YIN (mediana)", "", ["dry-classical", "bluegrass"], "lab", ""),
    ("rauc", "Instantes ásperos (d' > 0,25)", "", ["dry-classical", "bluegrass"], "lab", "raucous / multi-slip"),
    ("Ataque", None),
    ("fast.settle_ms", "Tiempo hasta estable (altura a 20 c)", "ms", REF_STYLE + ["classical-scales"], "pool", ""),
    ("fast.steady_share", "Parte estable de cada nota", "", REF_STYLE + ["classical-scales"], "pool", ""),
    ("fast.burst_db", "Ráfaga de ruido del ataque (re estable)", "dB", REF_STYLE + ["classical-scales"], "pool", ""),
    ("fast.burst_ms", "Duración de la ráfaga", "ms", REF_STYLE + ["classical-scales"], "pool", ""),
    ("Cambio de arco y re-articulación", None),
    ("fast.flux_cue_share", "Notas rápidas con señal de re-articulación", "", REF_STYLE + ["classical-scales"], "pool", ""),
    ("fast.dip_db", "Bajón de nivel en el cambio", "dB", REF_STYLE + ["classical-scales"], "pool", ""),
    ("Vibrato", None),
    ("vibrato.share", "Notas largas con vibrato", "", REF_STYLE, "pool", ""),
    ("vibrato.rate_hz", "Frecuencia del vibrato", "Hz", REF_STYLE + REF_DRY, "pool", ""),
    ("vibrato.extent_c", "Profundidad (p-p)", "c", REF_STYLE + REF_DRY, "pool", ""),
    ("vib_on_s", "Inicio del vibrato en la nota", "s", ["dry-classical", "bluegrass"], "lab", ""),
    ("Dobles cuerdas", None),
    ("double.level_lo_minus_hi_db", "Voz grave menos aguda", "dB", REF_DS, "pool", ""),
    ("double.abs_et_dev_c", "Intervalo fuera del temperado", "c", REF_DS, "pool", ""),
    ("double.interval_sd_c", "Oscilación del intervalo", "c", REF_DS, "pool", ""),
    ("Solape de nota a nota", None),
    ("fast.ring60_db", "Nota anterior 60 ms después", "dB", REF_DRY + REF_STYLE, "pool", "las salas alargan las referencias"),
    ("Cola", None),
    ("room.t60_s", "Caída después de parar (T60)", "s", REF_DRY, "pool", "nuestros stems son secos; la cola simpática va aquí"),
]


def ref_files():
    by = {}
    for p in glob.glob(str(W("arcus", "fiddle", "files", "*.json"))):
        d = json.loads(Path(p).read_text(encoding="utf-8"))
        by.setdefault(d.get("group", "?"), []).append(d)
    return by


def ours_files(key):
    s = SETS[key]
    out = []
    cache = W("arcus", "fiddle", "scratch", f"analysis-{key}.json")
    if cache.exists():
        return json.loads(cache.read_text(encoding="utf-8"))
    for song in SONGS:
        stem = Path(s["dir"]) / f"{song}-stems" / STEM
        ns, _ = ft.notes(Path(s["dir"]) / f"{song}.cantus")
        truth = ns["Violín country"]
        print(f"[{key}] {song}", flush=True)
        r = fm.analyse(stem, "ours", truth=truth)
        r["scratch"] = fs.measure(stem)
        out.append(r)
    lab = {n: fs.measure(Path(s["lab"]) / f"{n}-ahora.wav") for n in LAB}
    res = {"songs": out, "lab": lab}
    cache.write_text(json.dumps(res, default=float), encoding="utf-8")
    return res


def spread(groups, key, pooled, scratch):
    lo, hi, meds = [], [], []
    for g in groups:
        if g in scratch:
            v = [r.get(key) for _, r in scratch[g] if r.get(key) is not None and np.isfinite(r.get(key))]
            if v:
                lo.append(np.percentile(v, 25))
                hi.append(np.percentile(v, 75))
                meds.append(np.median(v))
        elif g in pooled and pooled[g].get(key):
            s = pooled[g][key]
            lo.append(s["p25"])
            hi.append(s["p75"])
            meds.append(s["med"])
    if not lo:
        return None
    return {"lo": float(min(lo)), "hi": float(max(hi)), "med": float(np.median(meds))}


def ours_value(res, key, src):
    if src == "lab":
        v = [r.get(key) for r in res["lab"].values()]
    else:
        g = fr.pool(res["songs"])
        s = g.get(key)
        return s["med"] if s else None
    v = [x for x in v if x is not None and np.isfinite(x)]
    return float(np.median(v)) if v else None


def verdict(v, sp, unit):
    if v is None or sp is None:
        return "sin dato", 0.0
    if sp["lo"] <= v <= sp["hi"]:
        return "dentro", 0.0
    d = v - sp["hi"] if v > sp["hi"] else v - sp["lo"]
    u = f" {unit}" if unit else ""
    return (f"fuera por {'+' if d > 0 else '−'}{abs(d):.2f}{u}".replace(".", ","), float(d))


def ltas(files):
    rows = [f["timbre"]["ltas_third_oct_db"] for f in files if f.get("timbre")]
    keys = list(rows[0].keys())
    return keys, np.array([[r[k] for k in keys] for r in rows])


def chart_ltas(out, refs, ours):
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    fig, ax = plt.subplots(figsize=(9, 4.2), dpi=110)
    for g, col, name in (("fiddle-bluegrass", "#9ca3af", "violín country (grabaciones)"),
                         ("classical-scales", "#d6c48a", "violín clásico seco (CNSM)")):
        keys, M = ltas(refs[g])
        f = np.array([float(k) for k in keys])
        k = (f >= 160) & (f <= 12700)
        ax.fill_between(f[k], np.percentile(M, 10, axis=0)[k], np.percentile(M, 90, axis=0)[k],
                        color=col, alpha=0.35, label=name + " p10–p90")
    for key, col in (("antes", "#dc2626"), ("ahora", "#2563eb")):
        keys, M = ltas(ours[key]["songs"])
        f = np.array([float(k) for k in keys])
        k = (f >= 160) & (f <= 12700)
        ax.plot(f[k], M.mean(axis=0)[k], color=col, lw=2, label=SETS[key]["label"])
    ax.set_xscale("log")
    ax.set_xlabel("Frecuencia (Hz), tercios de octava")
    ax.set_ylabel("dB re el total")
    ax.set_title("Espectro de largo plazo: el violín de Tren y Hoedown contra las grabaciones")
    ax.grid(alpha=0.3, which="both")
    ax.legend(fontsize=8, loc="lower left")
    fig.tight_layout()
    fig.savefig(out / "espectro.png")
    plt.close(fig)


def chart_noise(out, scratch, ours):
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    keys = ["n2-4", "n4-8", "n8-12", "act4-8 p90"]
    names = ["2–4 kHz", "4–8 kHz", "8–12 kHz", "4–8 kHz\nataques p90"]
    fig, axs = plt.subplots(1, 2, figsize=(10, 4), dpi=110, gridspec_kw={"width_ratios": [2.2, 1]})
    x = np.arange(len(keys))
    for i, (g, col) in enumerate((("dry-classical", "#d6c48a"), ("bluegrass", "#9ca3af"))):
        v = np.array([[r.get(k, np.nan) for k in keys] for _, r in scratch[g]])
        med, lo, hi = np.nanmedian(v, 0), np.nanpercentile(v, 25, 0), np.nanpercentile(v, 75, 0)
        axs[0].bar(x + (i - 1.5) * 0.2, med, 0.2, yerr=[med - lo, hi - med], color=col,
                   label={"dry-classical": "clásico seco", "bluegrass": "country"}[g], capsize=3)
    for i, (key, col) in enumerate((("antes", "#dc2626"), ("ahora", "#2563eb"))):
        v = [np.median([ours[key]["lab"][n].get(k, np.nan) for n in LAB]) for k in keys]
        axs[0].bar(x + (i + 0.5) * 0.2, v, 0.2, color=col, label=SETS[key]["label"])
    axs[0].set_xticks(x, names)
    axs[0].set_ylabel("ruido entre armónicos, dB re el nivel")
    axs[0].set_title("Ruido entre armónicos por banda (líneas sueltas)")
    axs[0].invert_yaxis()
    axs[0].legend(fontsize=8)
    axs[0].grid(alpha=0.3, axis="y")
    k2 = ["ap", "rauc"]
    x2 = np.arange(2)
    for i, (g, col) in enumerate((("dry-classical", "#d6c48a"), ("bluegrass", "#9ca3af"))):
        v = np.array([[r.get(k, np.nan) for k in k2] for _, r in scratch[g]])
        axs[1].bar(x2 + (i - 1.5) * 0.2, np.nanmedian(v, 0), 0.2, color=col)
    for i, (key, col) in enumerate((("antes", "#dc2626"), ("ahora", "#2563eb"))):
        v = [np.median([ours[key]["lab"][n].get(k, np.nan) for n in LAB]) for k in k2]
        axs[1].bar(x2 + (i + 0.5) * 0.2, v, 0.2, color=col)
    axs[1].set_xticks(x2, ["aperiodicidad\n(mediana)", "instantes\násperos"])
    axs[1].set_title("Periodicidad")
    axs[1].grid(alpha=0.3, axis="y")
    fig.tight_layout()
    fig.savefig(out / "ruido.png")
    plt.close(fig)


def chart_features(out, rows):
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    rows = [r for r in rows if r.get("spread")]
    fig, ax = plt.subplots(figsize=(9, 0.38 * len(rows) + 1.2), dpi=110)
    for i, r in enumerate(rows):
        sp = r["spread"]
        w = max(sp["hi"] - sp["lo"], 1e-6)
        ax.barh(i, 1.0, left=0.0, color="#9ca3af", alpha=0.45, height=0.6)
        for key, col, mk in (("antes", "#dc2626", "o"), ("ahora", "#2563eb", "D")):
            v = r[key]
            if v is not None:
                ax.plot(np.clip((v - sp["lo"]) / w, -2.5, 3.5), i, mk, color=col, ms=7)
    ax.set_yticks(range(len(rows)), [r["name"] for r in rows], fontsize=8)
    ax.invert_yaxis()
    ax.axvline(0, color="#6b7280", lw=0.8)
    ax.axvline(1, color="#6b7280", lw=0.8)
    ax.set_xlim(-2.6, 3.6)
    ax.set_xlabel("posición en el rango de las grabaciones (0 = borde bajo, 1 = borde alto; recortado a −2,5 … 3,5)")
    ax.set_title("Cada rasgo contra las grabaciones: rojo antes (raspado), azul ahora (mordida)")
    ax.grid(alpha=0.3, axis="x")
    fig.tight_layout()
    fig.savefig(out / "rasgos.png")
    plt.close(fig)


def main(argv):
    sys.stdout.reconfigure(encoding="utf-8")
    base = Path(argv[0]) if argv else W("listen", "arcus-analisis")
    out = base / "fiddle"
    out.mkdir(parents=True, exist_ok=True)
    refs = ref_files()
    pooled = {g: fr.pool(fs_) for g, fs_ in refs.items()}
    scratch = json.loads(W("arcus", "fiddle", "scratch_refs.json").read_text(encoding="utf-8"))
    ours = {k: ours_files(k) for k in SETS}
    rows = []
    for f in FEATURES:
        if f[1] is None:
            rows.append({"head": f[0]})
            continue
        key, name, unit, groups, src, note = f
        sp = spread(groups, key, pooled, scratch)
        r = {"key": key, "name": name, "unit": unit, "groups": groups, "src": src, "note": note, "spread": sp}
        for k in SETS:
            r[k] = ours_value(ours[k], key, src)
            r[k + "_verdict"], r[k + "_dist"] = verdict(r[k], sp, unit)
        rows.append(r)
    # the spectrum, band by band (bluegrass files' p10-p90)
    keys, M = ltas(refs["fiddle-bluegrass"])
    keys2, M2 = ltas(refs["classical-scales"])
    bands = []
    for k in SETS:
        _, O = ltas(ours[k]["songs"])
        o = O.mean(axis=0)
        for i, b in enumerate(keys):
            fb = float(b)
            if not 200 <= fb <= 10100:
                continue
            lo = min(np.percentile(M[:, i], 10), np.percentile(M2[:, i], 10))
            hi = max(np.percentile(M[:, i], 90), np.percentile(M2[:, i], 90))
            d = o[i] - hi if o[i] > hi else (o[i] - lo if o[i] < lo else 0.0)
            bands.append({"set": k, "band_hz": fb, "ours_db": float(o[i]), "lo": float(lo), "hi": float(hi), "out_db": float(d)})
    chart_ltas(out, refs, ours)
    chart_noise(out, scratch, ours)
    chart_features(out, [r for r in rows if "key" in r])
    (out / "parte1.json").write_text(json.dumps({"rows": rows, "bands": bands}, indent=1, default=float), encoding="utf-8")
    for r in rows:
        if "head" in r:
            print("##", r["head"])
            continue
        sp = r["spread"]
        rng = f"[{sp['lo']:.2f}, {sp['hi']:.2f}]" if sp else "-"
        a, b = r["antes"], r["ahora"]
        print(f"{r['name'][:44]:44s} ref {rng:18s} antes {a if a is None else round(a, 2)!s:>8} {r['antes_verdict']:22s} "
              f"ahora {b if b is None else round(b, 2)!s:>8} {r['ahora_verdict']}")
    for k in SETS:
        print(k, "bands out:", [(int(b["band_hz"]), round(b["out_db"], 1)) for b in bands if b["set"] == k and b["out_db"] != 0])


if __name__ == "__main__":
    main(sys.argv[1:])
