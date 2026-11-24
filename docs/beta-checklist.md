# Lista de pendientes para la beta pública (v0.1.0-beta.1)

*Escrita para el dueño, en español, el 2026-10-11 (los detalles técnicos están en
[`BETA.md`](BETA.md) y en [`STATUS.md`](STATUS.md)). Marca cada casilla al terminar.*

**Ya hecho:** la cadena de publicación (etiqueta → instalador, portable y sumas en GitHub
Releases), la auto-actualización, sesión de 2 horas sin fallos, recuperación tras cierres,
exportar MP3/FLAC, volumen parejo de instrumentos, avisos de terceros, privacidad, manual
EN/ES, plantillas de issues, licencias (app GPL, motor MPL, CLA, marca — ADR 0047).

## 1. Pruebas que solo puedes hacer tú

- [ ] **Audio en vivo 10 minutos sin cortes** con la Behringer UMC en dúplex (entrada y
      salida), al buffer por defecto y a 256; anotar el resultado en `docs/hardware.md`.
      Conecta la interfaz *antes* de abrir Cantus.
- [ ] **Tabla de al menos 3 interfaces**: UMC, ASIO4ALL, FlexASIO (ideal: una Focusrite).
- [ ] Desconectar y reconectar la interfaz con Cantus abierto: se recupera.
- [ ] Desde un primer arranque, grabar una guitarra en menos de un minuto.
- [ ] **2–3 canciones completas** (grabar, programar, arreglar, mezclar, masterizar,
      exportar), anotando cada fricción. Las peores se arreglan antes de la beta.
      *Avance (2026-10-16): ya lo has usado bastante con los instrumentos digitales de
      Cantus; faltan las grabaciones (voz, guitarra) con interfaz real.*
- [ ] **Instalador a mano** en una máquina limpia (Windows Sandbox sirve): instalar, abrir,
      desinstalar; instalar una versión vieja y dejar que se actualice.
- [ ] Probar a mano lo de las rondas 24 y 25:
  - [ ] grabar con buffer ASIO de 2048 (la toma empieza y para bien);
  - [ ] guardar un proyecto grande mientras suena (la ventana no se congela);
  - [ ] clips enlazados: editar una copia, *Hacer único* otra;
  - [ ] cables: arrastrar y clic-clic, con y sin Ctrl/Alt, y deshacer;
  - [ ] un envío PRE sigue sonando al bajar el fader;
  - [ ] matriz de ruteo (Alt+F9); mover un grupo arrastrando y con Alt+flechas;
  - [ ] soltar una pista sobre un canal del mixer;
  - [ ] los 12 temas; el color del icono con Cantus **anclado** a la barra de tareas;
  - [ ] Lamina se escucha al nivel de los demás.
- [ ] Probar a mano lo de la ronda 26:
  - [ ] *Enlazar con…* (Alt+N) desde un clip, y renombrar un grupo en la fila *Nombre* del
        inspector;
  - [ ] el navegador de clips (Alt+F5) con la herramienta de pintar (B o 2 con el arreglo
        enfocado): clic coloca, arrastrar pinta, clic derecho borra, Esc la desactiva;
  - [ ] instalar el piano Salamander en *Paquetes de sonidos* y tocarlo (8 y 16 capas; la
        primera carga llena la caché de muestras);
  - [ ] la tarjeta de paquete que falta (abrir un proyecto con un paquete sin instalar);
  - [ ] *Nivel ▸ Escribir un nivel…* en el menú de ruta de un cable;
  - [ ] un nombre en japonés o coreano en una pista (las letras del sistema llegan solas).
- [ ] Probar a mano lo de la ronda 28 (sonido):
  - [ ] instalar **DRSKit** en *Paquetes de sonidos* (2,8 GB de descarga, se mezcla a estéreo
        al instalar) y tocar un ritmo: las filas de la batería en el piano roll, el charles
        abierto que se corta al cerrarlo, ningún golpe repetido idéntico en un redoble;
  - [ ] **Chorda**: tocarlo con un teclado de 88 teclas y pedal **continuo** (medio pedal),
        y el pedal suave si tu teclado lo tiene (CC 67); los presets, por oído (tienen la
        última palabra).
