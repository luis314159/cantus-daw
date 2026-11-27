"""The Paganini comparison's page (round 42): escuchar.html in Spanish, beside the WAVs —
the original, Arcus aligned (both versions, hall and dry), Arcus at the score's tempo, a
player that switches between them at the same moment, the short A/B, and the analysis
(`paganini_compare.py`'s charts and numbers) ending in a ranked list of what sounds unreal.

    python -I tools/listening/paganini_page.py <out dir> <render info>

<render info>: a short line naming the two binaries (written into the page as given).
"""
import html
import json
import os
import sys

CSS = """
:root{--bg:#f7f5f0;--fg:#1f2328;--mut:#57606a;--card:#fff;--line:#d8d4ca;--acc:#8a5a00;
--s1:#2a78d6;--s2:#eb6834;--s3:#1baf7a}
@media (prefers-color-scheme:dark){:root:not([data-theme="light"]){--bg:#111418;--fg:#e6e6e6;
--mut:#a0a8b0;--card:#1b1f24;--line:#30363d;--acc:#f0b44c;--s1:#3987e5;--s2:#d95926;--s3:#199e70}}
:root[data-theme="dark"]{--bg:#111418;--fg:#e6e6e6;--mut:#a0a8b0;--card:#1b1f24;--line:#30363d;
--acc:#f0b44c;--s1:#3987e5;--s2:#d95926;--s3:#199e70}
body{background:var(--bg);color:var(--fg);font:15px/1.55 system-ui,-apple-system,"Segoe UI",sans-serif;
margin:0 auto;max-width:1040px;padding:16px}
h1{font-size:24px;margin:8px 0}h2{font-size:20px;margin:28px 0 6px;border-bottom:1px solid var(--line);
padding-bottom:4px}h3{font-size:16px;margin:18px 0 4px}.mut{color:var(--mut);font-size:.9em}
table{border-collapse:collapse;width:100%;font-size:.88em;margin:.6em 0 1.2em;display:block;overflow-x:auto}
th,td{border-bottom:1px solid var(--line);padding:.3em .45em;text-align:left;vertical-align:top}
img{max-width:100%;height:auto;border-radius:4px;background:#fcfcfb}
.clips{display:flex;flex-wrap:wrap;gap:12px}.clip{background:var(--card);border:1px solid var(--line);
border-radius:8px;padding:8px 10px;flex:1 1 280px;min-width:0}.clip b{display:block;font-size:13px;color:var(--acc)}
audio{width:100%}ol li{margin-bottom:.7em}nav a{margin-right:12px;color:var(--acc)}
.sw{display:flex;flex-wrap:wrap;gap:8px;margin:8px 0}.sw button{font:inherit;padding:6px 12px;border-radius:6px;
border:1px solid var(--line);background:var(--card);color:var(--fg);cursor:pointer}
.sw button[aria-pressed="true"]{border-color:var(--acc);box-shadow:0 0 0 2px var(--acc) inset;font-weight:600}
.key{display:inline-block;width:10px;height:10px;border-radius:2px;margin-right:6px;vertical-align:middle}
"""

JS = """
(function(){
  const box=document.getElementById('sync');
  const auds=[...box.querySelectorAll('audio')];
  const btns=[...box.querySelectorAll('button[data-i]')];
  let cur=0;
  function pick(i){
    const a=auds[cur], b=auds[i], playing=!a.paused, t=a.currentTime;
    if(i!==cur){a.pause(); try{b.currentTime=Math.min(t,(b.duration||t))}catch(e){} }
    auds.forEach((x,k)=>x.hidden=k!==i);
    btns.forEach((x,k)=>x.setAttribute('aria-pressed',k===i?'true':'false'));
    cur=i; if(playing) b.play();
  }
  btns.forEach((b,i)=>b.addEventListener('click',()=>pick(i)));
  document.addEventListener('keydown',e=>{const n=parseInt(e.key,10); if(n>=1&&n<=btns.length&&!e.target.closest('input,textarea')) pick(n-1);});
  pick(0);
})();
"""


def clip(title, src, note=""):
    return (f'<div class="clip"><b>{html.escape(title)}</b><audio controls preload="none" '
            f'src="{src}"></audio>{f"<span class=mut>{note}</span>" if note else ""}</div>')


