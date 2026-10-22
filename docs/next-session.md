# Prompt para la próxima sesión (ronda 30)

*Escrito el 2026-10-21 al cerrar la ronda 29. Copia el bloque de abajo tal cual en una
sesión nueva de Claude Code abierta en `F:\projects\cantus`.*

---

Lee primero `CLAUDE.md`, `docs/STATUS.md` (la cabecera, «The next session» y la sección de
la ronda 29), `docs/round29/integration.md`, `docs/BACKLOG-excellence.md` §0.4 y §2 (Linux,
macOS y la lista «Prepared without a Mac»), el ADR 0049 con su enmienda del 2026-10-20 (el
repositorio público `luis314159/cantus-daw` y `tools/public/publish.sh`) y la memoria
(feedback-no-leftovers, feedback-review-cost, gotcha-boot-path, cantus-parallel-builders,
round-29, decision-round30-platforms, public-repo-cantus-daw).

**Ronda 30: Cantus en los tres sistemas — releases de Windows, Linux y macOS al final.**
Busca la excelencia (Colosenses 3:23): tests antes del código, nada a medias, mira lo que
construyes con los ejemplos shoot. Cada hueco que deje una función se cierra en la misma
ronda; lo que reporte un constructor como «Left» se le devuelve como trabajo.

1. **Linux, producto completo:** audio de baja latencia por PipeWire y JACK (ALSA de
   respaldo), prioridad de tiempo real en `cantus-rt` (rtkit / `SCHED_FIFO`, el equivalente
   de MMCSS), las ventanas de los editores de plug-ins en X11 y Wayland (hoy solo existen en
   Windows), el proceso aislado de plug-ins probado de punta a punta, el escáner de
   plug-ins y sus carpetas, diálogos de archivo y fuentes del sistema, y el paquete
   **AppImage** (después Flatpak) construido en CI, con la tarjeta de descarga de la web.
2. **macOS, todo lo que se puede preparar sin la Mac** (BACKLOG §2): el `.app` con su icono y
   el permiso de micrófono, binario universal (Apple silicon e Intel), `cantus-scan` y
   `cantus-sandbox` dentro del paquete, el `.dmg` hecho en CI; firma con runtime endurecido
   y notarización en el workflow (los secretos como marcadores); CoreAudio con los hilos del
   motor en el *audio workgroup* del dispositivo; el aislamiento de plug-ins sin futex
   (`os_sync_wait_on_address` o semáforos) y el ayudante que muere con Cantus; las ventanas
   de los editores en Cocoa; el preset de teclas ⌘/⌥, la barra de menú global, el trackpad y
   Retina; la suite y el fuzz en verde en el runner de macOS y los hashes de referencia
   **idénticos** en Windows, Linux y macOS.
3. **Una lista corta para mi sesión con la Mac prestada:** qué instalar, qué probar
   (interfaz en dúplex 10 minutos, un plug-in real con su editor, el permiso de micrófono,
   instalar el `.dmg` y abrirlo sin Gatekeeper, actualizar desde una versión vieja, CPU en
   Apple silicon) y cómo anotar los resultados. Avísame qué día la necesitas.
4. **Releases:** un solo tag publica los tres instaladores en `cantus-daw` (el de Windows ya
   sale del workflow `release.yml`), la web cambia «Muy pronto» por «Disponible» en Linux y
   macOS solo cuando sus releases existan, y el actualizador sabe qué archivo pedir en cada
   sistema.
5. **Mis fricciones** (si te paso alguna al empezar, las anoto aquí):
   - **Ajustes es difícil de encontrar** (2026-10-22: «¿dónde está settings?»): hoy solo
     está en *Ver ▸ Ajustes…*, sin atajo. Hazlo evidente como en FL (un menú *Opciones*) y
     en Ableton (`Ctrl+,`), con un acceso directo a *Ajustes ▸ Plug-ins* desde el selector
     de dispositivos (p. ej. *Buscar más plug-ins…* cuando el estante CLAP/VST3 está vacío).

Organízalo como las rondas grandes: commit de contrato, un worktree por constructor en
`F:\projects\daw-r30\` (jobs = 3, TMP en F:; máximo tres constructores compilando a la vez),
revisiones adversariales **pequeñas** (como mucho ~6 agentes por revisión: tres buscadores
con lentes combinadas y un verificador por buscador; solo sobre lo que cambió) en cada rama,
en cada ronda de arreglos y en el árbol integrado, e integración. Borra el `target/` de
cada constructor al integrarlo. Al final: suite completa + fuzz + clippy en Windows, en
Linux (WSL, `tools/wsl/check.sh`) y en el runner de macOS; STATUS, ADRs, manual EN/ES y notas
de la versión; memoria copiada a `docs/claude-memory/`; subir a `origin` (privado) y publicar
con `tools/public/publish.sh` a `cantus-daw` (nunca empujes el historial privado al
público). Tienes mi permiso para subir y publicar cuando todo esté en verde. Escríbeme en
español.

---

**Lo que solo puedes hacer tú** (no lo pide el prompt): la Mac prestada (y, si hace falta, la
cuenta Apple Developer para firmar y notarizar), probar el instalador de Windows de
`v0.1.0-beta.1` y tus interfaces (UMC en dúplex), escuchar el warp
(`F:\projects\daw-r29\shots\warp\listen\`), probar MIDI learn con tu controlador y un
plug-in real aislado (también uno con diálogo de licencia), la firma de código (SignPath o
Certum), la revisión legal de Chorda y del CLA, el permiso de la RVR1960 y el registro de la
marca.
