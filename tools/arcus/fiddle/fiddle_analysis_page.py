"""fiddle_analysis_page.py - r42: the owner's analysis pages (Spanish), from the numbers.

    python -I tools/arcus/fiddle/fiddle_analysis_page.py [<dir>]

`<dir>` (default $CANTUS_WORK/listen/arcus-analisis) holds `fiddle/parte1.json` and the
charts (`fiddle_analysis.py`), and `clasico/parte2.html` (the classical part, an HTML
fragment). It writes `fiddle/parte1.html` (the fiddle's fragment), `index.html` (both parts)
and `resumen.html` (the short summary). The clips are played from `../fiddle/chat/` and
`clasico/`; the ranked lists and the summary's sentences are the data below, written from
the measured numbers (`docs/round42/fiddle.md` §6).
"""
import html
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import fiddle_groups as fg  # noqa: E402
import fiddle_analysis_text as tx  # noqa: E402

CSS = """
:root{--bg:#f7f5f0;--fg:#1f2328;--mut:#57606a;--card:#fff;--line:#d8d4ca;--in:#1e7d3a;--out:#c0392b;--acc:#8a5a00}
@media (prefers-color-scheme:dark){:root:not([data-theme="light"]){--bg:#111418;--fg:#e6e6e6;--mut:#a0a8b0;--card:#1b1f24;--line:#30363d;--in:#4cc46e;--out:#ff6b5b;--acc:#f0b44c}}
:root[data-theme="dark"]{--bg:#111418;--fg:#e6e6e6;--mut:#a0a8b0;--card:#1b1f24;--line:#30363d;--in:#4cc46e;--out:#ff6b5b;--acc:#f0b44c}
body{background:var(--bg);color:var(--fg);font:15px/1.55 system-ui,-apple-system,"Segoe UI",sans-serif;margin:0 auto;max-width:1040px;padding:16px}
h1{font-size:24px;margin:8px 0}h2{font-size:20px;margin:28px 0 6px;border-bottom:1px solid var(--line);padding-bottom:4px}
h3{font-size:16px;margin:18px 0 4px}p,li{color:var(--fg)}.mut{color:var(--mut);font-size:.9em}
table{border-collapse:collapse;width:100%;font-size:.88em;margin:.6em 0 1.2em;display:block;overflow-x:auto}
th,td{border-bottom:1px solid var(--line);padding:.3em .45em;text-align:left;vertical-align:top}
td.in b{color:var(--in)}td.out b{color:var(--out)}tr.head td{font-weight:600;color:var(--acc);padding-top:.8em}
img{max-width:100%;height:auto;border-radius:4px;background:#fff}
.clips{display:flex;flex-wrap:wrap;gap:12px}.clip{background:var(--card);border:1px solid var(--line);border-radius:8px;padding:8px 10px;flex:1 1 280px}
.clip b{display:block;font-size:13px;color:var(--acc)}audio{width:100%}
ol li{margin-bottom:.7em}nav a{margin-right:12px;color:var(--acc)}
"""


def page(title, body):
    return ('<!doctype html><html lang="es"><head><meta charset="utf-8"><meta name="viewport" '
            f'content="width=device-width,initial-scale=1"><title>{html.escape(title)}</title>'
            f'<style>{CSS}</style></head><body>{body}</body></html>')


def fmt(v, unit=""):
    if v is None:
        return "—"
    s = f"{v:.2f}".replace(".", ",").replace("-", "−")
    return s + (f" {unit}" if unit else "")


def clips(items):
    cells = "".join(f'<div class="clip"><b>{html.escape(lab)}</b><audio controls preload="none" '
                    f'src="{html.escape(src)}"></audio></div>' for src, lab in items)
    return f'<div class="clips">{cells}</div>'


