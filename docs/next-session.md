# Prompt para la próxima sesión (ronda 32)

*Escrito el 2026-10-27 al cerrar la ronda 31, con el orden que pedí ese día (beta.6 publicada para Windows y Linux; macOS
sigue esperando la sesión con la Mac prestada). Copia el bloque de abajo tal cual en una
sesión nueva de Claude Code abierta en `F:\projects\cantus`. Si prefieres otro orden
(BACKLOG §0.4), cámbialo antes de pegarlo.*

---

Lee primero `CLAUDE.md`, `docs/STATUS.md` (la cabecera, «The next session» y la sección de
la ronda 31), `docs/round31/integration.md` y `docs/round31/hosted-view.md`,
`docs/BACKLOG-excellence.md` §0.2 (notación), §0.4 y §6h, y la memoria
(feedback-no-leftovers, feedback-review-cost, gotcha-boot-path, cantus-parallel-builders,
ui-quality-bar, round-31, public-repo-cantus-daw).

**Ronda 32: cada efecto e instrumento con su propia cara, y la partitura desde el piano
roll.** (Lo decidí el 2026-10-27: esto va antes de las comparativas con otros DAW y del Mac,
porque aún no tengo instalados REAPER ni FL Studio para medir ni la Mac prestada.) Busca la
excelencia (Colosenses 3:23): tests antes del código, nada a medias, mira lo que construyes
con los ejemplos shoot. Cada hueco que deje una función se cierra en la misma ronda.

0. **Lo que dejó la ronda 31, primero:** si `macos.yml` marcó algo como *FLAKY on macOS VM*,
   busca la causa (la ronda 31 ya encontró la de `session::a_crash_mid_song…`,
   `docs/round31/mac-flaky.md`). Si te paso lo que escuché (arpegiador, swings, acordes,
   presets de inicio, crossfades, Touch/Latch) o lo que vi en la vista de Cantus con Surge XT
   o Vital, aplícalo. Si para entonces ya hice la sesión con la Mac (`docs/mac-session.md`),
   aplica sus resultados y que la beta.7 lleve el `.dmg` (`CANTUS_RELEASE_MACOS=true`).
1. **§6h punto 1 — un diseño propio para cada unidad**, como los de FL (Love Philter, Gross
   Beat, Vocodex, Harmor), la familia de FabFilter y los instrumentos de Arturia y u-he como
   referencia: cada efecto y cada instrumento con su color y material, su pantalla
   característica (las líneas BBD de Concentus, la sala de Spatium, la curva de Tonsor, la
   tabla armónica de Chorda…), sus formas de control y acentos de tipografía, dibujado con
   las primitivas vectoriales del kit (sin mapas de bits; nítido a cualquier escala y en
   ambos temas), con las reglas de la familia (unidades etiqueta/valor, nada encimado,
   redimensionable, teclado y lector de pantalla, el mismo menú de parámetro en cada
   control). Una hoja de estilo por unidad (`kit::skin`), no código de pintura suelto; los
   CLAP/VST3 exportados llevan la misma cara. **Hecho cuando** cada unidad tiene su piel
   fotografiada en los dos temas al 100 % y al 200 %, el presupuesto de cuadro aguanta con
   tres editores abiertos y un test falla si una unidad se queda con el aspecto liso.
   Repártelo por familias (dinámica, modulación, espacio, instrumentos…) entre
   constructores, con un contrato que fije `kit::skin` primero.
2. **La partitura desde el piano roll** (§0.2 *Notation*, §0.4 punto 10): la parte de una
   pista o de un clip vista como pentagrama junto al piano roll — cuantizada para leer (sin
   tocar las notas), con ligaduras, silencios, barras, plicas, armadura (la tonalidad de la
   canción de la ronda 31) y compás, cambios de tempo y de compás, claves según el rango,
   pentagrama doble para piano, dinámicas desde la velocidad si se pide — bien grabada, no
   solo correcta (Dorico, MuseScore y Sibelius como referencia de grabado); exportar a
   MusicXML y a PDF (y abrirse en MuseScore); imprimir. Edición básica sobre el pentagrama
   si cabe con calidad; si no, lo dejas medido y anotado. Golden tests del grabado (una
   melodía, un piano a dos manos, tresillos, síncopas, notas largas ligadas) y un test de
   ida y vuelta MusicXML.
3. **Después, en este orden, cuando haya con qué** (no en esta ronda salvo que sobre
   tiempo): lo central de Ableton (§0.4 punto 5: vista de sesión, Link, reloj MIDI y MTC,
   MPE); la matriz contra cada líder y las mediciones tipo DAWbench contra REAPER y FL
   Studio (§0.4 punto 2, §4e) cuando los tenga instalados; el Mac cuando tenga la
   computadora.
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
`cantus-daw`, y la beta.7 con su tag cuando todo esté en verde (Windows y Linux; macOS
si ya hice la sesión con la Mac).
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
la marca. Para las comparativas: instalar REAPER y FL Studio en esta máquina. Para después,
en el backlog: la voz cantada como instrumento (§7a).
