"""The Paganini comparison's measures (round 42): the original and Arcus (aligned, in the
hall, both versions) measured note by note on the same notes (`notes.json`'s passage times)
and over the whole passage — the long-term spectrum in third octaves, the noise between
the harmonics, the intonation and the vibrato, the notes' envelopes (how far each starts
and ends under its peak), the attacks, and the bow-change cues. PNGs and `analisis.json`
into <out>/analisis/.

    python -I tools/listening/paganini_compare.py <work dir> <out dir>
"""
import json
import os
import sys

import numpy as np
from scipy import signal

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import paganini_common as pc  # noqa: E402
import paganini_notes as pn  # noqa: E402
from soundlab import core, features  # noqa: E402

import matplotlib  # noqa: E402
matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402

# the reference palette's first three categorical slots (validated: dataviz's script)
WHO = [("original", "Original (QMUL 2014)", "#2a78d6", "-", "o"),
       ("arcus-actual-alineado-sala", "Arcus, versión actual", "#eb6834", "--", "s"),
       ("arcus-mejora-alineado-1cuerda-sala", "Arcus, versión en mejora (una cuerda)", "#1baf7a", ":", "^")]
SURF, INK, INK2, GRID = "#fcfcfb", "#0b0b0b", "#52514e", "#e4e2dc"
# variation 2's crossings: the three above and the controls (one string at a time)
CROSS = [WHO[0], WHO[1],
         ("arcus-mejora-alineado-sala", "Arcus en mejora, dos cuerdas", "#1baf7a", ":", "^"),
         ("arcus-actual-alineado-1cuerda-sala", "Arcus actual, una cuerda (control)", "#eda100",
          "-.", "D")]


def style(ax, title, xl, yl):
    ax.set_facecolor(SURF)
    ax.set_title(title, color=INK, fontsize=11, loc="left")
    ax.set_xlabel(xl, color=INK2)
    ax.set_ylabel(yl, color=INK2)
    ax.tick_params(colors=INK2, labelsize=8)
    ax.grid(True, color=GRID, lw=0.6)
    for s in ax.spines.values():
        s.set_color(GRID)


def fig(w=9, h=4, rows=1):
    f, ax = plt.subplots(rows, 1, figsize=(w, h), dpi=110, squeeze=False)
    f.patch.set_facecolor(SURF)
    return f, ax[:, 0]


def save(f, path):
    f.tight_layout()
    f.savefig(path, facecolor=SURF)
    plt.close(f)


def band_db(m, sr, lo, hi, hop_s=0.005):
    n = 1024
    h = int(hop_s * sr)
    fr, t, z = signal.stft(m, sr, nperseg=n, noverlap=n - h, boundary=None, padded=False)
    s = (fr >= lo) & (fr < hi)
    return t, 10 * np.log10((np.abs(z[s]) ** 2).sum(axis=0) + 1e-20)


