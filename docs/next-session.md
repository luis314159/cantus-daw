# Prompt para la próxima sesión (ronda 36)

*Escrito el 2026-11-02 al cerrar la ronda 35 (el personaje que baila: Saltator, la familia de
seis personajes, los personajes importados, el video y la ventana de compañía para
streaming). La beta.10 salió para Windows y Linux; macOS sigue esperando la sesión con la Mac
prestada. La ronda 36 que propongo es **el estudio profesional**, el punto 6 del orden de
BACKLOG §0.4 (el 5, el núcleo de Ableton, se hizo en la ronda 33; el 9, la IA, en la 34): es
recomendación del integrador, tú decides. Si prefieres otra cosa — la voz cantada (§7a), el
panel de asistente dentro de Cantus (§7 fase C), Rive para el bailarín (§6i), la comparativa
medida contra REAPER y FL Studio (§4e, cuando estén instalados) —, cambia el bloque antes de
pegarlo. Copia el bloque de abajo tal cual en una sesión nueva de Claude Code abierta en
`F:\projects\cantus`.*

---

Lee primero `CLAUDE.md`, `docs/STATUS.md` (la cabecera, «The next session» y la sección de
la ronda 35), `docs/round35/integration.md`, `docs/BACKLOG-excellence.md` §0.4 (punto 6),
§4c (entera), §3 *Mixing* y *Editing*, §2 (superficies de control) y §3c (ARA 2), y la
memoria (feedback-no-leftovers, feedback-review-cost, gotcha-boot-path,
cantus-parallel-builders, ui-quality-bar, round-35, public-repo-cantus-daw).

**Ronda 36: el estudio profesional (BACKLOG §0.4, punto 6).** Busca la excelencia
(Colosenses 3:23): tests antes del código, nada a medias, mira lo que construyes con los
ejemplos shoot. Cada hueco que deje una función se cierra en la misma ronda.

0. **Lo que dejó la ronda 35, primero:** si `macos.yml` marcó algo como *FLAKY on macOS VM*,
   busca la causa. Si te paso lo que vi — la ventana de compañía en mi escritorio y en mi
   OBS, un clip subido a TikTok, Instagram o YouTube, el MOV transparente en mi editor —,
   aplícalo. Si ya hice la sesión con la Mac (`docs/mac-session.md`, ahora con la fila 18:
   VideoToolbox, AudioToolbox y la transparencia de la ventana de compañía), aplica sus
   resultados y que la beta.11 lleve el `.dmg` (`CANTUS_RELEASE_MACOS=true`). Si hay issues
   nuevos en `luis314159/cantus-daw`, anótalos y repártelos.
1. **El modo de grabación de alto rendimiento** (§4c, entero, con las decisiones del
   2026-10-10): *Preparar para grabar* pre-renderiza cada canal que entra al máster (después
   de inserciones, envíos y automatización de efectos, antes del fader y el paneo), el
   motor solo lee y suma, las pistas armadas siguen en vivo, el máster y los controles de
   mezcla en vivo o bloqueados, un aviso permanente mientras dura, y el chip con el buffer,
   la ida y vuelta medida y la CPU antes y después. Test nulo: con los controles sin tocar,
   idéntico bit a bit a la reproducción normal.
2. **La mezcla de estudio**: faders VCA (y grupos que los usen), instantáneas del mezclador
   con recuperación (todo o por canal, automatizables como escenas), y lo que haga falta
   para que se sientan como en una consola.
3. **El equipo externo**: instrumentos y efectos externos como unidades (salidas y entradas
   de la interfaz) con un **ping de latencia automático** que compense el recorrido; y
   superficies **Mackie Control / HUI** (faders, transporte, bancos, pantallas) además del
   mapeo genérico en dos clics.
4. **Un editor de audio** (nivel Edison): ediciones destructivas con deshacer, vista
   espectral, reducción de ruido por perfil, normalizar, fundidos, invertir, cortar
   silencios; abierto desde un clip y de vuelta a él.
