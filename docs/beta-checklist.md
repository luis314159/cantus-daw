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
      la beta sale con la **RV1909** (dominio público) como predeterminada.
- [ ] **Marca "Cantus"**: ver la búsqueda preliminar abajo; pagar una búsqueda profesional
      y registrar (IMPI en México primero; luego Protocolo de Madrid si conviene).
- [ ] **Revisión legal del CLA** (`CLA.md`) antes de aceptar el primer aporte de otra
      persona.
- [ ] **Hacer público el repositorio** (descargas, auto-actualización y SignPath lo
      necesitan): antes, sacar del historial las capturas de otros productos
      (`research/*/screenshots/`) y revisar que no haya secretos.

## 3. Lo que hace Claude cuando lo pidas

- [ ] La limpieza del historial en una copia aparte, para revisarla antes de publicar.
- [ ] La solicitud a SignPath y la página de política de firma en la web (si eliges SignPath).
- [ ] RV1909 como predeterminada mientras no haya permiso de la RVR1960.
- [ ] Las notas de la versión (`docs/releases/v0.1.0-beta.1.md`), al día con las rondas 23–25.
- [ ] El botón de la web que descargue el instalador directo (no la página de releases).
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

Una ruta armada sobrevive a ocultar el mixer (Esc la suelta); escribir un nivel en el menú de
ruta; *Añadir envío* del inspector podría ofrecer *Nuevo retorno*; los carriles de los cables
se reacomodan al pasar el ratón; el hint gasta ~17 asignaciones por cuadro al pasar sobre el
mixer; muestras de color en los menús de tema base; en español la leyenda dice SALIDA igual que
la fila SALIDA del modo ancho.

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
