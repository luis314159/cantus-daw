# Prompt para la próxima sesión (ronda 40)

*Escrito el 2026-11-10, al cerrar la ronda 39 (beta.14), y puesto al día el 2026-11-11 con las
ramas tardías (lapis2, delay, themes, fix2). Es una **propuesta**: el dueño decide
el orden y el alcance. La ronda 38 propuso para la 40 la tercera pasada del voicing de Chorda con
la ABX 4, la edición en cascada (insertar y borrar tiempo) y el siguiente paso de la memoria; la
reverb de convolución, que también estaba propuesta, ya salió en la ronda 39 (Aula). Antes de
construir nada, el dueño mira la ronda 39 en su pantalla y decide lo que quedó abierto. Copia el
bloque de abajo tal cual en una sesión nueva de Claude Code abierta en `F:\projects\cantus`.*

---

Lee primero:
- `CLAUDE.md`;
- `docs/STATUS.md` (la cabecera, «The next session: what round 39 left» y la sección de la ronda 39);
- `docs/round39/integration.md` (sobre todo «The owner's decisions still open» y «What is left»);
- `docs/round38/chorda.md` §2 (la tercera pasada del voicing, empezada y sin enviar);
- `docs/round39/memory-measured.md`, `docs/adr/0107-the-engines-memory-grows-with-the-song.md` y `docs/scorecard/README.md`;
- `docs/BACKLOG-excellence.md` §3 (la edición en cascada y el arrastre de secciones) y §6j (lo que la ronda 39 dejó para el dueño);
- la memoria: round-39, decision-slate-default, feedback-more-agents-r39, ui-quality-bar, feedback-work-standards-r36, feedback-no-leftovers, feedback-review-cost, gotcha-boot-path, cantus-parallel-builders.

Busca la excelencia (Colosenses 3:23). Escríbeme en español.

## 0. Antes de empezar

1. Revisa los issues de `cantus-daw` y las corridas de la beta.14 (Release, macOS): si algo
   falló, se arregla primero.
2. **Mis ojos sobre la ronda 39** (`docs/beta-checklist.md`, *Ronda 39*): todo lo de esta ronda
   se dibujó con el renderizador de CPU, nunca en una pantalla real. Mientras no lo vea en mi
   pantalla, nada nuevo se apoya en el aspecto. Lo que encuentre se arregla antes que la ronda 40.
3. **Mis decisiones abiertas** (las veintidós de `docs/round39/integration.md`). Pregúntame las que
   frenen la ronda; las otras pueden esperar:
   - el lugar del panel derecho del mezclador (junto a la última tira, o fijo en el borde);
   - el medidor en *Nivel* del inspector (se conservó el de la tira fijada; la hoja no lo dibuja);
   - qué oculta *Maximizar* (todo, como hoy, o deja el inspector al lado, como la hoja);
   - «1/2» o «½ compás» en el menú de la rejilla;
   - *Seguir* activado al arrancar;
   - el selector de tipos del explorador (*Todo · Sonidos · Plug-ins · Presets*), que pide un ADR;
   - la pista de mezclador propia de cada pad en el rack (un cambio de modelo);
   - el arrastre de una sección que mueve la canción (va con la edición en cascada, abajo);
   - las cinco preguntas de des-controls: el violeta de la perilla automatizada, el peso del
     tapón del fader, los menús y diálogos de Papel, barras de valor en el inspector, Silencio
     activado en gris;
   - la revisión de los juegos de iconos 3–5 y sus preguntas abiertas;
   - las unidades con caras de madera (siguen marrones porque esa es su cara, no el tema);
   - Pizarra lapislázuli: si la línea del cursor de reproducción del arreglo va también en oro
     (hoy solo su tapa; la línea es el acento, lapislázuli), y si el tinte de la selección
     (18 % en Noche, 10 % en Papel) y el azul de su espacio tienen la fuerza justa;
   - la fuerza de cada tema redibujado o nuevo en mi pantalla (la crema de Pergamino, el vino de
     Burdeos, el violeta de Ultravioleta, el gris medio de Peltre);
   - Delay: un tiempo sincronizado con el tempo (un parámetro y DSP nuevos; hoy *Al tempo* lo
     fija una vez) y exportarlo como plug-in CLAP/VST3 (nunca lo fue; sus ids serían para
     siempre);
   - la ronda del tema vintage y qué hacer con los bailarines.
4. Espacio: C: tenía unos 16 GB libres; para Linux usa el clon `~/ci-r38-integ` (su `target/`
   ya está caliente). Ninguna medición de rendimiento con agentes corriendo.

## Ronda 40 (propuesta): Chorda de oído, la edición en cascada, la memoria

**Por qué estas tres:**
- **Chorda** es nuestro piano y la tercera pasada quedó a medias en la ronda 38: el código del
  voicing por registro está, plano (no cambia ninguna muestra); falta enviarlo.
- **La edición en cascada / insertar y borrar tiempo** la tienen 6 de los 8 líderes
  (`docs/scorecard/features.md`), y la ronda 39 dibujó el arrastre de secciones que la necesita.
- **La memoria** es la única fila que seguimos perdiendo: 140 MB contra 62 de REAPER en una
  canción vacía (por 78), 143 contra 133 de FL Studio en *Beat* (por 10), la beta.14 medida en
  una máquina tranquila el 2026-11-11 (`docs/scorecard/README.md`); 97 MB menos que la beta.13.

**Rondas pequeñas: dos o tres constructores**, con la revisión integrada a mitad de ronda.

1. **`chorda`: la tercera pasada del voicing y la ABX 4** (`docs/round38/chorda.md` §2).
   - El ataque *fortissimo*: un camino directo que dependa de la velocidad.
   - La coherencia estéreo del agudo (la correlación empeoró, 0,43 → 0,56) y el registro nasal
     cerca del do central.
   - Una segunda búsqueda; enviar el voicing elegido como predeterminado: subir `DSP_VERSION` con
     los goldens en un commit y el tamaño medido del cambio (`CANTUS_GOLDEN_DUMP`), los 24 presets
     reajustados a ±2 LU, la tercera enmienda del ADR 0098.
   - **El kit ABX 4 para mí** (`chorda_abx`): yo escucho y mando el JSON. El oído decide.
2. **`ripple`: insertar y borrar tiempo, y la edición en cascada.**
   - Insertar tiempo y borrar tiempo en un rango: los clips partidos en los bordes, el resto de la
     canción desplazado con su automatización, sus cambios de tempo y de compás, sus marcadores y
     sus secciones.
   - **El arrastre de una sección mueve sus compases en todas las pistas** (Ctrl copia, Shift
     inserta), como dibujan las hojas de des-arrange (ADR 0113).
   - Un modo de cascada en el arreglo (mover o borrar un clip desplaza lo que viene detrás en su
     pista), si los líderes lo hacen así; investígalo primero en sus manuales.
   - Cada operación un `Command` con su inversa exacta y su brazo en el fuzz de deshacer; una
     prueba de extremo a extremo por `CantusApp::assemble`; un ADR.
3. **`memory`: los bytes privados fuera del heap vivo** (ADR 0107).
   - El heap vivo de Cantus en una canción vacía es de unos 25 MB; el resto de los 140 MB está en
     los segmentos reservados del heap, los búferes del controlador gráfico, las pilas de los
     hilos y los heaps de los controladores de audio y MIDI.
   - Mapéalos con `tools/scorecard/memmap.ps1` sobre la release integrada; cada palanca que salga
     con su ADR y su prueba (`memory_e2e` falla si un arranque crece una décima).
   - **Nunca digas que ganamos sin medirlo**: el scorecard lo mide el integrador en una máquina
     tranquila, con todo parado.

**Lo que no cambia:**
- Las reglas de `CLAUDE.md`, palabra por palabra: pruebas primero, nulos al bit, tiempo real,
  esquema con migración y fixture, ambos idiomas, el kit dibujado a mano, una prueba de extremo a
  extremo por función.
- El aspecto de la ronda 39 no se toca salvo lo que yo pida después de verlo.
- Muéstrame avances seguido; el que acepta soy yo.

**Lista cuando:**
- apruebo la ABX 4 y la edición en cascada en mi pantalla;
- la suite completa, clippy y fmt pasan en Windows y Linux;
- el scorecard se midió de nuevo en una máquina tranquila;
- STATUS, ADRs y el manual están al día;
- sale la **beta.15**, si me parece bien publicarla (pregúntame antes).

---

**Lo que solo puedes hacer tú** (está en `docs/beta-checklist.md`):
- la ronda 39 en tu pantalla: Pizarra y Pizarra lapislázuli al 100, 125 y 150 %, los
  veinticuatro temas, la barra superior, la cabecera del mezclador en la fila de pestañas, el
  brillo de los cables a 60 Hz y los conectores, el selector de iconos de pista, la ventana de
  Delay con una interfaz real sonando, el logo en la barra de tareas y en el `.exe`;
- Aula de oído, y la licencia del paquete de OpenAIR;
- la ventana de paquetes de sonidos con una descarga real;
- la lectura legal de Pizarra junto a FL Studio;
- y lo pendiente de rondas anteriores: la UMC en dúplex, la Mac y la firma de código, la revisión
  legal, el permiso de la RVR1960 y el registro de la marca.

**Después (propuesta, tú decides).** Por cuántos líderes lo tienen: grupos de clips (6), AU y la
sesión con la Mac (5), ARA fuera de proceso (5), la pista de vídeo (5). Y lo que la ronda 39 dejó
como rondas propias: el tema vintage (un diseñador primero, luego constructores) y qué debe
ser un bailarín en el aspecto Pizarra. (Los temas anteriores ya volvieron, redibujados, en la
ronda 39.)
