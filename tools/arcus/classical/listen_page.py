"""r42 arcus-classical — the owner's listening page (Spanish): the analysis's four classical
passages (each solo's cantabile window, the analysis's own clips) before and now beside the
recording, and the quartet demo before and now; 16-bit WAVs under 30 MB each.

    python -I tools/arcus/classical/listen_page.py <listen dir> <notes.json>

The folder holds `antes-<inst>.wav`, `ahora-<inst>.wav`, `grabacion-<inst>.wav` (inst: violin,
viola, violonchelo, contrabajo) and `antes-cuarteto.wav`, `ahora-cuarteto.wav`; `notes.json`
holds the page's Spanish text: {"titulo", "intro", "cambios": [...], "numeros": [[rasgo, antes,
ahora, grabaciones], ...], "escuchar": [...], "abierto": [...]}.
"""
import html
import json
import sys
from pathlib import Path

INSTS = [("violin", "Violín"), ("viola", "Viola"), ("violonchelo", "Violonchelo"),
         ("contrabajo", "Contrabajo")]
CSS = """
:root{--bg:#fbfaf7;--ink:#1d1c1a;--mute:#6b6a65;--line:#e4e2dc;--card:#fff;--acc:#2a5db0}
@media (prefers-color-scheme:dark){:root{--bg:#161615;--ink:#eceae4;--mute:#a19f98;--line:#33322f;--card:#1f1f1d;--acc:#8fb1f0}}
body{background:var(--bg);color:var(--ink);font:16px/1.55 system-ui,-apple-system,Segoe UI,sans-serif;margin:0}
main{max-width:920px;margin:0 auto;padding:24px 16px 64px}
h1{font-size:1.6rem;margin:0 0 .3rem}h2{font-size:1.15rem;margin:2rem 0 .6rem}
p,li{color:var(--ink)}.mute{color:var(--mute)}
.card{background:var(--card);border:1px solid var(--line);border-radius:10px;padding:14px 16px;margin:12px 0}
.row{display:grid;grid-template-columns:110px 1fr;gap:6px 12px;align-items:center}
audio{width:100%}table{border-collapse:collapse;width:100%;font-size:.92rem}
td,th{border-bottom:1px solid var(--line);padding:6px 8px;text-align:left;vertical-align:top}
th{color:var(--mute);font-weight:600}
"""


def audio_row(label, f):
    return f'<div>{html.escape(label)}</div><audio controls preload="none" src="{html.escape(f)}"></audio>'


def main(d, notes):
    d = Path(d)
    n = json.loads(Path(notes).read_text(encoding="utf-8"))
    out = [f"<!doctype html><html lang='es'><head><meta charset='utf-8'><meta name='viewport' content='width=device-width,initial-scale=1'>",
           f"<title>Arcus clásico</title><style>{CSS}</style></head><body><main>",
           f"<h1>{html.escape(n['titulo'])}</h1><p class='mute'>{html.escape(n['intro'])}</p>"]
    out.append("<h2>Qué escuchar</h2><ul>" + "".join(f"<li>{html.escape(x)}</li>" for x in n["escuchar"]) + "</ul>")
    out.append("<h2>Las cuatro frases (la misma ventana que el análisis)</h2>")
    for key, name in INSTS:
        rows = [audio_row("Antes", f"antes-{key}.wav"), audio_row("Ahora", f"ahora-{key}.wav")]
        if (d / f"grabacion-{key}.wav").exists():
            rows.append(audio_row("Grabación", f"grabacion-{key}.wav"))
        out.append(f"<div class='card'><b>{name}</b><div class='row'>{''.join(rows)}</div></div>")
    out.append("<h2>El cuarteto</h2><div class='card'><div class='row'>"
               + audio_row("Antes", "antes-cuarteto.wav") + audio_row("Ahora", "ahora-cuarteto.wav") + "</div></div>")
    out.append("<h2>Qué cambió</h2><ul>" + "".join(f"<li>{html.escape(x)}</li>" for x in n["cambios"]) + "</ul>")
    out.append("<h2>Números (notas reservadas del laboratorio y frases)</h2><table><tr><th>Rasgo</th><th>Antes</th><th>Ahora</th><th>Grabaciones</th></tr>")
    for r in n["numeros"]:
        out.append("<tr>" + "".join(f"<td>{html.escape(str(c))}</td>" for c in r) + "</tr>")
    out.append("</table>")
    out.append("<h2>Lo que queda abierto</h2><ul>" + "".join(f"<li>{html.escape(x)}</li>" for x in n["abierto"]) + "</ul>")
    out.append("<p class='mute'>Las grabaciones de las frases son de HSD Düsseldorf (Mozart K465, CC BY 4.0) y, la del contrabajo, de Aalto (sólo para escuchar aquí, no se publica). Arcus y la grabación a la misma sonoridad (−23 LUFS).</p></main></body></html>")
    (d / "escuchar.html").write_text("\n".join(out), encoding="utf-8")
    print("written", d / "escuchar.html")


if __name__ == "__main__":
    main(*sys.argv[1:3])