def per_note(notes, m, sr, t, k, ap, te, e, ht, he):
    rows = []
    for i, n in enumerate(notes):
        on = n["p_on"]
        nxt = notes[i + 1]["p_on"] if i + 1 < len(notes) and notes[i + 1]["section"] == n["section"] \
            else n["p_off"] + 0.1
        ioi = nxt - on
        r = dict(i=i, on=on, ioi=ioi, key=n["key"], section=n["section"], art=n["art"],
                 new_bow=n["new_bow"])
        s = (te >= on) & (te < nxt)
        if s.sum() < 5:
            rows.append(r)
            continue
        seg_t, seg = te[s], e[s]
        p = int(np.argmax(seg))
        pk = float(seg[p])
        r["peak_db"] = pk
        r["start_under"] = pk - float(seg[min(int(0.015 / 0.002), len(seg) - 1)])
        tail = seg[seg_t >= nxt - 0.03]
        r["end_under"] = pk - float(tail.min()) if len(tail) else None
        reach = np.nonzero(seg >= pk - 3.0)[0]
        r["rise_ms"] = 1000 * float(seg_t[reach[0]] - on) if len(reach) else None
        body = (he[(ht >= on + 0.25 * ioi) & (ht < on + 0.85 * ioi)])
        head = he[(ht >= on) & (ht < on + 0.03)]
        if len(body) and len(head):
            r["hf_burst_db"] = float(head.max() - np.median(body))
        # the bow-change dip: the least level within 30 ms of the onset, under the two
        # notes' medians
        if i > 0 and notes[i - 1]["section"] == n["section"]:
            prev = (te >= notes[i - 1]["p_on"]) & (te < on)
            near = (te >= on - 0.03) & (te < on + 0.03)
            if prev.sum() > 3 and near.any():
                ref = 0.5 * (np.median(e[prev]) + np.median(seg))
                r["dip_db"] = float(ref - e[near].min())
                r["joined"] = notes[i - 1].get("gap", 1.0) < 0.03
        # pitch: the note's own frames
        sel = (t >= on + 0.04) & (t < nxt - 0.02) & (ap < 0.2)
        if sel.sum() >= 6:
            c = 100 * (k[sel] - n["key"])
            c = (c + 600) % 1200 - 600
            r["cents"] = float(np.median(c))
            if ioi >= 0.3 and sel.sum() >= 50:
                tt = t[sel]
                dt = float(np.median(np.diff(tt)))
                w = max(int(0.2 / dt) | 1, 3)
                trend = np.convolve(c, np.ones(w) / w, mode="same")
                cy = features.vibrato_cycles(tt, c - trend, 6.0)
                if len(cy) >= 2:
                    r["vib_pp"] = float(np.median([q[2] for q in cy]))
                    r["vib_hz"] = float(1.0 / np.median([q[1] for q in cy]))
                else:
                    r["vib_pp"] = 0.0
        # the noise between the harmonics, on the middle of a note of 0.2 s or more
        if ioi >= 0.2 and "cents" in r:
            a0 = int((on + 0.25 * ioi) * sr)
            L = int(0.6 * ioi * sr)
            f0 = 440.0 * 2 ** ((n["key"] + r["cents"] / 100 - 69) / 12)
            fc, hnr, valid = features.between_partials(m, sr, a0, L, f0, 0.0)
            r["hnr"] = [h if v else None for h, v in zip(hnr, valid)]
            r["hnr_fc"] = fc
        rows.append(r)
    return rows


def med(rows, key, pred=lambda r: True):
    v = [r[key] for r in rows if r.get(key) is not None and pred(r)]
    return (float(np.median(v)), float(np.percentile(v, 25)), float(np.percentile(v, 75)), len(v)) \
        if v else None