def f1(v):
    return f"{v:.1f}".replace(".", ",")


def main(out, info):
    full = json.load(open(os.path.join(out, "analisis", "analisis.json"), encoding="utf-8"))
    a = full["summary"]
    cr = full["crossings"]
    O, A, M = a["original"], a["arcus-actual-alineado-sala"], a["arcus-mejora-alineado-1cuerda-sala"]
    fc = O["ltas_fc"]

    def ltas_diff(s, lo, hi):
        d = [x - y for f, x, y in zip(fc, s["ltas_db"], O["ltas_db"]) if lo <= f <= hi]
        return f"{f1(min(d))} … {f1(max(d))}"

    def hnr(s, f):
        i = min(range(len(s["hnr_fc"])), key=lambda k: abs(s["hnr_fc"][k] - f))
        return s["hnr_med"][i]

    rows = [
        ("Afinación media (cents sobre A = 440)", O["tuning_cents"], A["tuning_cents"], M["tuning_cents"]),
        ("Desvío de afinación nota a nota (mediana, cents)", O["intonation_spread_cents"],
         A["intonation_spread_cents"], M["intonation_spread_cents"]),
        ("Inicio bajo el pico (dB, mediana)", O["start_under"][0], A["start_under"][0], M["start_under"][0]),
        ("Final bajo el pico (dB, todas las notas)", O["end_under"][0], A["end_under"][0], M["end_under"][0]),
        ("Final bajo el pico (dB, notas de 0,3 s o más)", O["end_under_long"][0], A["end_under_long"][0],
         M["end_under_long"][0]),
        ("Subida del ataque (ms, mediana)", O["rise_ms"][0], A["rise_ms"][0], M["rise_ms"][0]),
        ("Ráfaga 3–10 kHz del ataque (dB)", O["hf_burst"][0], A["hf_burst"][0], M["hf_burst"][0]),
        ("Hueco en el cambio de arco (dB)", O["dip_rebow"][0], A["dip_rebow"][0], M["dip_rebow"][0]),
        ("Hueco entre notas ligadas (dB)", O["dip_slur"][0], A["dip_slur"][0], M["dip_slur"][0]),
        ("Notas largas con vibrato ≥ 15 cents (%)", 100 * O["vib_share"], 100 * A["vib_share"],
         100 * M["vib_share"]),
        ("Armónicos sobre el ruido a 10 kHz (dB)", hnr(O, 10000), hnr(A, 10000), hnr(M, 10000)),
        ("Margen dinámico de las notas, p5–p95 (dB)", O["dyn_range_db"], A["dyn_range_db"], M["dyn_range_db"]),
    ]
    table = "".join(f"<tr><td>{html.escape(r[0])}</td>" + "".join(
        f"<td>{'—' if v is None else f1(v)}</td>" for v in r[1:]) + "</tr>" for r in rows)

    versions = [("original.wav", "Original"),
                ("arcus-actual-alineado-sala.wav", "Arcus actual"),
                ("arcus-mejora-alineado-1cuerda-sala.wav", "Arcus en mejora, una cuerda"),
                ("arcus-mejora-alineado-sala.wav", "Arcus en mejora, dos cuerdas (falla)"),
                ("arcus-actual-alineado-1cuerda-sala.wav", "Arcus actual, una cuerda"),
                ("antes-doble/arcus-actual-alineado-sala.wav", "Antes (transcripción)"),
                ("arcus-actual-alineado-seco.wav", "Arcus actual (seco)"),
                ("arcus-mejora-alineado-1cuerda-seco.wav", "Arcus en mejora, una cuerda (seco)")]

    # variation 2's crossings, bar by bar
    C = ["original", "arcus-actual-alineado-sala", "arcus-mejora-alineado-sala",
         "arcus-actual-alineado-1cuerda-sala"]
    names = ["A", "A#", "B", "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#"]

    def nm(k):
        return f"{names[(k - 9) % 12]}{k // 12 - 1}"

    cross_rows = ""
    for i, r0 in enumerate(cr["original"]):
        cells = "".join(
            f"<td>{f1(cr[c][i]['ring_db'])} / {f1(cr[c][i]['balance_db'])} / "
            f"{'—' if cr[c][i]['interval_cents'] is None else f1(cr[c][i]['interval_cents'])}</td>"
            for c in C)
        mark = "" if r0["marked"] else ", sin digitación"
        cross_rows += (f"<tr><td>{r0['bar']} (c. {r0['src_bar']}{mark})</td>"
                       f"<td>{nm(r0['hi'])} sobre {nm(r0['lo'])}</td>{cells}</tr>")
    X = a["cross"]
    CTRL = X["arcus-actual-alineado-1cuerda-sala"]
    sync = "".join(f'<button data-i="{i}" aria-pressed="false">{i + 1} · {html.escape(n)}</button>'
                   for i, (_, n) in enumerate(versions))
    sync_a = "".join(f'<audio controls preload="metadata" src="{f}"></audio>' for f, _ in versions)

    ab = [("tema-c1-2", "Tema, compases 1–2"), ("tema-c9-10", "Tema, compases 9–10 (segunda mitad)"),
          ("var2-c1-3", "Variación 2, compases 1–3")]
    ab_html = ""
    for tag, title in ab:
        ab_html += f"<h3>{title}</h3><div class=clips>"
        ab_html += clip("Original", f"ab-{tag}-original.wav")
        ab_html += clip("Arcus actual", f"ab-{tag}-arcus-actual.wav")
        ab_html += clip("Arcus en mejora", f"ab-{tag}-arcus-mejora.wav")
        ab_html += clip("Original y luego Arcus actual", f"ab-{tag}-actual.wav")
        ab_html += clip("Original y luego Arcus en mejora", f"ab-{tag}-mejora.wav")
        ab_html += "</div>"

    page = f"""<!doctype html><html lang="es"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Paganini en Arcus</title><style>{CSS}</style></head><body>
<h1>Paganini en Arcus: el Capricho n.º 24 frente a una grabación real</h1>
<p class="mut">Ronda 42 · {html.escape(info)}</p>
<nav><a href="#escuchar">Escuchar</a><a href="#ab">A/B corto</a><a href="#analisis">Análisis</a>
<a href="#irreal">Qué suena irreal</a><a href="#tecnicas">Lo que Arcus no toca aún</a><a href="#como">Cómo se hizo</a></nav>

<h2>Qué se compara</h2>
<p><b>La obra:</b> Niccolò Paganini, Capricho op. 1 n.º 24 en la menor (dominio público). <b>El pasaje</b>
(35 s): el <b>tema</b> completo con la repetición de su primera mitad, tal como lo toca el intérprete
(0,3–17,4 s de la toma), y la <b>Variación 2</b> completa (35,9–53,9 s), unidos en el silencio que
hay entre las variaciones. La Variación 1 queda fuera: son arpegios que saltan por tres octavas con el
arco rebotando entre cuerdas, y Arcus no tiene ese golpe (ver <a href="#tecnicas">abajo</a>).</p>
<p><b>La grabación:</b> «Paganini – 24th Caprice», grabada en el Performance Lab de Queen Mary
University of London (2014), publicada en Internet Archive (<code>Paganini_Caprice24</code>) con la
<b>Public Domain Mark 1.0</b>. Toma 03, el par de micrófonos C414 (izquierdo y derecho), 96 kHz → 48 kHz.
El intérprete no está identificado en la ficha. Crédito: Queen Mary University of London, Performance Lab.</p>
<p><b>Las notas:</b> el tema viene del MIDI de dominio público de Wikimedia Commons
(«Paganini caprice24 theme.mid», Hyacinth, 2018), alineado con la grabación. La Variación 2 sale ahora
de la <b>partitura de dominio público</b> de Wikimedia Commons («Caprice-24-a4.pdf», grabado por David Chan
para el proyecto Mutopia, PD-old). Sus notas, adornos, ligaduras y digitaciones se pasaron a mano y se
alinearon igual que el tema.</p>
<p class="mut"><b>Corrección (26-11):</b> la primera versión transcribía la Variación 2 con un detector
de una sola voz, que se quedaba con una cuerda. En la partitura, el primer tiempo de cada compás alterna
dos cuerdas: una cuerda al aire (digitación 0) o una nota pisada (1) con su vecina en la cuerda de abajo
(4), así que la de arriba sigue sonando bajo la de abajo. Ahora Arcus la toca con un arco sobre dos
cuerdas (<code>bow.pair</code>, ADR 0164). La toma anterior se guarda en <code>antes-doble/</code>.</p>
<p>Todos los archivos están a <b>−23 LUFS</b>, en WAV de 16 bits a 48 kHz. Las versiones «sala» llevan la
sala pequeña de Cantus (Aula, respuesta «chamber», 1,3 s, cortada bajo 160 Hz y sobre 8 kHz, a 9 dB
bajo la señal seca), para acercarse a la sala de la grabación.</p>

<h2 id="escuchar">Escuchar</h2>
<p>Este reproductor cambia de versión <b>en el mismo instante</b>. Pulsa los botones o las teclas 1–8
mientras suena.</p>
<div id="sync" class="clip"><div class="sw">{sync}</div>{sync_a}</div>
<h3>Cada versión por separado</h3>
<div class=clips>
{clip("Original (35 s)", "original.wav")}
{clip("Arcus actual, alineado, sala", "arcus-actual-alineado-sala.wav", "El tempo, el rubato y la dinámica de la grabación.")}
{clip("Arcus en mejora, alineado, sala (una cuerda)", "arcus-mejora-alineado-1cuerda-sala.wav", "La rama del sonido clásico en curso, con su intérprete clásico entero.")}
{clip("Arcus en mejora, alineado, sala, dos cuerdas", "arcus-mejora-alineado-sala.wav", "Con bow.pair y el intérprete clásico entero (pasada 2: ya no se calla ninguna nota).")}
{clip("Arcus actual, alineado, seco", "arcus-actual-alineado-seco.wav")}
{clip("Arcus en mejora, alineado, seco (una cuerda)", "arcus-mejora-alineado-1cuerda-seco.wav")}
{clip("Control: Arcus actual, Variación 2 en una sola cuerda", "arcus-actual-alineado-1cuerda-sala.wav", "Las mismas notas sin bow.pair.")}
{clip("Antes: Arcus actual con la Variación 2 transcrita", "antes-doble/arcus-actual-alineado-sala.wav", "La toma anterior (antes-doble).")}
{clip("Arcus actual, al tempo de la partitura, sala", "arcus-actual-partitura-sala.wav", "A 150 negras por minuto (el MIDI), mf fijo, sin rubato.")}
{clip("Arcus en mejora, al tempo de la partitura, sala", "arcus-mejora-partitura-sala.wav")}
{clip("Arcus actual, al tempo de la partitura, seco", "arcus-actual-partitura-seco.wav")}
{clip("Arcus en mejora, al tempo de la partitura, seco", "arcus-mejora-partitura-seco.wav")}
</div>

<h2 id="ab">A/B corto (2–3 compases)</h2>
<p>Primero el original, luego los mismos compases en Arcus (alineado, con sala). Los archivos «y luego»
tocan los dos seguidos con 0,4 s de silencio entre ellos.</p>
{ab_html}

<h2 id="analisis">Análisis</h2>
<p>Medido nota a nota sobre las mismas notas en los tres archivos: el original y Arcus alineado, con sala,
ambas versiones. Se usa el micrófono izquierdo, porque sumar un par separado produce filtro de peine.
<span class="key" style="background:var(--s1)"></span>original ·
<span class="key" style="background:var(--s2)"></span>Arcus actual ·
<span class="key" style="background:var(--s3)"></span>Arcus en mejora.</p>
<table><tr><th>Medida</th><th>Original</th><th>Arcus actual</th><th>Arcus en mejora (una cuerda)</th></tr>{table}</table>

<h3>Espectro de largo plazo</h3>
<img src="analisis/espectro.png" alt="Espectro en tercios de octava del original y de Arcus, y su diferencia">
<p>Entre 250 Hz y 8 kHz, Arcus queda a pocos dB del original. Faltan
<b>3–7 dB en 2–3,2 kHz</b> (presencia y brillo del puente) y sobran o faltan 5–7 dB alternos en
800 Hz – 1,3 kHz: las resonancias del cuerpo caen en otro sitio. Arriba faltan <b>11–13 dB en
10–12,7 kHz</b>: el aire y la crin. Bajo 150 Hz el original lleva el ruido de la sala, que no es del
violín. El pico de +14 dB en la banda de 200 Hz es sobre todo la fundamental (220 Hz) del La3 final. La nota
entera suena sólo 4–6 dB más fuerte que en la grabación, pero su fundamental unos 16 dB más: un violín
real radia poco bajo su resonancia de aire (≈ 270 Hz), y el cuerpo de Arcus no. También hay un golpe
ancho de 150–400 Hz bajo las notas agudas fuertes (8,6–9,0 s), 15–20 dB sobre la grabación.</p>

<h3>Ruido entre armónicos</h3>
<img src="analisis/ruido.png" alt="Relación armónicos/ruido por tercio de octava">
<p>De 1 a 4 kHz los dos están parecidos. Sobre 6 kHz <b>Arcus es más limpio</b>: a 10 kHz sus armónicos
están {f1(hnr(A, 10000))}–{f1(hnr(M, 10000))} dB sobre el ruido entre ellos, contra {f1(hnr(O, 10000))} dB en la
grabación. Parte de ese ruido es sala y micrófono, pero cuadra con la falta de 10–13 kHz del espectro:
en un violín real la crin rasca y se oye arriba.</p>

<h3>Afinación y vibrato a lo largo del pasaje</h3>
<img src="analisis/afinacion.png" alt="Desvío en cents respecto a la nota temperada a lo largo del pasaje">
<img src="analisis/vibrato.png" alt="Profundidad del vibrato por nota larga">
<p>El intérprete afina unos {f1(O["tuning_cents"])} cents alto (La ≈ 442 Hz) y su desvío nota a nota es de
{f1(O["intonation_spread_cents"])} cents. Arcus afina más exacto de lo que toca un violinista:
{f1(A["intonation_spread_cents"])} cents la versión actual y {f1(M["intonation_spread_cents"])} la versión en
mejora. En este pasaje rápido el intérprete casi no usa vibrato:
sólo un {f1(100 * O["vib_share"])} % de las notas largas pasa de 15 cents, a unos {f1(O["vib_hz"][0])} Hz. En Arcus
es un {f1(100 * A["vib_share"])} %; la versión en mejora vibra más lento ({f1(M["vib_hz"][0])} Hz), aunque sobre
pocas notas.</p>

<h3>Envolventes: cómo empieza y termina cada nota bajo su pico</h3>
<img src="analisis/envolventes.png" alt="Diagramas de caja: inicio y final de cada nota bajo su pico">
<p>Las notas del intérprete empiezan {f1(O["start_under"][0])} dB bajo su pico y terminan
{f1(O["end_under"][0])} dB bajo él: cada golpe de arco tiene forma. En Arcus <b>empiezan más bajo</b>
({f1(A["start_under"][0])} / {f1(M["start_under"][0])} dB) y <b>terminan más arriba</b>
({f1(A["end_under"][0])} / {f1(M["end_under"][0])} dB): la nota llega tarde a su nivel y luego no lo suelta.
En las notas de 0,3 s o más, la versión actual se parece al intérprete
({f1(A["end_under_long"][0])} contra {f1(O["end_under_long"][0])} dB). La versión en mejora las termina más llenas
({f1(M["end_under_long"][0])} dB), así que en este pasaje va en la dirección contraria.</p>

<h3>Ataques</h3>
<img src="analisis/ataques.png" alt="Diagramas de caja: subida del ataque y ráfaga de agudos">
<p>El intérprete llega a 3 dB de su pico en {f1(O["rise_ms"][0])} ms (mediana). Arcus tarda
{f1(A["rise_ms"][0])} / {f1(M["rise_ms"][0])} ms y trae menos ráfaga de agudos en el golpe
({f1(A["hf_burst"][0])} / {f1(M["hf_burst"][0])} dB, contra {f1(O["hf_burst"][0])} dB). En un tema quasi presto
eso se oye como un arco blando, sin el «mordisco» de cada nota.</p>

<h3>Cambios de arco y ligaduras</h3>
<img src="analisis/cambios-de-arco.png" alt="Diagramas de caja: hueco de nivel en cambios de arco y en ligaduras">
<p>El hueco de nivel en un cambio de arco coincide: {f1(O["dip_rebow"][0])} dB en el original,
{f1(A["dip_rebow"][0])} / {f1(M["dip_rebow"][0])} dB en Arcus. Entre notas ligadas Arcus marca un poco más
({f1(A["dip_slur"][0])} / {f1(M["dip_slur"][0])} contra {f1(O["dip_slur"][0])} dB), así que la ligadura suena
algo re-articulada. Ojo: qué notas van ligadas se dedujo del nivel de la grabación, no de una partitura.</p>

<h3 id="cuerdas">Las dos cuerdas de la Variación 2</h3>
<p>Para cada compás de la Variación 2 (16, con la repetición) se mide su primer tiempo, que alterna una
nota de arriba con su vecina de abajo. Tres cosas: cuánto cae la cuerda de arriba mientras suena la de
abajo (si son dos cuerdas sigue sonando y cae poco), el equilibrio entre las dos en ese momento y la
afinación del intervalo frente al temperado. La sala de la grabación también sostiene la nota de
arriba, así que el control es Arcus con las mismas notas en una sola cuerda y la misma sala.</p>
<img src="analisis/cuerdas.png" alt="Variación 2 compás a compás: caída de la cuerda de arriba, equilibrio e intervalo">
<p>Medianas de los 16 compases. La cuerda de arriba cae {f1(X["original"]["ring_db"])} dB en el
original, {f1(X["arcus-actual-alineado-sala"]["ring_db"])} en Arcus actual y
{f1(X["arcus-mejora-alineado-sala"]["ring_db"])} en la versión en mejora, contra
{f1(CTRL["ring_db"])} dB con una sola cuerda. Equilibrio (arriba menos abajo):
{f1(X["original"]["balance_db"])} dB en el original;
{f1(X["arcus-actual-alineado-sala"]["balance_db"])} y
{f1(X["arcus-mejora-alineado-sala"]["balance_db"])} dB en Arcus;
{f1(CTRL["balance_db"])} dB con una cuerda. Intervalo:
{f1(X["original"]["interval_ct"])} cents en el original (variación
{f1(X["original"]["interval_spread_ct"])}),
{f1(X["arcus-actual-alineado-sala"]["interval_ct"])} /
{f1(X["arcus-mejora-alineado-sala"]["interval_ct"])} cents en Arcus.</p>
<table><tr><th>Compás</th><th>Notas</th><th>Original</th><th>Arcus actual (dos cuerdas)</th><th>Arcus en mejora (dos cuerdas)</th>
<th>Una cuerda (control)</th></tr>{cross_rows}</table>
<p class="mut">Cada celda: caída de la cuerda de arriba (dB) / equilibrio (dB) / intervalo (cents). La
partitura marca dos cuerdas en todos los compases menos el 6 y el 8 (compases 10 y 12 de la
interpretación), que no llevan digitación. Las medidas del original dicen si el intérprete también los
cruza.</p>
<p><b>La versión en mejora y bow.pair (pasada 2).</b> En la primera toma de la rama, <code>bow.pair</code>
apagaba el intérprete clásico y encendía el del fiddle, y la Variación 2 con dos cuerdas se callaba en 9 de
los 16 compases (notas ligadas hasta −90 dB). Ahora el violinista de fiddle es un control propio
(<code>player.fiddle</code>), las dos cuerdas guardan el intérprete clásico, y la segunda cuerda de una
doble cuerda lleva su propia afinación (la tendencia del semitono incluida) y la conserva cuando queda
sonando al cruzar. En esta toma <b>ninguna</b> de las 137 notas de la Variación 2 cae más de 30 dB bajo
la mediana, con dos cuerdas ni con una. En el análisis general y en el A/B, «en mejora» sigue siendo la
toma de una cuerda, para comparar lo mismo que antes.</p>

<h3>La dinámica nota a nota</h3>
<img src="analisis/dinamica.png" alt="Pico de cada nota en Arcus frente al original">
<p>Arcus sigue la dinámica de la grabación con r = {f1(A["dyn_r"])} / {f1(M["dyn_r"])}. Las notas suaves
quedan algo altas y las fuertes algo bajas (pendiente {f1(A["dyn_slope"])} / {f1(M["dyn_slope"])}), aunque el margen p5–p95 es de {f1(A["dyn_range_db"])} dB
contra {f1(O["dyn_range_db"])} dB. Parte de esto es mía: la curva CC1 sale del nivel de la grabación con
una correspondencia lineal sencilla.</p>

<h2 id="irreal">Qué suena irreal, por orden</h2>
<ol>
<li><b>El ataque es blando y lento.</b> Arcus tarda {f1(A["rise_ms"][0])} ms en llegar al pico, contra
{f1(O["rise_ms"][0])} ms; empieza unos 3 dB más bajo y casi sin ráfaga de agudos. En un tema de notas cortas
y separadas es lo primero que delata la máquina: falta el «clic» de la crin al agarrar la cuerda.</li>
<li><b>Las notas no se apagan al final del golpe.</b> Terminan solo {f1(A["end_under"][0])} /
{f1(M["end_under"][0])} dB bajo su pico, contra {f1(O["end_under"][0])} dB. Las notas cortas suenan unidas y como de
órgano en vez de habladas. La versión en mejora (pasada 2) ya sabe cuánto dura cada nota y la apaga antes
de la siguiente; la diferencia que queda está en las notas cortas.</li>
<li><b>Falta brillo y aire.</b> Hay −3 a −7 dB en 2–3,2 kHz y −11 a −13 dB en 10–12,7 kHz, y el ruido
entre armónicos es 4–7 dB más limpio sobre 6 kHz. El violín real rasca un poco arriba y Arcus suena
pulido y algo apagado. <i>Pasada 2 (en mejora): −3,7 dB a 2 kHz y −7,4 / −6,0 dB a 10 / 12,7 kHz,
y más ruido de crin entre armónicos.</i></li>
<li><b>Graves que el violín no radia.</b> Hay un golpe ancho de 150–400 Hz bajo las notas agudas
fuertes, unos 15–20 dB sobre la grabación al empezar la segunda mitad del tema (La5). Y la fundamental
del La3 grave sale unos 16 dB más fuerte. Suena a caja más grande o a «bombeo», no a violín.
<i>Pasada 2 (en mejora): el violín radia bajo su modo de aire como un bass-reflex (cae 24 dB por octava
bajo 260 Hz); el tercio de 200 Hz queda a −2,3 dB de la grabación (era +15,7).</i></li>
<li><b>La afinación es demasiado exacta</b>: {f1(A["intonation_spread_cents"])} /
{f1(M["intonation_spread_cents"])} cents de desvío nota a nota, contra {f1(O["intonation_spread_cents"])}. En las
dos cuerdas de la Variación 2, el intérprete estrecha el semitono (la sensible alta):
{f1(X["original"]["interval_ct"])} cents frente al temperado. Arcus lo deja temperado
({f1(X["arcus-actual-alineado-sala"]["interval_ct"])}), y su control <code>dstop.just</code> lo movería
hacia 16/15, que es más ancho, al revés que el intérprete. Además el intérprete afina a La ≈ 442: en el
A/B, esos 8–10 cents se oyen como otro color, no como error.</li>
<li><b>La dinámica nota a nota se sigue sólo a medias</b> (r = {f1(A["dyn_r"])}, pendiente
{f1(A["dyn_slope"])}): los acentos del intérprete no salen tan marcados. Puede ser en parte mi mapeo
de CC1.</li>
<li><b>Las dos cuerdas suenan demasiado parejas.</b> Con <code>bow.pair</code>, la cuerda de arriba de la
Variación 2 suena por fin bajo la de abajo. El equilibrio pasa de {f1(CTRL["balance_db"])} dB con una
cuerda a {f1(X["arcus-actual-alineado-sala"]["balance_db"])} dB, contra {f1(X["original"]["balance_db"])} dB en el
original. Pero cae sólo {f1(X["arcus-actual-alineado-sala"]["ring_db"])} dB mientras suena la otra, contra
{f1(X["original"]["ring_db"])} dB en el original: Arcus mantiene la cuerda libre casi tan fuerte como si
siguiera bajo el arco (ver <a href="#cuerdas">las dos cuerdas</a>).</li>
<li><b>Detalles menores:</b> el hueco entre notas ligadas ({f1(A["dip_slur"][0])} / {f1(M["dip_slur"][0])} contra
{f1(O["dip_slur"][0])} dB) y el vibrato de la versión en mejora, más lento ({f1(M["vib_hz"][0])} Hz contra
{f1(O["vib_hz"][0])} Hz, sobre pocas notas).</li>
</ol>
<p class="mut">Esta lista sale de las medidas, no de una escucha: el orden es el que espero que oiga el
oído, y queda para que lo confirmes con el reproductor de arriba.</p>

<h2 id="tecnicas">Lo que Arcus no puede tocar todavía</h2>
<ul>
<li><b>Ricochet y spiccato saltado entre cuerdas</b> (Variación 1: arpegios de tres octavas, el arco
rebotando de cuerda en cuerda). Arcus tiene staccato y spiccato por nota, pero no el rebote de varias
notas en un solo lanzamiento ni el cruce rápido de cuerdas como un solo gesto.</li>
<li><b>Pizzicato de mano izquierda</b> mezclado con notas con arco (Variación 9). Arcus puntea con
la técnica de pizzicato, pero no a la vez que el arco suena.</li>
<li><b>Octavas y décimas en dobles cuerdas a velocidad</b> (Variaciones 3 y 6). El arco sobre dos
cuerdas existe (`bow.pair`, la doble cuerda del fiddle), pero no está probado en un pasaje clásico de
octavas ligadas.</li>
<li><b>Armónicos naturales y artificiales</b>, que Paganini usa en otros caprichos: Arcus no los tiene.</li>
<li><b>Glissandos expresivos y adornos</b> (trinos, mordentes) como gesto propio. Aquí van como notas
sueltas, así que su forma es la de cualquier ligadura.</li>
</ul>

<h2 id="como">Cómo se hizo</h2>
<ul>
<li><b>Alineación:</b> DTW con pasos (1,1), (1,2) y (2,1) entre la altura de la partitura y el
seguimiento YIN de la grabación, con un coste por inicio de nota (flujo espectral), anclado en la
primera nota. Después cada nota se lleva al inicio detectado más cercano con la misma altura, y se
corrige cuando eso la deja con menos de la mitad de su valor.</li>
<li><b>Articulación, arco y dinámica:</b> salen del nivel de la grabación. Una nota termina donde cae
9 dB bajo su pico. Con 30 ms o más de silencio el golpe es separado (staccato si dura menos de
0,16 s). En la Variación 2 los arcos son los de la partitura: cada compás, una ligadura. El CC1 sigue el nivel
suavizado (80 ms) y la velocidad sale del pico de cada nota. El vibrato es el automático de Arcus.</li>
<li><b>Arcus:</b> el violín con el preset «Violín cantabile» tal como lo lee el motor
(bow.pos 1,09, vibrato 1,2× desde 160 ms, bow.attack 1,4, dyn 0,65, body.ring 1,15), con el intérprete
(player) activo, en modo legato. Se renderiza fuera de línea con el ejemplo
<code>paganini_render</code> (cantus-dsp), sin tocar el DSP.</li>
<li><b>Versión al tempo de la partitura:</b> las mismas notas y articulaciones a 150 negras por minuto
(el tempo del MIDI), con el CC1 fijo en 0,6.</li>
<li><b>Las dos cuerdas:</b> el tema y la Variación 2 se renderizan aparte y se suman. La Variación 2 lleva
<code>set pair 1</code>, y cada nota del primer tiempo, su cuerda según la digitación de la partitura:
la de arriba en la cuerda aguda (La al aire = 2, Mi al aire o pisada = 3) y su vecina en la de abajo.
Los adornos (apoyaturas breves) van 50 ms antes de su nota, dentro de la ligadura.</li>
<li>Scripts: <code>tools/listening/paganini_notes.py</code>, <code>paganini_mix.py</code>,
<code>paganini_compare.py</code> y <code>paganini_page.py</code> (rama r42/paganini).</li>
</ul>
<script>{JS}</script>
</body></html>"""
    open(os.path.join(out, "escuchar.html"), "w", encoding="utf-8").write(page)
    print("written", os.path.join(out, "escuchar.html"))


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2] if len(sys.argv) > 2 else "")
