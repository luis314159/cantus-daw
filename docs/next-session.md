# Prompt para la próxima sesión (ronda 34)

*Escrito el 2026-10-30 al cerrar la ronda 33, con el orden que pedí el 2026-10-28: después
de lo central de Ableton, la IA. La ronda 35 será el personaje que baila en pantalla (BACKLOG
§6i — dejé que Claude decidiera el 2026-10-29 y la puso entera, sin partirla). La beta.8 salió
para Windows y Linux; macOS sigue esperando la sesión con la Mac prestada. Copia el bloque de
abajo tal cual en una sesión nueva de Claude Code abierta en `F:\projects\cantus`. Si prefieres
otro orden (BACKLOG §0.4), cámbialo antes de pegarlo.*

---

Lee primero `CLAUDE.md`, `docs/STATUS.md` (la cabecera, «The next session» y la sección de
la ronda 33), `docs/round33/integration.md`, `docs/BACKLOG-excellence.md` §7 (con lo aprendido
haciendo la demo, «What an AI needs to make great music in Cantus»), `research/ai/` (sobre
todo `mcp-server.md`, `llm-assistant.md`, `legal.md`) y la memoria (feedback-no-leftovers,
feedback-review-cost, gotcha-boot-path, cantus-parallel-builders, ui-quality-bar, round-33,
decision-round33-order, decision-round35-dancer, public-repo-cantus-daw).

**Ronda 34: la IA — primero el servidor MCP, después stems, audio a MIDI y el asistente de
mastering.** Busca la excelencia (Colosenses 3:23): tests antes del código, nada a medias,
mira lo que construyes con los ejemplos shoot. Cada hueco que deje una función se cierra en la
misma ronda.

0. **Lo que dejó la ronda 33, primero:** si `macos.yml` marcó algo como *FLAKY on macOS VM*,
   busca la causa. Si te paso lo que vi o escuché de la vista de sesión, del Launchpad o el
   APC, de Link con otra app, del reloj MIDI con mi equipo o de un controlador MPE, aplícalo.
   Si ya hice la sesión con la Mac (`docs/mac-session.md`), aplica sus resultados y que la
   beta.9 lleve el `.dmg` (`CANTUS_RELEASE_MACOS=true`).
1. **Un servidor MCP de Cantus** (BACKLOG §7 fase B, `research/ai/mcp-server.md`): que Claude
   y otros asistentes manejen Cantus de punta a punta — crear y leer pistas, clips, notas,
   escenas de la sesión, el mezclador, efectos e instrumentos con sus parámetros en sus
   unidades, automatización, exportar — **cada herramienta un paso de deshacer** por el mismo
   `CommandBus`, con lo que pide el BACKLOG: escuchar y recibir números en una llamada
   (`audition`: loudness por sección, true peak, espectro por octavas contra una curva,
   correlación, clics), análisis por pista y matriz de enmascaramiento, el *linter* musical,
   el catálogo legible por máquina de cada unidad (generado de las tablas de parámetros), las
   herramientas «gruesas» (`write_part`, `voice_chords`, `automate`, `set_mix_targets`…) y lo
   que Cantus necesita por dentro para ellas (render de un rango desde un estado guardado,
   tomas de análisis por canal en el render offline, «live = offline» comprobado). Seguro por
   diseño: nada fuera del proyecto sin permiso, el usuario ve y deshace todo.
2. **Stems**: separar una canción en voz, batería, bajo y lo demás, con un modelo que se
   descarga cuando hace falta (nunca en el repositorio; licencia del modelo y de sus pesos
   revisada, ADR 0030/0042), en su propio proceso, con la calidad medida (SDR) contra la
   referencia.
3. **Audio a MIDI** (melodía, polifonía, batería) con el mismo cuidado, y **el asistente de
   mastering** en el canal master: escucha, propone una cadena con nuestros efectos y
   objetivos de loudness por plataforma, y todo queda editable a mano.
4. **Mis fricciones y lo que digan los usuarios de la beta** (si te paso alguna al empezar,
   o hay issues nuevos en `luis314159/cantus-daw`, las anotas aquí y las repartes).

Organízalo como la ronda 33, ahorrando tokens: commit de contrato, un worktree por
constructor en `F:\projects\daw-r34\` (jobs = 3, TMP en F:; máximo tres compilando a la
vez; **borra el `target/` de cada constructor en cuanto se integre — en la ronda 33 F: se
llenó dos veces**), **un revisor por rama, uno sobre el árbol integrado y uno sobre las
rondas de arreglos**, e integración. Vigila el espacio en F: y en C: (la imagen de WSL crece
ahí; borra el `target/` de cada clon de WSL al terminar). Al final: suite completa + fuzz +
clippy en Windows y en Linux (WSL, `tools/wsl/check.sh` con `CANTUS_CI_DIR` propio); macOS
lo prueba el runner al publicar (lee los registros con la API y el token de git; un job de
release que falle por el runner se relanza con `rerun-failed-jobs`); STATUS, ADRs, manual
EN/ES, README y la página de inicio con capturas de lo nuevo, notas de la versión; memoria
copiada a `docs/claude-memory/`; merge a master aplastado en un commit, subir a `origin` y
publicar con `tools/public/publish.sh` a `cantus-daw`, y la beta.9 con su tag cuando todo
esté en verde (Windows y Linux; macOS si ya hice la sesión con la Mac).
Tienes mi permiso para subir, publicar y etiquetar cuando todo esté en verde. Escríbeme en
español.

---

**Lo que solo puedes hacer tú** (no lo pide el prompt): probar la ronda 33 con equipo real —
un Launchpad o un APC (en el Pro MK3 los botones laterales y *Stop Clip*; en los APC el
parpadeo y los colores), Link contra Live o una app del teléfono (Windows pregunta por el
cortafuegos la primera vez), reloj MIDI y MTC con un aparato (o instalar loopMIDI / Windows
MIDI Services para un loopback), un controlador MPE y un plug-in MPE — y escuchar una
actuación con lanzamientos, legato y acciones de seguimiento; todo está en
`docs/beta-checklist.md`. Y lo de rondas anteriores: mirar las caras de las unidades y la
partitura; la revisión legal de los nombres de modelo, de Chorda y del CLA; la sesión con la
Mac prestada (`docs/mac-session.md`) y la cuenta Apple Developer; una hora en un Linux de
escritorio real; Flathub; compactar la imagen de WSL como administrador; la UMC en dúplex, la
firma de código de Windows, el permiso de la RVR1960 y el registro de la marca; instalar
REAPER y FL Studio para las comparativas. La ronda 35 es el personaje que baila (BACKLOG §6i);
la voz cantada (§7a) más adelante.
