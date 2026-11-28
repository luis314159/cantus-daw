# Prompt para la próxima sesión (ronda 43)

*Escrito el 2026-11-27 al cerrar la ronda 42 (la afinación del proyecto; Arcus, la cuerda frotada,
con su violinista y su violista; Chorda 18; Tympanum contra baterías reales; los plug-ins solos
desde la web). El orden lo decidió el dueño (memorias `decision-round43-piano-roll-first`,
`decision-arcus-sections`, `decision-tympanum-generated`); el resto lo decide el integrador con
el criterio de la excelencia (`decision-owner-delegates-excellence`). Copia el bloque de abajo tal
cual en una sesión nueva de Claude Code abierta en `F:\projects\cantus`.*

---

Lee primero:
- `CLAUDE.md` (las reglas: un `Command` con su inverso y su brazo en el fuzz de deshacer, pruebas
  nulas primero, una prueba de punta a punta por función a través de `CantusApp::assemble`,
  protocolos y formatos probados con bytes de fuera, el esquema con su migración y su fixture,
  ambos idiomas, el kit dibujado a mano, `git add` por ruta explícita, compilaciones sin debug);
- `docs/STATUS.md` (la cabecera, «The next session: what round 42 left» y la sección de la ronda 42);
- `docs/round42/integration.md` (el orden de los merges, las cadenas de esquema y `DSP_VERSION`,
  los rojos, la auditoría de pasadas vacías, lo abierto);
- `docs/PLAN-after-round-42.md` (las dos pistas: el dueño trae usuarios, Claude va a lo hondo);
- `docs/BACKLOG-excellence.md`: «One piano roll that adapts to each instrument» y «An
  instrument's playing profile» (§3e), «Arcus, after round 42», «The musician who plays
  expressively», §6a «Instruments and effects to download on their own»;
- `docs/RELEASING.md` (antes de cualquier release);
- la memoria: round-42, decision-round43-piano-roll-first, decision-arcus-sections,
  decision-tympanum-generated, decision-r42-publish-all, feedback-work-standards-r36,
  feedback-no-leftovers, feedback-review-cost, feedback-release-lessons,
  machine-f-drive-build-quirks, cantus-parallel-builders.

Busca la excelencia (Colosenses 3:23). Escríbeme en español.

## 0. Antes de empezar

1. **Si la ronda 42 no está publicada todavía, publícala primero** (el dueño, 2026-11-26:
   *«En cuanto termines publica todo»*): el chequeo de Linux en WSL sobre `r42/integration`
   (`bash tools/wsl/guarded.sh r42/integration test`; vigila C: y F:, el disco de WSL vive en
   `F:\wsl`; nunca a la vez que otra suite o compilación) y su número en la cabecera de STATUS;
   `cargo fmt --all -- --check` en Linux (en Windows falla con el error 206); el merge a master y
   el push; la **0.1.0-beta.18** por `docs/RELEASING.md` (las notas ya están en
   `docs/releases/v0.1.0-beta.18.md`, con el párrafo de la actualización a mano para Windows
   beta.16/17; el ensayo verde en los tres sistemas, el preflight L1–L10, el tag); el repositorio
   público con `tools/public/publish.sh`; la web con la sección de Plug-ins. Después, la
   actualización dentro de la app del beta-checklist.
2. La suite completa sobre master con `--no-fail-fast`: Windows con
   `bash tools/test/suite-windows.sh` y luego —nunca a la vez— Linux con `guarded.sh`.
3. La RAM y el disco antes de lanzar constructores: hasta tres a `-j 2` (`tools/build/slot.sh`),
   `TMP`/`TEMP` de cada uno en su carpeta de F:, `df -h /f /c` antes de cada suite.

## 1. La ronda 43: pocas cosas, con contrato primero

**Rondas pequeñas: dos o tres funciones**, no seis. Lo de abajo es el orden del dueño: la ronda 43
toma lo primero y lo que quepa a su lado sin pisarlo, y lo demás pasa a la 44 en el mismo orden.
Contrato primero (tipos, firmas, números de esquema y ADR, áreas de locale vacías,
`docs/round43/BUILDING.md` con el dueño de cada archivo), un worktree por constructor, la primera
entrega integrada pronto y **la revisión del árbol integrado a mitad de ronda** (≤ 6 agentes),
todas las herramientas nuevas en `tools/`.

