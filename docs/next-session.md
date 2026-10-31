# Prompt para la próxima sesión (ronda 35)

*Escrito el 2026-10-30 al cerrar la ronda 34 (la IA: el servidor MCP, las herramientas del
proyecto, la escucha y el asistente de mastering, los stems y el audio a MIDI). La ronda 35 es
el personaje que baila en pantalla (BACKLOG §6i), entera, sin partirla: el 2026-10-29 dejé que
Claude decidiera y la puso así. La beta.9 salió para Windows y Linux; macOS sigue esperando la
sesión con la Mac prestada. Copia el bloque de abajo tal cual en una sesión nueva de Claude Code
abierta en `F:\projects\cantus`. Si prefieres otro orden (BACKLOG §0.4), cámbialo antes de
pegarlo.*

---

Lee primero `CLAUDE.md`, `docs/STATUS.md` (la cabecera, «The next session» y la sección de
la ronda 34), `docs/round34/integration.md`, `docs/BACKLOG-excellence.md` §6i (entera) y §0.2
*Video*, y la memoria (feedback-no-leftovers, feedback-review-cost, gotcha-boot-path,
cantus-parallel-builders, ui-quality-bar, round-34, decision-round35-dancer,
public-repo-cantus-daw).

**Ronda 35: el personaje que baila en pantalla (BACKLOG §6i), entero.** Busca la excelencia
(Colosenses 3:23): tests antes del código, nada a medias, mira lo que construyes con los
ejemplos shoot. Cada hueco que deje una función se cierra en la misma ronda.

0. **Lo que dejó la ronda 34, primero:** si `macos.yml` marcó algo como *FLAKY on macOS VM*,
   busca la causa. Si te paso lo que vi o escuché — un agente trabajando en Cantus desde
   Claude Code o Claude Desktop (la extensión `.mcpb`), lo que escriben `write_part`,
   `voice_chords` y `pedal_from_chart`, los másteres del asistente y su A/B, una separación
   o una conversión a MIDI de mis grabaciones, un Windows limpio sin el redistribuible de
   Visual C++, una memoria FAT32 o exFAT —, aplícalo. Si ya hice la sesión con la Mac
   (`docs/mac-session.md`, ahora con las filas 17 en adelante: el puente MCP y el ayudante de
   ML en el paquete firmado, CoreML), aplica sus resultados y que la beta.10 lleve el `.dmg`
   (`CANTUS_RELEASE_MACOS=true`). Si hay issues nuevos en `luis314159/cantus-daw`, anótalos
   y repártelos.
1. **Un personaje que baila** (§6i): una unidad visual propia — en el mezclador o en una
   pista, como la de FL — dibujada sobre el espacio de trabajo en vectores con el kit,
   nítida a cualquier escala, flotante o acoplada, siempre encima si se pide, con fondo
   transparente; fuera del hilo de audio (lee el transporte y sus parámetros, nunca toca una
   muestra: un test nulo) y dentro del presupuesto de cuadro.
2. **Movido por automatización**: sus parámetros (paso, pose o baile, velocidad, salto,
   giro, tamaño, posición, ánimo, color) son destinos normales de automatización y de
   moduladores (ADR 0057), de MIDI learn y de notas (una nota por movimiento, como un mapa
   de batería); por defecto baila **al compás** sin automatización y puede reaccionar al
   nivel de una pista.
3. **Personajes incluidos**, nuestros y originales (para toda la familia, en ambos temas),
   cada uno con sus movimientos, en un selector con vista previa; **personajes importados**:
   evalúa primero por licencia y por lo que permiten Rive, Lottie, hojas de sprites (Aseprite,
   TexturePacker) y DragonBones; glTF/VRM en 3D después; Spine y Live2D fuera salvo que su
   licencia sirva a una app GPL gratuita. Un archivo de personaje se revisa y se aísla (no
   corre scripts) y muestra su licencia.
4. **Exportar el clip como video** con el audio de la canción (MP4/WebM, cuadrado y vertical
   para redes, o una capa transparente para un editor; opción de croma). **Es el riesgo de
   la ronda**: Cantus no tiene codificador de video. Mídelo y decídelo en el contrato, con su
   ADR, antes de repartir: códecs y contenedores, licencias y patentes de cada camino (un
   crate en Rust, una biblioteca del sistema, un programa externo), el audio sincronizado al
   cuadro, y una exportación de 30 segundos medida.