def main(work, out):
    od = os.path.join(out, "analisis")
    os.makedirs(od, exist_ok=True)
    notes = json.load(open(os.path.join(work, "notes.json"), encoding="utf-8"))["notes"]
    res, ltas, tracks = {}, {}, {}
    for tag, label, col, ls, mk in WHO:
        x, sr = pc.audio.read(os.path.join(out, tag + ".wav"))
        m = x[:, 0]
        t, k, ap = pc.pitch_track(m, sr)
        te, e = pc.env_db(m, sr)
        ht, he = band_db(m, sr, 3000, 10000)
        rows = per_note(notes, m, sr, t, k, ap, te, e, ht, he)
        fc, p = core.band_powers(m, sr, 3, 50.0, 16000.0)
        ltas[tag] = (fc, 10 * np.log10(p / p.sum() + 1e-20))
        tracks[tag] = (t, k, ap)
        res[tag] = rows
        print(tag, "notes measured", sum(1 for r in rows if "peak_db" in r))

    summary = {}
    for tag, *_ in WHO:
        rows = res[tag]
        cents = [r["cents"] for r in rows if r.get("cents") is not None]
        off = float(np.median(cents))
        long_ = [r for r in rows if r.get("vib_pp") is not None]
        summary[tag] = dict(
            tuning_cents=off,
            intonation_spread_cents=float(np.median(np.abs(np.array(cents) - off))),
            start_under=med(rows, "start_under", lambda r: r["ioi"] >= 0.15),
            end_under=med(rows, "end_under", lambda r: r["ioi"] >= 0.15),
            end_under_long=med(rows, "end_under", lambda r: r["ioi"] >= 0.3),
            rise_ms=med(rows, "rise_ms", lambda r: r["ioi"] >= 0.15),
            hf_burst=med(rows, "hf_burst_db", lambda r: r["ioi"] >= 0.15),
            dip_rebow=med(rows, "dip_db", lambda r: r.get("joined") and r["new_bow"]),
            dip_slur=med(rows, "dip_db", lambda r: r.get("joined") and not r["new_bow"]),
            vib_pp=med(long_, "vib_pp"), vib_hz=med(long_, "vib_hz"),
            vib_share=float(np.mean([r["vib_pp"] >= 15 for r in long_])) if long_ else None,
            long_notes=len(long_),
        )
        hn = [r["hnr"] for r in rows if r.get("hnr")]
        fcs = next(r["hnr_fc"] for r in rows if r.get("hnr"))
        summary[tag]["hnr_fc"] = fcs
        summary[tag]["hnr_med"] = [
            (float(np.median([h[b] for h in hn if h[b] is not None]))
             if sum(h[b] is not None for h in hn) >= 5 else None) for b in range(len(fcs))]
        summary[tag]["ltas_fc"] = [float(v) for v in ltas[tag][0]]
        summary[tag]["ltas_db"] = [float(v) for v in ltas[tag][1]]

    # 1 — the long-term spectrum
    f, (ax, ax2) = fig(9, 6.4, 2)
    for tag, label, col, ls, mk in WHO:
        fc, d = ltas[tag]
        ax.semilogx(fc, d, color=col, ls=ls, lw=2, label=label)
        if tag != "original":
            ax2.semilogx(fc, d - ltas["original"][1], color=col, ls=ls, lw=2, label=label)
    ax2.axhline(0, color=INK2, lw=1)
    style(ax, "Espectro de largo plazo (tercios de octava, todo el pasaje)", "Hz", "dB respecto al total")
    style(ax2, "Arcus menos el original", "Hz", "dB")
    ax.legend(fontsize=8, frameon=False)
    ax2.legend(fontsize=8, frameon=False)
    save(f, os.path.join(od, "espectro.png"))

    # 2 — noise between harmonics
    f, (ax,) = fig(9, 3.8)
    for tag, label, col, ls, mk in WHO:
        fc, h = summary[tag]["hnr_fc"], summary[tag]["hnr_med"]
        xs = [a for a, b in zip(fc, h) if b is not None]
        ys = [b for b in h if b is not None]
        ax.semilogx(xs, ys, color=col, ls=ls, lw=2, marker=mk, ms=6, label=label)
    style(ax, "Armónicos sobre el ruido entre ellos (mediana de las notas de 0,2 s o más)",
          "Hz (tercio de octava)", "dB (más alto = más limpio)")
    ax.legend(fontsize=8, frameon=False)
    save(f, os.path.join(od, "ruido.png"))

    # 3 — intonation and vibrato over the passage
    f, axs = fig(9, 7.2, 3)
    for ax, (tag, label, col, ls, mk) in zip(axs, WHO):
        t, k, ap = tracks[tag]
        key_at = np.full(len(t), np.nan)
        for i, n in enumerate(notes):
            nxt = notes[i + 1]["p_on"] if i + 1 < len(notes) else n["p_off"]
            key_at[(t >= n["p_on"] + 0.03) & (t < nxt - 0.01)] = n["key"]
        c = 100 * (k - key_at)
        c = (c + 600) % 1200 - 600
        c[ap >= 0.2] = np.nan
        ax.plot(t, c, color=col, lw=0.8)
        ax.set_ylim(-80, 80)
        ax.axhline(0, color=INK2, lw=0.8)
        style(ax, f"{label}: cents respecto a la nota temperada (A = 440 Hz)", "s (pasaje)", "cents")
    save(f, os.path.join(od, "afinacion.png"))

    f, (ax,) = fig(9, 3.8)
    for tag, label, col, ls, mk in WHO:
        rows = [r for r in res[tag] if r.get("vib_pp") is not None]
        ax.scatter([r["on"] for r in rows], [r["vib_pp"] for r in rows], s=36, color=col,
                   marker=mk, edgecolor=SURF, lw=1.5, label=label)
    style(ax, "Profundidad del vibrato en cada nota de 0,3 s o más (pico a valle)", "s (pasaje)", "cents")
    ax.legend(fontsize=8, frameon=False)
    save(f, os.path.join(od, "vibrato.png"))

    # 4 — envelopes, 5 — attacks, 6 — bow changes: box plots side by side
    def boxes(ax, key, pred, title, yl):
        data = [[r[key] for r in res[tag] if r.get(key) is not None and pred(r)] for tag, *_ in WHO]
        bp = ax.boxplot(data, widths=0.5, patch_artist=True, showfliers=False)
        for patch, (tag, label, col, ls, mk) in zip(bp["boxes"], WHO):
            patch.set_facecolor(col)
            patch.set_alpha(0.35)
            patch.set_edgecolor(col)
        for mline in bp["medians"]:
            mline.set_color(INK)
        ax.set_xticks([1, 2, 3])
        ax.set_xticklabels(["Original", "Arcus actual", "Arcus en mejora"])
        style(ax, title, "", yl)

    f, (a1, a2) = fig(9, 6.4, 2)
    boxes(a1, "start_under", lambda r: r["ioi"] >= 0.15,
          "Cuánto empieza cada nota bajo su pico (a los 15 ms)", "dB")
    boxes(a2, "end_under", lambda r: r["ioi"] >= 0.15,
          "Cuánto termina cada nota bajo su pico (lo más bajo de sus últimos 30 ms)", "dB")
    save(f, os.path.join(od, "envolventes.png"))

    f, (a1, a2) = fig(9, 6.4, 2)
    boxes(a1, "rise_ms", lambda r: r["ioi"] >= 0.15, "Subida del ataque: hasta 3 dB bajo el pico", "ms")
    boxes(a2, "hf_burst_db", lambda r: r["ioi"] >= 0.15,
          "Ráfaga del ataque: 3–10 kHz en los primeros 30 ms sobre el cuerpo de la nota", "dB")
    save(f, os.path.join(od, "ataques.png"))

    f, (a1, a2) = fig(9, 6.4, 2)
    boxes(a1, "dip_db", lambda r: r.get("joined") and r["new_bow"],
          "Cambio de arco entre notas unidas: el hueco de nivel", "dB bajo las dos notas")
    boxes(a2, "dip_db", lambda r: r.get("joined") and not r["new_bow"],
          "Notas ligadas (un arco, Variación 2): el hueco de nivel", "dB bajo las dos notas")
    save(f, os.path.join(od, "cambios-de-arco.png"))

    # 7 — the dynamic note by note: each note's peak on Arcus against the original's
    f, (ax,) = fig(6.4, 5.2)
    o = res["original"]
    for tag, label, col, ls, mk in WHO[1:]:
        pairs = [(a["peak_db"], b["peak_db"]) for a, b in zip(o, res[tag])
                 if a.get("peak_db") is not None and b.get("peak_db") is not None and a["ioi"] >= 0.1]
        xa, ya = np.array(pairs).T
        slope, icpt = np.polyfit(xa, ya, 1)
        r = float(np.corrcoef(xa, ya)[0, 1])
        summary[tag]["dyn_slope"] = float(slope)
        summary[tag]["dyn_r"] = r
        summary[tag]["dyn_range_db"] = float(np.percentile(ya, 95) - np.percentile(ya, 5))
        ax.scatter(xa, ya, s=30, color=col, marker=mk, edgecolor=SURF, lw=1.2,
                   label=f"{label}: pendiente {slope:.2f}, r {r:.2f}")
    xo = np.array([a["peak_db"] for a in o if a.get("peak_db") is not None and a["ioi"] >= 0.1])
    summary["original"]["dyn_range_db"] = float(np.percentile(xo, 95) - np.percentile(xo, 5))
    lim = [min(xo.min(), -45), -8]
    ax.plot(lim, lim, color=INK2, lw=1)
    style(ax, "La dinámica nota a nota (pico de cada nota, archivos a −23 LUFS)",
          "original, dB", "Arcus, dB")
    ax.legend(fontsize=8, frameon=False)
    save(f, os.path.join(od, "dinamica.png"))

    # 8 — variation 2's two strings: bar by bar, how far the upper string falls under the
    # lower note, the balance there, and the interval's tuning
    cross = {}
    for tag, label, col, ls, mk in CROSS:
        x, sr = pc.audio.read(os.path.join(out, tag + ".wav"))
        cross[tag] = pn.crossings(x[:, 0], sr, notes, key_t="p_on")
    xs = {}
    for tag, rows in cross.items():
        iv = [r["interval_cents"] for r in rows if r["interval_cents"] is not None]
        xs[tag] = dict(ring_db=float(np.median([r["ring_db"] for r in rows])),
                       balance_db=float(np.median([r["balance_db"] for r in rows])),
                       interval_ct=float(np.median(iv)),
                       interval_spread_ct=float(np.median(np.abs(np.array(iv) - np.median(iv)))))
    summary["cross"] = xs
    f, axs = fig(9, 9.0, 3)
    for tag, label, col, ls, mk in CROSS:
        rows = cross[tag]
        bars = [r["bar"] for r in rows]
        for ax, key in zip(axs, ("ring_db", "balance_db", "interval_cents")):
            ax.plot(bars, [r[key] for r in rows], color=col, ls=ls, lw=2, marker=mk, ms=7,
                    mec=SURF, mew=1.5, label=label)
    style(axs[0], "Variación 2: cuánto cae la cuerda de arriba bajo la nota de abajo (tiempo 1 de cada compás)",
          "compás (con la repetición)", "dB bajo su pico")
    style(axs[1], "Equilibrio de las dos cuerdas mientras suena la de abajo", "compás", "dB, arriba menos abajo")
    style(axs[2], "Afinación del intervalo (arriba menos abajo) frente al temperado", "compás", "cents")
    for ax in axs:
        ax.set_xticks(range(1, 17))
        ax.legend(fontsize=7, frameon=False, ncol=2)
    save(f, os.path.join(od, "cuerdas.png"))

    json.dump(dict(summary=summary, crossings=cross, notes={k: v for k, v in res.items()}),
              open(os.path.join(od, "analisis.json"), "w", encoding="utf-8"), indent=1)
    for tag, s in summary.items():
        if tag == "cross":
            print("cross", s)
            continue
        print(tag)
        for kk, v in s.items():
            if kk.startswith("ltas") or kk.startswith("hnr"):
                continue
            print("   ", kk, v)
        print("    hnr", [None if v is None else round(v, 1) for v in s.get("hnr_med", [])])
    fc = summary["original"]["ltas_fc"]
    for tag in ("arcus-actual-alineado-sala", "arcus-mejora-alineado-1cuerda-sala"):
        d = np.array(summary[tag]["ltas_db"]) - np.array(summary["original"]["ltas_db"])
        print(tag, "ltas diff", [(int(a), round(b, 1)) for a, b in zip(fc, d)])


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
