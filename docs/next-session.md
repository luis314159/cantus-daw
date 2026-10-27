# Prompt para la próxima sesión (ronda 32)

*Escrito el 2026-10-27 al cerrar la ronda 31 (beta.6 publicada para Windows y Linux; macOS
sigue esperando la sesión con la Mac prestada). Copia el bloque de abajo tal cual en una
sesión nueva de Claude Code abierta en `F:\projects\cantus`. Si prefieres otro orden
(BACKLOG §0.4), cámbialo antes de pegarlo.*

---

Lee primero `CLAUDE.md`, `docs/STATUS.md` (la cabecera, «The next session» y la sección de
la ronda 31), `docs/round31/integration.md`, `docs/BACKLOG-excellence.md` §0.1, §0.4, §6h y
§4b/§4e (lo que la ronda nombra), y la memoria (feedback-no-leftovers, feedback-review-cost,
gotcha-boot-path, cantus-parallel-builders, round-31, public-repo-cantus-daw).

**Ronda 32: la vista de Cantus para plug-ins externos, saber dónde estamos medido, y lo
central de Ableton.** Busca la excelencia (Colosenses 3:23): tests antes del código, nada a
medias, mira lo que construyes con los ejemplos shoot. Cada hueco que deje una función se
cierra en la misma ronda.

0. **Lo que dejó la ronda 31, primero:** si ya hice la sesión con la Mac prestada, aplica
   sus resultados (`docs/mac-session.md`) y prepara la beta.7 con el `.dmg`
   (`CANTUS_RELEASE_MACOS=true`); si `macos.yml` marcó algo como *FLAKY on macOS VM*,
   busca la causa. Si te paso lo que escuché (arpegiador, swings, acordes, presets de
   inicio, crossfades, Touch/Latch), aplícalo.
1. **§6h punto 2** (si no quedó hecho al final de la sesión de la ronda 31): los plug-ins
   externos abren su editor nativo, con un interruptor a la vista de Cantus de sus
   parámetros (agrupados, con buscador, cada control con su menú de modular, MIDI learn y
   automatizar) y *Usar como predeterminado para este plug-in*, dentro y fuera de proceso,
   y para plug-ins sin editor.
2. **Saber dónde estamos, medido** (§0.4 punto 2, §0.1, §4e): la matriz de funciones
   contra cada líder (tener / mejor / falta, de sus manuales actuales) y las primeras
   corridas medidas en mi máquina — una comparación tipo DAWbench con REAPER y FL Studio,
   tiempos de carga y de arranque, un soak largo. Lo que muestre ordena el trabajo de
   rendimiento de §4b.
3. **Lo central de Ableton** (§0.4 punto 5): la vista de sesión (clips y escenas, follow
   actions, grabada al arreglo), Ableton Link, reloj MIDI y MTC de entrada y salida, MPE de
   punta a punta.
4. **Mis fricciones y lo que digan los usuarios de la beta** (si te paso alguna al empezar,
   o hay issues nuevos en `luis314159/cantus-daw`, las anotas aquí y las repartes).

Organízalo como la ronda 31, ahorrando tokens: commit de contrato, un worktree por
constructor en `F:\projects\daw-r32\` (jobs = 3, TMP en F:; máximo tres compilando a la
vez; cuando uno se integra, su `target/` pasa al siguiente), **un revisor por rama, uno
sobre el árbol integrado y uno sobre las rondas de arreglos**, e integración. Vigila el
espacio en F: y en C: (la imagen de WSL crece ahí). Al final: suite completa + fuzz +
clippy en Windows y en Linux (WSL, `tools/wsl/check.sh` con `CANTUS_CI_DIR` propio y la
copia del script de la rama); macOS lo prueba el runner al publicar (lee los registros con
la API y el token de git; un job de release que falle por el runner se relanza con
`rerun-failed-jobs`); STATUS, ADRs, manual EN/ES y notas de la versión; memoria copiada a
`docs/claude-memory/`; merge a master aplastado en un commit (las capturas intermedias no
entran al historial), subir a `origin` y publicar con `tools/public/publish.sh` a
`cantus-daw`, y la siguiente beta con su tag cuando los tres sistemas estén en verde.
Tienes mi permiso para subir, publicar y etiquetar cuando todo esté en verde. Escríbeme en
español.

---

**Lo que solo puedes hacer tú** (no lo pide el prompt): escuchar lo nuevo de la ronda 31
(arpegiador, swings, acordes, presets de inicio, crossfades, Touch/Latch con tu
controlador); la sesión con la Mac prestada (`docs/mac-session.md`) y la cuenta Apple
Developer con sus cinco secretos en `cantus-daw`; una hora en un Linux de escritorio real;
subir el Flatpak a Flathub; compactar la imagen de WSL como administrador; y lo que seguía
de antes: la UMC en dúplex, canciones completas, la firma de código de Windows (SignPath o
Certum), MIDI learn con tu controlador, la revisión legal de Chorda, del CLA y de los
nombres de modelo de los vehículos de los núcleos, el permiso de la RVR1960 y el registro de
la marca. Para después, en el backlog: un diseño propio para cada plug-in (§6h punto 1), la
voz cantada como instrumento (§7a) y las partituras desde el piano roll (§0.2).
