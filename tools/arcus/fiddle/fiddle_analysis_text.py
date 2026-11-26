"""fiddle_analysis_text.py - r42: the Spanish sentences of the owner's analysis pages
(`fiddle_analysis_page.py`), written from the measured numbers in `parte1.json` and
`clasico/parte2.json` (`docs/round42/fiddle.md` §6). Data only."""

C = "../fiddle/chat/"

INDEX_INTRO = (
    "<p>Pediste un análisis completo comparando las grabaciones con lo que genera Arcus. "
    "Aquí está, en dos partes: el violín country (antes y después de quitar el raspado) y el "
    "modo clásico (violín, viola, violonchelo y contrabajo). Cada rasgo dice si caemos dentro "
    "del rango de las grabaciones y, si no, por cuánto. Cada parte termina con una lista "
    "ordenada: lo que suena irreal, la causa medida y el arreglo propuesto. Junto a cada "
    "gráfica están los audios para comparar de oído.</p>"
    '<p class="mut">Las grabaciones de referencia son sólo para medir y escuchar aquí; nunca '
    "se distribuyen. Las de violín country son de TheFlyFishingFilmmaker en Freesound (CC BY "
    "4.0) y otras con licencia libre (docs/round42/fiddle.md §1.1).</p>")

P1_INTRO = (
    "<p><b>Qué se compara.</b> Las grabaciones: 12 clips de bluegrass y blues de "
    "TheFlyFishingFilmmaker (CC BY 4.0, vistas previas MP3, con sala), tres temas de "
    "Wikimedia y Freesound, y como referencia seca el violín clásico (las escalas del CNSM, "
    "los primeros violines anecoicos de PHENICX). Lo generado: el violín de <i>Tren de la "
    "medianoche</i> y de <i>Hoedown en Re</i> (su pista sola, seca), antes del arreglo "
    "(«antes», el raspado que te molestó) y ahora (la mordida según cada nota), más las "
    "frases sueltas del laboratorio para el ruido (el ruido entre armónicos se infla en las "
    "dobles cuerdas, así que se mide en líneas de una sola nota).</p>"
    '<p class="mut">El rango de un rasgo es la unión de los cuartiles (p25–p75) de los grupos '
    "de grabaciones que aplican; las salas alargan las colas y suben el ruido de las "
    "referencias con sala, por eso el ruido y el solape se comparan también con las secas.</p>")

P1_CLIPS = [
    (C + "tren-raspado-antes.wav", "Tren — antes (raspado)"),
    (C + "tren-mordida.wav", "Tren — ahora (mordida según cada nota)"),
    (C + "antes-raspado-hoedown.wav", "Hoedown — antes (raspado)"),
    (C + "ahora-hoedown.wav", "Hoedown — ahora"),
    (C + "crescendo-mordida.wav", "Crescendo de p a f: la mordida crece"),
    (C + "comparar-hornpipe-ref.wav", "Grabación: hornpipe (TheFlyFishingFilmmaker, CC BY 4.0)"),
    (C + "comparar-hornpipe-cantus.wav", "Cantus tocando las notas de esa grabación"),
]

P1_SPECTRUM = (
    "<p>El brillo quedó donde lo querías: centroide / f0 2,18 (grabaciones 1,95–3,31), la "
    "energía sobre 2 kHz −7,5 dB (−13,6 … −4,6) y la «mordida» de 1,6–3,2 kHz dentro en cada "
    "tercio de octava. Lo que sobraba de agudos era el ruido del arco: 5 kHz bajó de −18,0 a "
    "−20,9 dB y ya está dentro. Lo que falta está abajo: 250–315 Hz quedan 3–4 dB por debajo. "
    "En parte es el repertorio (Tren y el Hoedown casi no bajan a la cuerda Sol; las "
    "grabaciones sí) y en parte el modo de aire del cuerpo (~280 Hz) con el micrófono "
    "cercano; falta separar las dos causas con una frase en las cuerdas Sol y Re.</p>")

P1_NOISE = (
    "<p>El raspado era esto. Antes, el ruido entre armónicos estaba 5–11 dB por encima de las "
    "grabaciones en las tres bandas, y la cuerda perdía su periodo en el 40 % de los "
    "instantes (las grabaciones: 2–15 %): el ruido de la crin, +4 dB fijos más el grano cerca "
    "del puente, sacaba a la cuerda de su vibración limpia. Ahora las tres bandas, los "
    "ataques y la periodicidad caen dentro. Con la mordida, cada nota recibe su parte del "
    "grano según su dinámica: en el crescendo de prueba el ruido crece 0, +1,2, +2,3 y +2,6 dB "
    "de piano a forte (contra la mordida en 0).</p>")

P1_NOTES = (
    '<p class="mut">El ruido y la periodicidad se miden en las frases sueltas del laboratorio '
    "(una sola nota a la vez); el resto, en el violín de Tren y del Hoedown. «Antes» del "
    "solape (−5 dB) estaba inflado por el mismo ruido, que caía en las frecuencias de la nota "
    "anterior; ahora (−20 dB) es el anillo real de la cuerda que el arco deja. La cola de "
    "0,16 s es la pista seca; las grabaciones tienen sala.</p>")