5. **ARA 2**: mide primero (licencia del SDK de Celemony, lo que exige a una app GPL, el
   costo) y decide en un ADR antes de repartir; si se puede, que Melodyne funcione dentro de
   Cantus como en Studio One; si no, que el BACKLOG diga exactamente por qué y qué falta.
6. **Listo cuando** una sesión con 64 pistas con efectos graba con el buffer más bajo de la
   interfaz sin cortes en el modo de grabación y sale de él idéntica, un VCA mueve su grupo
   con la automatización de cada canal intacta, una instantánea se recupera exacta, un
   sintetizador externo suena a tiempo después del ping, una superficie Mackie (o su
   emulación) maneja el mezclador, y el editor de audio limpia una toma y la devuelve al
   clip. Que un agente MCP también pueda usarlo (herramientas y catálogo).

Organízalo como la ronda 35, ahorrando tokens: commit de contrato, un worktree por
constructor en `F:\projects\daw-r36\` (jobs = 3, TMP en F:, el script de turnos para que no
compilen más de tres a la vez; **borra el `target/` de cada constructor en cuanto se
integre**), **un revisor por rama, uno sobre el árbol integrado y uno sobre las rondas de
arreglos**, e integración. Vigila el espacio en F: y en C: (la imagen de WSL crece ahí; borra
el `target/` de cada clon de WSL al terminar). Al final: suite completa + fuzz + clippy en
Windows y en Linux (WSL, `tools/wsl/check.sh` con `CANTUS_CI_DIR` propio); macOS lo prueba
el runner al publicar (lee los registros con la API y el token de git; un job de release que
falle por el runner se relanza con `rerun-failed-jobs`); STATUS, ADRs, manual EN/ES, README
y la página de inicio con capturas de lo nuevo, notas de la versión; memoria copiada a
`docs/claude-memory/`; merge a master aplastado en un commit, subir a `origin` y publicar
con `tools/public/publish.sh` a `cantus-daw`, y la beta.11 con su tag cuando todo esté en
verde (Windows y Linux; macOS si ya hice la sesión con la Mac). Tienes mi permiso para
subir, publicar y etiquetar cuando todo esté en verde. Escríbeme en español.

---

**Lo que solo puedes hacer tú** (no lo pide el prompt): probar la ronda 35 — la ventana de
compañía en tu escritorio (Alt+Mayús+D; modo para streaming con Ctrl+Mayús+D) y en tu propio
OBS (*Captura de ventana*, método *Windows 10 (1903 y posterior)*; si el Firewall de Windows
pregunta por el OBS portátil, *Cancelar* es seguro); la *Captura de juego* de OBS en una
máquina con una sola GPU; subir un MP4 vertical de prueba a TikTok, Instagram y YouTube;
importar el MOV transparente en tu editor (Resolve, Premiere, CapCut); y que la revisión
legal sume las condiciones del binario OpenH264 de Cisco — todo está en
`docs/beta-checklist.md`. Y lo de rondas anteriores: un Windows limpio sin el redistribuible
de Visual C++ para el ayudante de ML; la extensión `.mcpb` en Claude Desktop; el equipo de la
ronda 33 (un Launchpad o un APC, Link contra Live, reloj MIDI y MTC, un controlador MPE); la
revisión legal (nombres de modelo, Chorda, el CLA, Basic Pitch, Demucs); la sesión con la Mac
prestada (`docs/mac-session.md`) y la cuenta Apple Developer; una hora en un Linux de
escritorio real (también la captura de la ventana de compañía con su compositor); Flathub;
compactar la imagen de WSL como administrador; la UMC en dúplex, la firma de código de
Windows, el permiso de la RVR1960 y el registro de la marca; instalar REAPER y FL Studio para
las comparativas. Para la ronda 36 ayuda tener a mano un sintetizador o un efecto externo con
cable de ida y vuelta, y una superficie Mackie o HUI si tienes una.
