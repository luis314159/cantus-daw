# Prompt para la próxima sesión (ronda 38)

*Escrito el 2026-11-06 al cerrar la ronda 37 (contenido, instrumentos y el aspecto: Chorda
modelo 2 y su escena, Sectio el cortador, Pulsus III con un canal y una cadena por pad, la
biblioteca de 511 presets que se oyen antes de elegir —también en la web— con los paquetes de
Cantus bajo demanda, Fuga y Volta, el aspecto dibujado a mano juzgado a ciegas contra FL
Studio, y el primer scorecard). La beta.12 salió para Windows y Linux, y macOS como **vista
previa** sin firmar. La ronda 37 creció a siete ramas por tus añadidos (el aspecto, la escena
de Chorda, los personajes) — la regla es **tres constructores como máximo**; esta ronda
vuelve a ella. Propongo **el rendimiento medido contra el mercado** — la prioridad permanente
después de los bloqueos de la beta (`CLAUDE.md`, BACKLOG §4b, ADR 0031) — junto con **las dos
funciones que más líderes tienen y Cantus no** según el scorecard (§0.4): la reverb de
convolución (7 de 8) y la edición en cascada (*ripple*) / insertar y borrar tiempo (6 de 8). Es
recomendación del integrador; tú decides. Si prefieres otra cosa — **AU y la sesión con la
Mac** (5 líderes; necesita la Mac prestada), los grupos de clips (6), ARA fuera de proceso
(5), la pista de vídeo (5), la voz cantada (§7a) —, cambia el bloque antes de pegarlo. Copia
el bloque de abajo tal cual en una sesión nueva de Claude Code abierta en `F:\projects\cantus`.*

---

Lee primero `CLAUDE.md`, `docs/STATUS.md` (la cabecera, «The next session» y la sección de la
ronda 37), `docs/round37/integration.md`, `docs/scorecard/features.md`,
`docs/BACKLOG-excellence.md` §0.1 (el scorecard), §0.4 (el orden medido), §4b, §3 *Editing* y
§3b (*Reverbs*), `docs/perf/benchmark.md`, y la memoria (feedback-work-standards-r36,
feedback-no-leftovers, feedback-review-cost, gotcha-boot-path, cantus-parallel-builders,
feedback-showcase-sell, round-37).

**Ronda 38: el rendimiento medido contra el mercado, la reverb de convolución y la edición en
cascada (BACKLOG §4b, §0.1 y §0.4).** Busca la excelencia (Colosenses 3:23): tests antes del
código, una prueba de extremo a extremo por función, nada a medias, mira lo que construyes
con los ejemplos shoot.

