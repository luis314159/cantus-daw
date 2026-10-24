# Prompt para la próxima sesión (ronda 31)

*Escrito el 2026-10-24 al cerrar la ronda 30 (beta.4 y beta.5 publicadas para Windows y
Linux; macOS verde en GitHub, esperando la sesión con la Mac prestada). Copia el bloque de
abajo tal cual en una sesión nueva de Claude Code abierta en `F:\projects\cantus`. Si
prefieres otro orden (BACKLOG §0.4), cámbialo antes de pegarlo.*

---

Lee primero `CLAUDE.md`, `docs/STATUS.md` (la cabecera, «The next session» y la sección de
la ronda 30), `docs/round30/integration.md`, `docs/BACKLOG-excellence.md` §0.4 y §3 (lo que
la ronda nombra), y la memoria (feedback-no-leftovers, feedback-review-cost,
gotcha-boot-path, cantus-parallel-builders, round-30, public-repo-cantus-daw).

**Ronda 31: los huecos diarios que un usuario de FL o Ableton nota primero (BACKLOG §0.4
punto 3), y la Mac lista para la beta.6.** Busca la excelencia (Colosenses 3:23): tests
antes del código, nada a medias, mira lo que construyes con los ejemplos shoot. Cada hueco
que deje una función se cierra en la misma ronda.

0. **Lo que dejó la ronda 30, primero:** la causa real (no más espera) de
   `session_faults5::an_instance_removed_during_a_dialog_is_ended_after_it`, que falla a
   veces en los runners de Windows (2 instancias destruidas bajo el diálogo en la release de
   la beta.5); y lo que marque como *FLAKY on macOS VM* el último `macos.yml`
   (`bridge_faults4`). Si ya hice la sesión con la Mac prestada, aplica sus resultados
   (`docs/mac-session.md`, la tabla que llené) y prepara la beta.6 con el `.dmg`
   (`CANTUS_RELEASE_MACOS=true`).
1. **Automatización**: modos escribir / tocar / enganchar (write, touch, latch) y los
   carriles del piano roll para CC, pitch-bend, velocidad y release.
2. **MIDI**: arpegiador, acordes y escalas como efectos de nota, resaltado y ajuste a la
   escala, plantillas de groove.
3. **Edición**: crossfades entre clips que se solapan y envolventes de ganancia por clip.
4. **Flujo**: paleta de comandos, panel del historial de deshacer, plantillas de proyecto y
   de pista.
5. **Efectos que suenan al insertarlos** (§3b, opción B, decidido el 2026-10-16): insertar
   carga un preset de inicio musical; los valores por defecto siguen siendo nulos.
6. **Mis fricciones y lo que digan los usuarios de la beta** (si te paso alguna al empezar,
   o hay issues nuevos en `luis314159/cantus-daw`, las anotas aquí y las repartes).

Organízalo como la ronda 30, ahorrando tokens: commit de contrato, un worktree por
constructor en `F:\projects\daw-r31\` (jobs = 3, TMP en F:; máximo tres compilando a la
vez), **un revisor por rama y uno sobre el árbol integrado**, e integración. Borra el
`target/` de cada constructor al integrarlo y vigila el espacio en F: y en C: (la imagen de
WSL crece ahí). Al final: suite completa + fuzz + clippy en Windows y en Linux (WSL,
`tools/wsl/check.sh` con `CANTUS_CI_DIR` propio y la copia del script de la rama); macOS lo
prueba el runner al publicar (lee los registros con la API y el token de git; un job de
release que falle por el runner se relanza con `rerun-failed-jobs`); STATUS, ADRs, manual
EN/ES y notas de la versión; memoria copiada a `docs/claude-memory/`; subir a `origin` y
publicar con `tools/public/publish.sh` a `cantus-daw`, y la siguiente beta con su tag cuando
los tres sistemas estén en verde. Tienes mi permiso para subir, publicar y etiquetar cuando
todo esté en verde. Escríbeme en español.

---

**Lo que solo puedes hacer tú** (no lo pide el prompt): la sesión con la Mac prestada
(`docs/mac-session.md`) y, si quieres un `.dmg` notarizado, la cuenta Apple Developer con
sus cinco secretos en `cantus-daw`; una hora en un Linux de escritorio real (una interfaz
en dúplex por PipeWire, un plug-in real con su editor en X11 y Wayland); subir el Flatpak a
Flathub; compactar la imagen de WSL como administrador; mandar los mensajes a creadores
(`docs/outreach-influencers.md`, privado); y lo que seguía de antes: la UMC en dúplex,
canciones completas, la firma de código de Windows (SignPath o Certum), escuchar el warp,
MIDI learn con tu controlador, la revisión legal de Chorda y del CLA, el permiso de la
RVR1960 y el registro de la marca.
