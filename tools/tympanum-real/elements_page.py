#!/usr/bin/env python3
"""The element-by-element page (Spanish), from `elements.py`'s output and the verdicts.

    python -I tools/tympanum-real/elements_page.py <out dir> <verdicts.json>

`verdicts.json`: {"intro": html, "pieces": {piece id: html}, "summary": html} — the words are
the analyst's, written after reading `numbers.json` and the figures.
"""
import html
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from elements import PIECES  # noqa: E402

CSS = """
:root{--bg:#fcfcfb;--ink:#0b0b0b;--muted:#52514e;--line:#e4e3df;--card:#fff;--acc:#2a78d6;--bad:#b3261e}
@media (prefers-color-scheme:dark){:root{--bg:#1a1a19;--ink:#f0efec;--muted:#c3c2b7;--line:#33322f;--card:#232321;--acc:#3987e5;--bad:#f2b8b5}}
body{margin:0;background:var(--bg);color:var(--ink);font:16px/1.55 system-ui,sans-serif}
main{max-width:1100px;margin:0 auto;padding:24px 16px 64px}
h1{font-size:1.6rem}h2{margin-top:2.2em;border-top:1px solid var(--line);padding-top:12px}
h3{font-size:1rem;margin:1.2em 0 .3em}
img{max-width:100%;height:auto;border:1px solid var(--line);border-radius:6px;background:#fff}
table{border-collapse:collapse;font-size:.88rem;margin:.4em 0;width:100%}
td,th{border:1px solid var(--line);padding:3px 6px;text-align:left;vertical-align:top}
.clips{display:grid;grid-template-columns:repeat(auto-fill,minmax(250px,1fr));gap:6px}
.clip{background:var(--card);border:1px solid var(--line);border-radius:6px;padding:6px}
.clip.t{border-left:4px solid var(--acc)}audio{width:100%}small,.muted{color:var(--muted)}
.verdict{background:var(--card);border-left:4px solid var(--bad);padding:8px 12px;border-radius:4px}
nav a{margin-right:10px}
"""


def table(rows, head):
    out = ["<table><tr>" + "".join(f"<th>{html.escape(h)}</th>" for h in head) + "</tr>"]
    for r in rows:
        out.append("<tr>" + "".join(f"<td>{html.escape(str(c))}</td>" for c in r) + "</tr>")
    out.append("</table>")
    return "\n".join(out)


def main():
    out = Path(sys.argv[1])
    v = json.loads(Path(sys.argv[2]).read_text(encoding="utf-8"))
    num = json.loads((out / "numbers.json").read_text(encoding="utf-8"))
    p = ["<!doctype html><html lang='es'><head><meta charset='utf-8'><meta name='viewport' "
         "content='width=device-width,initial-scale=1'><title>Tympanum pieza a pieza</title><style>"
         + CSS + "</style></head><body><main><h1>Tympanum pieza a pieza, frente a baterías grabadas</h1>",
         v["intro"], "<nav>" + "".join(f"<a href='#{pid}'>{html.escape(n)}</a>" for pid, n, *_ in PIECES if pid in num)
         + "<a href='#resumen'>Resumen</a></nav>"]
    octs = ["63", "125", "250", "500", "1k", "2k", "4k", "8k", "16k"]
    for pid, name, kind, art in PIECES:
        if pid not in num:
            continue
        n = num[pid]
        p.append(f"<h2 id='{pid}'>{html.escape(name)}</h2>")
        p.append(f"<div class='verdict'>{v['pieces'].get(pid, '')}</div>")
        p.append("<h3>1. Golpes sueltos, a la misma sonoridad</h3>")
        for vn, clips in n["clips"].items():
            p.append(f"<p class='muted'>{vn}</p><div class='clips'>")
            for lab, f in clips:
                t = " t" if lab.startswith(("Studio", "Jazz")) else ""
                p.append(f"<div class='clip{t}'><small>{html.escape(lab)}</small>"
                         f"<audio controls preload='none' src='{html.escape(f)}'></audio></div>")
            p.append("</div>")
        p.append("<h3>2. Espectrogramas (0–20 kHz, frecuencia logarítmica; 1,5 s y los primeros 100 ms)</h3>"
                 f"<img src='{pid}/espectrograma.png' alt='espectrogramas'>")
        p.append("<h3>3. Espectro en tercios de octava (primer 1,5 s), con el rango de las grabadas en gris</h3>"
                 f"<img src='{pid}/tercios.png' alt='tercios'>")
        rows = [[k, ", ".join(f"{c:.0f} Hz {o:+.0f}" for c, o in bands) or "dentro en todas"]
                for k, bands in n["thirds"].items()]
        if rows:
            p.append("<p class='muted'>Bandas fuera del rango de las grabadas por 3 dB o más (dB fuera):</p>"
                     + table(rows, ["Tympanum", "bandas fuera"]))
        p.append("<h3>4. Caída por octava y los primeros 100 ms por banda</h3>"
                 f"<img src='{pid}/caida.png' alt='caída'>")
        p.append(table([[k] + [("—" if x is None else x) for x in t] for k, t in n["t30"].items()],
                       ["T60 (ms)"] + octs))
        p.append("<h3>5. Parciales (los picos del espectro de 20–300 ms; los platos, 50–600 ms)</h3>"
                 f"<img src='{pid}/parciales.png' alt='parciales'>")
        p.append(table([[k, ", ".join(f"{f:.0f}" for f, _ in pk)] for k, pk in n["partials"].items()],
                       ["", "picos (Hz)"]))
        p.append("<h3>6. De un golpe al siguiente: 8 golpes iguales superpuestos</h3>"
                 f"<img src='{pid}/variacion.png' alt='variación'>")
        p.append(table([[k, x["espectro_db"], x["nivel_db"]] for k, x in n["var"].items()],
                       ["", "diferencia de espectro (dB)", "diferencia de nivel (dB)"]))
        if "mics" in n:
            p.append("<h3>7. Imagen de micrófonos: cercano, aéreos, sala (mf)</h3>"
                     f"<img src='{pid}/micros.png' alt='micrófonos'>")
            mic = lambda x, m: "—" if x.get(m) is None or x[m] < -300 else x[m]  # noqa: E731
            p.append(table([[k, mic(x, "close"), mic(x, "oh"), mic(x, "room")] for k, x in n["mics"].items()],
                           ["nivel de los primeros 300 ms (dBFS)", "cercano", "aéreos", "sala"]))
    p.append(f"<h2 id='resumen'>Resumen, de peor a mejor</h2>{v['summary']}")
    p.append("<p><small>Grabadas: DRSKit (DrumGizmo, CC BY 4.0); Big Rusty Drums, Gogodze Phu II, Virtuosity "
             "Drums (CC0). Todas tocadas por el propio cargador de Tympanum, con su mezcla por defecto.</small></p>")
    p.append("</main></body></html>")
    (out / "index.html").write_text("\n".join(p), encoding="utf-8")
    print(out / "index.html")


if __name__ == "__main__":
    main()
