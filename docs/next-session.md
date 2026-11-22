# Prompt para la próxima sesión (ronda 42)

*Escrito el 2026-11-22 al cerrar la ronda 41 (Fides, Nitor, los pedales de Fragor, la afinación de
Chorda, la entrada de Fides por sidechain; la beta.16 publicada para Windows, Linux y macOS; el
video de lanzamiento terminado). Es una **propuesta**: el dueño decide el orden y el alcance; el
2026-11-22 dejó en manos del integrador las decisiones pendientes con el criterio de la
excelencia (memoria `decision-owner-delegates-excellence`). Copia el bloque de abajo tal cual en
una sesión nueva de Claude Code abierta en `F:\projects\cantus`.*

---

Lee primero:
- `CLAUDE.md`;
- `docs/STATUS.md` (la cabecera, «The next session: what round 41 left» y la sección de la ronda 41);
- `docs/round41/integration.md` (lo que entró, la revisión integrada, el fuzz de deshacer, lo que quedó);
- `docs/BACKLOG-excellence.md` §0.2 (la afinación: microtonal, Scala, MTS-ESP) y §3e (cuerdas y
  vientos, «modelled and recorded, rendered as Chorda and Tympanum are»);
- `docs/RELEASING.md` (antes de cualquier release);
- la memoria: round-41, decision-owner-delegates-excellence, feedback-tools-in-repo,
  feedback-work-standards-r36, feedback-no-leftovers, feedback-review-cost,
  machine-f-drive-build-quirks, cantus-parallel-builders.

Busca la excelencia (Colosenses 3:23). Escríbeme en español.

## 0. Antes de empezar

1. Corre la suite completa sobre master con `--no-fail-fast`: Windows con
   `bash tools/test/suite-windows.sh` y Linux en WSL con `bash tools/wsl/guarded.sh master test`
   (vigila C:; el disco de WSL y el archivo de paginación viven ahí). Ninguna otra compilación al
   mismo tiempo que una suite (las pruebas de tiempo).
2. Revisa la RAM antes de lanzar constructores: con el navegador del dueño abierto quedan 2–4 GB;
   hasta tres constructores a `-j 2`, cada uno esperando ≥ 3 GB libres (`tools/build/slot.sh`).

## 1. La ronda 42: dos o tres cosas, con contrato primero

Propuesta, en este orden:

1. **Microafinación** (pasó de la ronda 41): escalas `.scl`/`.kbm` de Scala y MTS-ESP, una afinación
   del proyecto que siguen todos los instrumentos con altura, con un ajuste por instrumento;
   afinaciones históricas (mesotónica, Werckmeister, Vallotti) y temperamentos justos por tonalidad.
   Pruebas con archivos Scala reales de fuera (el archivo de escalas de Scala), nunca solo con los
   nuestros. Chorda, Tympanum, Fides y los sintetizadores la siguen; un control en su valor por
   defecto (12-TET) no toca ni un bit.
2. **Cuerdas y vientos** (BACKLOG §3e, pedido del dueño el 2026-11-20): modelados y grabados, con
   renders profesionales como Chorda y Tympanum — empezar por la familia que más falta (cuerdas de
   orquesta: violín, viola, chelo, contrabajo, arco y pizzicato), con su investigación verificada
   (licencias y patentes) antes de construir, y el oído del dueño como juez.
3. Si cabe, **un efecto** del BACKLOG que acerque a FL (§6l.B).

Como siempre: un commit de contrato (tipos, firmas, número de esquema y de ADR, áreas de locale
vacías, `docs/round42/BUILDING.md` con los dueños de cada archivo), un worktree por constructor,
la primera entrega integrada pronto y la revisión del árbol integrado a mitad de ronda (≤ 6
agentes), una prueba de punta a punta por función a través de `CantusApp::assemble`, y todas las
herramientas nuevas en `tools/` (también las de escucha y de render).

## 2. Pendiente de la ronda 41 (no lo pierdas)

- **El video de lanzamiento** está terminado en `F:\projects\video-work\final\` (45 de 45
  revisiones). Falta publicarlo: el dueño lo sube a YouTube desde su cuenta (título, descripción
  con capítulos, subtítulos en `cantus-<lang>.srt` y `chapters-<lang>.txt`); el loop y el video
  van a la web (portada y página del video), con la sesión que lleve la web.
- Las imágenes del selector listas antes de la primera vista (BACKLOG §6j).
- La afinación del arco de Fides (unos cents, como da la física; `docs/round41/fides.md`).
- El token clásico `cantus-github-public` del dueño: revocarlo (el PR #12325 ya se fusionó).