def table(rows):
    out = ['<table><tr><th>Rasgo</th><th>Grabaciones (rango)</th><th>Antes (raspado)</th>'
           '<th>Ahora</th><th>Nota</th></tr>']
    for r in rows:
        if "head" in r:
            out.append(f'<tr class="head"><td colspan="5">{html.escape(r["head"])}</td></tr>')
            continue
        sp = r["spread"]
        rng = f'{fmt(sp["lo"])} … {fmt(sp["hi"])}' if sp else "—"
        cells = []
        for k in ("antes", "ahora"):
            v = r["antes_verdict"] if k == "antes" else r["ahora_verdict"]
            cls = "in" if v == "dentro" else "out"
            cells.append(f'<td class="{cls}">{fmt(r[k], r["unit"])}<br><b>{html.escape(v)}</b></td>')
        out.append(f'<tr><td>{html.escape(r["name"])}</td><td>{rng}</td>{"".join(cells)}'
                   f'<td class="mut">{html.escape(r["note"])}</td></tr>')
    out.append("</table>")
    return "".join(out)


def bands_text(bands):
    out = []
    for k, name in (("antes", "Antes"), ("ahora", "Ahora")):
        off = [b for b in bands if b["set"] == k and b["out_db"] != 0]
        if not off:
            out.append(f"<li>{name}: todas las bandas de 200 Hz a 10 kHz dentro del rango.</li>")
            continue
        s = ", ".join(f'{b["band_hz"]:.0f} Hz {fmt(b["out_db"])} dB' for b in off)
        out.append(f"<li>{name}: fuera — {s}.</li>")
    return "<ul>" + "".join(out) + "</ul>"


def part1(d):
    data = json.loads((d / "fiddle" / "parte1.json").read_text(encoding="utf-8"))
    rows, bands = data["rows"], data["bands"]
    body = [f'<section id="parte1"><h2>Parte 1 — El violín country: grabaciones contra lo generado</h2>',
            tx.P1_INTRO,
            '<h3>Escuchar</h3>', clips(tx.P1_CLIPS),
            '<h3>Espectro de largo plazo</h3><img src="fiddle/espectro.png" alt="Espectro por tercios de octava">',
            tx.P1_SPECTRUM, bands_text(bands),
            '<h3>Ruido entre armónicos y periodicidad</h3><img src="fiddle/ruido.png" alt="Ruido por banda y periodicidad">',
            tx.P1_NOISE,
            '<h3>Cada rasgo contra las grabaciones</h3><img src="fiddle/rasgos.png" alt="Rasgos contra el rango">',
            table(rows), tx.P1_NOTES,
            '<h3>Lo que suena irreal, por orden</h3><ol>',
            "".join(f"<li><b>{html.escape(a)}</b> {html.escape(b)} <i>Propuesta:</i> {html.escape(c)}</li>"
                    for a, b, c in tx.P1_RANKED),
            "</ol></section>"]
    frag = "".join(body)
    (d / "fiddle" / "parte1.html").write_text(frag, encoding="utf-8")
    return frag


def main(argv):
    d = Path(argv[0]) if argv else fg.cp.work("listen", "arcus-analisis")
    p1 = part1(d)
    p2 = (d / "clasico" / "parte2.html").read_text(encoding="utf-8")
    nav = '<nav><a href="resumen.html">Resumen</a><a href="#parte1">Parte 1: el violín country</a><a href="#parte2">Parte 2: el modo clásico</a></nav>'
    (d / "index.html").write_text(page("Arcus contra grabaciones", "<h1>Arcus contra las grabaciones</h1>" + nav + tx.INDEX_INTRO + p1 + p2), encoding="utf-8")
    (d / "resumen.html").write_text(page("Arcus: resumen", "<h1>Arcus: resumen del análisis</h1>"
                                         '<nav><a href="index.html">El análisis completo</a></nav>' + tx.SUMMARY), encoding="utf-8")
    print(d / "index.html", d / "resumen.html")


if __name__ == "__main__":
    main(sys.argv[1:])