0. **Lo que dejó la ronda 37, primero:** si te paso los resultados de la tercera ABX de
   Chorda (`F:\projects\daw-r37\abx-kit3\`), lo que oí de *Micros*, *Tapa* y *Tamaño*, de
   Sectio en mis loops, de los kits y los presets bajos, o lo que vi en los trece recortes
   del aspecto, en la escena de Chorda o en Fuga y Volta, aplícalo. Si el runner de macOS
   marcó algo como *FLAKY on macOS VM*, o llegó algún reporte en `luis314159/cantus-daw`,
   busca la causa y arréglalo. Y busca la causa de la variación de `stage_budget` (dos
   aplicaciones construidas aparte difieren a veces en una sección entera del inspector,
   +4 540 triángulos, con la suite cargada). Y los dos elementos que FL Studio aún ganó para
   los dos últimos jueces ciegos en ambos temas (Cantus 20 y 22 de 26): los iconos de la
   barra y las teclas del piano roll (BACKLOG §8, con lo que dijeron los jueces).
1. **El rendimiento, medido contra el mercado** (§4b, §0.1, §4e): las filas de rendimiento
   del scorecard con su método — un DAWbench (DSP y VI) contra FL Studio y REAPER si ya los
   tengo instalados (si no, deja el arnés listo y dime qué instalar), arranque y carga, la
   interfaz con 200 pistas — en mi máquina, publicadas con su versión y build; luego la
   siguiente palanca de §4b que esos números señalen, empezando por **que cada canal
   descanse como la cadena de un pad** (exacto en el fuzz de canales, medido). ADR 0102 ya
   tiene el método, y la primera medición (`docs/scorecard/README.md`) deja cuatro filas que
   Cantus pierde: el trabajo de 6–42 s tras abrir la ventana (trázalo), la memoria (267 MB
   contra 133 de FL y 62 de REAPER), el reposo (29 % de un núcleo: el motor procesando el
   demo en silencio y el bailarín redibujando la ventana entera 12 veces por segundo) y el
   costo de un cuadro de la ventana completa (~16 ms de CPU).
2. **Una reverb de convolución de primera** (§3b *Reverbs*, fila F4 del scorecard): sin
   latencia (particionada; el convolucionador de Chorda como punto de partida), respuestas al
   impulso propias generadas por Cantus y las del usuario (WAV, AIFF; estéreo y real
   estéreo), recorte, predelay, ancho, EQ de la cola, presets por estilo y papel en la
   biblioteca, fragmentos en la web; probada con IR de fuentes externas y CC0.
3. **Insertar y borrar tiempo, edición en cascada** (§3 *Editing*, fila E11): en el arreglo,
   en la vista de sesión donde aplique, con la automatización, los marcadores, el tempo y los
   compases moviéndose juntos, un paso de deshacer cada uno, en el fuzz.
4. **Listo cuando** el scorecard publica sus filas de rendimiento con el método y Cantus gana
   o empata donde se pueda medir hoy (lo que no, dicho con números), la reverb de convolución
   suena y mide al nivel de las de los líderes, y la edición en cascada funciona de extremo a
   extremo. Que un agente MCP también pueda usarlo.

Organízalo como dice `CLAUDE.md`: commit de contrato, **tres constructores como máximo** en
`F:\projects\daw-r38\` (jobs = 3, TMP en F:, el script de turnos; borra el `target/` de cada
constructor en cuanto se integre), un revisor por rama, **el revisor del árbol integrado en
cuanto lleguen las primeras entregas** y otro al final, uno sobre las rondas de arreglos.
Compila el árbol principal sin información de depuración y vigila el espacio en F: y en C:
antes de cada suite. Al final: suite completa + fuzz + clippy en Windows y en Linux (WSL,
`tools/wsl/check.sh` con `CANTUS_CI_DIR` propio y una rama, no un commit); macOS lo prueba el
runner al publicar; STATUS, ADRs, manual EN/ES, README y la página de inicio vendiendo bien lo
nuevo, notas de la versión; memoria copiada a `docs/claude-memory/`; merge a master aplastado
en un commit, subir a `origin` y publicar con `tools/public/publish.sh` a `cantus-daw`, y la
beta.13 con su tag cuando todo esté en verde (Windows, Linux y macOS en vista previa). Tienes
mi permiso para subir, publicar y etiquetar cuando todo esté en verde. Escríbeme en español.

---

**Lo que solo puedes hacer tú** (no lo pide el prompt): la tercera ABX de Chorda
(`F:\projects\daw-r37\abx-kit3\index.html`, envía el JSON) y, mientras suena una frase,
*Micros*, *Tapa* y *Tamaño* y qué perspectiva querrías por defecto; Sectio en tus propios
loops; los ocho kits nuevos y los 20 presets que quedan bajos por su margen; la pregunta de
los presets de sidechain (los que agachan el nivel necesitan una señal clave); la escena de Chorda contra las imágenes de las bibliotecas de
piano premium; los trece recortes del aspecto en la aplicación real a 100/125/150 % y Fuga y
Volta en el escenario y en la ventana de compañía; instalar REAPER (y aclarar la licencia de
FL Studio) para las comparativas. Todo está en `docs/beta-checklist.md`. Y lo de la ronda 36:
el búfer más bajo de tu interfaz con y sin el modo de grabación (ASIO y WASAPI, dúplex, 10
minutos), un sintetizador o efecto externo con el ping, una superficie Mackie y un HUI,
escribir a Celemony y ejecutar `tools/ara/build-test-plugin.sh`, un plug-in ARA comercial, la
reducción de ruido y el quitar clics en material real, y la vista previa de macOS en una Mac
(`docs/mac-session.md`) con la cuenta Apple Developer para firmar. Y lo de rondas anteriores:
la ventana de compañía en tu OBS y los clips en redes; un Windows limpio para el ayudante de
ML; la extensión `.mcpb` en Claude Desktop; el equipo de la ronda 33; la revisión legal
(nombres de modelo, Chorda, el CLA, Basic Pitch, Demucs, OpenH264); un Linux de escritorio
real; Flathub; compactar la imagen de WSL como administrador; la UMC en dúplex, la firma de
código de Windows, el permiso de la RVR1960 y el registro de la marca.
