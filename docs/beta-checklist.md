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
- [ ] **El instalador de `v0.1.0-beta.1`**: el botón de descarga de cantusdaw.com lo baja de
      GitHub; instalarlo (Windows avisará «Windows protegió tu PC» mientras no haya firma:
      *Más información ▸ Ejecutar de todas formas*), abrirlo, desinstalarlo.
- [ ] **Chorda — prueba a ciegas** (ADR 0059): ≥ 20 oyentes, frases iguales en Chorda,
      Pianoteq 9 y un piano de muestras (p. ej. Salamander); que no sepan cuál es cuál.
      Anotar preferencias y comentarios en `docs/round28/sound.md`.

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
