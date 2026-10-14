# Prompt para la próxima sesión (ronda 26)

*Escrito el 2026-10-13 al cerrar la ronda 25. Copia el bloque de abajo tal cual en una
sesión nueva de Claude Code abierta en `F:\projects\cantus`.*

---

Lee primero `CLAUDE.md`, `docs/STATUS.md` (la cabecera, «The next session» y la sección de
la ronda 25), `docs/beta-checklist.md`, `docs/BETA.md` y `docs/BACKLOG-excellence.md` §0 y
§6d. La ronda 25 ya está en master (cables del mixer y sus filtros de vista, temas con
Cantus Night refinado, icono, licencias MPL/GPL + CLA + marca, RVR1960 en espera de permiso).

**Ronda 26: dejar la beta lista en todo lo que no necesita mi hardware ni mis trámites.**
Busca la excelencia (Colosenses 3:23): tests antes del código, nada a medias, mira lo que
construyes con los ejemplos `shoot`.

1. **CI en verde en Linux y Windows.** Antes de cerrar la ronda, la verificación de Linux
   tiene que pasar igual que en GitHub Actions (`RUSTFLAGS=-D warnings`, clippy y tests):
   usa `tools/wsl/check.sh <rama> clippy|test` en WSL Ubuntu 24.04 (ver
   `docs/DEVELOPING.md`, sección de Linux). Nada se sube con CI en rojo.
2. **Preparar el repositorio público** sin publicarlo: en una *copia* del repositorio, sacar
   del historial `research/*/screenshots/` y (si aún no hay permiso) el archivo de la
   RVR1960 `crates/cantus-ui/locales/verses/es.txt`; buscar secretos en todo el historial;
   dejarme un informe con lo que cambiaría y los comandos para hacerlo de verdad. No hagas
   push forzado ni toques el repositorio real sin preguntarme: el historial se queda como
   está hasta el día en que yo decida hacerlo público.
2b. **Abaratar GitHub Actions** (los minutos del repositorio privado se acabaron el
   2026-10-13; Windows cuenta ×2 y macOS ×10): macOS solo a mano (`workflow_dispatch`) o
   una vez por semana, no correr cuando el commit solo cambia documentos (`paths-ignore`),
   cancelar la corrida vieja cuando llega una nueva (`concurrency`). macOS quedó sin
   validar en CI tras sus últimos arreglos.
3. **Firma de código**: redacta la solicitud a SignPath Foundation y la página de
   «política de firma» que piden en la web (en un borrador, sin publicarla), y verifica que
   cumplimos sus condiciones (`docs/beta-checklist.md` §2).
4. **Notas de la versión** `docs/releases/v0.1.0-beta.1.md` al día con las rondas 23–25, en
   inglés y español; y el botón de la web que descargue el instalador directo (con un nombre
   de archivo estable en el release) en vez de la página de releases.
5. **Los pendientes pequeños de la ronda 25** (STATUS «Small things left» y
   `docs/round25/*.md`): una ruta armada que sobrevive a ocultar el mixer; escribir un nivel
   en el menú de ruta; *Añadir envío* del inspector con *Nuevo retorno*; los carriles de
   cables que se reacomodan al pasar el ratón; las ~17 asignaciones por cuadro del hint;
   muestras de color en los menús de tema base; la palabra SALIDA repetida en la leyenda en
   español; la leyenda de cables invisible cuando la banda está a su mínimo.
6. **CC BY en `cantus-content`** (ADR 0042): el test del manifiesto acepta CC0 y CC BY
   3.0/4.0 con crédito y archivo de licencia, la vista de créditos en *Acerca de* y en
   *Paquetes de sonidos*, y la sección *Content* de `THIRD-PARTY-NOTICES.md`.
7. **Lo que queda de §6d.2** (clips enlazados): una fila en el Inspector para el grupo
   enlazado y un selector «Enlazar a…» por nombre.

Organízalo como las rondas grandes (commit de contrato, un worktree por constructor,
revisores adversariales, integración), con `jobs = 3` por worktree. Al final: suite
completa + fuzz + clippy en Windows **y** en Linux (WSL), STATUS (cabecera con el número de
tests, «The next session», sección de la ronda 26), memoria copiada a `docs/claude-memory/`,
una versión de prueba en `F:\projects\daw-r26\builds\`, y pídeme permiso antes de subir a
master. Escríbeme en español.

---

**Lo que solo puedes hacer tú** (no lo pide el prompt): las pruebas con la UMC y las 2–3
canciones, enviar el correo de la RVR1960, elegir SignPath o Certum, registrar CANTUS en el
IMPI — todo en `docs/beta-checklist.md`.