P1_RANKED = [
    ("Las notas rápidas sueltas se oyen re-articuladas.",
     "Medido: el 49 % de las notas rápidas trae una señal de cambio de arco, contra el "
     "11–20 % de las grabaciones. Cada cambio da una ráfaga de 17 dB y 55 ms, como la de un "
     "violín seco; los violinistas de country ligan la mayoría de las notas rápidas "
     "(los patrones de arco «shuffle») y sus cambios a esa velocidad son más suaves.",
     "en el intérprete, ligar 2–3 notas por arco en los pasajes rápidos (shuffle de "
     "Nashville y de Georgia) y suavizar el cambio cuando la nota dura menos de 0,2 s."),
    ("El grave de 250–315 Hz queda flojo.",
     "Medido: 3–4 dB por debajo de las grabaciones. En parte es el repertorio y en parte el "
     "modo de aire del cuerpo con el micrófono cercano.",
     "medir una frase en las cuerdas Sol y Re y, si sigue faltando, dar al micrófono "
     "cercano el modo de aire (A0) medido."),
    ("Casi no hay vibrato.",
     "Medido: el 2 % de las notas largas, contra el 4–30 % de los violinistas "
     "(unos 54 cents de pico a pico, ~6 Hz). El preset lo tiene en 0,35 con 450 ms de espera.",
     "vibrato 0,6 con 300 ms de espera en las notas largas del preset country."),
    ("La nota que el arco deja suena poco bajo la siguiente.",
     "Medido: −20 dB a los 60 ms, dentro del rango pero en su borde bajo (los violines "
     "secos −10 a −16 dB).",
     "alargar el anillo de la cuerda que el arco deja (la caída de la cuerda libre) unas "
     "dos veces."),
    ("Las dobles cuerdas son un poco quietas.",
     "Medido: el intervalo oscila 3,2 cents, justo bajo el rango (3,3–9,6).",
     "el balanceo del arco (pendiente, docs/round42/fiddle.md §5): la inclinación como una "
     "curva que se mueve con el arco."),
    ("La cola al parar es corta.",
     "Medido: 0,16 s en la pista seca, contra 0,22–1,04 s con la sala de las grabaciones.",
     "nada en el instrumento: la sala de la canción la da; las cuerdas simpáticas ya suman "
     "+4 dB a la cola."),
]

SUMMARY = (
    "<h2>El violín country</h2>"
    "<p><b>El raspado está resuelto.</b> Era el ruido de la crin: con el arco pesado y cerca "
    "del puente sacaba a la cuerda de su vibración limpia. El ruido entre armónicos estaba "
    "5–11 dB sobre las grabaciones y ahora está dentro en todas las bandas, y la cuerda pasó "
    "de perder el periodo un 40 % del tiempo a un 7 %. Se conservan el brillo (centroide / f0 "
    "2,18; sobre 2 kHz −7,5 dB) y la mordida de 1,6–3,2 kHz.</p>"
    "<p><b>Nuevo control «Mordida».</b> Reparte el grano del arco por la dinámica de cada "
    "nota: las suaves como B, las normales como C (lo que elegiste), las fuertes y acentuadas "
    "como A. Se automatiza y se asigna a un control MIDI.</p>"
    "<p><b>Lo siguiente, por orden:</b></p><ol>"
    "<li>Las notas rápidas sueltas: el 49 % suena re-articulado, contra el 11–20 %. Hay que "
    "ligar más en los pasajes rápidos y suavizar el cambio de arco.</li>"
    "<li>El grave de 250–315 Hz está 3–4 dB por debajo.</li>"
    "<li>Casi no hay vibrato (2 % de las notas largas, contra 4–30 %).</li>"
    "<li>La nota que el arco deja suena poco (−20 dB a los 60 ms).</li></ol>"
    "<h2>El modo clásico (violín, viola, violonchelo, contrabajo)</h2>"
    "<p><b>Dentro, en los cuatro:</b> la afinación media, la subida del ataque, el vibrato "
    "(frecuencia y profundidad) y el centroide.</p>"
    "<p><b>Fuera, por orden:</b></p><ol>"
    "<li>Las notas largas suenan planas, «de órgano»: empiezan y terminan sólo 2–5 dB bajo "
    "su pico, contra 7–35 dB en los intérpretes.</li>"
    "<li>El ataque se asienta demasiado pronto (t_steady): 0,05–0,34 s, contra 0,15–0,85 s "
    "en las grabaciones.</li>"
    "<li>Los finales de nota del violín y de la viola se cortan pronto.</li>"
    "<li>El violonchelo fuerte y el agudo del contrabajo son opacos: el brillo no sube con "
    "la fuerza del arco.</li>"
    "<li>La afinación en la música es demasiado perfecta: 3–7 cents, contra 13–16 en los "
    "intérpretes.</li>"
    "<li>Con el «intérprete» activo, la afinación rompe el margen de 1 cent: un 36 % de las "
    "notas en la viola y un 20 % en el violonchelo.</li>"
    "<li>El pizzicato es demasiado limpio y unos 4 cents bajo.</li>"
    "<li>El vibrato empieza en 0 s; los intérpretes lo empiezan a los 0,04–0,12 s.</li></ol>"
    '<p><a href="index.html">El análisis completo, con gráficas y audios</a></p>')
