"""listen_page.py - r42 fiddle: the owner's listening page (Spanish) and the chat copies.

    python -I tools/arcus/fiddle/listen_page.py <listen-dir>

`<listen-dir>` (default $CANTUS_WORK/listen/fiddle) holds, each part optional:

- `lab/<frase>-{antes,ahora}.wav`   the lab's phrases (`cargo run -p cantus-dsp --release
  --example fiddle_lab -- <dir>/lab`): dobles, rapidas, bordon, simpatia
- `antes/<demo>.wav`                the demos as round 42's arcus-expr left them (copies)
- `ahora/<demo>.wav`                the demos re-rendered (`arcus_demo -- <dir>/ahora fiddle`)
- `comparar/<clip>.wav`             reference-style comparison clips (the owner's ear only)

It writes `escuchar.html` beside them and `chat/` - 16-bit WAV copies of every clip, each
under 30 MB (44.1 kHz when 48 kHz would not fit, then mono).
"""
import html
import sys
from pathlib import Path

import numpy as np
import soundfile as sf
from scipy.signal import resample_poly

LIMIT = 30 * 1000 * 1000

LAB = [
    ("dobles", "Dobles cuerdas", "Un lick de bluegrass en La: terceras y sextas contra la Mi y "
     "la La al aire. Antes: dos violines (la nota de abajo en un segundo Arcus). Ahora: un solo "
     "violín, un arco sobre dos cuerdas (la misma velocidad, el mismo ruido de crin, los mismos "
     "cambios de arco al sample), las cuerdas al aire vibrando por simpatía."),
    ("rapidas", "Notas rápidas", "Una carrera a 160 BPM que cruza cuerdas (dos ligadas, dos "
     "sueltas). Ahora la cuerda que el arco deja sigue sonando bajo la siguiente (medido: la nota "
     "anterior 60 ms después, -29 dB antes, -12 dB ahora; los violines reales -10 a -16 dB), el "
     "grano del arco y el cambio de arco sin levantar."),
    ("bordon", "Bordón", "Una melodía en la cuerda Mi sobre la La al aire, re-arqueadas juntas: "
     "un violín, no dos."),
    ("simpatia", "Cuerdas simpáticas", "Notas sueltas que una cuerda al aire contesta (Re5, La5, "
     "Sol4, Mi5), soltadas: el anillo que queda después del arco."),
]
DEMOS = [
    ("hoedown", "Hoedown en Re", "La melodía con la banda (piano honky-tonk, contrabajo, batería)."),
    ("tren", "Tren de la medianoche", "El breakdown a 160 BPM: dobles cuerdas, el silbato del "
     "tren, los dos violines (la armonía sí es un segundo violinista)."),
    ("pasaje-country", "Pasaje country, violín solo", "La parte B, el violín solo."),
    ("pasaje-clasico", "Pasaje clásico, violín solo", "Lo mismo tocado clásico, para comparar."),
]


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
            return
    raise SystemExit(f"{src}: too long for {LIMIT} bytes")


def player(rel, label):
    return (f'<div class="p"><b>{html.escape(label)}</b><audio controls preload="none" '
            f'src="{html.escape(rel)}"></audio></div>')


def section(title, text, items):
    cells = "".join(player(r, lab) for r, lab in items)
    return (f'<section><h2>{html.escape(title)}</h2><p>{html.escape(text)}</p>'
            f'<div class="row">{cells}</div></section>\n')


def main():
    if len(sys.argv) > 1:
        d = Path(sys.argv[1])
    else:
        sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "sound-refs"))
        import cantus_paths as cp
        d = Path(cp.work("listen", "fiddle"))
    body = []
    clips = []
    for stem, title, text in LAB:
        items = [(f"lab/{stem}-{v}.wav", lab) for v, lab in (("antes", "Antes"), ("ahora", "Ahora"))
                 if (d / f"lab/{stem}-{v}.wav").exists()]
        if items:
            body.append(section(title, text, items))
            clips += [r for r, _ in items]
    for stem, title, text in DEMOS:
        items = [(f"{v}/{stem}.wav", lab) for v, lab in (("antes", "Antes"), ("ahora", "Ahora"))
                 if (d / f"{v}/{stem}.wav").exists()]
        if items:
            body.append(section(title, text, items))
            clips += [r for r, _ in items]
    if (d / "ahora-aeae/tren.wav").exists():
        body.append(section("Tren de la medianoche, afinación AEAE",
                            "El mismo tema con el violín afinado La Mi La Mi, como afinan los "
                            "violinistas old-time para un tema en La: las cuerdas al aire La y "
                            "Mi suenan bajo todo el tema.", [("ahora-aeae/tren.wav", "AEAE")]))
        clips.append("ahora-aeae/tren.wav")
    cmp_dir = d / "comparar"
    if cmp_dir.exists():
        items = [(f"comparar/{p.name}", p.stem) for p in sorted(cmp_dir.glob("*.wav"))]
        if items:
            body.append(section("Comparación con una grabación de referencia",
                                "Solo para tus oídos, nunca se distribuye: la misma frase en el "
                                "mismo estilo, nuestro violín junto a la grabación medida.", items))
            clips += [r for r, _ in items]
    for rel in clips:
        chat_copy(d / rel, d / "chat" / rel.replace("/", "-"))
    page = ('<!doctype html><html lang="es"><head><meta charset="utf-8"><meta name="viewport" '
            'content="width=device-width,initial-scale=1"><title>El violín country</title>'
            '<style>body{font:15px/1.5 system-ui,sans-serif;background:#111827;color:#e5e7eb;'
            'margin:0 auto;max-width:980px;padding:16px}h1{font-size:22px}h2{font-size:17px;'
            'margin:22px 0 4px}p{color:#cbd5e1;margin:4px 0 8px}.row{display:flex;flex-wrap:wrap;'
            'gap:12px}.p{background:#1f2937;border-radius:8px;padding:8px 10px}.p b{display:block;'
            'font-size:13px;color:#fbbf24}audio{width:300px;max-width:100%}</style></head><body>'
            '<h1>El violín country: un arco, dos cuerdas</h1><p>Antes y ahora. Las copias de 16 '
            'bits para el chat están en <code>chat\\</code>.</p>' + "".join(body) +
            '</body></html>')
    (d / "escuchar.html").write_text(page, encoding="utf-8")
    print(d / "escuchar.html", len(clips), "clips")


if __name__ == "__main__":
    main()
