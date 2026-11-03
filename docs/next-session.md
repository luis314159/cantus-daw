# Prompt para la próxima sesión (ronda 37)

*Escrito el 2026-11-03 al cerrar la ronda 36 (el estudio profesional: el modo de grabación,
VCA e instantáneas con escenas, el equipo externo con ping de latencia, Mackie Control y HUI,
el editor de audio y ARA 2). La beta.11 salió para Windows y Linux, y macOS como **vista
previa** sin firmar (decisión del 2026-11-03). Es la primera ronda con las reglas nuevas que
aprobaste el 2026-11-03 (`CLAUDE.md`): **dos o tres funciones**, una prueba de extremo a
extremo por función, protocolos y formatos probados con datos de fuentes externas, el revisor
del árbol integrado a mitad de ronda. Propongo el punto 7 del orden de BACKLOG §0.4 — **el
contenido y los instrumentos**, la ventaja visible más grande de los líderes después de las
plataformas — junto con **medir dónde estamos** (§0.1), porque sin números «el mejor DAW» es
una impresión. Es recomendación del integrador; tú decides. Si prefieres otra cosa — la voz
cantada (§7a), el panel de asistente (§7 fase C), exportar nuestro editor de tono como plug-in
ARA (idea del 2026-11-03), Rive para el bailarín —, cambia el bloque antes de pegarlo. Copia el
bloque de abajo tal cual en una sesión nueva de Claude Code abierta en `F:\projects\cantus`.*

---

Lee primero `CLAUDE.md` (sobre todo las reglas nuevas de la ronda 36), `docs/STATUS.md` (la
cabecera, «The next session» y la sección de la ronda 36), `docs/round36/integration.md`,
`docs/BACKLOG-excellence.md` §0.1 (el scorecard), §0.4 (punto 7), §3e y §5, y la memoria
(feedback-work-standards-r36, feedback-no-leftovers, feedback-review-cost, gotcha-boot-path,
cantus-parallel-builders, feedback-showcase-sell, decision-effects-sound-on-insert, round-36).

**Ronda 37: contenido, instrumentos y la primera medición (BACKLOG §0.4 punto 7 y §0.1).**
Busca la excelencia (Colosenses 3:23): tests antes del código, una prueba de extremo a extremo
por función, nada a medias, mira lo que construyes con los ejemplos shoot.

0. **Lo que dejó la ronda 36, primero:** si el runner de macOS marcó algo como *FLAKY on macOS
   VM*, o si llegó algún reporte de la vista previa de Mac en `luis314159/cantus-daw`, busca la
   causa y arréglalo. Si te paso lo que vi — el modo de grabación con mi interfaz, un
   sintetizador después del ping, una superficie, un plug-in ARA, la reducción de ruido en una
   toma real, la Mac —, aplícalo. Instala `gh` si hace falta para leer issues y runs, o usa la
   API con el token de git.
1. **Chorda a oído y los instrumentos que faltan** (§3e, decisión del 2026-10-16): primero el
   sonido de Chorda medido contra pianos de referencia; luego el slicer (§3e) y las salidas y
   cadenas por pad de Pulsus.
2. **Una biblioteca de sonidos y presets que compita** (§5): presets para cada instrumento y
   efecto en los estilos que más se producen, paquetes descargables bajo demanda (ADR 0030,
   nunca en el repositorio), con licencias aceptadas (ADR 0042), y que se oigan desde la web.
3. **El scorecard medido** (§0.1, §4e): la matriz de funciones contra los líderes desde sus
   manuales actuales (tiene / mejor / falta), y las primeras mediciones repetibles en mi
   máquina — carga y arranque, un DAWbench contra REAPER y FL Studio si ya los instalé (si no,
   deja el arnés listo y dime qué instalar).
4. **Listo cuando** Chorda suena a la altura de un piano de referencia en una prueba ABX que
   yo haga, el slicer y Pulsus por pad funcionan de extremo a extremo, cada instrumento y
   efecto tiene presets que valen la pena oír desde la web, y el scorecard publica números con
   su método. Que un agente MCP también pueda usarlo.

Organízalo como dice `CLAUDE.md`: commit de contrato, **tres constructores como máximo** en
`F:\projects\daw-r37\` (jobs = 3, TMP en F:, el script de turnos; borra el `target/` de cada
constructor en cuanto se integre), un revisor por rama, **el revisor del árbol integrado en
cuanto lleguen las primeras entregas** y otro al final, uno sobre las rondas de arreglos.
Compila el árbol principal sin información de depuración y vigila el espacio en F: y en C:
antes de cada suite. Al final: suite completa + fuzz + clippy en Windows y en Linux (WSL,
`tools/wsl/check.sh` con `CANTUS_CI_DIR` propio y una rama, no un commit); macOS lo prueba el
runner al publicar; STATUS, ADRs, manual EN/ES, README y la página de inicio vendiendo bien lo
nuevo (instrumentos y efectos con su detalle), notas de la versión; memoria copiada a
`docs/claude-memory/`; merge a master aplastado en un commit, subir a `origin` y publicar con
`tools/public/publish.sh` a `cantus-daw`, y la beta.12 con su tag cuando todo esté en verde
(Windows, Linux y macOS en vista previa). Tienes mi permiso para subir, publicar y etiquetar
cuando todo esté en verde. Escríbeme en español.

---

**Lo que solo puedes hacer tú** (no lo pide el prompt): probar la ronda 36 — el búfer más bajo
de tu interfaz con y sin el modo de grabación y con el máster pre-renderizado (ASIO y WASAPI,
dúplex, 10 minutos) y que una toma caiga a tiempo; un sintetizador o efecto externo con cable
de ida y vuelta y el ping; una superficie Mackie (o una X-Touch/Icon/FaderPort en modo MCU) y
un HUI si tienes; escribir a Celemony sobre el nombre y el logo de ARA; ejecutar una vez
`tools/ara/build-test-plugin.sh`; una prueba de Melodyne o SpectraLayers; oír la reducción de
ruido, el quitar clics, un VCA y una escena en material real; y abrir la vista previa de macOS
en una Mac. Todo está en `docs/beta-checklist.md`. Y lo de rondas anteriores: la sesión con la
Mac prestada (`docs/mac-session.md`) y la cuenta Apple Developer para firmar; la ventana de
compañía en tu OBS y los clips en redes; un Windows limpio para el ayudante de ML; la extensión
`.mcpb` en Claude Desktop; el equipo de la ronda 33; la revisión legal (nombres de modelo,
Chorda, el CLA, Basic Pitch, Demucs, OpenH264); un Linux de escritorio real; Flathub; compactar
la imagen de WSL como administrador; la UMC en dúplex, la firma de código de Windows, el permiso
de la RVR1960 y el registro de la marca; instalar REAPER y FL Studio para las comparativas.
