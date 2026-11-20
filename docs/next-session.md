# Prompt para la próxima sesión (ronda 41)

*Escrito el 2026-11-14, al cerrar la ronda 40 en `r40/integration` (todavía sin pasar a master
ni publicar la beta.15). Es una **propuesta**: el dueño decide el orden y el alcance. Antes de
construir nada nuevo, el dueño escucha lo que la ronda 40 dejó esperando su oído y decide lo que
quedó abierto. Copia el bloque de abajo tal cual en una sesión nueva de Claude Code abierta en
`F:\projects\cantus` (o en `F:\projects\daw-r40\integration` mientras la ronda 40 no esté en
master).*

---

Lee primero:
- `CLAUDE.md`;
- `docs/STATUS.md` (la cabecera, «The next session: what round 40 left» y la sección de la ronda 40);
- `docs/round40/integration.md` (sobre todo «What is left», el rastro de Chorda y el de Tympanum);
- `docs/round40/chorda.md` (los candidatos, hasta el 16) y la nota de traspaso más reciente en
  `F:\projects\daw-r40\voicing\` (`NEXT-c16.md`);
- `docs/round40/ladder.md` («Model 6, per element») y `docs/round40/tymp-engine.md` (las secciones de arriba);
- `docs/adr/0124-tympanum-the-acoustic-drummer.md`, `0125`, `0126`;
- `docs/BACKLOG-excellence.md` §0.2 (afinación) y §4b;
- la memoria: round-40, decision-chorda-like-salamander, feedback-work-standards-r36,
  feedback-no-leftovers, feedback-review-cost, gotcha-boot-path, cantus-parallel-builders,
  machine-f-drive-build-quirks.

Busca la excelencia (Colosenses 3:23). Escríbeme en español.

## 0. Antes de empezar

0. **Primero, el error del motor que encontré en la beta.15** (`docs/BACKLOG-excellence.md` §1,
   el primer punto): un efecto que nunca se envía cuando abro un proyecto después de otro (el EQ
   del master de Aurora tras Neon Tide). Salió en la beta.15 como problema conocido por mi
   decisión; se arregla antes que todo lo demás, con su prueba de extremo a extremo.
1. **Lo que espera mi oído** (nada sale sin él):
   - **Chorda, la ABX del candidato 16** (https://claude.ai/artifact/Jpwa91Tz4V8rbcRrgiXMKg; el
     zip FLAC `compartir\cantus-chorda-abx16.zip` para quien esté fuera de la organización). Por
     mi decisión del 2026-11-14 el 16 **ya es el sonido por defecto** (`DSP_VERSION` 7, la tercera
     enmienda del ADR 0098; los 24 presets reajustados); mi escucha y mi ABX siguen pendientes
     (`docs/BACKLOG-excellence.md`) y deciden la próxima pasada de voz. Abierto conmigo: el nivel
     de un Chorda nuevo queda 10.4 LU bajo la referencia de la familia — subirlo aprieta su
     fortissimo contra el techo de salida.
   - **Tympanum b10 y b11** (`compartir\tympanum-b10\` y `compartir\tympanum-b11\`: 71 de 162
     pistas medidas, el costo 7.3 % de un núcleo), el kit por defecto (hoy *Studio Maple*) y
     **el híbrido** contra el generado (los paquetes v3 de drums-gen 3 ya están en la versión
     `content-r40` del repositorio público, subidos con mi OK; sus escuchas en
     `F:\projects\daw-r40\tmp\hyb\v3\listen-{jazz,studio}\`).
   - **bitKlavier contra el Salamander** (`compartir\pianos-c\`): decide el piano grabado por
     defecto.
2. **Mis ojos y mis manos** (`docs/beta-checklist.md`, *Ronda 40*): todo se dibujó con el
   renderizador de CPU y se tocó con eventos sintéticos. Lo que encuentre se arregla primero.
3. **Mis decisiones** (`docs/round40/integration.md`, «What is left»): los ids de Cantus Delay
   (para siempre), el número del pad con canal propio en color de acento, la tecla del teclado
   del ordenador que se pliega en *Más*, el *Cueing* de Compás, los azules del reparto sobre
   croma, las hojas de Papel, una sala por defecto para un piano grabado, el kit por defecto de
   Tympanum (ya está en el selector desde el 2026-11-14), y los correos de las licencias CC BY
   escritas.
4. **Cerrar la ronda 40**: la suite completa, clippy y fmt en Windows y en Linux (WSL) sobre el
   árbol final, el scorecard medido en una máquina tranquila (con todo parado), y solo con mi
   OK: master, la beta.15 y el sitio. Revisa también los issues de `cantus-daw` y la CI del
   último commit público.
5. Espacio: F: se llenó una vez en la ronda 40 (los commits fallaron). `df -h /f /c` antes de
   cada compilación; borra los `target/` de las ramas ya fusionadas.

## Ronda 41 (propuesta): el modelo 6 de la batería, la microafinación, y Tympanum a ciegas

**Por qué estas tres:**
- **La batería generada** es lo que más lejos está: *"nos falta un montón"* tras la b9. La
  escalera mostró qué representación necesita cada pieza; el modelo 5 ya no da más.
- **La microafinación** la pedí para profesionales (2026-11-13): ninguno de los líderes la
  tiene en su piano y su batería a la vez con esta profundidad.
- **Tympanum** ya está en el selector (2026-11-14), pero su sonido no se ha juzgado a ciegas.

**Rondas pequeñas: dos o tres constructores**, con la revisión integrada a mitad de ronda.

1. **`drums-model6`: el modelo 6 de Tympanum** (`docs/round40/ladder.md`, «Model 6, per element»).
   - El bombo sobre la tabla fina de ruido (R3: STFT 1024/256).
   - Caja, aro, *sidestick* y toms: las 38 bandas del modelo 5 con colas monótonas, más los
     **cinco granos de ataque de fase mínima** (R2) por pieza, zona y velocidad; después, un
     ataque resuelto en el tiempo por encima de 1,5 kHz y una cadena de eventos de la
     bordonera.
   - Charles y platos: **la capa por bin de R3b** junto a sus modos (los modos se quedan); sin
     comodulación (R4 fue la peor).
   - Costo meta ≈ 6,5–7 % de un núcleo el modelo solo, y el kit con todos los micros bajo el
     10 % (tras drums-gen 3, el modelo 5 solo va en 6,0 % y el kit con todo en 7,3 %). Medido
     contra las grabaciones con `tools/tympanum-fit/` y las quince pistas (`accept_b10.py`);
     una escucha mía por paso, a ciegas cuando valga la pena (MUSHRA como la escalera).
   - Los paquetes híbridos se reajustan al modelo nuevo (`refit_hybrid.sh`, versión nueva;
     subirlos me lo preguntas).
2. **`tuning`: la microafinación de Chorda y Tympanum** (BACKLOG §0.2).
   - Una afinación del proyecto que siguen todos los instrumentos con altura (Chorda, Exemplar,
     Partes, los pads de Pulsus en *seguir*), con excepción por instrumento; Scala `.scl` +
     `.kbm` y MTS-ESP (cliente y maestro; revisa antes la licencia de su biblioteca), probados
     con archivos y volcados de fuera.
   - Chorda: diapasón (380–480 Hz; 415, 430, 432, 440, 442, 443), temperamentos (igual,
     pitagórico, mesotónicos, Werckmeister, Kirnberger, Vallotti, Young, Kellner, justa en una
     tonalidad, cualquier Scala), **presets por tonalidad** (cada histórico girado a las 12
     tonalidades; *seguir la tonalidad de la canción*), estiramiento (*como lo afina un
     técnico*), anchura de unísono por registro, y un **editor por tecla** con un afinador
     estroboscópico; un círculo de quintas que muestra qué tonalidades suenan más puras.
   - Tympanum: cada pieza en semitonos y cents, mostrada en Hz y nota; las dos membranas por
     separado (el intervalo entre ellas da el *glide* y la sustentación); **afinar los toms a la
     tonalidad de la canción**; los platos en tamaño o altura.
   - Todo plegado en *Editar ▸ Afinación ▸ Avanzado*: nada nuevo en *Tocar*; por defecto
     temperamento igual a 440 Hz, nulo al bit. Esquema con migración y fixture, brazos en el
     fuzz de deshacer, una prueba de extremo a extremo por `CantusApp::assemble` (un proyecto en
     Werckmeister III a 415 donde Chorda, Exemplar y un plug-in MTS-ESP suenan afinados entre sí).
3. **Tympanum, la prueba a ciegas** (ya está en el selector y en `InstrumentKind::ALL` desde el
   2026-11-14): un kit como el de Chorda — cada kit modelado contra el grabado del que se midió,
   pieza por pieza y en un groove, ABX, preferencia y «¿grabación o síntesis?» —, su tarjeta en el
   sitio y en el README; y mi veredicto antes de dar su sonido por terminado (`docs/BACKLOG-excellence.md`).

**Lo que no cambia:**
- Las reglas de `CLAUDE.md`, palabra por palabra: pruebas primero, nulos al bit, tiempo real,
  esquema con migración y fixture, ambos idiomas, el kit dibujado a mano, una prueba de extremo a
  extremo por función, formatos probados con bytes de fuera.
- El aspecto de las rondas 39 y 40 no se toca salvo lo que yo pida después de verlo; *Dibujado*
  sigue siendo el modo de imágenes por defecto.
- Muéstrame avances seguido; el que acepta soy yo.

**Lista cuando:**
- apruebo de oído el modelo 6 y la afinación (Chorda y Tympanum) en mi equipo;
- Tympanum está en el selector y lo usé en una canción;
- la suite completa, clippy y fmt pasan en Windows y Linux;
- el scorecard se midió de nuevo en una máquina tranquila;
- STATUS, ADRs y el manual están al día;
- sale la **beta.16** (o la beta.15, si la 40 no salió aún), si me parece bien publicarla
  (pregúntame antes).

---

**Lo que solo puedes hacer tú** (está en `docs/beta-checklist.md`):
- escuchar: la ABX del candidato 16 de Chorda (y repartirla), Tympanum b10/b11, el
  híbrido, bitKlavier contra el Salamander;
- ver la ronda 40 en tu pantalla: *Fotográfico* en una GPU real a 100 y 150 %, las bandas del
  mezclador, las vistas de Chorda y Tympanum, los doce bailarines, tus respuestas de aspecto;
- con tu interfaz: Tympanum y Chorda grabado a búfer bajo, *Escuchar la clave*, una toma con FX
  en dúplex, Cantus Delay en otro DAW;
- instalar desde un archivo una DrumGizmo real, cambiar de kit con la mezcla hecha, descargar el
  híbrido y los pianos grabados desde la app;
- confirmar los ids de Cantus Delay; mandar los correos de licencias CC BY;
- y lo pendiente de rondas anteriores: la UMC en dúplex, la Mac y la firma de código, el permiso
  de la RVR1960 y el registro de la marca.

**Después (propuesta, tú decides).** Por cuántos líderes lo tienen: grupos de clips (6), AU y la
sesión con la Mac (5), ARA fuera de proceso (5), la pista de vídeo (5). De la ronda 40: los
cinco micrófonos del modelo de Chorda tocados por separado, una caché de flujo más grande (o en
FLAC) para varios micrófonos de bitKlavier a la vez, prever las necesidades de memoria que la
automatización no prevé hoy (el modo de un EQ, la respuesta de Aula), los *Grooves* de Tympanum
(BACKLOG §5), *Fotográfico* por defecto cuando sea rápido y ligero, y el tema vintage.