5. **Compañía al trabajar y en directo**: una ventana pequeña siempre encima que baila con
   lo que suene (transporte, nivel y pulso del máster) sin automatización; un **modo para
   streaming** que OBS o Streamlabs capturen limpio (título de ventana propio, fondo
   transparente o croma, tamaño y posición fijos entre sesiones, nada del resto de la
   interfaz); un visualizador ligero al lado (espectro o nivel del máster, compás y pulso,
   nombre y tempo de la canción); barato para dejarlo encendido todo el día (su costo de CPU
   y GPU medido y a la vista; se pausa oculto o minimizado).
6. **Listo cuando** un personaje incluido baila al compás sin automatización, un clip de
   automatización cambia sus movimientos justo en los compases donde se dibujó, un personaje
   Rive o Lottie importado (y una hoja de sprites) se mueve con sus movimientos ligados a
   nuestros parámetros, el presupuesto de cuadro aguanta con él en pantalla, un clip de 30
   segundos se exporta como video con el audio a tiempo, y OBS captura la ventana de
   streaming con fondo transparente mientras el personaje sigue la música una hora, con su
   costo medido. Que un agente MCP también pueda moverlo (sus parámetros en el catálogo de
   unidades).

Organízalo como la ronda 34, ahorrando tokens: commit de contrato, un worktree por
constructor en `F:\projects\daw-r35\` (jobs = 3, TMP en F:, el script de turnos de la ronda
34 para que no compilen más de tres a la vez; **borra el `target/` de cada constructor en
cuanto se integre**), **un revisor por rama, uno sobre el árbol integrado y uno sobre las
rondas de arreglos**, e integración. Vigila el espacio en F: y en C: (la imagen de WSL crece
ahí; borra el `target/` de cada clon de WSL al terminar). Al final: suite completa + fuzz +
clippy en Windows y en Linux (WSL, `tools/wsl/check.sh` con `CANTUS_CI_DIR` propio); macOS
lo prueba el runner al publicar (lee los registros con la API y el token de git; un job de
release que falle por el runner se relanza con `rerun-failed-jobs`); STATUS, ADRs, manual
EN/ES, README y la página de inicio con capturas de lo nuevo (y un video corto del personaje),
notas de la versión; memoria copiada a `docs/claude-memory/`; merge a master aplastado en un
commit, subir a `origin` y publicar con `tools/public/publish.sh` a `cantus-daw`, y la beta.10
con su tag cuando todo esté en verde (Windows y Linux; macOS si ya hice la sesión con la Mac).
Tienes mi permiso para subir, publicar y etiquetar cuando todo esté en verde. Escríbeme en
español.

---

**Lo que solo puedes hacer tú** (no lo pide el prompt): probar la ronda 34 — un Windows
limpio (o una máquina virtual) sin el redistribuible de Visual C++ separando stems y
convirtiendo a MIDI; instalar la extensión `.mcpb` en Claude Desktop y pedirle algo; una
memoria USB real en FAT32 o exFAT; escuchar lo que escriben `write_part`, `voice_chords` y
`pedal_from_chart`, los másteres del asistente y su A/B; convertir tus propias grabaciones a
MIDI (una voz, un piano, un loop de batería); firmar `cantus-mcp.exe` y `cantus-ml.exe` con el
resto; y que la revisión legal sume Basic Pitch y nuestra adaptación de su creación de notas,
la conversión a ONNX del modelo de batería de Magenta, HT-Demucs y las condiciones de las DLL
de Visual C++ — todo está en `docs/beta-checklist.md`. Y lo de rondas anteriores: el equipo de
la ronda 33 (un Launchpad o un APC, Link contra Live, reloj MIDI y MTC, un controlador MPE);
mirar las caras de las unidades y la partitura; la revisión legal de los nombres de modelo, de
Chorda y del CLA; la sesión con la Mac prestada (`docs/mac-session.md`) y la cuenta Apple
Developer; una hora en un Linux de escritorio real; Flathub; compactar la imagen de WSL como
administrador; la UMC en dúplex, la firma de código de Windows, el permiso de la RVR1960 y el
registro de la marca; instalar REAPER y FL Studio para las comparativas. Después de la ronda
35 quedan en el BACKLOG §7 el panel de asistente dentro de Cantus (fase C) y la generación de
audio (fase E), y la voz cantada (§7a).
