"""listen_page.py - r42 arcus-expr: the owner's listening page (Spanish) and the chat copies.

    python -I tools/arcus/expr/listen_page.py <listen-dir>

`<listen-dir>` holds the takes, each folder optional:

- `pase6/arcus-cuarteto.wav`          the quartet as pass 6 left it (copied from its render)
- `etapa1/arcus-cuarteto.wav`         the quartet performed with what Arcus can do (stage 1)
- `etapa2/arcus-cuarteto.wav`         the same performance on the stage-2 instrument
- `solos1/<inst>-{plano,interpretado}.wav`, `solos2/<inst>-interpretado.wav`

It writes `escuchar.html` beside them and `chat/` — 16-bit WAV copies of the main clips (the
quartet takes and the performed solos), each kept under 30 MB (44.1 kHz when 48 kHz would not
fit, then mono).
"""
import html
import sys
from pathlib import Path

import numpy as np
import soundfile as sf
from scipy.signal import resample_poly

LIMIT = 30 * 1000 * 1000
INST = [("violin", "Violín"), ("viola", "Viola"), ("violonchelo", "Violonchelo"),
        ("contrabajo", "Contrabajo")]


def chat_copy(src, dst):
    x, sr = sf.read(str(src), dtype="float64", always_2d=True)
    for rate, mono in ((sr, False), (44100, False), (44100, True), (32000, True)):
        ch = 1 if mono else x.shape[1]
        if len(x) * rate / sr * ch * 2 + 1000 <= LIMIT:
            y = x.mean(axis=1, keepdims=True) if mono else x
            if rate != sr:
                y = resample_poly(y, rate, sr, axis=0)
            y = np.clip(y, -1.0, 1.0)
            dst.parent.mkdir(parents=True, exist_ok=True)
            sf.write(str(dst), y, rate, subtype="PCM_16")
            return dst.stat().st_size, rate, ch
    raise SystemExit(f"{src}: too long for {LIMIT} bytes")


def player(rel, label):
    return (f'<div><b>{html.escape(label)}</b><audio controls preload="none" '
            f'src="{html.escape(rel)}"></audio></div>')


def row(title, items):
    cells = "".join(player(r, l) for r, l in items)
    return f'<div class="r"><span>{title}</span>{cells}</div>\n'


def main():
    d = Path(sys.argv[1])
    have = lambda rel: (d / rel).exists()  # noqa: E731
    out = ['<!doctype html><html lang="es"><head><meta charset="utf-8"><meta name="viewport" '
           'content="width=device-width,initial-scale=1"><title>Arcus, tocado como una persona'
           '</title><style>body{font:15px/1.5 system-ui,sans-serif;background:#111827;color:'
           '#e5e7eb;max-width:980px;margin:0 auto;padding:24px 16px}h1{font-size:24px}h2{font-'
           'size:17px;margin-top:28px;color:#93c5fd}.r{display:flex;gap:12px;align-items:center;'
           'flex-wrap:wrap;margin:6px 0;padding:8px 10px;background:#1f2937;border-radius:8px}'
           '.r span{flex:1 1 200px}.r b{display:inline-block;min-width:96px;color:#9ca3af;font-'
           'weight:500}audio{width:280px;max-width:100%}p,li{color:#9ca3af}.n{color:#fbbf24}'
           '</style></head><body>']
    out.append('<h1>Arcus tocado como una persona</h1>')
    out.append(open(Path(__file__).with_name("listen_intro.html"), encoding="utf-8").read())
    out.append('<h2>1. El cuarteto (Andante cantabile)</h2>')
    items = [(r, l) for r, l in (("pase6/arcus-cuarteto.wav", "Pase 6"),
                                 ("etapa1/arcus-cuarteto.wav", "Etapa 1"),
                                 ("etapa2/arcus-cuarteto.wav", "Etapa 2")) if have(r)]
    out.append(row("Cuarteto completo (2½ min)", items))
    out.append('<h2>2. Solos: la misma frase, sin interpretar y interpretada</h2>')
    out.append('<p>«Plano» son las notas con sus ligaduras y marcas, una sola velocidad y '
               'dinámica fija: lo que sale al escribir las notas y nada más. «Interpretado» es '
               'la misma partitura tocada por el intérprete del demo.</p>')
    for key, name in INST:
        items = [(r, l) for r, l in ((f"solos1/{key}-plano.wav", "Plano"),
                                     (f"solos1/{key}-interpretado.wav", "Interpretado (etapa 1)"),
                                     (f"solos2/{key}-interpretado.wav", "Interpretado (etapa 2)"))
                 if have(r)]
        if items:
            out.append(row(name, items))
    out.append('<h2>3. Copias para el chat (WAV 16 bits)</h2><ul>')
    chat = d / "chat"
    jobs = [("pase6/arcus-cuarteto.wav", "cuarteto-pase6.wav"),
            ("etapa1/arcus-cuarteto.wav", "cuarteto-etapa1.wav"),
            ("etapa2/arcus-cuarteto.wav", "cuarteto-etapa2.wav")]
    for key, _ in INST:
        jobs.append((f"solos1/{key}-plano.wav", f"{key}-plano.wav"))
        jobs.append((f"solos1/{key}-interpretado.wav", f"{key}-etapa1.wav"))
        jobs.append((f"solos2/{key}-interpretado.wav", f"{key}-etapa2.wav"))
    for src, name in jobs:
        if have(src):
            size, rate, ch = chat_copy(d / src, chat / name)
            out.append(f'<li><a href="chat/{name}">{name}</a> — {size / 1e6:.1f} MB, '
                       f'{rate} Hz, {"estéreo" if ch == 2 else "mono"}</li>')
            print(f"chat/{name}: {size / 1e6:.1f} MB {rate} Hz {ch} ch")
    out.append('</ul></body></html>')
    (d / "escuchar.html").write_text("\n".join(out), encoding="utf-8")
    print("wrote", d / "escuchar.html")


if __name__ == "__main__":
    main()
