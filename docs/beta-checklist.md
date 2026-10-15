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
  - [ ] **Decidir**: un efecto de modulación recién insertado no cambia nada hasta que
        subes su *Mezcla* o *Profundidad* o eliges un preset (como Fervor). Si prefieres que
        suene al insertarlo (como Spatium), es un valor por defecto por efecto
        (`docs/round28/modfx.md`, notas para el integrador).
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
- [ ] **Biblia RVR1960**: los términos de la American Bible Society permiten hasta 500
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