1. **Primero: el piano roll adaptativo por instrumento** (el dueño, 2026-11-25: encabeza la ronda
   43; el motor de guitarra viene en la 44, encima de él; el músico expresivo solo cuando los dos
   músicos de Arcus estén al 100 %). Un solo piano roll que se adapta, no un editor por
   instrumento (verifícalo con investigación antes del ADR: Expression Maps de Cubase,
   Articulation Sets de Logic, Sound Variations de Studio One):
   - el **perfil de ejecución** de cada instrumento: sus técnicas como un conjunto abierto de ids
     (no el enum de seis marcas de hoy, `note_art.rs`), sus parámetros, sus expresiones
     continuas, los nombres de sus teclas, sus herramientas; el piano roll, la partitura,
     MusicXML, el servidor MCP y la grabación leen el mismo perfil;
   - los ids siguen el **perfil de articulación orquestal de MIDI 2.0** (M2-123-UM), probado con
     bytes del documento publicado;
   - escribir fácil: el pincel de técnica, la tecla sostenida al dibujar (M, X), el carril de
     técnicas, un glifo y un color por nota que nunca choquen;
   - el rasgueo como herramienta y la tablatura como otra vista de las mismas notas;
   - Arcus como primer cliente: el sentido del arco, las técnicas y el vibrato por nota;
   - **una migración**: las seis marcas pasan a técnicas de los perfiles de cuerda frotada y
     grabada, y los proyectos viejos abren igual (esquema, fixture, fuzz de deshacer).
2. **Las secciones de cuerda de Arcus** (aprobado el 2026-11-25; el estudio es
   `research/instruments-next/strings-sections-design.md`): **un solo Arcus con la opción
   *Sección*** de 1 a 16 intérpretes (con N = 1 es el Arcus solista, bit a bit); por defecto
   **«Film» 14/12/10/10/6**; render *Balanced* por defecto (*Full* opcional; el *Auto* híbrido solo
   si pasa el ABX); la fase 43-A del estudio (intérpretes con semilla, divisi de verdad, el
   escenario por los caminos de Tympanum) y **el escenario compartido del proyecto** (un cambio de
   esquema) en 43-B. No vuelvas a preguntar estas tres decisiones.
3. **Los músicos al 100 %** (el dueño ve cada hoja antes de darlo por hecho):
   - **la decisión del rig de la mano de canto** arriba en el mástil (la pasada 8 la midió
     inalcanzable con este rig): (a) el talón de la mano apoyado en el hombro del instrumento,
     (b) una palma que se estrecha al girar, o (c) más alcance para el brazo de ella en la viola;
     decidir primero, luego `edge_probe.py` y `blend_fix.py --rules`;
   - **resolver la tabla de dedos de las figuras realistas bajo la cámara del Stage** (6°), donde
     la vista las muestra, y no la del Studio (8°) (`docs/round42/arcus-upper-next.md`, *The r42
     integration*); seguir desde las 39 filas que quedaron sin el marco de dedos;
   - **la banda lejana de Drawn** (el cuello de la ropa), **el punto del cuello** de ella en Drawn
     y **los píxeles sueltos de la mano del arco**;
   - **los cuatro trinquetes**, cada uno hasta su meta: `arcus_hand_table` 31 reglas / 20 casos →
     ninguno; `arcus_figure_specks` 150 sueltos → ninguno; `arcus_hand_form` 1 mota → ninguna;
     `arcus_hand_shape` `LOBE_REACH` 0.9 → todo el camino hasta la muñeca.
4. **Tympanum: realismo generado por la física** (`docs/round42/tympanum-real.md` §8 y
   `research/instruments-next/drums-physics-survey.md`; el dueño, 2026-11-26: *«la idea es que
   Tympanum sean sonidos generados (aunque permita sonidos grabados)»*):
   - antes que nada, las tablas que el análisis elemento por elemento señala (los agudos del
     bombo, el hueco y el «boing» de la caja, los graves de toms, hats y crash, la campana del
     ride);
   - el contacto **Hunt–Crossley** del palo y del mazo a los 3–8 ms medidos (el medio seno de
     0.8–1.8 ms del modelo 6 es de tres a cinco veces más corto), con el timbre propio del palo
     y sus contactos secundarios;
   - **el glissando de altura por energía** (Avanzini, Marogna y Bank 2012) en lugar del ajustado
     por capa;
   - **la carga de aire por modo** (y los dos parches del bombo acoplados por el aire);
   - **la bordonera como colisiones estocásticas** movidas por el parche de abajo, con el retardo
     de la cavidad; más bordonera cuanto más suave el golpe;
   - **la cascada de los platillos** (una aproximación en tiempo real a la turbulencia de ondas;
     §8.4 la pone en la ronda 44 junto con el hi-hat de dos platos: elige con el criterio de
     rondas pequeñas);
   - **los kits grabados son las referencias y los blancos del ABX** (DRSKit, Virtuosity, Big
     Rusty, Unruly, Gogodze): un ABX ciego por pieza hasta el azar, y el análisis por elementos
     dentro de la dispersión grabada (ninguna banda de tercio de octava a más de 3 dB).
   - Nota: la encuesta de física se escribió para un giro «primero muestras» que el dueño
     rechazó; su física y sus pasos para la familia modelada valen, su columna «sample» para los
     kits insignia no.