- [ ] Probar a mano lo de la ronda 28 (voz, efectos, barra superior, moduladores y comping):
  - [ ] **Tonus con una voz real**: abrir `F:\projects\daw-r28\shots\voice\demo\tonus-demo.cantus`
        (*Archivo ▸ Abrir*). Suena *Voice · natural*; pon en solo *Voice · hard tune* o
        *Voice · MIDI melody* para comparar. Arma *Your voice* (entrada 1, con Tonus natural
        en su canal) y canta escuchándote, o suelta ahí una voz grabada: ¿natural a
        velocidad natural, limpio el efecto duro, ningún chasquido al cambiar de nota?
        Para regenerarlo con el build de prueba:
        `cargo run -p cantus-ui --example tonus_demo -- <carpeta>`.
  - [ ] **El editor de afinación con una voz real**: abrir
        `F:\projects\daw-r28\shots\pitch\demo\pitch-demo.cantus`: suena *Voice ·
        corrected*; quita el silencio a *Voice · as sung* (la toma sin tocar), *Voice · a
        third above* (el doblaje) o *Melody (MIDI)* para comparar. Graba una voz en *Your voice*
        y haz doble clic en la toma: corregir, mover notas ±3 semitonos, estirar una nota,
        el carril de afinación: ¿algún artefacto que se oiga? Regenerarlo:
        `cargo run -p cantus-ui --example pitch_demo -- <carpeta>` (el de `shots` se
        escribió antes de las revisiones: mejor regenerarlo).
  - [ ] (Opcional) la comparación a ciegas: Tonus contra Auto-Tune o Newtone; el editor
        contra Melodyne o Newtone (BACKLOG §3b, §3c).
  - [ ] **Los 12 efectos nuevos y sus presets, de oído**: Concentus (los modos Juno y
        Dimension contra tus referencias), Flexus, Phasis, Tremor, Vibrans, Anulus,
        Translatio, Fluctus; Tonsor (las tres curvas, y el techo de pico real muy
        empujado), Impetus y Discrimen sobre baterías reales, Speculum acoplado y flotante.
        Cómo se sienten sus editores en la mano (capturas en `F:\projects\daw-r28\shots\modfx\`
        y `…\shots\dynfx\`).
  - [x] **Decidir**: un efecto de modulación recién insertado no cambia nada hasta que
        subes su *Mezcla* o *Profundidad* o eliges un preset. *Decidido el 2026-10-16:
        opción B — al insertarlo carga un preset inicial que suena; los valores por defecto
        siguen siendo neutros (BACKLOG §3b; por implementar).*
  - [ ] **La barra superior contra la de FL**: mirar `F:\projects\daw-r28\shots\topbar\`
        (156 PNG: de 800 a 2560 px, 75–200 %, EN/ES, Noche y Papel, ⋯ abierto y cerrado) y
        `F:\projects\daw-r28\shots\fix2\topbar\` (1920 y 2560 px con un dispositivo) junto a
        la de FL. En el programa: Tap (Mayús+T), el compás, Patrón/Canción (L), el medidor
        en LUFS, renombrar el proyecto, *Personalizar la barra superior…*, una ventana
        estrecha (los grupos en ⋯).
  - [ ] **Moduladores, de oído**: un LFO en un efecto, una envolvente en el corte de un
        filtro, un seguidor que baja una pista bajo otra, pasos en un parámetro de un
        plug-in tuyo (CLAP o VST3); *Convertir en automatización*; mover una perilla
        mientras grabas (queda como automatización).
  - [ ] **Comping, de oído**: grabar tomas en bucle y combinarlas; ¿se oyen las uniones
        (fundido de 10 ms)? Mover una unión, comps alternativos (*Comp. A ▾*), *Consolidar y
        fusionar*, y dos pistas de un mismo grupo del mezclador combinadas juntas.
- [ ] Probar a mano lo de la ronda 29:
  - [ ] **Warp, de oído**: el juego de escucha en `F:\projects\daw-r29\shots\warp\listen\`
        (cada modo, estirado y transpuesto); soltar un loop de otro tempo en el arreglo
        (suena a tiempo al instante), *Cuantizar audio* en una batería, *Poner el tempo del
        proyecto desde este clip*, Alt+arrastrar el borde de un clip.
  - [ ] **Pista de tempo**: una rampa y un cambio de compás; marcadores (saltar, bucle,
        *Exportar entre marcadores*); exportar MIDI y DAWproject y abrirlos en otro DAW.
  - [ ] **MIDI learn con tu controlador**: clic derecho ▸ *Aprender controlador MIDI* en un
        fader, una perilla de efecto y la de un plug-in; el modo relativo si tu controlador
        lo tiene; mover una perilla grabando (queda como automatización); el panel
        *Controladores*.
  - [ ] **Un plug-in real en su propio proceso** (*Juntos*, el predeterminado), también uno
        con diálogo de licencia; *Recargar* tras cerrarlo a la fuerza desde el Administrador
        de tareas (Cantus sigue sonando).
  - [ ] **Congelar** una pista pesada y descongelarla; *Bounce in place* con y sin efectos;
        grabar una voz con *Escucha de baja latencia* (sin el retraso de los plug-ins).
  - [ ] **La vista de sesión** (ronda 33, ADR 0077): `Tab` entre sesión y arreglo; lanzar
        clips y escenas a tiempo, *Puerta* manteniendo el clic, acciones de seguimiento,
        *Capturar escena*, grabar en una ranura con la pista armada, *Grab. arreglo* y
        comprobar que el arreglo suena como lo tocaste; aprender un pad sobre una ranura.
  - [ ] **Una rejilla real** si tienes alguna (Launchpad Mini MK3, X, Pro MK3, APC mini mk2,
        APC40 mkII): que se reconozca sola al conectarla, que los pads lancen y se iluminen
        con los colores de los clips (verde latiendo al sonar, parpadeando al esperar, rojo
        al grabar), las flechas muevan el recuadro naranja y los botones de escena y de
        detener funcionen. En el **Pro MK3**, comprobar las flechas (botones laterales y
        superiores), *Shift*, que *Stop Clip* (el último de la fila etiquetada) mantenido
        convierta la fila bajo la rejilla en botones de detener y, con *Shift*, lo detenga
        todo, y que *Session*, *Note* y *Chord* no hagan nada; en los **APC**, la velocidad del parpadeo y del
        latido. Anotar en `docs/round33/launcher.md` lo que no coincida.
- [ ] Probar a mano lo de la ronda 33 (sincronía, ADR 0078–0079):
  - [ ] **Link con otra aplicación de verdad**: Ableton Live (o su prueba), una app Link del
        teléfono (p. ej. *Link to MIDI*, un metrónomo con Link) u otro PC con Cantus en la
        misma red. Encender LINK en Cantus (el cortafuegos de Windows pregunta una vez:
        permitir en redes privadas), ver el número de pares, cambiar el tempo desde cada
        lado, arrancar con *Inicio/parada* y oír que los tiempos caen juntos. (Aquí no se
        pudo compilar LinkHut del SDK: la máquina no permitió compilar código de fuera.)
  - [ ] **Reloj MIDI con un aparato real** (caja de ritmos, sinte con arpegiador): Cantus
        como maestro (marcar *Reloj* en la salida) y como esclavo (marcar *Reloj* en la
        entrada); probar Start, Stop, Continue y un salto de posición.
  - [ ] **MTC** con un aparato o programa que lo lea o lo envíe (vídeo, otro DAW), a 25 y a
        29.97 drop-frame, con código inicial 01:00:00:00.
  - [ ] **Un loopback MIDI en Windows** (loopMIDI o Windows MIDI Services; aquí no hay
        ninguno instalado): enviar reloj por un puerto y seguirlo por el otro en un segundo
        Cantus.
- [ ] Probar a mano lo de la ronda 33 (MPE, ADR 0080):
  - [ ] **Un controlador MPE real**, si tienes uno (Seaboard, LinnStrument, Osmose, Sensel;
        o una app MPE en el teléfono): Ajustes ▸ MIDI, la columna *MPE* encendida en su
        puerto; tocar Unda, Calor y Modulus (inflexión, presión, timbre por nota); grabar y
        ver las curvas en el piano roll; un plug-in MPE (Vital, Surge XT, Pigments) en
        *Automática* y en *MPE (MIDI)*. Anotar cómo se siente en `docs/round33/mpe.md`.
- [ ] Probar a mano lo de la ronda 34 (la IA, ADR 0081–0086):
  - [ ] **Un Windows limpio** (un equipo o una máquina virtual que nunca tuvo el
        redistribuible de Visual C++): instalar la beta.9, *Separar stems…* en un clip y
        *Convertir a MIDI…* en otro. Las DLL que Cantus lleva en `ml\` deberían bastar (el
        orden de carga lo dice), pero esta máquina tiene el redistribuible instalado.
  - [ ] **Claude Desktop con la extensión**: *Ajustes ▸ IA ▸ Guardar la extensión
        (.mcpb)…*, instalarla en Claude Desktop y pedirle algo pequeño (una pista, unas
        notas); ver la tarjeta de confirmación, la actividad de IA y que Ctrl+Z lo deshace.
        También *Copiar configuración para* ▸ Claude Code si lo usas.
  - [ ] **Una memoria USB real en FAT32 o exFAT**: que un agente exporte un WAV o un MIDI a
        una carpeta permitida en ella (sin enlaces duros: el renombrado que nunca
        reemplaza), y que con los ajustes de Cantus en esa memoria los agentes externos se
        nieguen con palabras claras.
  - [ ] **Escuchar a los escritores**: lo que escriben `write_part`, `voice_chords` y
        `pedal_from_chart` en Chorda, Lamina y Gravis — el espaciado de los acordes, los
        arpegios y las líneas de bajo, el levantar del pedal. Anotar en `docs/round34/tools.md`.
  - [ ] **Escuchar los másteres del asistente** (*Asistente de mastering…* en el menú de la
        tira máster) sobre música real, y el A/B: las curvas son guías de promedios
        publicados, no el oído de nadie. Anotar en `docs/round34/listen.md`.
  - [ ] **Convertir tus propias grabaciones a MIDI**: una línea cantada, un piano, un loop
        de batería; decir si las notas se sienten bien. Anotar en `docs/round34/transcribe.md`.
  - [ ] **La sesión con la Mac**, filas 17 en adelante de `docs/mac-session.md`: el puente
        MCP y el ayudante de ML dentro del paquete firmado, una separación con CoreML y con
        el procesador, el tiempo de cada una.
- [ ] Probar a mano lo de la ronda 35 (el personaje que baila, ADR 0087–0091):
  - [ ] **La ventana de compañía en tu escritorio**: agrega una pista *Visual ▸ Saltator*,
        reproduce una canción y abre la compañía con **Alt+Mayús+D**; muévela, cámbiale el
        tamaño, prueba el menú (clic derecho o ⋯) y el **modo para streaming**
        (**Ctrl+Mayús+D**). Que se vea bien sobre tu fondo y tus ventanas.
  - [ ] **Tu OBS**: *Captura de ventana* de *Cantus Stage* con el método *Windows 10 (1903 y
        posterior)* — fondo transparente; con *BitBlt* sale el color clave. Aquí se usó un OBS
        portátil y nunca se tocó tu configuración. Si el Firewall de Windows pregunta por el
        OBS portátil, *Cancelar* es seguro.
  - [ ] **La *Captura de juego* de OBS en una máquina con una sola GPU**: en esta laptop de
        dos GPU no puede abrir la ventana (OBS y Cantus en GPU distintas).
  - [ ] **Publicar un clip**: *Archivo ▸ Exportar video…*, vertical, 30 s, MP4; subirlo a
        TikTok, Instagram y YouTube y ver que el audio y la imagen sigan juntos.
  - [ ] **El MOV transparente en tu editor** (`alpha_png.mov` o uno tuyo): importarlo en
        Resolve, Premiere o CapCut y ver la transparencia.
  - [ ] **La captura en Linux** (Xcomposite, PipeWire) en un Linux de escritorio real.
  - [ ] **La sesión con la Mac**, fila 18 de `docs/mac-session.md`: VideoToolbox y
        AudioToolbox, y la transparencia de la ventana de compañía en macOS.
- [ ] **El instalador de `v0.1.0-beta.1`**: el botón de descarga de cantusdaw.com lo baja de
      GitHub; instalarlo (Windows avisará «Windows protegió tu PC» mientras no haya firma:
      *Más información ▸ Ejecutar de todas formas*), abrirlo, desinstalarlo.
- [ ] **Chorda — prueba a ciegas** (ADR 0059): ≥ 20 oyentes, frases iguales en Chorda,
      Pianoteq 9 y un piano de muestras (p. ej. Salamander); que no sepan cuál es cuál.
      Anotar preferencias y comentarios en `docs/round28/sound.md`.
- [ ] **Chorda modelo 2 — tu ABX** (ronda 37, ADR 0098) <!-- r37:piano -->: en una
      terminal, `cargo run --release -p cantus-ui --example chorda_abx -- F:\projects\chorda-abx`
      (instala el Salamander si falta, ~740 MB, y escribe la carpeta en unos minutos). Abre
      `F:\projects\chorda-abx\index.html` con auriculares, el volumen fijo desde el primer
      clip: **ABX 1** (Chorda 2 / Salamander, 16 ensayos), **ABX 2** (Chorda 1 / Chorda 2,
      16 ensayos), **Preferencia** (14 preguntas ciegas, con una nota del porqué). En
      *Resultados*, *Descargar resultados (JSON)* y envía el archivo (o pega los números y
      las notas en `docs/round37/piano.md`). Lo que cierra Chorda: el modelo 2 preferido al
      modelo 1 y sin preferencia significativa por el Salamander. `LEEME.md` en la carpeta
      lo explica.
  - [x] Primera ABX hecha (2026-11-05): ABX 1 15/16, ABX 2 15/16; preferencia Chorda 2
        sobre Chorda 1 7 de 7, Salamander sobre Chorda 2 7 de 7 («muchos medios, algo
        borroso, un poco nasal; el Salamander más vivo, mejores graves y agudos»).
  - [ ] **Tercera ABX, con el Chorda integrado** <!-- r37:piano -->: la carpeta
        `F:\projects\daw-r37\abx-kit3\` (ya escrita; abre `index.html`; la de `abx-kit2`
        quedó vieja), igual que la primera, y envía el JSON. Además, en Chorda (modelo 2) gira **Micros** de 0 % (sobre los martillos) a
        100 % (en el público), **Tapa** cerrada / media / abierta y **Tamaño** de la sala, y
        prueba cinco presets (Cercano e íntimo, Pop al frente, Cola en sala, Cinemático
        suave, Estudio seco): ¿algún ajuste
        suena a filtro de peine, nasal o con saltos al moverlo mientras suena? ¿Qué
        perspectiva querrías por defecto?
- [ ] **Sectio, el cortador de Exemplar — por el oído** (ronda 37, ADR 0099) <!-- r37:piano -->:
      arrastra a la tarjeta *Cortes* de un Exemplar vacío cinco o seis loops CC0 de
      batería y uno de piano o guitarra (anota de dónde salen y su licencia, p. ej.
      OpenGameArt o Freesound con CC0), y en cada uno di si los cortes caen donde oyes los
      golpes; *Volcar al piano roll* debe repetir el loop igual. Medido: 91,9 % de los golpes
      a menos de 10 ms en loops hechos con Cantus, 75,8 % en MDB Drums (grabaciones reales
      anotadas a mano; 88 % a menos de 20 ms) — el objetivo era 95 %. Si un loop sale mal,
      guárdalo y dímelo.
      Prueba también el editor de la tarjeta (arrastrar, clic derecho, doble clic, *Volver a
      cortar* con vista previa) y dos estilos de la biblioteca (*Break de batería*, *Corte
      vocal*) sobre tu propio loop.
- [ ] **Los kits y los presets bajos, por el oído** (ronda 37, ADR 0100 y 0101)
      <!-- r37:drums -->: los ocho kits nuevos de Pulsus (en el menú de kits) y los 20
      presets que quedaron por debajo de su ventana de volumen por su margen (la lista
      está en `docs/round37/library.md`, *Open*): ¿alguno suena flojo o se pasa? Y la
      **pregunta de los presets de sidechain**: los presets de ducking que necesitan una
      señal clave (Porta y Comprimo *Duck*) no suenan solos al insertarlos; ¿los dejamos
      así (con su nota) o se quitan de la lista de inicio?
- [ ] **La escena de Chorda, a la vista** (ronda 37, ADR 0104) <!-- r37:scene -->: abre
      Chorda, gira *Cuerpo* y *Acabado* y compárala con las fotos de las librerías de piano
      premium (`docs/round37/scene/`): ¿se ve profesional y creíble?
- [ ] **El acabado, en la app real** (ronda 37, ADR 0103) <!-- r37:finish -->: las trece
      zonas (barra superior, transporte, botones, mezclador, tira, fader/LED/perilla,
      arreglo, encabezados, rack, teclado del piano roll, EQ, navegador, tipografía) a
      100/125/150 % en los temas Noche y Papel; anota lo que se vea mal.
- [ ] **Fuga y Volta** (ronda 37, ADR 0105) <!-- r37:chars -->: en el escenario y en la
      ventana de compañía, con una canción sonando: ¿se ven cool y serios?
- [x] **REAPER instalado** (2026-11-06): REAPER 7.85, evaluación de 60 días, portátil en
      `F:\projects\daw-r37\reaper` (bórralo con la carpeta); el arranque ya se midió contra
      Cantus y FL (`docs/scorecard/README.md`).
- [ ] **El DAWbench en vivo** (ronda 37, ADR 0102) <!-- r37:integration -->: con tu interfaz
      conectada y el mismo búfer en los tres (64 y 128), arma la sesión de
      `docs/perf/benchmark.md` §3 en Cantus, FL y REAPER: una sierra sostenida por pista (en FL
      y REAPER, la pista 1 de Cantus exportada a WAV) y 8 Comprimo por pista (en FL y REAPER el
      CLAP/VST3: `cargo run -p xtask -- bundle comprimo`), umbral −30 dB, 4:1. Empieza con 8
      pistas, toca el loop 60 s; si no hay cortes (el contador de cada programa: la barra de
      Cantus, *underruns* de FL, el *Performance Meter* de REAPER), agrega 8 más, y de una en
      una cerca del límite. Tres pasadas, la mediana. Anota versión, interfaz, driver y
      temperatura al empezar, y pásame los números; luego lo mismo con Surge XT (VI, §4).

### El aspecto nuevo en tu pantalla (ronda 39, ADR 0108–0114) <!-- r39:docs -->

**Aceptado el 2026-11-11**: lo miraste en tu pantalla y no encontraste nada malo nuevo
(«no encontré nada malo nuevo»). Las casillas de abajo quedan para lo que vuelvas a ver.

Todo lo de la ronda 39 se dibujó con el renderizador de CPU (`raster.rs`) y se probó con clics
sintéticos; nada se vio en un monitor ni con un ratón de verdad. Usa la release de la beta.14 (o la
release integrada que te pase) en tu monitor. Para cada punto, si
algo se solapa, sale cortado, se ve borroso o no se entiende, haz una captura (Win+Shift+S) y
pásamela con la escala y el tema.

- [ ] **Pizarra y Pizarra lapislázuli a 100, 125 y 150 %**: con un proyecto de demostración
      abierto (*Archivo ▸ Canciones de demostración ▸ Neon Tide*), abre *Ajustes ▸ Apariencia* y
      elige **Pizarra**, modo *Noche*. Pon la escala de la interfaz en 100 % (`Ctrl+0`, luego
      `Ctrl+Menos` o `Ctrl+Más` hasta leer 100 % en el zoom de la barra, o en *Más*), y mira la ventana
      entera con el arreglo, el mezclador, el piano roll (`F7`), el rack (`F6`) y el explorador
      (`Alt+F8`) delante, uno a la vez. Repite a 125 % y a 150 %. Luego todo en *Papel*, y luego en
      **Pizarra lapislázuli** (Noche y Papel): debe distinguirse de Pizarra a primera vista — un
      espacio azul noche, la selección en lapislázuli, y en oro el reloj, el tempo, los números de
      las reglas, la tapa del cursor de reproducción y los bordes del bucle; dime si el tinte de
      la selección y el azul tienen la fuerza justa. Anota si el texto se ve nítido y si algún
      icono se ve movido de su píxel.
- [ ] **Los veinticuatro temas**: en *Ajustes ▸ Apariencia*, a 125 %, elige uno a uno los temas
      redibujados (Cantus Noche y Papel, Hora azul, Luz de día, Peltre, Tiza, Tinta y oro,
      Pergamino, Ultravioleta, Laguna) y los nuevos (Siempreverde Noche y Papel, Burdeos,
      Imprenta) con Neon Tide sonando y el mezclador abajo. En cada uno: ¿se lee todo sin
      esfuerzo (las palabras de los clips, las líneas de compás, los colores de las pistas, el
      solo y la grabación)? ¿Tiene la fuerza justa (la crema de Pergamino, el vino de Burdeos, el
      violeta de Ultravioleta, el gris medio de Peltre)? Si usabas un tema en la beta.13, Cantus
      debe abrir en él, redibujado. Los menús de arriba deben verse enteros (con *Ayuda*) en todos.
- [ ] **La ventana de Delay con una interfaz real**: en una pista con voz o guitarra, *Añadir
      inserción ▾ ▸ Espacio ▸ Delay* y clic en la inserción: se abre su ventana. Toca: cada
      repetición debe encenderse cuando el sonido llega a ella, y la ventana quedarse quieta en
      silencio. Arrastra los ecos a lo ancho (tiempo) y arriba o abajo (realimentación); `Ctrl+Z`
      lo deshace de una vez. Prueba *Al tempo* con una negra y el ping-pong.
- [ ] **El tamaño que sigue a la pantalla**: en una carpeta de datos nueva (o tras borrar
      `%APPDATA%\Cantus\settings.json`, guardando antes una copia), abre Cantus: en una pantalla de
      1920 píxeles con Windows al 100 %, la interfaz debe salir al 125 %; con Windows al 125 %, al
      100 % de Cantus. Dime tu pantalla, la escala de Windows y lo que salió.
- [ ] **La barra superior nueva**: a 1920 × 1080 y 125 %, la barra es una sola fila: el logo y los
      menús sobre el nombre del proyecto, Pat/Canción, el transporte y el bucle, el metrónomo, el
      tempo y el compás, el reloj grande, la rejilla, deshacer, las ventanas; a la derecha los
      medidores, las entradas, el máster, el dispositivo, Ajustes y **Más**. Pulsa Play: el reloj
      corre sin que las cifras bailen; graba un compás (`R`): el reloj se pone rojo. Haz la ventana
      más estrecha poco a poco: los grupos deben pasar a **Más** sin chocar; abre **Más** y prueba
      el metrónomo, una ventana y el zoom desde ahí. Repite con la interfaz en español.
- [ ] **La cabecera del mezclador en la fila de pestañas**: con el mezclador acoplado bajo el
      arreglo, la densidad (Compacta · Ancha · Grande), *Filas*, *Cables*, *Instantáneas* y `+`
      deben estar en la misma fila que las pestañas *Mezclador · Piano roll · Rack*, a su derecha.
      Arrastra una pestaña a otro sitio y de vuelta, haz doble clic en una pestaña (maximiza) y otra
      vez (devuelve), y haz flotar el mezclador (clic derecho en su pestaña): flotando, la cabecera
      vuelve a su propia fila. Nada debe quedar encima de las pestañas.
- [ ] **El brillo de los cables a 60 Hz y los conectores**: con Neon Tide sonando, el mezclador al
      frente, elige el Kick y pulsa **Cables**: en estilo *Flujo* (el predeterminado) una luz corta
      recorre el cable, más rápida cuanto más suena. Debe moverse suave, sin saltos ni parpadeo, y
      quedarse quieta al parar. Pasa el puntero por un cable: se ilumina su ruta hasta el máster.
      Cambia a **Jack** (menú de *Cables* ▸ *Estilo de cables*), arrastra desde la salida de Chords
      hasta el Drum bus y suelta: el cable cuelga, oscila 0,3 s y se queda quieto. Mira a 125 % y
      150 % que los anillos de los conectores y los enchufes se vean nítidos.
- [ ] **El selector de iconos de pista**: clic derecho en el encabezado de una pista ▸ *Icono…*;
      escribe «guitarra», luego «piano», luego «kick»: deben aparecer en español y en inglés.
      Elige uno: sale delante del nombre de la pista y en la cabeza de su tira del mezclador
      (Compacta, Ancha y Grande). Clic en el icono para cambiarlo; *Ninguno* lo quita; `Ctrl+Z` lo
      deshace. Mira las familias (sobre todo las guitarras) a 100 % y 150 %.
- [ ] **El logo #03 en la barra de tareas y en el .exe**: instala la beta.14 y mira el icono del
      instalador, el del acceso directo, el del `.exe` en el Explorador (vista de iconos grandes y
      de lista), el de la barra de tareas con Cantus abierto (con la barra clara y oscura de
      Windows) y la ventana de *Alt+Tab*. A 16 px debe leerse una C dorada con su cruz, sin
      manchas. En *Ajustes ▸ Apariencia*, el color del icono de la barra de tareas sigue pintando la
      marca.
- [ ] **Aula de oído**: en una pista con voz o guitarra, *Añadir inserción ▾ ▸ Espacio ▸ Aula*.
      Escucha las quince respuestas propias (*Studio room* … *Long spring*), sobre todo las placas
      y los dos muelles, y dime cuáles suenan mal o falsas. Carga un archivo de respuesta tuyo
      (*Cargar archivo*) mientras suena: el cambio no debe hacer clic. Prueba *Muestras ▸ Conseguir más ▸ Respuestas al
      impulso de OpenAIR* y la catedral de York. Mira la CPU con una catedral larga a tu buffer de
      siempre.
- [ ] **El estado de descarga de los paquetes con una descarga real**: en una carpeta de datos sin
      paquetes (o tras quitar uno en la ventana), abre *Paquetes de sonidos…*: cada paquete es una
      portada con *Obtener · tamaño*. Pulsa *Obtener* en uno pequeño: debe verse la barra de 4 px,
      «42 % de … MB» que avanza y *Cancelar*; al terminar, *Instalado* con la marca verde y *Abrir*.
      Prueba *Cancelar* a media descarga en otro: vuelve a *Obtener*. Los filtros (*Todos ·
      Instalados* …) y *Orden* deben funcionar. (En la ronda el estado de descarga solo se dibujó
      con un gancho de prueba.)
- [ ] **El explorador**: busca «kick»: los resultados salen agrupados por paquete o carpeta, cinco
      por grupo y «Mostrar los otros N sonidos»; clic en el título de un grupo deja la búsqueda en
      ese sitio (una ficha con ×). Marca un sonido con la estrella: aparece arriba en *Favoritos*,
      también tras cerrar y abrir Cantus. En la tarjeta: reproducir, bucle y nivel; mueve el nivel
      mientras suena: no debe volver a empezar.
- [ ] **El arreglo con un ratón de verdad**: prueba las herramientas de la fila (Seleccionar `E`,
      Dibujar `P`, Cortar `C`, Borrar `D`, Silenciar `T`, Zoom `8`), *Seguir* con la canción
      sonando (la página pasa cuando el cursor sale de la vista), el menú de la rejilla y los
      tresillos (`Shift+3`), la esquina de un clip de notas para repetirlo, crear una sección y una
      carpeta (clic derecho en el carril de secciones; *Meter en una carpeta nueva* en el menú de la
      pista), y maximizar el arreglo (doble clic en su pestaña): abajo queda la fila de pestañas.
- [ ] **El inspector**: elige una tira; abre *Nivel* (el medidor fino bajo el volumen debe moverse
      al sonar y quedarse quieto en silencio), *Automatización ▸ Mostrar en el arreglo* (el arreglo
      se abre en los carriles de esa pista), y congela una tira (*Congelar la tira*): debe decir
      cuántas inserciones se renderizaron y cuánto ocupa.
- [ ] **El teclado**: `Ctrl+F6` pasa de una zona a otra; `Shift+F10` sobre un clip abre su menú y
      las flechas lo recorren; `Esc` cierra lo último que se abrió; `Ctrl+F` sobre una pista abre
      sus presets.

### Ronda 40 — insertar y borrar tiempo, arrastrar una sección, ripple (ADR 0116) <!-- r40:ripple -->

Probado con eventos sintéticos (el arrastre y las teclas los dio la prueba de extremo a extremo);
lo que solo tus manos confirman:

- [ ] **Insertar y borrar tiempo**: en una canción tuya, arrastra en la regla dos compases en
      medio de un clip, clic derecho dentro de la selección, *Insertar tiempo*: todo lo de después se
      mueve dos compases (clips, automatización, marcadores, secciones, tempo). `Ctrl+Supr`
      los quita: la canción debe quedar exactamente como estaba (escúchala: sin cortes).
- [ ] **Arrastrar una sección**: arrastra una sección del carril de secciones sin nada, con
      `Ctrl`, con `Mayús` y con `Ctrl+Mayús`: mira la vista previa (fantasma, franja o línea de
      acento, hueco con trazos, la línea de ayuda) y que al soltar haga lo que dijo. El arrastre
      simple *inserta* (como Logic y Studio One); *Ajustes ▸ Edición ▸ Arreglo ▸ Arrastrar una sección:
      Reemplazar* lo cambia (y `Mayús` hace siempre lo otro). Dime cuál prefieres por defecto.
- [ ] **Ripple**: `Alt+Mayús+P` (o la tecla junto a *Seguir*) a *esta pista*, borra un clip y
      mueve otro: lo que sigue debe moverse con él; a *todas las pistas*, igual en todas.

### Ronda 40 — `look`: tus respuestas sobre el aspecto, en tu pantalla <!-- r40:look -->

Todo se vio solo en el renderizador de CPU; los clics de las pruebas son sintéticos. En la release
integrada de la ronda 40, a 100 % y a 150 %, en Pizarra Noche y Papel:

- [ ] **Pizarra lapislázuli, la C**: elige *Pizarra lapislázuli* en *Ajustes ▸ Apariencia*; el
      espacio debe verse claramente azul (Noche: azul lapislázuli profundo; Papel: un papel azul
      claro), con el texto tan nítido como en Pizarra. Dime si el azul es el que elegiste.
- [ ] **Maximizar**: doble clic en la pestaña del arreglo — el inspector debe quedarse a su lado,
      a su ancho; `I` lo oculta solo a él. En *Ajustes ▸ Apariencia ▸ La ventana ▸ Al maximizar,
      queda: Nada*, maximizar da todo el espacio.
- [ ] **El dock derecho del mezclador**: con Neon Tide, los retornos (Reverb, Delay) en el borde
      derecho de la ventana y, antes, los canales vacíos numerados; plegar la mesa (más estrecha)
      los deja en el borde. *Tras la última tira* los devuelve junto a los canales.
- [ ] **La barra superior**: *Extras de la barra: Ambos* — el armado de la automatización tras el
      bucle (violeta al armarlo; su ▾ con los modos) y la tecla del teclado del ordenador antes de
      las entradas (a 1920 × 1080 y 100 % las entradas, y con ellas esa tecla, pasan a *Más*;
      dime si prefieres que se queden en la barra a costa de otro grupo).
- [ ] **Valores del inspector y Silencio**: elige una tira y abre *Nivel*: volumen, paneo y
      anchura como barras de valor; silencia dos canales: su **M** en el color del acento (en la
      mesa y en la cabecera de la pista). *Barras finas* y *Gris* devuelven lo de la ronda 39.
- [ ] **Menús y diálogos en Papel**: abre *Edición* en la barra y un diálogo (*Archivo ▸ Nuevo*
      con cambios sin guardar): el borde de la hoja más nítido, una sombra pegada al borde, el velo
      detrás más claro. ¿Se ve tan terminado como en Noche?
- [ ] **Burdeos y Peltre más suaves**: elígelos con Neon Tide sonando; ¿la fuerza es la justa y
      se lee todo sin esfuerzo?

### Ronda 40 — Delay al tempo y Delay como plug-in CLAP/VST3 (ADR 0119) <!-- r40:delay -->

Probado con pruebas de bits y renders (el clic en la ventana lo dio la prueba de extremo a
extremo); lo que solo tus oídos, tus manos y otro DAW confirman:

- [ ] **Al tempo, de oído**: en una canción con una rampa en la pista de tempo (p. ej. 120 → 90 en
      cuatro compases) y un cambio brusco después, pon Delay en *Corchea con puntillo* (ya sigue el
      tempo) sobre una guitarra o una voz y escucha la rampa y el cambio: en la rampa las
      repeticiones se deslizan (el tono baja un poco, como una cinta); en el cambio brusco pasan
      con un fundido de 50 ms. Dime si prefieres que la rampa también funda en vez de deslizar.
- [ ] **El interruptor**: *Libre · Al tempo* en la ventana; *Al tempo* elige la figura más cercana,
      *Libre* conserva el tiempo en milisegundos. La perilla grande gira la figura con *Al tempo*.
- [ ] **Cantus Delay en otro DAW**: `cargo run -p xtask -- bundle delay`, copia
      `target/bundled/Cantus Delay.clap` y `.vst3` a las carpetas de plug-ins y ábrelo en REAPER
      (o FL Studio / Bitwig): su ventana, *Al tempo* siguiendo el tempo del DAW (cámbialo con el
      transporte parado y sonando), una figura larga (4/1) sin cortes, guardar y abrir el proyecto.
- [ ] **Los ids** (para siempre): CLAP `com.github.lalmazan.cantus.delay`, VST3
      `625C4F90AB5A380458F1F1D3D1DCA44C` — confírmalos antes de que la ronda se fusione.

### Ronda 40 — el canal propio de un pad, escuchar la clave, grabar antes/después de FX (ADR 0117, 0118) <!-- r40:pads -->

Probado con eventos sintéticos y un motor manejado a mano (entrada empujada por la prueba); lo
que solo tus oídos y tu interfaz confirman:

- [ ] **El canal de un pad**: en el rack, clic en el número de un pad de un kit, *Primer canal
      libre*: el pad debe sonar solo en ese canal (sube y baja su fader, ponle un efecto), y el resto
      del kit en el suyo. *El canal del kit* lo devuelve. Dime si quieres el número de un pad con
      canal propio en color de acento (hoy es la tinta del boceto).
- [ ] **Escuchar la clave**: un compresor con clave desde el bombo, *Cadena lateral ▸ Escuchar la
      clave*: debes oír el bombo como lo oye el detector (sube el filtro de la clave y escúchalo
      cambiar); otro clic vuelve a la pista. Exporta mientras está encendido: el archivo no lo lleva.
- [ ] **Grabar después de FX**: con tu interfaz en dúplex (WASAPI y ASIO), una pista con un EQ o
      una saturación, *Grabación ▸ Se graba: Con FX*, escucha en *Auto*: graba unos compases y
      compara la toma con lo que oíste (en estéreo, con el efecto), y que caiga a tiempo sobre la
      rejilla; con un compresor con anticipación también. *Sin FX* debe dar la toma limpia.

### En reposo (ronda 38, ADR 0106) <!-- r38:scorecard -->

Probado en esta máquina con la interfaz del portátil; lo que solo un aparato confirma:

- [ ] **Enchufar y desenchufar sin sondeo**: con Cantus abierto en un proyecto vacío y sin tocar
      nada, conecta tu UMC por USB: la tarjeta *¿Cambiar a UMC204HD?* debe aparecer en menos de
      un segundo (Cantus ya no lista los dispositivos cada 1,5 s; espera a que Windows avise). Si
      tarda unos 10 s, Windows no avisó en tu equipo y Cantus usó la revisión de seguridad:
      dímelo. Desenchúfala mientras suena algo: el aviso de *interfaz perdida* igual de rápido.
- [ ] **Un puerto MIDI de reloj elegido en reposo**: *Ajustes ▸ MIDI*, marca *Reloj* en un
      puerto con el transporte parado y luego pulsa Play: el aparato debe seguir desde el primer
      compás (el hilo de salida, que ahora duerme 20 ms sin puertos, abre el puerto a tiempo).
- [x] **El reposo contra REAPER, con la máquina quieta** (2026-11-09, con todo cerrado y WSL
      apagado): Cantus vacío 1,6 % de un núcleo contra REAPER 2,3 % (ticks) y 3,7 % (ciclos);
      Cantus *Beat* 1,8–2,0 % contra FL Studio 6,6 %. Ganada; `docs/scorecard/README.md`.
- [ ] **El bailarín en reposo**: Cantus ya abre sin bailarín (tu decisión). Añade uno (el
      selector de instrumentos) y no toques nada: respira unos 10 s y se queda quieto; mueve el
      ratón sobre la ventana y vuelve a respirar desde la misma postura, sin saltos.

### Equipo externo y superficies (ronda 36, ADR 0094 y 0095) <!-- r36:external -->

Necesita tu hardware; en la ronda se probó todo con una interfaz virtual y con unidades Mackie y HUI emuladas.

- [ ] **Ping de un sintetizador**: crea un instrumento externo en una pista, elige su puerto MIDI y las entradas donde vuelve su sonido, pulsa *Ping* con el transporte parado. Debe decir unos milisegundos plausibles (el tamaño del búfer más lo del sintetizador) con variación 0-2 muestras; pulsa *Medir de nuevo* tres veces: el número no debe moverse más de 2 ms.
- [ ] **A tiempo**: graba/toca una nota de batería de Cantus y la misma nota del sintetizador en el mismo golpe: deben sonar juntas (a ojo y en el analizador) al reproducir y en el WAV exportado.
- [ ] **Ping de un efecto**: un cable de una salida a una entrada de tu interfaz (loopback), *Efecto externo* con ese par: debe medir lo mismo que *Ajustes ▸ Audio ▸ latencia de ida y vuelta* (±1 muestra). Con el cable fuera dice *no volvió nada*.
- [ ] **Interfaz desenchufada**: con el sintetizador/interfaz del proyecto apagados, la unidad dice *hay otra interfaz abierta* o *no está el puerto* y *Ping* no se ofrece. Nunca cargues un driver ASIO de una interfaz desconectada.
- [ ] **Mackie Control** (o una compatible: X-Touch, Icon, FaderPort en modo MCU): añade la superficie, mueve un fader (el canal cambia y un solo deshacer lo devuelve), toca un fader mientras cambias el volumen con el ratón (el motor no pelea), banco y canal, mute/solo/rec/select con sus luces, V-pot, jog, transporte, las pantallas de nombre y los medidores. Con una extensión, dieciséis canales.
- [ ] **HUI**: si tienes una, lo mismo con sus ocho canales y el transporte. Abre el registro bajo la lista: cualquier interruptor que diga *unmapped HUI zone … port …* mándalo y se añade a la tabla (la tabla de zonas y puertos se tomó de las notas públicas del protocolo y solo se probó con la emulación).

### Memoria (ronda 40, `memory`, ADR 0120) <!-- r40:memory -->

- [ ] **La fila de memoria, con la máquina quieta**: el integrador mide la versión final con
      `tools/scorecard/startup.ps1` (vacío y *Beat*), sin nada más corriendo. En el mapa de la
      ronda, con otras compilaciones activas, bajó de unos 138–140 MB a unos 122–124 MB; el
      suelo del controlador OpenGL de AMD (una ventana vacía de egui) es de unos 105 MB en tu
      portátil. No se dice que ganemos a REAPER ni a FL hasta esa medición.
- [ ] **Exemplar sin espera**: abre el selector de instrumentos y elige Exemplar enseguida:
      debe aparecer sin un tirón (sus sonidos internos se calculan al abrir el selector, ya no
      en cada arranque). Si notas una pausa al elegirlo, dímelo.
- [ ] **Cambiar de idioma**: *Ajustes ▸ Idioma* de inglés a español y al revés: toda la
      ventana cambia en el acto (el otro idioma se lee cuando se elige).

### Ronda 40 — lo que espera tu oído, y lo que solo tu equipo confirma <!-- r40:docs -->

La ronda 40 está en `r40/integration`. Por tu decisión del 2026-11-14 (*"Add Tympanum at once.
Ship … with Chorda's newest version"*), **el candidato 16 ya es el sonido por defecto de Chorda**
(`DSP_VERSION` 7) y **Tympanum ya está en el selector de instrumentos** (junto a Pulsus, en
*Batería*); tu escucha y tu ABX de los dos siguen pendientes (`docs/BACKLOG-excellence.md`), y
la beta.15 espera tu OK. Todo lo de la ronda se dibujó con el renderizador de CPU, se tocó con
eventos sintéticos y se midió sin oído; las pruebas de cada constructor están arriba
(*Ronda 40 — …*) y estas son las que faltaban.

- [ ] **Chorda, la ABX del candidato 16**: https://claude.ai/artifact/Jpwa91Tz4V8rbcRrgiXMKg
      (para quien esté fuera de tu organización, el zip `compartir\cantus-chorda-abx16.zip`; cada
      quien te manda su JSON y lo importas en el resumen de la página). Pásala a tus conocidos.
      Reemplaza a la del 14 (su página sigue arriba, sin usarse): el 14 aún tenía el acorde
      desafinado de tu queja (la quinta Re3–La3 del último acorde suave batía a 4.46 Hz; en el 16,
      1.18 contra 0.68 del Salamander). El 16 ya es el sonido por defecto; lo que digas decide
      la próxima pasada de voz (o que Chorda está terminado).
- [ ] **Chorda, el candidato 16 de cerca** (`compartir\chorda-preview-16\`): cumple tu barra en
      medidas — toda tecla a menos de 10 cents (la peor, Do1, +9.7), el Do central a 2.26 kHz
      +0.3 dB, ninguna tecla La0–Do5 con flanger, la octava de arriba sin cambio, los batidos del
      grave en dos etapas 0.50, el color de arriba 1.48 — pero nadie lo ha oído. Lo que costó: la
      escala rápida con −4.6 dB de presencia, el acorde ff con +2.6 dB.
- [ ] **Tympanum b10** (`compartir\tympanum-b10\`, lee `NOTA.md`): las quince pistas del
      análisis medidas, el ataque de la caja ya no llega tarde, las colas de los platos sin el
      «iuuu». Di qué pieza sigue sonando menos real y por qué.
- [ ] **Tympanum b11** (`compartir\tympanum-b11\`, lee `NOTA.md`; b10 · b11 · grabación): el
      costo bajó de 18.7 a 7.3 % de un núcleo con todos los micrófonos sin cambiar el sonido;
      pasa 71 de 162 pistas medidas (b10 57, b9 21): los toms sin ruido grave bajo su
      fundamental, el tono del bombo de estudio, la bordona de la caja de estudio. Siguen
      fallando en medidas: la correlación L/R del groove, el subgrave de los toms fuertes, los
      picos de los primeros 50 ms, los agudos y graves del groove de jazz, el bombo de jazz y el
      ride.
- [x] **Los paquetes híbridos versión 3** (`tympanum-hybrid-jazz-3.zip`, `…-studio-3.zip`)
      subidos a `content-r40` con tu OK (2026-11-14); la descarga pública verificada y la
      prueba de red pasa contra ellos. Son los que baja la app.
- [ ] **El kit modelado por defecto**: *Studio Maple* o *Jazz Birch*.
- [ ] **El híbrido de oído**: `compartir\tympanum-hibrido-1\` y las escuchas de las versiones 2
      y 3 (`F:\projects\daw-r40\tmp\hyb\v2\listen-jazz\` y `listen-studio\`; `…\v3\listen-jazz\`
      y `listen-studio\`): generada · híbrida · grabación a la misma sonoridad. ¿El híbrido vale
      como opción?
- [ ] **El híbrido descargado desde la app**: Tympanum con un kit modelado, *Voz: Híbrida*,
      *Bajar* — la barra de progreso, la licencia (CC0 el de jazz, CC BY 4.0 el de estudio), y
      al terminar se oyen los ataques grabados; sin internet sigue sonando. Antes de tenerlo,
      *Híbrida* suena igual que *Generada*.
- [ ] **Los pianos grabados descargados desde la app**: en Chorda › *Fuente*, *Obtener* el
      Salamander y una imagen de bitKlavier: la barra, *Cancelar* a la mitad (no debe quedar
      nada en disco), otra vez hasta el final; *Gestionar…* muestra lo que pesa cada uno (se
      guardan como FLAC). Probado aquí solo con descargas hechas por los constructores, nunca
      desde la ventana de la app.
- [ ] **Imágenes de instrumentos *Fotográficas* en tu GPU y tu pantalla**, a 100 % y 150 %:
      *Ajustes ▸ Apariencia ▸ Imágenes de instrumentos: Fotográficas*; abre Chorda y Tympanum,
      cambia el acabado, la tapa y la sala: anota cuánto tarda cada imagen en aparecer completa
      (mientras arrastras un control se ve una versión rápida), que el audio no corte a 64
      muestras mientras se dibuja, y que el uso de CPU vuelva al de reposo unos segundos
      después. *Dibujadas* (por defecto) debe verse casi igual de bien. Medido aquí solo en un
      proceso de ejemplo con la máquina ocupada (Chorda 1–1,7 s, Tympanum 0,7–1,3 s a 1,5×).
- [ ] **Las bandas del mezclador**: selecciona tres canales, *Nuevo grupo con los
      seleccionados*: una banda sobre ellos; pliégala (un canal con sus medidores lado a lado) y
      despliégala; arrastra su borde derecho para tomar el canal siguiente (`Esc` lo deja como
      estaba). Con Tympanum, *Cada micro a su canal*: una banda de familia (*Batería ·
      Tympanum*) con sus subgrupos (Bombo, Caja, Toms, Platos…); un bus alimentado solo por ella,
      menú ▸ *Añadir a la banda*. Desplázate: el nombre de la banda sigue visible.
- [ ] **Tympanum en tu interfaz a búfer bajo** (ASIO y WASAPI en dúplex, 64 y 128 muestras):
      *Studio Maple* con *Cada micro a su canal*, y DRSKit (13 micrófonos, en streaming), un
      groove desde tu controlador un minuto con el pedal del charles: sin chasquidos; anota el
      medidor de CPU de Cantus (aquí, con otras compilaciones corriendo, el modelo solo marcó
      10,8 % de un núcleo y el kit con todos los micros 18,6 %). Lo mismo con Chorda grabado
      (el Salamander) a 64 muestras.
- [ ] **La prueba del sandbox que se cuelga, en una máquina tranquila**
      (`cargo test -p cantus-sandbox --test session a_hang_never_blocks_the_interfaces_frames_and_is_ended`):
      en WSL falló una vez bajo carga y pasó 5 de 5 sola; en Windows pasa con la máquina
      tranquila. Con tu OK para gastar minutos de Actions, la corro 20 veces en un runner de
      GitHub (Linux y Windows) para confirmar que solo falla bajo carga.

### Cedo, el atenuador (ronda lateral de efectos, ADR 0141) <!-- fxb:cedo -->

En la ronda se probó todo con el motor de Cantus y con nuestro propio anfitrión CLAP/VST3; falta tu oído y un DAW de verdad.

- [ ] **Un bajo atenuado por un bombo real**: pon Cedo en la tira del bajo, *Disparo ▸ Cadena lateral*, elige la tira del bombo en *Clave* y el preset *El bombo atenúa el bajo*. El bajo debe ceder en cada golpe sin chasquidos, y nada antes del primer golpe.
- [ ] **El bombeo por tempo**: con el bombo silenciado, *Bombeo clásico*: el bombeo debe caer en cada negra; exporta el WAV y compáralo con la reproducción (deben sonar igual).
- [ ] **Una guitarra bajo una locución real**: graba o carga una voz en otra pista y usa *Locución* y *Fondo musical de pódcast* en la guitarra: la voz debe quedar clara y la guitarra volver despacio, sin bombeo.
- [ ] **Solo los graves en fase lineal**: *Bombeo de graves en fase lineal* y arrastra el cruce: la latencia (la bandera *Latencia*) no debe cambiar hasta soltar; al soltar, la pista sigue alineada con las demás. Prueba los suelos fijos 40/60/80/120 Hz y di cuál prefieres por defecto (hoy *Automático*).
- [ ] **En otro DAW** (Reaper, Bitwig o FL): carga *Cantus Cedo*, enruta un bombo a su entrada de cadena lateral y elige *La entrada de cadena lateral del complemento* en *Clave*; luego el disparo MIDI con una pista de notas; y el tempo del DAW. Al cambiar el suelo de la fase lineal, el DAW debe recompensar la latencia.

### Issue #1 — la ventana que parpadea (ADR 0146) <!-- fix/flicker -->

La causa encontrada no depende de la GPU: la escala que sigue a la pantalla (ADR 0111 §7) se
medía con el zoom que ella misma ponía y alternaba entre dos tamaños cada dos fotogramas en
la mayoría de las pantallas (no en la tuya, 1920 × 1080 al 125 %, que es de las pocas que
se quedan quietas). Corregido y probado solo en **emulación** (`tests/window_stability.rs`
alimenta los fotogramas como egui-winit); esto es lo que solo una pantalla real confirma.
Ya comprobado en tu portátil (2026-11-17, compilación de la rama, carpeta de datos vacía, tu
125 %): la ventana abrió en la **Radeon integrada** (`opengl renderer: AMD Radeon(TM) Graphics`,
controlador `22.40.01.54.230214` — de febrero de 2023, conviene actualizarlo), una sola
decisión de escala y ningún cambio más en 20 s, con y sin `--safe-graphics`.
Antes de cada paso borra `settings.json` de la carpeta de datos de una copia portátil (o usa
`CANTUS_DATA_DIR` apuntando a una carpeta vacía) para que sea una instalación nueva.

- [ ] **Reproducir el fallo en beta.14 en tu portátil**: *Configuración ▸ Sistema ▸
      Pantalla ▸ Escala* al **100 %** (1920 px al 100 % alterna 1.0 ↔ 1.25), abre la beta.14
      portátil recién descomprimida: debe parpadear en ambos ejes como dice el issue. Pulsa
      Ctrl + 0: debe parar (el arreglo provisional para el usuario). Vuelve a tu 125 %.
- [ ] **La versión corregida, en la misma pantalla al 100 %, 125 %, 150 % y 175 %**: con una
      copia portátil nueva de esta rama en cada escala, abre Cantus y mira 10 s sin tocar
      nada: ni un salto. `cantus.log` (en `data\logs\`) no debe tener ninguna línea
      `window watchdog`. Con `$env:CANTUS_WINDOW_TRACE = "1"`, el registro muestra *una*
      línea `the scale … for the screen` y ninguna más mientras no toques nada.
- [ ] **En la GPU integrada AMD de tu portátil** (tienes una Radeon junto a la RTX 4060):
      *Configuración ▸ Sistema ▸ Pantalla ▸ Gráficos* → `cantus-x86-64-v3.exe` y
      `cantus-x86-64-v2.exe` en *Ahorro de energía* (la Radeon); repite el paso anterior al
      100 % y al 150 %. Las primeras líneas de `cantus.log` deben nombrar el renderizador AMD
      (`opengl renderer: AMD Radeon(TM) Graphics`). Prueba también maximizar/restaurar y
      arrastrar la ventana a otro monitor si tienes uno con otra escala.
- [ ] **Cada opción de gráficos seguros arranca bien** en la Radeon y en la NVIDIA:
      `.\cantus.exe --safe-graphics`, `--no-splash`, `--vsync off`, `--msaa 4`, `--scale 150`,
      `--no-splash --window 1600 900`, y `$env:CANTUS_GRAPHICS = "safe"`. La primera línea de
      `graphics:` del registro debe decir lo pedido.
- [ ] **Si el usuario del issue contesta**: con la versión corregida y su pantalla, ¿para?
      Si no, que envíe `cantus.log` con `CANTUS_WINDOW_TRACE=1` (diez segundos), su resolución
      y escala, cuántos monitores y la versión de Adrenalin, y qué opción de
      `docs/troubleshooting-graphics.md` lo detiene. Solo él puede confirmar su Radeon 800M,
      su 25H2 y su panel (probablemente 2880 × 1800, que alternaba en *todas* las escalas).

### Issue #1 (2) — menús a 200 %: submenús del mezclador y presets de instrumento (ADR 0146, enmienda 1) <!-- fix/flicker -->

El usuario volvió a escribir: a 2880 × 1800 con la escala de Windows al **200 %**, los
submenús del menú contextual de una pista del mezclador se abrían "muy arriba", y la lista
de presets de un instrumento se abría **transparente y lejos** de su botón (también al
100 %). Corregido y probado solo en **emulación** (`tests/menus_at_scale.rs` alimenta los
fotogramas como egui-winit a 100, 125, 150 y 200 %); esto es lo que solo una pantalla real
confirma. Si tu portátil no llega a 200 % con 1920 × 1080, usa *Configuración ▸ Sistema ▸
Pantalla ▸ Escala* → *Escala personalizada* 200 (cierra sesión), o un monitor 4K al 200 %.

- [ ] **Submenús del mezclador a 200 %**: abre el mezclador, clic derecho sobre el número o
      el nombre de una pista. Pasa el ratón por *Color*, *Añadir efecto*, y baja con la
      rueda hasta *Salida*, *Enviar a*, *Ducking*, *Dock* y *Grupo*: cada submenú se abre
      **al lado** de su fila (a la derecha del menú, o a la izquierda si no cabe), a su
      altura — sube solo lo justo para no salirse por abajo —, entero (sin barra de
      desplazamiento si cabe en la ventana), y nunca muy por encima de la fila.
- [ ] **Lo mismo en *Adónde va…* (Mayús+F10)** y su submenú *Nuevo retorno ▸*.
- [ ] **Presets de un instrumento, a 100 % y 200 %**: abre el editor de un instrumento
      (Unda, Chorda, Tympanum…), clic en el pozo del preset (el nombre en el centro de la
      cabecera): la lista se abre **justo debajo** del pozo, empezando en su borde
      izquierdo, **opaca** (no se ve el editor a través), y con rueda si es más larga que
      la ventana. El menú ⋯ y el de tamaños terminan en el borde derecho de su botón. En
      Chorda y Tympanum, el campo *Mezcla* de su mezclador abre la lista bajo él.
- [ ] **Otros temas**: repite el paso anterior con un tema que no sea de la familia Slate
      (p. ej. *Classic*) y con un tema claro: la lista sigue opaca y en su sitio.
- [ ] **Si el usuario del issue contesta**: con la versión corregida y su pantalla, ¿se abren
      en su sitio? Si no, una captura de pantalla con el menú abierto y su escala.

### La selección a un bus nuevo, en un paso (ADR 0147) <!-- fix/flicker -->

Probado en emulación (`tests/bus_from_selection_e2e.rs`, con el motor real); esto es lo que
confirma tu oído y tu mano:

- [ ] **Desde el mezclador**: clic en el número de una pista, Ctrl+clic en otras dos (o
      Mayús+clic para un tramo), **Ctrl+G**: aparece *Bus 1* justo después de la última, las
      tres salen por él (los cables lo muestran), sus pistas en una carpeta *Bus 1* del
      arreglo, suena igual que antes; silencia el bus y callan las tres. Ctrl+Z lo deshace
      en un paso.
- [ ] **Desde el menú de una pista del mezclador**: *Enrutar la selección a un bus nuevo*
      (muestra Ctrl+G). Con pistas de un mismo grupo, el bus se llama *Bus ‹grupo›* y queda en
      el grupo; si ya iban a otro bus, el nuevo va a ese bus (anidado), no al máster.
- [ ] **Desde el arreglo**: clic en la cabecera de una pista, Ctrl+clic en otra (las dos se
      iluminan), clic derecho → *Enrutar la selección a un bus nuevo*, o Ctrl+G. Un clic
      normal vuelve a seleccionar una sola.
- [ ] **La paleta de comandos** (Ctrl+Mayús+P) encuentra *Route the selected to a
      new bus* / *Enrutar la selección a un bus nuevo*.
- [ ] **La carpeta (ADR 0147 §5, tu respuesta)**: tras Ctrl+G, en el arreglo aparece una
      carpeta llamada como el bus (*Bus 1*) con las pistas que van a él, y ninguna otra; si
      alguna ya estaba en una carpeta, pasa a la nueva (las carpetas no se anidan) y una
      carpeta que se queda vacía desaparece. Un solo Ctrl+Z devuelve rutas y carpetas.
      Desde el mezclador, un canal sin pista (un retorno, otro bus) se enruta pero no tiene
      pista que meter en la carpeta.

### Varias pantallas — ventanas propias (ADR 0148) <!-- fx/multimon -->

Verificado solo en emulación (las pruebas alimentan las pantallas como las da winit y corren
las pasadas de cada ventana como eframe): nada de esto se ha visto en pantallas reales.

- [ ] **Portátil + pantalla externa** (idealmente 200 % + 100 %, como el informante del
  issue #1): clic derecho en la pestaña *Mezclador* › *Mover a una ventana nueva*. La ventana
  debe abrir **maximizada en la otra pantalla**, sin aparecer antes en la equivocada ni
  saltar; el texto nítido en las dos (cada una a su escala); su barra con transporte,
  posición, tempo y medidor.
- [ ] **Teclas desde la ventana del mezclador**: Espacio reproduce/para, M silencia la
  franja seleccionada, Ctrl+Z la deshace, F9 la oculta y F9 la devuelve a su ventana; sin
  tener que hacer clic primero.
- [ ] **Desconectar la pantalla externa con Cantus abierto y la canción sonando**: el
  mezclador vuelve a la ventana principal, la barra de estado lo dice, la música no se
  corta. Volver a conectarla: la ventana regresa a su sitio (tarda hasta 3 s).
- [ ] **Arrancar con la pantalla desconectada y luego con ella conectada**: igual que
  arriba; nunca una ventana fuera de la vista.
- [ ] **Suspender y reanudar el portátil** con la ventana en la externa.
- [ ] **F11** en cada ventana: pantalla completa en su propia pantalla y vuelta.
- [ ] **Alt+1 … Alt+4** (Ver › disposiciones): *Pantalla ancha* (solo en 21:9), *Dos
  pantallas: arreglo | mezcla*, *Dos pantallas: arreglo | edición*, *Tres pantallas* (solo
  aparecen las que caben en tus pantallas); *Restablecer disposición* vuelve a una ventana.
  Shift+F11 manda el área en la que trabajas a una ventana nueva; Ctrl+F11 abre
  *Pantallas y ventanas*.
  Con un teclado español, AltGr+2 sigue escribiendo @ en un campo de texto.
- [ ] **Un plug-in real abierto desde la ventana del mezclador** (un VST3 o CLAP con
  editor propio, p. ej. uno de FabFilter o Vital): su editor debe aparecer **en la pantalla
  de esa ventana**, cerca de su esquina, y quedar **encima de ella** al hacer clic en la
  ventana del mezclador. Luego cierra la ventana del mezclador (o *Volver a la ventana
  principal*, o desconecta su pantalla): **el editor debe seguir abierto** (ahora encima de
  la ventana principal) y responder; Cantus no debe cerrarse. Repite con un plug-in aislado
  (en su proceso aparte) y con un editor ARA.
- [ ] **La barra de una ventana**: ⋯ › *Elegir qué muestra la barra…* (activa el
  metrónomo), ⋯ › *Cambiar el nombre de la ventana…* («Batería»): el título de la ventana en
  la barra de tareas cambia; al reabrir Cantus siguen igual.
- [ ] **Coste**: con el Administrador de tareas, memoria privada y CPU de Cantus en reposo
  con una y con dos ventanas (en reposo ninguna debe dibujar: 0 % de GPU).
- [ ] **Linux (X11) y macOS**: mover el mezclador a otra pantalla y volver.

## 2. Decisiones y trámites tuyos

- [ ] **Firma de código** (sin ella Windows muestra "Windows protegió tu PC"):
  - **SignPath Foundation** (gratis). Nuestras licencias sirven (GPL y MPL están aprobadas
    por la OSI), pero sus condiciones piden: repositorio y descargas públicas; **ninguna
    licencia comercial dual** (si algún día vendes una licencia comercial de Cantus, se
    pierde); **ningún componente propietario** en lo firmado (ojo: el texto de la RVR1960
    no es libre — ver Biblia abajo); una política de firma publicada en la web; el editor
    que verá la gente es "SignPath Foundation", no tu nombre.
  - **Certum Open Source** (≈ 30–70 €/año): sale tu nombre, sin esas condiciones.
  - Elegido: ______ · Nombre del editor (`CANTUS_PUBLISHER`, no cambiarlo después): ______
- [ ] **Biblia RVR1960** — *correo enviado; sin respuesta al 2026-10-16. La beta sale con la
      RV1909 mientras tanto.* Los términos de la American Bible Society permiten hasta 500
      versículos sin permiso en obras no comerciales, **pero piden permiso por escrito para
      "aplicaciones y otros medios nuevos, como sitios web"** — Cantus es una aplicación.
      Escribir a **licensing@americanbible.org** (o su formulario) pidiendo permiso para
      ≤ 300 versículos en una app gratuita, con el aviso de copyright. **El correo ya está
      redactado** en [`rvr1960-request.md`](rvr1960-request.md) (inglés y español), con la
      lista para adjuntar en [`rvr1960-verses.txt`](rvr1960-verses.txt). Mientras no llegue:
      la beta sale con la **RV1909** (dominio público) — **ya está hecho**: la app no lleva
      dentro el texto de la RVR1960 hasta que se active `RVR1960_PERMITTED`.
- [ ] **Marca "Cantus"**: ver la búsqueda preliminar abajo; pagar una búsqueda profesional
      y registrar (IMPI en México primero; luego Protocolo de Madrid si conviene).
- [ ] **Revisión legal del CLA** (`CLA.md`) antes de aceptar el primer aporte de otra
      persona.
- [ ] **Revisión legal de los modelos de IA** (ronda 34, ADR 0085–0086): Basic Pitch de
      Spotify (Apache-2.0) y nuestra adaptación a Rust de su creación de notas (su NOTICE va
      en `THIRD-PARTY-NOTICES.md`); la conversión a ONNX, hecha por un tercero, del modelo de
      batería de Magenta (Apache-2.0 según su ficha; Magenta publica el checkpoint en su
      repositorio Apache-2.0 sin una línea de licencia aparte); HT-Demucs de Meta (el
      repositorio es MIT; no hay licencia aparte para los pesos); y las condiciones de
      Microsoft para redistribuir las DLL de Visual C++ junto a `cantus-ml.exe`.
- [ ] **Revisión legal de OpenH264** (ronda 35, ADR 0088): en Linux y en Windows N, Cantus
      ofrece descargar el binario de Cisco desde `ciscobinary.openh264.org` (hash fijado, la
      licencia de Cisco a la vista antes, nunca incluido en Cantus); Cisco paga las regalías
      de H.264 solo por su binario descargado así. Confirmar que el flujo cumple sus
      condiciones.
- [ ] **Lectura legal de Pizarra junto a FL Studio** (ronda 39, ADR 0111): preguntaste si
      parecerse a FL podía traer problemas. Mi lectura es de riesgo bajo: los valores son otros
      (el fondo de FL es `#253035` con paneles `#34444E`; Pizarra `#1D2329` y `#262D34`), el
      acento es azul donde FL usa naranja y lima, y los iconos, la letra, el logo y el nombre son
      de Cantus, dibujados desde una hoja en blanco. La opinión es de un abogado, antes de hacer
      público el aspecto en más sitios.
- [ ] **La licencia del paquete de OpenAIR** (ronda 39, ADR 0112): el aviso CC BY 4.0 de sus
      páginas se leyó en la copia del Internet Archive de junio de 2024 (el certificado del sitio
      había caducado; no se saltó nada). Confírmalo en el sitio cuando vuelva, antes de anunciar
      el paquete.
- [ ] **Firmar `cantus-mcp.exe` y `cantus-ml.exe`** con el resto de los programas cuando
      haya firma de código.
- [ ] **Revisión legal de Chorda** frente a las patentes de modelado de piano que nombra el
      ADR 0059 (Modartt US 7,915,515; Casio US 8,178,772; Yamaha US 10,818,278 y US
      8,729,376; Kawai US 9,330,648; Casio US 10,109,268 y 11,881,196; Yamaha US
      12,039,960): la estructura está pensada para evitarlas, pero la opinión es de un
      abogado, antes de publicar.
- [ ] (Opcional) **Naked Drums**: escribir a Wilkinson Audio pidiendo por escrito una
      licencia CC BY o CC0 para su batería (la CC BY 4.0 de su versión SFZ la puso quien la
      convirtió, no el autor). Si la dan, entra como segunda batería por el mismo camino que
      DRSKit (ADR 0042, enmienda de la ronda 28).
- [ ] **`F:\projects\daw-r28\downloads`** (3,1 GB): el zip de DRSKit (2,7 GB, para la prueba
      con el archivo real, `CANTUS_DRSKIT_ARCHIVE`), la mezcla instalada `drsmix` (469 MB,
      `CANTUS_DRSKIT`) y los artículos leídos para Chorda (15 MB): conservar o borrar.
- [ ] **Ronda 40 — tus decisiones** <!-- r40:docs --> (las demás están en su prueba, arriba: los
      ids de Cantus Delay, el número del pad con canal propio, la tecla del teclado del
      ordenador en *Más*, los azules del reparto sobre croma, las hojas de Papel, la sala por
      defecto de un piano grabado, el piano grabado por defecto):
  - el *Cueing* de Compás (una postura añadida a un personaje aprobado:
    `docs/round40/fix-d/cast-compas-moods.png`, última columna): ¿se queda?;
  - el kit por defecto de Tympanum (hoy *Studio Maple*; ya está en el selector desde el
    2026-11-14) y su prueba a ciegas (un kit como el de Chorda, en el backlog);
  - *Fotográficas* como modo por defecto algún día (hoy *Dibujadas*; es una sola constante);
  - **los correos para pedir licencias CC BY por escrito** a Headroom (Nilsson), Ivy Audio y
    Wilkinson Audio: están redactados para que los mandes tú (pídemelos). Con una respuesta
    positiva, su piano o su batería entra por el modo *Paquete* (ADR 0042, enmienda de la
    ronda 40).
- [ ] **Hacer público el repositorio** (descargas, auto-actualización y SignPath lo
      necesitan): antes, sacar del historial las capturas de otros productos
      (`research/*/screenshots/`) y revisar que no haya secretos. Si para entonces no ha
      llegado el permiso de la RVR1960, sacar también `crates/cantus-ui/locales/verses/es.txt`
      (del repositorio y del historial): publicarlo en GitHub también requiere permiso.

## 3. Lo que hace Claude cuando lo pidas

- [x] La limpieza del historial en una copia aparte, para revisarla antes de publicar:
      hecha en una copia (ronda 26; `check.sh` dice READY), con las dos rutas escritas en
      [`round26/public.md`](round26/public.md). Publicarla sigue siendo decisión tuya.
- [ ] La solicitud a SignPath y la página de política de firma en la web (si eliges SignPath).
      **Borradores listos** en [`signing/`](signing/README.md) (ronda 26): ahí está qué
      condiciones fallan hoy y la decisión pendiente sobre el CLA.
- [x] RV1909 como predeterminada mientras no haya permiso de la RVR1960 (hecho 2026-10-11).
- [x] Las notas de la versión (`docs/releases/v0.1.0-beta.1.md`), al día con las rondas 23–25,
      en inglés y español (hecho 2026-10-13, ronda 26); y con las rondas 26–28, con sus
      limitaciones conocidas (2026-10-15, ronda 28).
- [x] El botón de la web que descargue el instalador directo (no la página de releases):
      cada versión publica también `Cantus-windows-x64-setup.exe` y la web enlaza
      `…/releases/latest/download/` (hecho 2026-10-13, ronda 26; funciona cuando el
      repositorio sea público).
- [ ] La etiqueta `v0.1.0-beta.1` cuando todo lo de arriba esté listo.

## 4. Marca "Cantus" — búsqueda preliminar

*Búsqueda preliminar del 2026-10-11 (no es asesoría legal): consultados USPTO (EE. UU.),
TMview (registro de la UE/EUIPO, oficinas nacionales europeas, Madrid, México y otros) e IMPI
(MARCia). No se pudo consultar WIPO Global Brand Database (captcha); su contenido está casi
todo en TMview.*

| Dónde | Riesgo | Por qué |
|---|---|---|
| **México** | **Bajo** | Ninguna marca CANTUS viva o en trámite en las clases 9, 42 o 41 (sí en cerveza, restaurantes y agroquímicos, que no chocan). |
| **EE. UU.** | Bajo–moderado | Ninguna CANTUS viva en la clase 9. Posibles objeciones: AURUM CANTUS (bocinas), CANTO (software de gestión de archivos digitales, otro giro) y una solicitud de 2026 para un SaaS de citas. |
| **Unión Europea** | **Moderado** | Una marca italiana CANTUS para "software" (Cianciulli/ACCA Software) vigente hasta ~2029, al parecer sin uso: podría oponerse; con más de 5 años registrada se le puede exigir prueba de uso. También CANTO, Inc. con una marca amplia. |
| Corea, China | Bloqueado / riesgo | CANTUS registrada para software en Corea; CANTUS (clase 42) y AURUM CANTUS en China. Solo importa si algún día se registra allí. |
| En el mercado | A cuidar | **"Cantus – Gregorian Chants"** de Best Service (librería de coros para Kontakt, desde ~2013, sin registro): no llamar "Cantus" a secas a un paquete de coros o canto gregoriano en cantusdaw.studio. |

"Cantus" (latín: canto) se considera **sugerente, no descriptivo** para un DAW: registrable,
aunque será una marca de protección estrecha porque hay muchos "cantus" en la música.

**Ruta recomendada:**
- [ ] Registrar la **marca nominativa CANTUS** (solo la palabra; "CANTUS DAW" no aporta) en
      el **IMPI, en línea**, clase 9 (y 42 si se quiere ofrecer algo en línea). Productos:
      *software descargable para grabar, editar, mezclar, masterizar y producir audio y
      música; plug-ins de audio descargables; archivos de muestras de audio descargables*.
      Costo oficial ≈ **MXN 2,814 por clase** (confirmar en el sitio del IMPI antes de pagar).
- [ ] **Dentro de 6 meses**, solicitud internacional por el **Protocolo de Madrid** con la
      prioridad de la mexicana, designando EE. UU. y la UE (opcional Reino Unido y Canadá).
      Oficiales ≈ CHF 653 base + CHF 460 (EE. UU.) + CHF 789 (UE). Total aproximado para la
      clase 9: **unos US$2,500** en tasas oficiales, sin honorarios.
- [ ] **Antes de designar la UE**: búsqueda profesional y opinión de un agente; revisar si
      las marcas italianas se usan; si sí, pedir a ACCA una carta de coexistencia.
- [ ] Ojo: durante 5 años el registro de Madrid depende del mexicano.

### ¿Y si registro "CANTUSDAW" y le sigo diciendo "Cantus"? (pregunta del 2026-10-11)

**No conviene.** Cuesta lo mismo (se paga por clase, no por nombre), no evita los choques y
protege menos:

- **No esquiva a nadie.** Al comparar marcas se mira la parte que distingue; "DAW" es
  descriptivo (estación de trabajo de audio digital) y se ignora o se "descarta". CANTUSDAW
  se compara como CANTUS: la marca italiana para software sigue siendo el mismo obstáculo en
  la UE.
- **Protege solo "CANTUSDAW".** Otro podría lanzar "Cantus Audio" o "Cantus Studio" y sería
  más difícil detenerlo con un registro de CANTUSDAW.
- **Hay que usar lo que se registra.** En México, a los 3 años se declara el uso de la marca
  tal como se registró; usar siempre "Cantus" con un registro de "CANTUSDAW" debilita esa
  declaración.
- Registrar **CANTUS** sí cubre "Cantus DAW", "Cantus" a secas y el dominio cantusdaw.com
  como forma de uso. El dominio no necesita registro de marca.

**Cómo gastar poco (recomendado):**

| Paso | Cuándo | Costo oficial aproximado |
|---|---|---|
| CANTUS en el **IMPI**, en línea, clase 9, sin abogado si quieres (el trámite "Marca en Línea" es guiado) | Ahora | **≈ MXN 2,814** (≈ US$150) |
| Usar **™** junto al nombre desde ya (no requiere registro; ® solo cuando esté registrada) — **hecho**: web, app (Acerca de) y README | Ahora | Gratis |
| Clase 42 en el IMPI (si algún día hay servicio en línea) | Cuando haga falta | ≈ MXN 2,814 |
| EE. UU. y UE por Madrid | Cuando haya usuarios o ingresos que lo justifiquen. Dentro de 6 meses se conserva la fecha mexicana; después se puede igual, solo sin esa prioridad (riesgo bajo de que alguien se adelante) | ≈ US$2,500 + búsqueda profesional para la UE |

### Anexo: marcas y productos encontrados

| Nombre | Titular | Qué cubre | Dónde | Estado |
|---|---|---|---|---|
| CANTUS | Guido Cianciulli (ligado a ACCA Software) | Clase 9 "software", 35, 42 | Italia | Registrada, renovada 2018 (≈ 2029) |
| CANTUS | ACCA Software S.r.l. | Clases 9, 35, 42 | Italia | Probablemente caducada (sin renovación registrada) |
| CANTUS | Stagetec GmbH | Consolas de mezcla digitales (clase 9) | Canadá (muerta en EE. UU., UE, Reino Unido, Alemania) | Registrada hasta 2028; producto descontinuado |
| CANTUS | Cantus Co., Ltd. | Software amplio, incluido audio/video (9), SaaS (42) | Corea del Sur | Registrada hasta 2033 |
| AURUM CANTUS | Penglai Jinlang | Aparatos de sonido (9) | EE. UU., Madrid (España, Japón, Australia…), China | Viva hasta 2035 |
| CANTUS | Petra Moore LLC | SaaS de citas y facturas (42) | EE. UU. | Solicitud de 2026, sin examinar |
| Cantus Lunaris | Gaby Grosser-Koß | Datos descargables, juegos (9), música (41) | UE | Registrada 2026 |
| LogoCantus | LogoCantus UG | Apps de aprendizaje (9), enseñanza musical (41) | Alemania | Registrada 2026 |
| encantus | Encantus GmbH | Software (9), 37, 42 | Alemania | Registrada |
| CANTUS PLANUS | Cantus Planus S.r.l. | CD/DVD (9), 41 | Italia | Registrada |
| CANTO | Canto, Inc. | Software de gestión de archivos digitales (9, 42) | UE, EE. UU. | Registrada |
| CANTUS | ACC Brands / BASF | Cerveza y restaurantes / agroquímicos | México | No chocan (otras clases) |
| Cantus – Gregorian Chants | Best Service GmbH | Librería de coros para Kontakt | Mundial, desde ~2013 | Sin registro (derechos por uso) |

Otros usos sin registro: Cantus Database (canto llano, universidad, no comercial), la app
Cantus+, CantuStudio (iOS), "Cantus 1" de Datura (2012, abandonado), el ensamble vocal Cantus
(Minneapolis), Antares Kantos (descontinuado).

## 5. Detalles conocidos que no bloquean la beta

Los siete de la ronda 25 están hechos en la ronda 26, con pruebas (`docs/round26/mixer.md`,
ADR 0043 y 0044):

- [x] una ruta armada sobrevive a ocultar el mixer — ahora se suelta;
- [x] escribir un nivel en el menú de ruta (*Nivel ▸ Escribir un nivel…*);
- [x] *Añadir envío* del inspector ofrece *Nuevo retorno*;
- [x] los carriles de los cables ya no se reacomodan al pasar el ratón;
- [x] las ~17 asignaciones del hint: el hint no gasta ninguna (las demás son de egui);
- [x] muestras de color en los menús de tema base;
- [x] en español, SALIDA ya no se lee dos veces (la fila del modo ancho dice *Destino*).

### Ronda 40 — `cast`: el reparto de Slate, los doce, en tu pantalla <!-- r40:cast -->

Los GIF lado a lado (diseño arriba, la app abajo) están en `docs/round40/cast/`; todo se vio con el
renderizador de la app en CPU, no en tu pantalla. En la release integrada:

- [ ] **Un Saltator nuevo es Compás**, en Pizarra Noche y Papel, flotando y acoplado; el selector
      muestra primero los doce de Slate y luego Fuga, Volta y la familia.
- [ ] **Cada uno de los doce** con *Mover: Al compás* sobre una canción: el groove, el paso y su
      número en el drop (Tono se sube el volumen, Bajo suelta el bajo, Eco se multiplica…). Dime si
      alguno no se ve como el GIF que aprobaste.
- [ ] **Espectro oye la canción**: con la ventana compañera cerrada, las piernas laten con el bombo
      y la cabeza con los platillos; exporta un vídeo con él y mira que baile igual.
- [ ] **Sobre croma azul** (vídeo con fondo azul o la ventana compañera): el lapislázuli de Cinta,
      Vinilo y Glissando se apaga un poco para que el recorte lo respete; dime si prefieres que se
      rediseñen esos azules.
- [ ] **Tiempo de cuadro**: con el estudio en silencio, `cast_bench` y `slate_worst` (Glissando con
      su halo es el más cercano a 1,5 ms).

### Ronda 40 · fase 3 — los pianos grabados de Chorda y su mezclador (ADR 0125) <!-- r40:chorda-src -->

- [ ] **Escucha** `F:\projects\daw-r40\compartir\pianos-c\` (NOTA.md) y haz el ABX de
      `abx\index.html`: bitKlavier (Stage Lid ORTF) contra el Salamander. Lo que elijas será el
      piano grabado por defecto.
- [ ] **Chorda · Tocar**: en *Fuente*, elige *Salamander* (si no está, *Obtener*), luego Chorda 2
      y Chorda 1 en *Modelos anteriores*: cada cambio es un solo paso de deshacer; las notas y la
      mezcla siguen igual.
- [ ] **bitKlavier**: *Obtener* descarga Stage Lid ORTF (unos 2,8 GB; se instala como FLAC,
      comprobado muestra a muestra). En *Mezclador*, *Obtener las otras* y carga dos o cuatro
      imágenes a la vez (su celda de arriba las carga o quita); *Cada micro a su canal* y escucha
      que la mezcla suena igual.
- [ ] **Accurate-Salamander** y **Upright Piano KW**, **Old Piano FB**: obtén y toca cada uno.
- [ ] **Piano in 162** y **Helsinki 1967 Pro** (*Manual*): *Abrir página*, descárgalo tú y usa
      *Instalar desde un archivo…*; debe sonar en Chorda.
- [ ] **Un piano comprado** (SFZ, Decent Sampler o SF2): *Instalar desde un archivo…*; si es
      Decent Sampler con salidas auxiliares, cada salida aparece en el mezclador.
- [ ] **bitKlavier a 64 muestras, en tu interfaz** (ASIO y WASAPI en dúplex): búfer de 64
      muestras, dos imágenes cargadas (Stage Lid ORTF y Piano Bar), toca acordes con pedal un
      minuto (el primero con la caché de muestras vacía: *Ajustes ▸ Caché de muestras*, su botón de vaciar).
      No debe haber chasquidos ni notas cortadas; si los hay, dime cuántos y si solo pasa la
      primera vez (la caché llenándose). Medido aquí solo sin tiempo real: 1 imagen 6 %, 2
      imágenes 12,6 % de un núcleo, bloques de 512.
- [ ] **Cuatro imágenes a la vez** (añade House ORTF y Decca Tail): el medidor de CPU de Cantus
      y el del Administrador de tareas mientras tocas; aquí costaron un 25 % de un núcleo y, sin
      tiempo real, un bloque de 317 ms cuando la caché llegó a su límite (16 GB). Sube el límite
      de la caché a 24 GB y repite: dime si los cortes se van.
- [ ] **Añadir nuestra sala** (canal *Sala* de un piano grabado): *Sala pequeña*, *Estudio*,
      *Auditorio* con el Salamander: que suene a sala y no a eco metálico; *No* debe sonar igual
      que antes. Dime cuál de las tres pondrías por defecto (hoy ninguna).
- [ ] **El retardo de un micro**: con bitKlavier en dos imágenes, automatiza el *Retardo* de una
      (5–20 ms): el sonido se abre o se colorea (filtro de peine); a 0 vuelve a ser el de antes.
- [ ] **La nueva vista Tocar de Chorda**: la primera vez, la línea de estado dice dónde quedaron
      *Sala*, *Anchura*, *Apagadores* y las macros; comprueba que encuentras cada uno.

## Fuentes (consultadas el 2026-10-11)

- Firma: [SignPath Foundation — condiciones](https://signpath.org/terms) (licencia OSI sin
  licencia comercial dual, sin componentes propietarios, compilación automatizada y
  verificable, política de firma en la web, el editor es "SignPath Foundation");
  opciones y precios en `docs/round20/signing.md`.
- Biblia: [American Bible Society — permisos](https://bibles.com/pages/american-bible-society-rights-and-permissions)
  (hasta 500 versículos sin permiso en obras no comerciales; permiso escrito para apps y
  "otros medios nuevos, como sitios web"; contacto licensing@americanbible.org).
- Marca: USPTO, TMview e IMPI MARCia; tasas: IMPI (guías que citan su tarifa, confirmar en
  el sitio del IMPI), [USPTO](https://www.uspto.gov/trademarks/trademark-fee-information),
  EUIPO y [WIPO Madrid](https://www.wipo.int/madrid/en/fees/). No es asesoría legal.

### Ronda 40 · fase 3 — Tympanum, el baterista acústico (ADR 0124) <!-- r40:tymp-engine -->

- [ ] **Escuchar `compartir\tympanum-b2\`** (lee `NOTA.md`): el mismo groove por el kit
  modelado «Studio» y por las baterías grabadas (Gogodze, DRSKit), a la misma sonoridad, y
  los golpes sueltos pieza a pieza. Di qué pieza suena menos real y por qué (ataque, cuerpo,
  cola, bordonero, platos).
- [ ] **En tu equipo, con tu interfaz**: un Tympanum con la DRSKit (13 micrófonos), *Cada
  micrófono a su canal*, tocar en vivo con tu controlador: que no haya cortes al cambiar de
  kit mientras suena (el kit anterior sigue sonando hasta que el nuevo está cargado).
- [ ] **El pedal del charles real (CC4)**: abrir y cerrar con el pie; el *chick* al cerrar.
- [ ] Verificado solo en emulación: las reglas de descarga *Desde la web del autor* y
  *Manual* (un servidor local de prueba, ningún sitio real).
- [ ] **Una batería DrumGizmo tuya desde un archivo** <!-- r40:fix-f -->: Tympanum › *Tus
  propios kits* › *Instalar desde un archivo…*, elige el `DRSKit_basic.xml` (o el zip) de
  una descarga real de drumgizmo.org. Se construye en segundo plano (la ventana sigue
  respondiendo, la barra avanza, un clic la cancela; anota cuánto tarda); al terminar debe
  sonar en sus 11 micrófonos y la carpeta del kit no debe guardar la copia descomprimida.
  Probado solo con los archivos XML publicados y golpes escritos por las pruebas.
- [ ] **Cambiar de kit con la mezcla hecha** <!-- r40:fix-f -->: Studio › *Cada micrófono a
  su canal*, ajusta niveles, cambia a Gogodze y vuelve: cada canal con el nombre de su
  micrófono, ninguno vacío, los grupos por familia; Ctrl+Z deshace el cambio entero.

### beta.14 — los faders y las listas desplegables (ADR 0103 enmienda, ADR 0146 enmienda 2) <!-- fix:look -->

- [ ] **Configuración ▸ Apariencia ▸ Faders** (ahora también ahí, no solo en el editor de
  apariencia): elige *Clásico* y luego *Metal* con el mezclador a la vista. Deben cambiar a
  la vez los faders de los canales, el del VCA, el del máster y los del mezclador de
  Chorda o Tympanum. *El del tema* deja el capuchón de siempre. Cierra Cantus y ábrelo:
  la elección sigue. Probado por las pruebas en Slate Paper, Slate Night y Chalk; tu
  tema exacto y tu escala de 100–125 % solo los confirma tu pantalla.
- [ ] **El popover de audio (el chip del dispositivo)**: abre *Dispositivo de entrada*. La
  lista debe abrir **debajo de su campo** (nunca sobre *Driver* ni sobre su propio campo),
  sobre una hoja opaca con borde y sombra, sin texto de detrás. Igual con *Interfaz*.
- [ ] **Configuración ▸ Audio**: con ASIO, abre *Dispositivo de salida* (tus cuatro
  controladores ASIO) y *Dispositivo de entrada*, *Frecuencia*, *Búfer*, *Núcleos*. Cada
  lista bajo su campo, opaca; desplaza la página con la lista abierta: la lista sigue a su
  campo. Un segundo clic en el campo la cierra.
- [ ] **El menú de una pista** (clic derecho en su cabecera): *Altura ▸*, *Automatización ▸*
  y *Más ▸* abren su página en el mismo menú (antes el menú se cerraba). *‹ Atrás* vuelve.
- [ ] **Modular un parámetro del inspector** (clic derecho ▸ *Modular…*): la ventanita se
  abre al lado del control, sin taparlo.
- [ ] Verificado solo en emulación: los dispositivos de la prueba son una copia de tus
  capturas (ASIO4ALL v2, FL Studio ASIO, FlexASIO, Microphone Array); tus controladores
  reales y su número solo los ve tu equipo.

### Ronda 41 — Fides, el resonador (ADR 0150) <!-- r41:fides -->

- [ ] **Escuchar los ocho presets nombrados, solos** (`F:\projects\daw-r41\fides-listen\`): Marimba,
  Vibráfono, Kalimba, Steel pan, Arpa, Guitarra de nylon, Chelo con arco y Cristal golpeado,
  cada uno en su frase (`NN-<preset>.wav`) y en cinco notas de Do2 a Do6 (`NN-<preset>-notes.wav`).
  Para cada uno: ¿es *convincente solo*? Anota lo que no lo sea (el ataque, la caída, el
  timbre de los graves o de los agudos, el cuerpo). El panel de diez oyentes es tuyo.
- [ ] **Tocar Fides en el teclado**: añade Fides (*Teclados*), toca acordes y escalas en todo el
  rango con el preset por defecto y con Marimba; la velocidad debe dar más fuerza y más brillo.
  Mantén una nota del Chelo con arco: debe sostener y crecer con el arco; suéltala: se apaga.
- [ ] **MPE (si tienes un controlador MPE)**: con el Chelo con arco, la presión de cada dedo
  debe apretar el arco (más áspero, más fuerte), el deslizamiento debe mover el arco hacia el
  puente o la cejilla (más brillante o más dulce), el bend de cada nota debe doblar solo esa
  nota. Probado solo con expresiones escritas por las pruebas.
- [ ] **El dibujo de Tocar**: arrastra sobre la cuerda o la barra para mover dónde se toca; los
  parciales de abajo deben seguir. Cambia el resonador entre los doce y el excitador entre los
  seis: el dibujo debe ser cada vez el objeto correcto, sin nada encimado.
- [ ] **La entrada en vivo** (excitador *Entrada*, ADR 0154 — un sidechain de un canal): en una
  canción con una batería o una voz en su canal, añade Fides, pon el excitador en *Entrada*; la
  tarjeta debe decir *Elige un canal: en silencio hasta entonces*. En *Entrada de*, elige el canal
  de la batería y mantén un acorde: el resonador debe sonar con cada golpe, a tiempo con la
  batería (también con un limitador en el canal de la batería). La lista no debe ofrecer el
  canal de Fides ni uno al que Fides llega. Exporta la canción: debe sonar igual que al
  tocarla. <!-- r41:fides-input -->
- [ ] **Un micrófono en Fides** (con una interfaz real, en dúplex): una pista con la entrada del
  micrófono armada y en escucha, en su canal; Fides con *Entrada de* en ese canal: canta o toca y
  mantén notas en Fides — el resonador debe responder en vivo, con la latencia de la interfaz y
  nada más. Verificado solo en emulación (pistas y renders de prueba). <!-- r41:fides-input -->

### Ronda 41 — Nitor, el realzador de una perilla (ADR 0151) <!-- r41:nitor -->

- [ ] **Insertar Nitor** (*Añadir inserción ▸ Masterización ▸ Nitor*) en una mezcla que
  suena: empieza en *Brillo suave*. Gira **Cantidad** de 0 a 100 % con cada carácter
  (Claro, Pegada, Cálido, Aire, Fuerte): el volumen debe quedar igual (*Igualar nivel*) y
  cambiar el color. Apágalo y enciéndelo (bypass): ¿se oye la mejora y no solo más volumen?
- [ ] **El sub en la tónica**: pon *El sub sigue* en *Tónica*, sube *Sub* en un bombo o un
  bajo. Cambia la tonalidad de la canción en el piano roll (por ejemplo de Sol a Re): el sub
  debe moverse al instante, sin clic. En tus monitores y en unos auriculares: ¿suena afinado?
- [ ] **Armónicos** (tarjeta *Graves*) en un portátil o un teléfono: la línea de bajo debe oírse donde
  antes no se oía.
- [ ] **Nitor como plug-in** (*Cantus Nitor*, CLAP y VST3) en otro DAW: pon su *Tonalidad*
  en la de la canción; debe sonar igual que en Cantus.
- [ ] Verificado solo en emulación: todo lo anterior se midió con renders y pruebas; ningún
  oído lo ha juzgado todavía. Los sonidos para escuchar están en
  `F:\projects\daw-r41\nitor-listen\` (seco y procesado, a la misma sonoridad).

### Ronda 41 — los cinco circuitos de Fragor como pedales propios en Tabula (ADR 0152) <!-- r41:pedals -->

- [ ] **Las caras**: abre Tabula, *Añadir pedal* ▸ estante *Distorsión*: deben estar el
  overdrive verde, la distorsión de op-amp, la de silicio, el fuzz de germanio y el muff
  triangular, cada uno con la caja, la pintura y la rotulación que tiene en Fragor, y sus
  tres perillas con sus nombres. Ponlos los cinco en la pedalera: nada se tapa ni se corta,
  en Slate y en Paper, al 100 % y al 150 %. (Las imágenes de `docs/round41/pedals/` son de
  la prueba; tu pantalla es la que decide.)
- [ ] **El sonido**: carga *Saturaciones en cadena* en una guitarra. Gira la macro
  *Saturación*: deben subir las dos saturaciones a la vez. Pisa el interruptor del muff:
  queda el empuje y el phaser. Cada pedal suena como ese circuito solo en Fragor (las
  pruebas lo comprueban bit a bit; tu oído decide si los presets nuevos — *Empuje limpio*,
  *Solo de silicio*, *Fuzz bajo el bajo*, *Muro de muff* — valen).
- [ ] **Bajo la pedalera** con un pedal de estos elegido: sus perillas, Puerta,
  Mezcla, Salida, Sobremuestreo, Canales y Circuito caben a tu escala; no aparece *Abrir su
  editor completo* (no tienen ventana propia: dinos si quieres la de Fragor con un solo
  circuito).

### Ronda 42 — la afinación del proyecto en cada instrumento (ADR 0155) <!-- r42:tuning -->

- [ ] **Escucha la afinación** en Chorda, Exemplar, Calor y Organum: un proyecto en
  *Werckmeister III en Eb, A4 = 415* (ventana *Opciones ▸ Afinación…*, Ctrl+Shift+T). Toca **a
  mano** I–IV–V–I en Eb mayor (Eb–G–Bb, Ab–C–Eb, Bb–D–F) y luego en F# mayor (F#–A#–C#,
  B–D#–F#, C#–E#–G#): la primera debe sonar más pura (terceras más quietas), la segunda más
  tensa — eso es el temperamento, no un error. (El botón *Tocar una cadencia* suena siempre en
  la tónica del temperamento, aquí Eb.) ¿Te suena bien?
- [ ] **Un sintetizador de hardware con MTS** (si tienes uno que lea *MIDI Tuning Standard*,
  p. ej. muchos de Sequential, Novation, Korg o un módulo con MTS): crea su unidad externa
  (*Equipo externo*), enciende **MTS** en su fila y pon el proyecto en Werckmeister III. El
  sintetizador debe afinarse solo (cada tecla a su altura); compara con un afinador. Apaga
  **MTS**: el sintetizador vuelve a recibir solo las notas.
- [ ] **Un plug-in CLAP o VST3 que lea la afinación por nota** (Surge XT, Vital): en un
  proyecto temperado debe sonar a la misma altura que Exemplar en la misma tecla.
- [ ] **Scala como prueba externa**: con el programa Scala (gratis) genera un volcado MTS
  de Werckmeister III (*Bulk tuning dump*) y un mensaje de escala/octava; impórtalos (o
  pásame los ficheros `.syx`) — deben coincidir tecla a tecla con los de Cantus a 0,01 cents.
- [ ] Verificado solo en emulación: las alturas se midieron en renders (0,1 cent), las
  SysEx en un puerto de prueba y con un decodificador independiente (el de ODDSound); ningún
  sintetizador real ni ningún oído las ha juzgado todavía.

### Ronda 42 — la afinación de la canción y MTS-ESP (ADR 0155, 0156) <!-- r42:tuning-face -->

Lo verificado solo en emulación: MTS-ESP se probó contra una biblioteca falsa con los mismos
símbolos C (`crates/cantus-rt/tests/fake_mts/`); ningún equipo de esta ronda tenía `LIBMTS`.

- [ ] **Instalar MTS-ESP** (el *MTS-ESP Mini* de ODDSound, gratuito, o Surge XT) en Windows (y,
  si puedes, en macOS y Linux). Abre Cantus: *Opciones ▸ Afinación…* (Ctrl+Shift+T). La banda
  *MTS-ESP* debe decir *Conectado* y la opción *MTS-ESP* del sistema ya no debe estar atenuada.
  Sin instalarlo, debe decir que no está instalado y por qué.
- [ ] **Cantus como cliente de Surge XT**: en Surge XT (como programa aparte, o como plug-in en
  otro DAW) carga una escala (*Tuning ▸ Load .scl*) y activa *Act as MTS-ESP source*. En Cantus
  elige *MTS-ESP* como sistema: la ventana debe decir *Siguiendo al maestro de MTS-ESP* (con el
  nombre de la escala); toca Chorda o Exemplar: deben sonar en la escala de Surge. Cambia la
  escala en Surge mientras suena: Cantus debe seguirla al momento.
- [ ] **Surge XT como cliente de Cantus**: con Surge sin ser fuente, en Cantus pon
  *Werckmeister III* en *Eb* a *415* y marca **Ser el maestro de MTS-ESP**. Inserta Surge XT
  como instrumento en una pista (y, si puedes, ábrelo en otro programa): debe tocar en la
  afinación de Cantus (su pantalla de afinación dice *MTS-ESP*). Cambia el temperamento en
  Cantus: Surge debe seguirlo. Desmarca la casilla: Surge vuelve a su afinación.
- [ ] **Sin afinar dos veces** <!-- r42 fix-engine -->: con Cantus como maestro (o siguiendo a
  Surge como fuente), toca Surge XT insertado en una pista que sigue al proyecto, en *A4* y en
  *Mi♭4*, y compáralo con un afinador (o con Exemplar en otra pista): debe dar el tono de la
  tabla, no el doble de su desvío. En el inspector de la pista, *Afinación por* debe decir
  *MTS-ESP*; cámbialo a *Expresión por nota* en un plug-in que no lea MTS-ESP (el sintetizador
  de prueba, o uno sin MTS-ESP): entonces debe ser él quien suene afinado.
- [ ] **Dos maestros**: con Surge como fuente, intenta *Ser el maestro* en Cantus: debe
  negarse diciendo que otro programa dirige. Cierra Cantus a la fuerza siendo maestro
  (Administrador de tareas), ábrelo otra vez: si MTS-ESP lo ofrece, *Reiniciar MTS-ESP* debe
  liberar el lugar.
- [ ] **Escuchar la cadencia** (*Tocar una cadencia en …*) en Chorda con Igual, Werckmeister III
  con *Tónica* = C y mesotónico de un cuarto de coma con *Tónica* = C: la cadencia suena en la
  tónica del temperamento (aquí C), y en el mesotónico sus terceras deben sonar puras y quietas.
  Luego, sin cambiar la tónica, toca **a mano** I–IV–V–I en F# (F#–A#–C#, B–D#–F#, C#–E#–G#):
  en el mesotónico debe sonar áspera (girar la tónica a F# la volvería pura: el temperamento
  entero se mueve con ella). Con una pista con afinación propia seleccionada, la cadencia del
  proyecto debe sonar en una pista que sigue al proyecto (la ventana dice cuál). ¿El círculo de
  quintas cuenta lo mismo que oyes?
- [ ] **Mirar la ventana** en tu pantalla, en Slate Lapis Night y Paper, al 100 % y al 200 %,
  en inglés y en español: nada se corta ni se solapa; la línea de la barra superior dice la
  afinación solo cuando no es la predeterminada. Con la ventana abierta, Supr, Espacio y Ctrl+N
  no deben tocar la canción de detrás (Ctrl+Z sí deshace); en Ubuntu/GNOME, Ctrl+Shift+T debe
  abrirla (Ctrl+Alt+T abre la terminal del sistema, por eso no es su atajo).
- [ ] **macOS**: la app firmada carga `/Library/Application Support/MTS-ESP/libMTS.dylib` (el
  permiso *disable-library-validation*).
