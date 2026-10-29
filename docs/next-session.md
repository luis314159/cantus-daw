# Prompt para la próxima sesión (ronda 33)

*Escrito el 2026-10-28 al cerrar la ronda 32, con el orden que pedí ese día: primero lo
central de Ableton y después la IA (ronda 34). La beta.7 salió para Windows y Linux; macOS
sigue esperando la sesión con la Mac prestada. Copia el bloque de abajo tal cual en una
sesión nueva de Claude Code abierta en `F:\projects\cantus`. Si prefieres otro orden
(BACKLOG §0.4), cámbialo antes de pegarlo.*

---

Lee primero `CLAUDE.md`, `docs/STATUS.md` (la cabecera, «The next session» y la sección de
la ronda 32), `docs/round32/integration.md`, `docs/BACKLOG-excellence.md` §0.2 (*Clip
launching / session view*, *Sync and networking*), §0.4 punto 5 y §3, y la memoria
(feedback-no-leftovers, feedback-review-cost, gotcha-boot-path, cantus-parallel-builders,
ui-quality-bar, round-32, decision-round33-order, public-repo-cantus-daw).

**Ronda 33: lo central de Ableton — la vista de sesión, Link, el reloj MIDI y MTC, MPE.**
(Lo decidí el 2026-10-28: esto va antes de la IA, que será la ronda 34; las comparativas
con otros DAW y el Mac siguen esperando a que tenga REAPER, FL Studio y la Mac prestada.)
Busca la excelencia (Colosenses 3:23): tests antes del código, nada a medias, mira lo que
construyes con los ejemplos shoot. Cada hueco que deje una función se cierra en la misma
ronda.

0. **Lo que dejó la ronda 32, primero:** si `macos.yml` marcó algo como *FLAKY on macOS VM*,
   busca la causa. Si te paso lo que vi de las caras de las unidades o de la partitura
   (cómo graba, cómo imprime en mi impresora, cómo se abre en MuseScore), aplícalo. Si ya
   hice la sesión con la Mac (`docs/mac-session.md`), aplica sus resultados y que la beta.8
   lleve el `.dmg` (`CANTUS_RELEASE_MACOS=true`).
1. **La vista de sesión** (Ableton Live y Bitwig como referencia, con la forma de FL donde
   aporte): una cuadrícula de clips por pista y escenas, lanzar clips y escenas
   cuantizados al compás (o a la fracción que se elija), *follow actions* (siguiente,
   anterior, aleatorio, otro, con probabilidades y repeticiones), grabar en un slot (audio y
   MIDI, con el conteo y el bucle), legato y modos de lanzamiento (trigger, gate, toggle,
   repeat), detener por pista y todo, y **grabar lo que se lanza al arreglo** (como en
   Ableton: la sesión tocada se vuelve arreglo, un paso de deshacer) y llevar clips del
   arreglo a la sesión. Cambiar entre arreglo y sesión al instante; las dos vistas comparten
   pistas, mezclador y automatización (la de la sesión por clip). Teclado, MIDI learn y
   controladores (una cuadrícula tipo Launchpad/APC si cabe con calidad). Todo con el kit,
   en ambos temas. Null tests: un proyecto sin sesión suena igual al bit; lanzar es
   sample-accurate; offline igual que tiempo real.
2. **Ableton Link** — tempo, fase y start/stop compartidos en la red local, con la latencia
   de salida compensada. Revisa primero la licencia del SDK frente a la nuestra (la app es
   GPL-3.0-or-later, el motor MPL-2.0; ADR 0047) o implementa el protocolo, que está
   documentado, en la parte GPL; pruébalo contra otra instancia de Cantus y, si se puede,
   contra una app de prueba de Link.
3. **Reloj MIDI y MTC, entrada y salida**: Cantus como maestro (reloj, SPP, start/stop/
   continue, MTC en sus cuatro frame rates) y como esclavo (seguir el reloj de otro equipo
   con su jitter filtrado, perseguir MTC), por puerto, con su compensación de latencia;
   probado con un loopback MIDI.
4. **MPE de punta a punta**: grabar y editar MPE en el piano roll (bend, presión y timbre
   por nota), tocar instrumentos nativos que lo entiendan (Unda, Calor, Modulus al menos) y
   los alojados (CLAP note expressions, VST3 note expression), configuración de zonas, y
   exportar/importar MIDI MPE. Después, si cabe, MIDI 2.0 (si no, lo dejas medido y
   anotado).
5. **Mis fricciones y lo que digan los usuarios de la beta** (si te paso alguna al empezar,
   o hay issues nuevos en `luis314159/cantus-daw`, las anotas aquí y las repartes).

Organízalo como la ronda 32, ahorrando tokens: commit de contrato, un worktree por
constructor en `F:\projects\daw-r33\` (jobs = 3, TMP en F:; máximo tres compilando a la
vez; cuando uno se integra, su `target/` se borra o pasa al siguiente), **un revisor por
rama, uno sobre el árbol integrado y uno sobre las rondas de arreglos**, e integración.
Vigila el espacio en F: y en C: (la imagen de WSL crece ahí; borra el `target/` de cada
clon de WSL al terminar). Al final: suite completa + fuzz + clippy en Windows y en Linux
(WSL, `tools/wsl/check.sh` con `CANTUS_CI_DIR` propio); macOS lo prueba el runner al
publicar (lee los registros con la API y el token de git; un job de release que falle por
el runner se relanza con `rerun-failed-jobs`); STATUS, ADRs, manual EN/ES, README y la
página de inicio con capturas de lo nuevo, notas de la versión; memoria copiada a
`docs/claude-memory/`; merge a master aplastado en un commit, subir a `origin` y publicar
con `tools/public/publish.sh` a `cantus-daw`, y la beta.8 con su tag cuando todo esté en
verde (Windows y Linux; macOS si ya hice la sesión con la Mac).
Tienes mi permiso para subir, publicar y etiquetar cuando todo esté en verde. Escríbeme en
español.

---

**Lo que solo puedes hacer tú** (no lo pide el prompt): mirar las caras de las unidades y la
partitura en la beta.7 y decirme qué no te gusta; imprimir una partitura en tu impresora;
abrir los MusicXML en MuseScore (ya está instalado; los archivos de prueba se generan con
`cargo run -p cantus-score --example dump_xml -- <carpeta>`); la revisión legal de los
nombres de modelo (los vehículos de los núcleos y los modos de Concentus «Juno I/II» y
«Dimension 1–4», que decidiste conservar), de Chorda y del CLA; la sesión con la Mac
prestada (`docs/mac-session.md`) y la cuenta Apple Developer con sus cinco secretos en
`cantus-daw`; una hora en un Linux de escritorio real; subir el Flatpak a Flathub;
compactar la imagen de WSL como administrador; escuchar lo de la ronda 31 (arpegiador,
swings, acordes, presets de inicio, crossfades, Touch/Latch con tu controlador); la UMC en
dúplex, canciones completas, la firma de código de Windows (SignPath o Certum), MIDI learn
con tu controlador, el permiso de la RVR1960 y el registro de la marca. Para las
comparativas: instalar REAPER y FL Studio en esta máquina. Para la ronda 34: la IA (BACKLOG
§7 — primero el servidor MCP, después stems, audio a MIDI y el asistente de mastering); la
voz cantada (§7a) más adelante.