5. **Los próximos objetivos de Chorda**: los de `docs/round42/chorda-abx17-verdict.md` que la 18
   dejó abiertos (el medio que delata en el Do central, el suelo entre parciales, la cresta de la
   frase aguda, la inarmonicidad de La0–Sol#1 sobre ambos pianos, la señal constante del 16/16) y
   **los resultados del ABX 18** cuando el dueño los mande (`ABX-Chorda18.zip`; léelos con
   `tools/listening/abx_read.py` y la llave en `chorda-abx18-key\`). Siempre contra los dos
   pianos grabados (el Salamander y el bitKlavier), y un kit ciego nuevo al final.
6. **Los instrumentos como plug-ins CLAP/VST3** (BACKLOG §6a, «Instruments and effects to download
   on their own»): Chorda, Arcus, Tympanum, Fides y los sintetizadores con entrada MIDI, MPE donde
   lo tengan, el mismo editor y los presets; sus paquetes de sonido bajo demanda en la carpeta
   compartida (ADR 0030); una prueba nula de que el plug-in rinde los mismos bits que el
   instrumento en Cantus; sus páginas en la web como las de los efectos (ADR 0167); probados en
   otros hosts con instalaciones reales (beta-checklist).

## 2. Lo que espera al dueño (no lo pierdas)

- El **ABX de Chorda 18** y los **veredictos de los ABX de Arcus**: Arcus entra en
  `InstrumentKind::ALL` y en el selector solo después de ellos.
- Su ojo en **las manos** (el marco de dedos, las hojas a 400 %) y en **Violet Paper** en una
  pantalla real.
- El **ABX de Tympanum** (`ABX-Tympanum.zip`), ahora para medir el camino generado.
- El video de lanzamiento en YouTube y las cuentas sociales (`docs/social/`).
- **Verificado solo en emulación**: la vista previa de macOS nunca se abrió en una Mac real;
  todas las compilaciones van sin firmar; los zips de plug-ins nunca se probaron en otro host;
  MTS-ESP contra un maestro real.

## 3. Bugs vistos al cerrar la ronda 42 (revisar al empezar)

- **`setup_done` que se pierde**: en las pruebas de `tympanum_kit_change_e2e`, una app que cargó
  `audio_setup.setup_done: true` guardó `false`, y la siguiente abrió el diálogo de audio del
  primer arranque. Las pruebas ya reescriben su configuración, pero falta saber si una app real
  puede guardar `false` sobre un `true` (`audio_setup/apply.rs`, `store_settings`). Si puede, un
  usuario vería otra vez el diálogo del primer arranque: escribe la prueba que lo reproduzca
  por `CantusApp::assemble` y corrígelo.
- **El pánico del `clamp` en `soak_smoke`**: visto una vez bajo carga, nunca reproducido; los tres
  `clamp` que podían invertirse tienen guarda. Si vuelve, guarda la semilla y la entrada.
- **La memoria de una canción vacía subió 5.1 → 5.9 MB** (memory_e2e, Windows y el runner de
  Linux, 2026-11-28): la cota pasó a 6.5 para publicar la beta.18. Encuentra dónde están los
  0.8 MB (las cadenas de la ronda 42 son +43 KB de inglés; el resto, alguna vista o tabla que
  ya se crea al arrancar) y devuélvela a ~5.1 con su cota de vuelta.
- **`cargo fmt --all` en Windows** falla con «os error 206» (rutas largas); se usa rustfmt archivo
  por archivo y `fmt --check` en WSL.
