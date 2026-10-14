# What stands between Cantus and excellent

*Requested by the owner on 2026-10-05 after round 19 ("agrega al backlog lo que debemos
para ser excelente"). Written from an honest assessment: the engineering is first-rank
(real-time safety, bit-identical renders, 1 400+ tests, a FabFilter-class effect family,
eight native instruments, professional recording); what is missing is mostly what only
**use** reveals, plus a set of features every top DAW has. Ordered by what pays most.
Each item says how we will know it is done.*

## 0. The final goal: undeniably the best DAW in the world (set by the owner, 2026-10-07)

*The owner: "The final goal is the best DAW in the world, among the free ones and the
commercial ones. Maybe not reachable at first, but in the future — in objective terms
(features, performance…). How much it is used is another matter we will work on in
parallel, but what we seek is that it can be undeniably said to be the best."*

"Undeniably" means **measured and published**, not claimed. This section defines the
scorecard, then lists what no other section of this backlog covers yet.

### 0.1 The scorecard — how we will know

A public, reproducible comparison (`docs/scorecard/`, re-run every release) against the
leaders: **FL Studio, Ableton Live, Bitwig Studio, Logic Pro, Cubase/Nuendo, Pro Tools,
Studio One, Reaper**, and the free ones (**Ardour, LMMS, Zrythm, Waveform Free**).

| Dimension | What is measured | "Best" means |
|---|---|---|
| **Performance** | DAWbench-style DSP and VI tests (§4b), the most tracks/plug-ins at 64/128/256 frames, round-trip latency, startup and project-load time, RAM, UI frame time with 200 tracks | first in every row on the same machine |
| **Audio quality** | summing null tests, offline = real time, deterministic renders, aliasing/THD of every effect and instrument against the references, 64-bit paths where they matter | provably transparent where it should be, and better-measured colour where it should not |
| **Stability** | crash rate per hour of use (opt-in reports), a plug-in crash never takes the session, 24-hour soaks, zero data loss in recovery tests | the lowest crash rate published by any DAW |
| **Features** | a matrix of every feature the leaders have (this backlog's §3–§3d and §0.2), each "have / better / missing" | no row "missing" against any leader; "better" in most |
| **Workflow speed** | timed tasks (a beat from scratch, a vocal comp, a mixdown, a stem export) by testers on each DAW | fastest median time on most tasks |
| **Built-in content** | instruments and effects judged blind against the leaders' stock ones (and against Serum/Vital, Kontakt, FabFilter, Valhalla) | preferred blind in most pairings |
| **Openness** | open source, open formats (DAWproject, CLAP/VST3 both ways), every platform, free | unique — no commercial DAW can match it |

### 0.2 What the leaders have that no section above covers yet

- **Clip launching / session view** (Ableton, Bitwig): a grid of clips and scenes for
  live performance and sketching, recorded into the arrangement; follow actions.
- **A modular environment** (Bitwig's Grid, Reaktor, Max for Live): build instruments and
  effects from modules inside the DAW, with polyphony and sample-accurate modulation.
- **Modulators everywhere** (Bitwig): LFOs, envelopes, step sequencers, followers, MIDI
  and math modulators on any parameter of any device, hosted plug-ins included.
- **Immersive and surround audio** (Logic, Nuendo, Pro Tools, Studio One): 5.1/7.1.4,
  **Dolby Atmos** beds and objects with an ADM BWF export, binaural rendering for
  headphones, ambisonics.
- **Video** (Logic, Cubase, Reaper, FL): a video track, frame-accurate sync, timecode,
  scoring to picture, export with the video.
- **Notation** (Logic, Cubase, Studio One): a score editor, lead sheets and chord
  symbols, print/MusicXML export.
- **Comping** of takes (Logic, Pro Tools, Studio One): swipe comping across loop takes,
  lanes per take, comp regions with crossfades (round 19 has takes — check what is left).
- **Post-production** (Pro Tools, Nuendo): AAF/OMF import/export, ADR tools, loudness
  for broadcast (EBU R128/ATSC A/85 reports), batch processing, a clip/region list.
- **Scripting and extensibility** (Reaper's ReaScript, Ableton's Max, Bitwig's
  controller API): a scripting API (Lua or WASM, sandboxed) for actions, tools and
  controller scripts; a package manager for community scripts and themes (ReaPack-style).
- **Sync and networking**: **Ableton Link**, MIDI clock and MTC in/out, OSC, **MIDI 2.0**
  and MPE end to end, network audio (AES67/Dante later).
- **Tuning**: microtonal scales (Scala files, MTS-ESP), per-note tuning in the piano roll.
- **Hardware integration**: external instruments and external effect inserts with
  automatic latency ping, CV/gate out through DC-coupled interfaces, per-device I/O presets.
- **Collaboration and safety of work**: project versions and snapshots (a history you can
  browse and branch), cloud backup, a collaboration mode (share a project, merge changes),
  bundled project export with every sample for moving between machines.
- **Touch and other form factors**: a touch-friendly mode (tablets, touch laptops), a
  remote/companion app for transport and mixing.
- **Sampling and sound design depth**: a slicer and sampler at Simpler/Sampler/Kontakt
  level (multisample mapping, round robins, scripting), a granular instrument, a physical
  modelling family, a drum sampler with per-pad chains (FL's FPC, Ableton's Drum Rack).
- **Pattern workflow** (FL's own strength): patterns and pattern clips, the step sequencer
  for every channel, the channel rack's graph editor — at least FL's depth, since FL is our
  reference.
- **Mixer depth**: unlimited routing with a routing matrix view, mixer snapshots and
  recall, VCA, per-strip oversampling switch, a channel strip à la SSL/Neve, "mix scenes".
- **Assistive intelligence** (Logic's Session Players, Mastering Assistant, Stem Splitter;
  Studio One's chord track): detailed in §7 (AI); listed here so the scorecard counts it.

### 0.3 The parallel track: people using it

Not the objective scorecard, but needed alongside it (the owner: "we must take care of
that in parallel"): the public beta and its feedback loop (`docs/BETA.md`), a website, a
community (forum/Discord), tutorials, presets and sounds made by the community, creators
who show what Cantus does, translations, and an honest public changelog. §6 covers the
product side.

**Done when** the scorecard, re-run on a release, shows Cantus first or tied-first in every
dimension against every leader — and anyone can re-run it and get the same answer.

## Resumen (español)

**La meta final (§0)**: ser, sin discusión, el mejor DAW del mundo —entre los gratuitos y
los comerciales— en lo objetivo: una tabla de puntaje pública y reproducible contra FL,
Ableton, Bitwig, Logic, Cubase, Pro Tools, Studio One, Reaper, Ardour y los demás
(rendimiento, calidad de audio, estabilidad, funciones, velocidad de trabajo, contenido,
apertura), más lo que aún no cubre este backlog: clip launcher, entorno modular,
moduladores en todo, audio inmersivo/Atmos, video, partituras, comping, posproducción,
scripting, Link/OSC/MIDI 2.0, microtonalidad, hardware externo, versiones y colaboración,
modo táctil, muestreo profundo, patrones al nivel de FL y más mezcla. En paralelo: que la
gente lo use (beta, comunidad, tutoriales). **(6a)** El sitio cantusdaw.com (y cantusdaw.studio) con
backend: descargas, manuales, paquetes de sonidos, actualizaciones servidas desde nuestro
dominio y un conteo de usuarios sin telemetría (descargas, consultas de actualización
anónimas, registro opcional).

Lo que nos separa de "excelente", en orden: **(1)** uso real — hacer canciones completas
con Cantus y corregir cada fricción; **(2)** probar con hardware real (tu Behringer UMC,
otras interfaces, ASIO dúplex); **(3)** las funciones que toda DAW top tiene y aún no:
warp y pista de tempo, automatización (ronda 20), instrumentos externos (ronda 20), MIDI
learn, crossfades, congelar pistas, marcadores, un editor de audio; **(3b)** más efectos: pedales de distorsión y fuzz, chorus, flanger, un multiefecto tipo Distructor, corrección de afinación (tipo Newtone), manipulación de tiempo (tipo Gross Beat/NewTime), controlador de automatización por fórmula, clipper, analizador tipo Wave Candy, controlador X-Y, separador de transitorios, procesador de transitorios, y el resto de la familia que tienen las DAW más profesionales; **(3c)** un editor de afinación y tiempo para grabaciones, nivel Melodyne (notas como bloques, deriva, vibrato, formantes, timing; no destructivo; luego polifónico y ARA 2); **(3d)** un afinador automático en tiempo real (tonalidad y escala, notas a elegir o desde MIDI, velocidad de corrección, formantes, baja latencia para monitorear al grabar); **(3e)** más instrumentos, tras una investigación verificada (FL Studio 2026, Live 12.4, Logic 12, Bitwig 6.1, Reason 14, Fender Studio Pro 8, Cubase 15 y los mejores plug-ins; licencias y patentes comprobadas), en este orden: un cortador de muestras (tipo Slicex) dentro de Exemplar, arpegiador y herramientas de acordes y escala como efectos MIDI, terminar Pulsus con salidas y cadenas de efectos por pad, un piano de cola modelado (ahora por guías de onda y física publicada; el método de Modartt se evaluará cuando expire su patente estadounidense, verificada vigente hasta el 2027-10-20; revisión legal antes de publicarlo, que cubra también siete patentes activas de resonancia de piano de Casio, Kawai y Yamaha), modos espectral y granular en Exemplar (el nivel del Sampler de Bitwig 6.1), resonadores por modelado físico (cuerdas, láminas, membranas), guitarra sobre el modelo de cuerda de Gravis con rasgueo en el piano roll, resíntesis aditiva para Partes (tipo Harmor), secciones orquestales con muestras CC0, un coro de vocales por formantes (el canto con letra después, cuando exista una voz grabada para Cantus con licencia libre: hoy no hay ninguna) y batería acústica multimicrófono; solo contenido CC0 bajo demanda hasta que decidas sobre CC-BY (Sonatina y Philharmonia quedan fuera), y más (chiptune, bucles tocables, sintetizador de texturas, MPE); **(4)** rendimiento con
sesiones grandes y un rendimiento por encima del mercado (procesamiento anticipado, motor multinúcleo, congelado automático en segundo plano, SIMD, benchmark publicado contra las demás DAW); **(5)** contenido (librería de sonidos, presets);
**(6)** configuración de audio directa (un chip siempre visible con interfaz, driver, frecuencia y buffer; asistente de primer uso; conectar y usar); un versículo de la Reina-Valera (1909, dominio público) en el idioma configurado en cada arranque (activado por defecto, con opción de apagarlo); producto: instalador, actualizaciones, firma de código, manual y tutoriales,
macOS/Linux; **(4c)** modo de grabación de alto rendimiento: un botón manual que pre-renderiza cada canal que entra al master (con efectos y automatizaciones, justo antes del fader) y deja el motor solo leyendo y sumando audio, para bajar el buffer sin cortes; el canal que graba sigue en vivo, el master y los controles de mezcla (fader, paneo, mute, solo) se eligen en vivo o bloqueados, y todo lo demás se bloquea con un aviso permanente hasta salir del modo; **(4d)** la GPU como acelerador opcional y configurable (exportar/congelar, convolución larga, trabajo espectral e IA, y un modo mixto CPU+GPU sobre el procesamiento anticipado; siempre con respaldo en CPU), tras un prototipo medido; **(4e)** el motor frente al mercado, para evaluar e investigar: una comparación medida tipo DAWbench contra REAPER y FL Studio, aislamiento de plug-ins, procesamiento anticipado, escala probada (200 pistas con 50 plug-ins de terceros, 10 minutos en vivo con interfaz real), división de voces para el resto de instrumentos y la extensión thread-pool de CLAP, warp/time-stretch, ARA 2, surround/Atmos, Linux y macOS, y modelos loom del núcleo concurrente; **(6d)** pedidos del dueño (2026-10-07): copiar/pegar (Ctrl+C/X/V) en el piano roll y el arreglo; clips enlazados (copiar un clip de instrumento o de audio da *el mismo clip*: editar uno cambia todos) con la opción "Hacer único"; mezclador con cableado entre canales como FL Studio y sin efectos del canal en la vista extra grande; un tema nuevo blanco con detalles azules (por defecto, con logo nuevo) y un tema oscuro con detalles azules; **(7)** pulido: nivel de los instrumentos, accesibilidad, detalles visuales.

## 1. Real use — the test that matters most

- **Make complete songs in Cantus**, start to finish (record a real instrument and voice,
  program drums, arrange, mix, master, export), and log every moment of friction, however
  small. *Done when:* three songs of different styles were finished in Cantus alone and
  the friction log is empty or scheduled.
- **Bugs in the audio export dialog (the owner, 2026-10-09) — a beta blocker.** Several of
  its options do not work (which ones is not yet logged). Round 23: audit **every** control
  of the export dialog (format WAV/FLAC/MP3/Ogg, bit depth and dither, sample rate, range /
  selection / loop, loudness preset and normalisation, tail, stems / per-track, file name
  and folder, the progress bar still pulsing instead of `render_with_progress`), reproduce each
  failure, write a test per option (the exported file's format, length, rate, depth and
  loudness are checked against what the dialog said; offline render = real time), fix them,
  and add the dialog to the `shoot` examples. The owner lists the failing options when they
  have them.
- **Bug: text looks wrong after changing theme — sometimes (the owner, 2026-10-09).** The
  labels change to a strange look (font, weight, size or colour — to be pinned down) after a
  theme switch, not every time. Suspects: fonts/text styles re-applied per theme without a
  full reset, the glyph/font atlas or galley cache kept across the switch, a theme-dependent
  `FontId`/text style, or an unfinished repaint. Round 23: reproduce (switch themes
  repeatedly, with and without a UI-zoom change and a language change), write a test that
  renders the same view before and after a round trip of themes and requires identical
  pixels (`shoot`), fix the cause, and keep that test in the suite. Must be fixed before the
  new themes of §6d.4 ship.
- **To verify: bypassing the EQ shifts the song in time (2026-10-09, found while rendering the website's A/B excerpts).** Rendering the demos with every `Aequalis` slot bypassed lines up with the shipped render in *house* and *trap* (lag 0–1 frames) but is shifted by 487–570 frames (10–12 ms) in *ballad*, *funk* and *lofi*, whose EQs run in natural-phase or linear-phase mode. Either the whole mix moves by the plug-in latency when a slot is bypassed (harmless for a render, but a gap when bypass is toggled during playback) or one strip moves against the others (a delay-compensation bug). Write a test: bypass one linear-phase EQ and require the mix's other strips to stay sample-aligned; then say which it is.
- **A small group of beta musicians** (FL, Ableton, Logic users), a public issue tracker,
  a feedback channel inside the app (opt-in, no telemetry by default). *Done when:* ten
  people outside the project have made a song and their reports are triaged.
- **Listening tests** for every native instrument and effect against its reference —
  someone has to *hear* them; the builders could only measure. *Done when:* each device
  has a listening note in its doc.

## 2. Hardware and platforms

- **Real interfaces**: Behringer UMC (the owner's), Focusrite Scarlett, a class-compliant
  USB mic, ASIO4ALL/FlexASIO; duplex on ASIO (Realtek's driver stalls — likely cpal's ASIO
  host); the loopback calibration with a real cable; hot-plug (device removed mid-session);
  sample-rate changes; Bluetooth outputs' latency. *Done when:* a compatibility table of
  at least five interfaces, all recording without dropouts at 128 samples.
- **MIDI controllers**: keyboards with knobs/pads, **MIDI learn** for any parameter,
  Mackie Control / HUI for faders and transport, controller templates. *Done when:* a
  generic controller maps in two clicks and a Mackie surface drives the mixer.
- **macOS and Linux** builds (cpal and the kit are portable; plug-in windows and ASIO-like
  low latency — CoreAudio, JACK/PipeWire — need work). *Done when:* release builds for
  three OSes pass the suite. **Linux first, then macOS (the owner, 2026-10-10):**
  - **Linux first** because the toolchain is free, a CI runner costs nothing, the test suite
    can run on it unchanged, and the open-source audio community lives there. Work: the
    audio backends (ALSA through **PipeWire and JACK**, the way to low latency on Linux);
    real-time priority in `cantus-rt` (rtkit / `SCHED_FIFO`, the Linux counterpart of
    MMCSS); **CLAP and VST3 hosting on Linux** (VST3 `.so` bundles) and plug-in windows on
    X11 and Wayland (today the open item of STATUS's hosting list); file dialogs and the
    system's fonts; the plug-in scanner; the installer as an **AppImage** (then a Flatpak,
    and `.deb`/`.rpm` if wanted); the website's download card (today it says "build from
    source" for Linux). The determinism work pays off here: the golden hashes and null
    tests must come out **identical** on Windows and Linux — a proof few DAWs can give.
  - **macOS second:** CoreAudio and its real-time workgroups, **Apple silicon (aarch64) and
    Intel**, AU hosting (listed in `idea.md`), window handling, a **signed and notarised**
    build (an Apple developer account, a yearly fee) and a `.dmg`; it needs Apple hardware to
    test on, so it waits for the first Linux release and a machine.
  - **Both:** a CI matrix (Windows, Ubuntu, later macOS) that builds, runs the tests and
    produces the installers; one ADR for the platform layer (`cantus-rt` is the only crate
    with OS code — `cantus-engine` stays `forbid(unsafe_code)`); a compatibility table of
    distributions and interfaces tried, kept like the Windows one.

## 3. Features every top DAW has

- **Time**: warp / time-stretch and pitch-shift for audio clips (élastique-class quality,
  or a free alternative like Rubber Band — licence check), a **tempo track** with ramps and
  **time-signature changes**, **markers** (named, with Start/Loop actions — STATUS round 12
  plan), audio quantize, tempo detection.
- **Automation** (round 20 builds clips and lanes): record automation by moving a control
  (write / touch / latch), automation in the piano roll (CC lanes, pitch bend, velocity
  and release), modulation of any parameter by an LFO or envelope (Bitwig-style).
- **Hosted instruments** (round 20) and **plug-in crash isolation** (a crashing plug-in
  must not take the session down — out-of-process hosting), per-plug-in CPU meter,
  plug-in presets browser, sidechain into hosted plug-ins.
- **Editing**: crossfades between overlapping clips (automatic and editable), clip gain
  envelopes, an audio editor window (Edison-like: destructive edits, spectral view, noise
  reduction, normalise, reverse a region), **pitch correction** (Newtone/Melodyne-lite
  for vocals), slicing to a sampler, consolidate / **bounce in place**, **freeze** tracks (§4c step 1), **low-latency monitoring** of armed tracks (§4c step 2),
  insert/delete time (STATUS round 12 plan), clip groups, ripple edit.
- **MIDI**: arpeggiator and chord tools as MIDI effects, scale highlighting and
  snap-to-scale, ghost notes from other clips, groove/swing templates (STATUS: swing as a
  clip property), step-sequencer per melodic channel (FL's mini piano roll), MPE.
- **Mixing**: track folders and groups with VCA faders, a spectrum analyser and loudness
  meter on the master always visible, reference-track A/B, mixer snapshots, channel
  strip presets, send/return templates, a correlation meter on the master.
- **Workflow**: project templates, track templates, an undo history panel, a quick
  command palette (search any action), customizable toolbars, multiple arrangements
  (STATUS playlist plan), a project/media browser with previews (round 20 builds the
  sample browser), autosave and crash recovery proven under real crashes.
- **Export**: MP3/AAC/FLAC/Ogg encoders, export presets (streaming, CD, stems for a
  mastering engineer), render the time selection, loudness-normalised export.

## 3b. More effects — the family a top DAW ships (requested 2026-10-07)

*The owner: "more effects — distortion pedals, chorus, flanger, fuzz, pitch correction,
time manipulation, a Distructor, a formula controller, a clipper, a Wave Candy, an X-Y
controller, a transient splitter, a transient processor, or more of what the most
professional DAWs and the top plug-ins have." Backlog only, not scheduled.*

What exists: Aequalis (EQ), Comprimo (compressor), Terminus (limiter), Spatium (reverb),
Multiplex (multiband), Sibilus (de-esser), Porta (gate), Fervor (saturation/waveshaper —
the Blood Overdrive/WaveShaper role), Latitudo (stereo/mid-side), Culmen (maximizer), a
delay, the strip EQ. Each new one follows the family's rules (ADR 0018): a null test at
its defaults, block-size independence, exact rest (`at_rest`/`skip`), deterministic
maths, the hand-drawn kit UI, a page in `docs/plugins/`, and the CLAP/VST3 export
bit-identical. Ours should end up better than the reference, not a clone.

**Asked for by name:**

| Effect | Reference | What makes it first-rank | Done when |
|---|---|---|---|
| **Distortion pedals** | Boss DS-1/RAT/Tube Screamer models, FL's Distructor chain | circuit-modelled stages (clipping diodes, tone stacks), oversampled, a pedalboard view | three pedal models sound like their circuits in a blind A/B against references |
| **Fuzz** | Fuzz Face, Big Muff | the transistor's bias/starve and gating, gated-fuzz splat | the bias knob reproduces the "dying battery" sound |
| **Chorus** | Juno-60 chorus, Dimension D, FL's Chorus | BBD-style modulated delays, 1–4 voices, stereo spread, mono-safe | Juno I/II/I+II modes and a mono-compatibility meter |
| **Flanger** | FL's Flanger, MXR/Electric Mistress | through-zero flanging, feedback ±, tempo-synced LFO and an envelope mode | through-zero null at the centre point holds to the bit |
| **Distructor-like multi-effect** | FL's Distructor | a rack of distortion + filter + modulation + delay modules in any order, macros, randomise | presets travel; each module is one of our units |
| **Pitch correction** | Newtone, Melodyne, Auto-Tune, Waves Tune | real-time (§3d) **and** a graphic note editor for recordings (§3c) | a sung take corrected in both modes; formants preserved; no artefacts at the bar of Newtone |
| **Time manipulation** | Gross Beat, NewTime, Ableton Warp, élastique | Gross Beat-style time/volume envelopes (stutter, half-time, tape stop, scratch) and NewTime-style warp markers/slices on audio | tape stop, half-time and stutter presets sample-exact at any tempo; warp markers on a clip (ties to §3 *Time*) |
| **Automation controller** | Fruity Formula Controller, Peak Controller, LFO Tool | a formula (sin, saw, inputs a/b/c, time, beat) or LFO/envelope drawing driving any parameter; an envelope follower from any strip | one controller drives several targets, recorded as automation if asked |
| **Clipper** | Fruity Soft Clipper, StandardCLIP, KClip | hard/soft/analog curves, oversampling, true-peak aware, clip meter | at threshold 0 dB it is a null; inter-sample overs measured |
| **Analysis and visualisation** | Wave Candy, SPAN, Insight, MiniMeters | oscilloscope, spectrum (FFT/constant-Q), spectrogram, vectorscope/goniometer, correlation, loudness (LUFS momentary/short/integrated, LRA, true peak), peak/RMS — resizable, docked or floating | it reads the same LUFS as the EBU test vectors; drawn at 60 fps under the frame budget |
| **X-Y controller** | Fruity X-Y Controller, XY pads | two (or more) parameters on one pad, inertia and paths, recordable, MIDI/automation mapping | a pad gesture records as two automation lanes |
| **Transient splitter** (the owner's "Transmitter") | Eventide SplitEQ (structural split), transient/tonal separators | low- or zero-latency transient detection splitting the signal into transient and sustain/tonal parts, each with its own dynamics and EQ, summing back to a null | both parts summed with no processing equal the input to the bit; zero-latency mode |
| **Transient processor** | FL's Transient Processor, SPL Transient Designer | attack/sustain gain, detection on level not threshold, split-band option, sidechain | a drum loop shaped without a threshold; default is a null |

**Others the most professional DAWs and plug-in makers ship** (in rough order of use):

- **Modulation**: phaser (4–12 stages, Small Stone/Phase 90 models), tremolo and
  auto-pan (tempo-synced), vibrato, rotary speaker (Leslie), ring modulator, frequency
  shifter, a Kickstart/LFO Tool-style **volume shaper** for sidechain pumping without a key.
- **Delay family**: tape echo (Space Echo), ping-pong and multi-tap, a ducking delay,
  reverse delay, a dub-style feedback filter — Valhalla/Echoboy as the bar.
- **Reverbs**: convolution (impulse responses, user IRs), plate and spring, shimmer — on
  top of Spatium's algorithmic halls.
- **Pitch and creative**: pitch shifter and harmonizer, formant shifter, vocoder
  (FL's Vocodex as the bar), granular effect, bitcrusher/sample-rate reducer, glitch/
  stutter, a filter with envelope and LFO (Autofilter), comb and resonator.
- **Dynamics**: dynamic EQ as its own unit and spectral dynamics (soothe-class resonance
  suppression), upward compression, a bus-glue compressor (SSL-style), opto and vari-mu
  models, a ducker.
- **Colour and analogue**: tape machine (wow/flutter, head bump, hiss), console channel
  emulation, an exciter/enhancer, amp and cabinet simulation with IR loading (guitar and
  bass — ties to recording), a lo-fi/vinyl unit.
- **Utility and repair**: tuner, phase/polarity and alignment, gain/utility, noise
  reduction and de-hum, de-click, de-reverb (the audio editor of §3 *Editing*), a
  mid/side and stereo imager beyond Latitudo, a loudness-match A/B.
- **Mastering extras**: linear-phase multiband, a mastering EQ with M/S per band (Aequalis
  has M/S), a reference-track comparator, a dither/noise-shaping unit on its own.

**Done when** each shipped unit passes the family's contract tests, has its manual page in
both languages, and holds up in a blind comparison against its reference.

### 3c. Pitch and time editing of recordings — a Melodyne-class editor (requested 2026-10-07)

*The owner: "a tuner that works from a recording, to fix vocals and the like — Newtone or
something more professional." The real-time tuner of §3b is for the effect sound; this is
the editor for **fixing a performance** after it was recorded.*

**Reference and bar:** Celemony Melodyne (the standard), Synchro Arts RePitch, FL's
Newtone, Logic's Flex Pitch, Cubase VariAudio. The bar is Melodyne's: a corrected vocal
that nobody can hear was corrected.

**How it works:**

1. **Analysis** on a worker when a take lands (or on demand), cached beside the audio
   file: a pitch track (pYIN-style probabilistic YIN, or a small neural tracker of the
   CREPE kind if its licence and size fit), voiced/unvoiced, sibilants and breaths, then
   **notes** — each a blob with its centre pitch, its drift and its vibrato kept apart.
2. **The editor**: blobs on a piano-roll grid over the waveform. Per note: move pitch
   (snap to scale or free), *pitch centre* (pull toward the note, keeping the natural
   drift), *pitch drift* and *vibrato* depth, *formant*, *amplitude*, start/end and
   *timing* (stretch a note without touching its neighbours), split and join. Macros for
   the whole selection: *correct pitch* (%), *correct drift* (%), *quantise timing* to the
   grid or to another track. The key and scale detected from the take, editable.
3. **Resynthesis** that keeps the voice: a formant-preserving method (PSOLA for
   monophonic voice, or a WORLD-style vocoder — BSD), with transitions between notes
   smoothed so a moved note never clicks. The edit is **non-destructive**: the original
   file is never touched; the clip carries the edits (a schema bump) and renders them,
   offline equal to real time, so bounce and stems hear exactly what the editor plays.
4. **Workflow**: open from a clip (double-click → *Pitch*), A/B against the original,
   undo per edit (each a `Command`), a *Pitch* lane in the arrangement for small fixes,
   MIDI out of the detected notes (melody to an instrument), and doubling (a second voice a
   third above from the same take).

**Later, the hard part:** polyphonic editing (Melodyne's DNA — chords in a guitar or
piano recording), and **ARA 2 hosting** so a user who owns Melodyne or RePitch can use it
inside Cantus the way Studio One, Cubase and Logic do (ARA is a Celemony SDK under the
Apache licence — check its terms).

**Licences to check before choosing:** Rubber Band (GPL — compatible with Cantus's
GPL-3.0-or-later), WORLD (BSD), pYIN's reference code (GPL), CREPE (MIT, a model file of
some megabytes). Our own implementation stays the default for deterministic renders.

**Done when** a real sung take (the owner's) is corrected in the editor with no audible
artefact at moderate corrections, formants preserved at ± 3 semitones, timing edits clean,
and the result bounces bit-identical to playback.

### 3d. A simple real-time auto-tuner (requested 2026-10-07)

*The owner: "and a simple real-time one where you set scales or notes — the automatic kind
that works from the input." The partner of §3c: §3c fixes a take afterwards; this corrects
the voice as it comes in — for monitoring while recording, for live use, and for the hard
"T-Pain" effect.*

**References:** FL's Pitcher, Auto-Tune (Auto mode, Access/Artist), Waves Tune Real-Time,
Logic's Pitch Correction, MeldaProduction MAutoPitch and Auburn Graillon (free, the bar
for "simple").

**What it does:**

- **Target notes**: a key and a scale (major, minor, modes, pentatonic, blues, chromatic…),
  or **any set of notes** toggled on a one-octave keyboard (with *remove* and *bypass* per
  note), or **from MIDI** — the notes played on a keyboard or a MIDI clip become the
  targets (FL's Pitcher, Auto-Tune's MIDI mode). The key can follow the project's key.
- **Retune speed**: from 0 ms (the hard, robotic effect) to slow and natural;
  **humanise** (long notes keep more of their natural movement), **flex/tolerance** (small
  deviations left alone), vibrato kept, added or removed.
- **Formant preservation** and a formant shift; a **throat/gender** control; transpose.
- **Low latency** so a singer can monitor through it while recording (target: under
  3 ms at 48 kHz, reported to delay compensation), a mono-voice tracker that does not
  wobble on breaths and sibilants, and a *detected note* display with the correction amount.
- **Harmony** (later): one or two extra voices a scale step away or from MIDI chords, as
  Pitcher does.
- One knob to start: *Key*, *Scale*, *Speed*, *Amount* on the front; the rest folds away.

**How:** the tracker and resynthesis can share §3c's pitch detector and formant-preserving
shifter, in a streaming, fixed-latency form (PSOLA on the detected period). A family unit:
a null at 100 % bypass or with every note allowed and speed at its slowest; deterministic;
exact rest; exported to CLAP/VST3 like the rest.

**Done when** a sung line through it at retune speed 0 gives the hard effect cleanly, at a
natural speed corrects without audible artefacts, the latency is under the target, and MIDI
targets follow a keyboard in real time.

### 3e. More instruments — the family a top DAW ships (requested 2026-10-10)

*The owner: "more instruments — the ones discussed and more, after research." Backlog
only, not scheduled. Research, verified against vendors' pages, release notes, licence files
and Google Patents on 2026-10-10: [`research/instruments-next/notes.md`](../research/instruments-next/notes.md)
(per DAW, per reference product, per candidate: techniques, papers, CPU, licences, patents,
and what could not be verified), on top of `research/instruments/` (rounds 17–18).*

**What the market ships now.** FL Studio 2026 (39 generators; Fruity Slicer 2 in 2025, a
rebuilt FLEX and multi-sample FPC pads in 2026), Live 12.4 Suite (21 instruments; Drum
Sampler; Collision, Tension and Electric in Standard), Logic Pro 12 (Alchemy, Sculpture,
Studio Piano/Bass/Strings/Horns, Session Players with the new Synth Player following Chord
ID), Bitwig 6.1 (a Sampler rebuilt with spectral stretch and 256-grain granular), Reason 14
(Europa, Grain, Kong…), Fender Studio Pro 8 (was Studio One), Cubase 15 (Drum Machine, and
**Omnivocal**, the first vocal synthesizer built into a DAW, in beta). The plug-ins that set
the bar: Serum 2, Vital, Pigments 7, Omnisphere 3, Zebra 3, Kontakt 8, Pianoteq 9, Keyscape,
BBC SO Discover, Splice INSTRUMENT (ex-LABS), Arcade, Battery 4, XO, Superior Drummer 3,
Ample Guitar 4, Strum GS-2, SWAM, Synthesizer V 2. **The 2025–26 bar moved to the sampler,
the note tools and the drum machine more than to new synths.**

**What exists** (rounds 17–18, ADR 0023, `docs/instruments/`): **Firmus**, **Calor**
(analog), **Unda** (wavetable, *with an additive engine, Partes, and a granular one, Grana*),
**Modulus** (FM), **Exemplar** (multisample sampler; SFZ, Decent Sampler, SF2), **Gravis**
(bass, *with a waveguide string engine*: dispersion, frets, pickups), **Lamina** (modelled
EP, *with a nonlinear hammer*), **Organum** (organ and rotary), **Pulsus II** (drums, *with
16 layers per pad, seeded round robin, chokes, a modal percussion voice and a per-pad output
hook*). Several items below therefore **grow an existing engine** rather than start one.
Linked, not repeated: a modular environment, modulators, MPE and microtuning (§0.2); the
factory library (§5); players that follow chords and stem separation (§7); the vocal
*effects* (§3b–3d).

Each new instrument follows the family's rules (ADR 0023): a null test at its defaults,
block-size independence at tolerance 0, exact rest, deterministic maths (`cantus_dsp::math`;
every random choice a hash of seed, note and event), analysis (onsets, partials, pitch) run
offline with its *result* stored in the project, the hand-drawn kit with the
Play/Edit/Matrix shell, a page in `docs/instruments/`, voices that split across the pool with
the bits of one thread (ADR 0038; anything that couples voices — a piano's soundboard, a
choir's ensemble — is a post-voice stage summed in voice order), presets made by ear, and a
model version stored per instance so an improved model never changes an old song.

**Legal and content constraints.** Packs download on demand and never enter the repository
(ADR 0030); the manifest admits **CC0 only** until the owner rules on CC-BY. Verified: VSCO 2
CE, VCSL, Osiris Piano, Virtuosity Drums and the Karoryfer libraries are CC0; Salamander
Grand and the best multi-mic kits (DrumGizmo, Naked Drums) are CC-BY; **Sonatina is excluded**
(Sampling Plus 1.0, retired by Creative Commons in 2011, NC-like, unknown sources) and so are
Philharmonia's samples for redistribution; Pianobook, Decent Samples, Splice/Spitfire and BBC
SO libraries are link-only. **No permissively licensed singing voice, choir or acoustic
guitar exists** — open singing corpora are mostly non-commercial and the standard open
vocoder (OpenVPI NSF-HiFiGAN) is CC BY-NC-SA. `cantus-dsp` is MPL-2.0 (ADR 0047), so GPL references
(aubio, Rubber Band, Loris, reSID) are read, not ported; MIT/BSD ones (Signalsmith Stretch,
WORLD, Clouds, STK) may be.

**Proposed, in priority order** (value for FL-style producers first — beats, melody,
vocals — then what blind comparisons demand, then build cost on what exists):

| Instrument | Reference | What makes it first-rank | Done when |
|---|---|---|---|
| **1. Slicer** — *Sectio*, a mode of Exemplar | FL Fruity Slicer 2 and Slicex, Ableton Simpler, Logic Quick Sampler, Bitwig 6.1 Sliced | **Technique:** spectral-flux onset detection with SuperFlux for sustained sounds and a beat grid by dynamic programming, offline; slice points stored in the project. Embedded markers read first (WAV cue, ACID); one sensitivity control; grid and manual slices; "dump to piano roll"; per-slice pitch, envelope, filter, reverse, choke; FL's *Random*, *Stutter*, *Quantize/Swing* as one-step commands; tempo-following by slice replay | ≥ 95 % of slice points within 10 ms of hand annotation on 50 CC0 loops; a loop replayed from its dumped MIDI equals the file to the bit; a tempo change keeps slices on the grid; drop-to-chopped in ≤ 2 clicks |
| **2. Arpeggiator, chord and scale tools** (note effects) | FL Arpeggiator and Chord tools, Ableton Arpeggiator/Chord/Scale and MIDI Tools, Kontakt 8.6 Tools, Serum 2's arp and clip sequencer | **Technique:** event processing on the absolute song tick (needs tempo and position in the instrument contract). Arp modes, rate with swing, gate, octaves, a step lane with probability and ratchets, latch; chord voicings with inversions and strum; scale lock to the project key; probability drawn from the note's hash | sample-accurate at block sizes 1/37/64/512; bounce = playback; a render from bar 5 equals bars 5… of a full render; works before native **and** hosted instruments; output recordable to the clip |
| **3. Pulsus III** — per-pad outputs and chains (finishing the drum sampler) | Ableton Drum Rack, FL FPC, Logic Drum Machine Designer, Battery 4, XO | Wire the existing per-pad outputs to strips; a per-pad insert chain; any Cantus instrument on a pad; kit import from a folder (name parsing, then similarity) and SFZ; XO-style similarity map shared with the browser; 808/909-class one-shots rendered from Pulsus and released CC0 | 16 pads, each with its own chain, split to 16 strips in one click; the split kit sums to the unsplit kit to the bit; a folder becomes a mapped kit in one drop; chains at rest cost nothing |
| **4. Modelled grand piano** — *Chorda* | Pianoteq 9 (Oct 2025, ~50 MB), Logic Studio Piano, Keyscape, Garritan CFX | **Technique:** digital waveguide strings with dispersion allpasses, a nonlinear felt hammer (Lamina's, extended with hysteresis), two- and three-string unisons with two-stage decay, phantom partials, a multirate soundboard and a sustain-pedal resonance bank, all from closed-form physics and published measurements; continuous half-pedalling; no task-skipping under load (unlike Pianoteq) — see the plan below | partials within 1 cent of the stretch target on every key; per-partial decay within −25 %/+40 % of published measurements; half-pedal continuous; a blind test (≥ 20 listeners) shows no significant preference for Pianoteq 9 or a sampled grand; 64 pedal-down voices inside 25 % of a core at 256 frames; the legal review done |
| **5. Spectral and granular modes of Exemplar** (Unda's *Grana* grown) | Bitwig 6.1 Sampler (Spectral, Fragments), Ableton Granulator III, Logic Sample Alchemy, Serum 2 | **Technique:** granular scheduling on the sample clock with seeded draws (as Grana), a mip-mapped source so transposing up does not alias, sync/count density, loop and freeze, capture from any track; a phase-locked vocoder for stretch and pitch with formant preservation (Signalsmith Stretch, MIT, as reference), STFT grid anchored to the note, long stretches pre-rendered and cached. The same engine serves warp (§3) and the pitch editor (§3c) | bit-identical at every block size and offline = real time; +24 st grains with no alias above −90 dB; a 2× vocal stretch keeps formants and transients (onset smear ≤ 5 ms); 16 voices × 64 grains inside 10 % of a core |
| **6. Physical-model resonators** (exciter → resonator) | Ableton Collision and Tension, Logic Sculpture, AAS Chromaphone 3, Pigments Modal | **Technique:** exciters (pluck, pick, bow by a friction curve, mallet, noise, sample, live input) into waveguide strings (Gravis's) or modal bars, plates, membranes and tubes (Pulsus's modal voice), coupled, with a body; MPE pressure and slide on bow pressure and position | marimba, vibraphone, kalimba, steel pan, harp, nylon guitar, bowed cello and glass presets each judged "convincing in solo" by a 10-listener panel; pitch within 0.1 cent at any bend; renders repeat to the bit |
| **7. Guitar** (acoustic and electric) | Ample Guitar 4.1, AAS Strum GS-2, NI Session Guitarist | **Technique:** Gravis's string model × 6 with string assignment, an acoustic body, stable sympathetic coupling, pick direction, palm mute, harmonics; a **strummer written as note properties in Cantus's piano roll** (strum time, direction, mute) following the chord track — what no plug-in can do; CC0 Karoryfer electric guitars as Exemplar presets | a chord progression strums like a guitar and follows the chord track; nylon and steel presets judged "a guitar" in a mix by a 10-listener panel; a fretboard strip shows string and fret |
| **8. Additive resynthesis** (Unda's *Partes* grown) | FL Harmor, Logic Alchemy, Zebra 3, Serum 2 Spectral | **Technique:** sinusoids-plus-noise analysis with partial tracking and harmonic assignment, offline, stored as an asset; resynthesis on Partes's rotator bank (no alias by construction), image-to-spectrum, spectral morph between two analyses, the original attack kept | a resynthesised cello, voice and piano pass a 10-listener ABX as "the same instrument"; no partial ever above 0.45·fs (asserted); null at defaults |
| **9. Orchestral section players** | BBC SO Discover (free), Splice INSTRUMENT, Logic Studio Strings/Horns | An Exemplar front-end over **CC0 VSCO 2 CE and VCSL**: articulations by key-switch and by note property, CC1 dynamics across layers, CC11, scripted legato (overlap window and interval-aware crossfade — VSCO has no recorded transitions), release triggers, a section mixer. Honest aim: a finished sketch with zero friction, not a win over BBC SO; later, Cantus's own recordings | a four-bar string line with legato and a CC1 swell is rated "finished" by 7 of 10 producers; presets per section; every pack CC0 and on demand |
| **10. Vowel choir**, then lyric singing | Cubase 15 Omnivocal, Synthesizer V 2, Arturia Augmented VOICES, Hollywood Choirs | **Choir now:** FOF/formant synthesis (Rodet's CHANT, Klatt) with published vowel formants, seeded per-voice jitter, vowel morphing — no recorded voice needed. **Lyric singing later:** WORLD-style source–filter synthesis (BSD) with phoneme timing, gated on a voice recorded for Cantus by consenting singers and released CC0; any neural part single-threaded with a fixed reduction order, or rendered offline and cached | a vowel choir pad judged "usable in a mix" by 7 of 10 producers; bit-identical renders. Singing (later): ≥ 90 % word intelligibility and < 10 cents pitch error on sustained notes; every voice's licence and consent in the notices |
| **11. Acoustic drums** | Superior Drummer 3, EZdrummer 3, Addictive Drums 2 | Mic buses and bleed on Pulsus's layers (each mic a strip), CC4 hi-hat openness, repetition smoothing, CC0 kits (Virtuosity Drums with six mics; Karoryfer) and CC-BY multi-mic kits if ruled in; a browser of CC0 grooves we write | rock, jazz and funk kits with per-mic faders; no identical consecutive sample in a roll (asserted); rated "usable in a mix" against EZdrummer 3's stock kit by a 10-listener panel |

**The piano, as the owner decided it (2026-10-10):**

1. **Now — build it on the published path, which needs nothing from Modartt:** digital
   waveguide strings (J. O. Smith), dispersion filters for inharmonicity (Bank, Välimäki),
   nonlinear hammer models (Chaigne & Askenfelt, Stulov), double and triple strings with
   sympathetic resonance and two-stage decay (Weinreich), a multirate soundboard and a
   sustain-pedal resonance model — coefficients from closed-form physics and published
   measurements, **not** offline finite-element eigen-analysis of a coupled string–soundboard
   with real-time interpolation of the resulting coefficients. Lamina's nonlinear hammer is
   the base.
2. **Later — evaluate adding Modartt's approach** (finite-element modal coefficients
   interpolated in real time) once its patent has expired in every territory where Cantus
   ships, if measurements show it beats ours. The engine's model is versioned, so a second
   model can be added without changing any project's sound.
3. **Before release — a legal review** of our method against the claims of every relevant
   patent found (this backlog is not legal advice). Verified on Google Patents: **US
   7,915,515** (Modartt) is active with every maintenance fee paid and expires
   **2027-10-20**; its claim 1 covers a presynthesis module that turns soundboard and string
   parameters into a damping and a frequency per partial for a real-time partial synthesizer
   — so a modal string bank needs the closest look, a waveguide the least; its European,
   French, Japanese and Chinese counterparts have **lapsed**. The review must also cover
   seven **active piano-resonance patents**: Casio US 8,178,772 (to ~2029), Yamaha US
   8,729,376 (~2032), Kawai US 9,330,648 (~2035), Casio US 10,109,268 (~2037), Yamaha US
   10,818,278 (duplex resonance, ~2038), Yamaha US 12,039,960 (~2041) and Casio US 11,881,196
   (~2042). The Karplus–Strong, Stanford waveguide and commuted-synthesis patents are expired.
   *Meanwhile*, Salamander Grand (CC BY 3.0) can download into Exemplar at once: the owner
   accepted CC BY with the credit shown (ADR 0042, 2026-10-10).

**Also worth adding** (after the table, by family):

- **Synths**: a *chip synth* (NES, Game Boy, SID, YM2612 styles; Plogue's chipsynth line as
  the bar — chipsounds is now legacy) by integer chip state and band-limited steps
  (minBLEP/polyBLEP): cheap, loved, deterministic by construction; a *hybrid/texture synth*
  (Omnisphere 3, Zebra 3, Modartt's new Airteq) once Unda, Exemplar's grains and the
  resonators exist — one Play page over all three; an *FM-plus* in the Sytrus class (Modulus
  can grow into it). Vintage polysynths and the 303 are covered by Calor and Gravis.
- **Samplers**: *playable loops* (Output Arcade: loops across the keys, synced and tuned,
  black-key modifiers) — falls out of the slicer plus the spectral mode; *disk streaming* in
  Exemplar (Kontakt-style preloaded heads) before large libraries; a *sample-instrument
  builder* with scripted round robins and legato.
- **Keys and acoustic**: clavinet, harpsichord, celesta and toy piano as Lamina or resonator
  models; tape keys (Mellotron-like) from CC0 recordings; brass and woodwinds by model (SWAM,
  which needs breath or MPE input) as research.
- **Voices**: lyric singing (item 10, gated); a *vocoder-choir* that sings a played chord
  from the user's own voice (shares §3d's tracker). **Avoid** Yamaha US 7,613,612 (active to
  ~2027-12: a choir made by applying one voice's envelope to a recorded ensemble spectrum) and
  check Yamaha US 11,756,558 (to ~2040) before any neural singing vocoder.
- **Playing**: an MPE-first instrument and on-screen MPE keyboard; drum-synth modules as
  small instruments (Pulsus covers the base); instrument racks (layer and split chains, FL's
  Layer channel); Session-Player-style drummer, bassist and keys that follow a chord track
  (§7), built on item 2.

**Done when** items 1–6 ship with the family's contract tests, a manual page in both
languages and presets made by ear; each meets its measurable criterion above and holds up in
its blind comparison; the piano's legal review is done; the scorecard's *sampling and sound
design* and *built-in content* rows (§0.1) show Cantus first or tied-first against every
DAW's stock instruments; and nothing in the repository is a third party's recording.

## 4. Performance at scale

- **A multicore engine**: the processor renders on one thread; top DAWs spread tracks
  and buses across cores with a dependency graph. *Done when:* a 100-track session with
  plug-ins plays at 128 samples on an 8-core laptop.
- Disk streaming of long audio (ADR 0025 left it), lazy loading of samples, a frame
  budget for 200-track arrangements, startup under two seconds.

## 4b. Performance beyond the market (requested 2026-10-05)

*The goal: more tracks and plug-ins than any DAW on the same machine, at a lower buffer,
with zero dropouts — proved by a published benchmark, not claimed.* The principle the best
engines share: **never do work twice, never do work nobody hears, and never let the
real-time path wait.**

**The engine**
- **Anticipative processing** (REAPER's) / **dual-path buffers** (Cubase's ASIO-Guard,
  Logic's live vs playback): tracks that are *not* armed or monitored are rendered ahead
  of time on worker threads in large blocks; only the live path (armed tracks, the input,
  the instrument being played) runs at the 32–64-sample device buffer. This is the single
  biggest lever — it is how REAPER plays sessions others choke on.
- **A multicore graph scheduler**: the track/bus/send/sidechain graph (we already compute
  `strip_order` and delay compensation) split into tasks, run by a lock-free
  work-stealing pool of real-time threads; the critical path first; no allocation, no
  mutex, no syscall in the callback.
- **Real-time OS priority**: MMCSS "Pro Audio" on Windows (`AvSetMmThreadCharacteristics`)
  for the audio and worker threads, Mach real-time policy on macOS, `SCHED_FIFO` /
  rtkit on Linux; worker threads pinned to physical cores, away from the UI thread.
- **A render cache — automatic background freeze** (beyond the market): a track whose
  inputs (notes, parameters, automation, plug-in state) have not changed since its last
  render is played from a cached render on disk instead of being computed; any edit
  invalidates exactly what it touches. Heavy synths cost nothing once they stop changing.
  Ableton/FL/Cubase freeze only by hand.
- **Silence and tail awareness**: suspend any instrument or effect whose input is silent
  once its tail has died (CLAP `tail`, our units' `is_quiet`), wake it on the next event;
  extend the "idle track" skip we have to every node of the graph.
- **SIMD everywhere hot**: explicit AVX2/AVX-512/NEON with runtime dispatch in the DSP
  kernels (filters, oscillators, convolution, oversampling); synth voices processed 8 at
  a time across SIMD lanes; structure-of-arrays voice state.
- **FTZ/DAZ** set in the callback (STATUS round 17 measured 6 % on Sibilus for free).
- **Memory**: per-thread arenas built off the audio thread, cache-line-aligned buffers,
  no false sharing between workers, buffer reuse by liveness analysis of the graph.
- **Plug-in hosting at scale**: out-of-process sandboxes (crash isolation) sharing audio
  through shared memory with lock-free rings, grouped per process to keep the overhead
  low (Bitwig's model); per-plug-in CPU accounting; plug-ins that report latency changes
  handled without a dropout.
- **Disk**: memory-mapped, prefetched audio streaming with a lock-free reader; projects of
  hours of multitrack audio open instantly (peaks cached — round 19 did this for drawing).
- **Research**: GPU DSP for convolution and large FFT work (experimental), and how far
  sample-accurate parameter smoothing can be made branch-free.

**Offline**
- **Export faster than any DAW**: render independent tracks in parallel and in large
  blocks (no real-time constraint), stems in parallel, at 50–200× real time; the render
  cache makes re-exports near-instant.
- Plug-in scanning in parallel, cached, incremental; never blocking start-up.

**The interface**
- Start-up under one second to a playable project (lazy plug-in loading, a cached scan,
  instruments built on first note), project open under a second for 100 tracks.
- 120/144 Hz rendering with input-to-photon latency measured; virtualised views for
  1 000 tracks and 10 000 clips; waveforms and notes as cached meshes (rounds 9 and 19
  started this); zero work when idle (round 18 already guarantees no repaint).

**Proof — a benchmark that keeps us honest**
- A reproducible **DAWbench-style suite** (DSP: N instances of a reference effect chain;
  VI: N voices of a reference synth; a 100-track mix) run on the same machine against
  REAPER, Ableton Live, FL Studio, Bitwig and Cubase; results published with the method.
- **Performance gates in CI**: per-kernel benchmarks, a frame-time budget (exists for the
  UI), callback time histograms; a regression fails the build.
- **A dropout detector with blame**: every xrun records which node ran long, shown to the
  user ("Spatium on Vocal took 3.1 ms") — no DAW tells you this today.
- *Done when:* Cantus sustains more instances than every DAW in the suite at 64 samples
  without a dropout, and exports faster than all of them, on a mid-range laptop.

## 4c. A recording mode for high performance (requested 2026-10-10)

*The owner: "a partial pre-render of every channel that feeds the master — audio, instruments,
with all their effects and automation — each one on its own, just before the master, so while
recording audio only audio is played and summed. Less lag, a smaller buffer, and the musicians
hear everything more faithfully. Manual, always clear that you are in it; to change anything,
leave the mode and pre-render again."*

**What it is.** A manual action, **Prepare to record**, renders each strip that feeds the master
to its own file (32-bit float, at the project's rate), then enters **Recording mode**: the engine
stops running the project's instruments and effects and only reads those files and sums them,
through the live master. It is Freeze, applied to the whole project at once and held as a mode
with a name.

**What it buys, said honestly** (the page and the dialog must say it the same way): it does not
shorten the interface's own round trip (buffer, driver, converters). It lets the buffer go
lower without dropouts because the CPU is nearly idle, it removes the delay compensation of the
frozen strips, and the musician's own channel is the only live path. The status chip shows the
buffer, the measured round-trip and the CPU before and after.

**Decisions (the owner, 2026-10-10)**
- **The recording strip(s) stay live.** Armed strips keep their input, monitoring and chain in
  real time, so the musician hears themself with the least delay. Everything else is baked.
- **The master is a choice**: *live* (default — the real master chain runs on the sum, the
  musician hears the final mix) or *baked* (the whole project is one stereo reference; the
  lightest, but nothing of the master can move).
- **The mix controls are a choice**: *live* — fader, pan, mute and solo (with their automation,
  a gain curve) act at the sum, so "more of my guitar in the headphones" needs no exit — or
  *locked* — those controls are greyed too. To make that work the render is taken **after the
  inserts, sends and effect automation and before the fader and pan**; with the controls
  untouched the result is bit-identical to normal playback.
- **Any other change is blocked, with a notice.** While the mode is on, everything that would
  change the sound (notes, clips, effects, instruments, automation, routing, tempo) is greyed
  out and says "Recording mode — leave it to edit". It is never entered or left by itself.
- **A bar that cannot be missed** across the window while the mode is on: its name, the
  buffer and round-trip, the CPU saved, how many strips are baked, and **Leave**. Leaving keeps
  the rendered files by default (or discards them, the user's choice).
- **Re-entering is cheap**: each strip's render is keyed by a hash of everything that makes its
  sound (its model, its notes and clips, its effects and their settings, its automation, the
  hosted plug-ins' state, the tempo map); only the strips whose hash changed are rendered again.

**Design questions for the ADR**
- *Buses and returns:* the renders are the strips whose output goes to the master. A bus is one
  render that includes what it sums, so a strip inside a bus has no separate mix controls in the
  mode (they are greyed); sends feeding a return are baked into the return's render.
- *Storage:* 10 minutes of stereo float at 48 kHz is about 230 MB a strip; 30 strips is about
  7 GB. The files live beside the project and are **streamed** by a reader thread with a
  prefetch ring (the audio thread does no I/O, ADR 0005), never held whole in memory; the
  dialog shows the size before it starts. Consider 24-bit or lossless compression.
- *Hosted plug-ins* must render offline and deterministically; one that cannot is reported by
  name and stays live (or stops the preparation, the user's choice).
- *Taking a take:* the new recording lands on the live strip and joins the project when the mode
  is left; during the mode it plays through its own strip.
- *The state is the session's, not the project's:* a project reopens out of the mode (no schema
  bump), the cache index and files stay on disk and are checked by hash.

**Tests first** (CLAUDE.md): a null test (baked + summed equals the normal render to the bit,
mix controls included); block-size independence at tolerance 0; delay compensation keeps the
strips aligned; **every `Command` is classified as changing the sound or not, and a new one
fails the build until it is** (the edit lock cannot drift out of date — ADR 0004); a changed
strip invalidates exactly its own render; a crashed session reopens out of the mode;
`rt_memory.rs` counts zero allocations in the mode's audio path.

**What the others offer — replicate each piece first** (from memory, 2026-10-10: *verify each
against the product before it is quoted anywhere public*). None of them has the whole mode as
named here; each has a part, and the parts are worth having on their own:

| Piece | Where it exists | Cantus |
|---|---|---|
| Freeze one channel to audio, with its effects (and un-freeze) | Ableton *Freeze/Flatten*, Logic *Freeze*, Cubase *Freeze*, Reaper *Freeze track*, Studio One *Transform to Audio*, Pro Tools *Commit* | step 1 |
| Low-latency monitoring: while a track is armed, skip the plug-ins whose delay is above a limit | Logic *Low Latency Mode*, Cubase *Constrain Delay Compensation*, Pro Tools *Low Latency Monitoring*, Studio One *Low Latency Monitoring* | step 2 |
| A reduced-latency path for the live input while monitoring | Ableton *Reduced Latency When Monitoring* | step 2 |
| Bounce the backing to one stereo file and record against it | every DAW, by hand | the mode keeps the strips apart |
| **The whole project baked at once as a named mode, a banner, a lock, mix controls and master as choices** | none that I know of | step 3 — Cantus's own |

**Order of work** (so the mode is the last layer on two things every serious DAW needs anyway):
1. **Freeze a channel to audio** (§3), with the per-strip hash (a content key for what makes a
   strip's sound) and the on-disk cache. Freeze, Un-freeze and "frozen" shown on the strip.
2. **Low-latency monitoring** for armed strips: a limit (samples or ms) above which the
   plug-ins of a monitored strip are bypassed while recording, listed on screen by name, and
   restored on stop; plus the round-trip figure on the audio chip (§6b).
3. **The recording mode** above, on top of 1 and 2: *Prepare to record* freezes every strip that
   feeds the master in one go; the lock, the bar and the choices are this step.

**Where it helps, and where not** (the owner's question, 2026-10-10): it helps a heavy project
(big instruments, many latency plug-ins) on a modest machine recording at a low buffer; it gives
the musician a headphone mix that can still be changed (each strip stays apart, the fader is
live); and it removes the dropout in the middle of a good take. It does **not** shorten the
interface's own round trip, many musicians already solve the case by bouncing a stereo backing
or opening a clean project, it costs real disk (about 7 GB for a large project) and a lot of
care (hashes, the command lock, bit-exact tests), and it matters less as the multicore engine
(§4b, wave 2) gets faster. So: keep it, build it last, and let steps 1 and 2 earn their place
first.

**Related:** automatic background freeze (§4b, §3 "freeze tracks") shares the cache and the
per-strip hash; the CPU-saved figure reuses the callback histogram and dropout blame (§4b).
**Done when** a 40-strip project with heavy instruments records an audio take at the interface's
smallest stable buffer with a CPU load a fraction of normal playback's, the musician can tell
the mode is on at a glance, and leaving it, editing and coming back re-renders only what changed.


## 4d. The GPU as an optional accelerator (asked 2026-10-10)

The owner asked whether the engine could use the GPU, optional and configurable, or CPU
and GPU together. **Not a replacement for the engine**: real-time audio is small blocks
under 1–5 ms deadlines, a CPU→GPU→CPU round trip costs latency, and no operating system
schedules the GPU with real-time guarantees. **Useful where the work is massively
parallel or not under a deadline**:

- **offline**: export, freeze, bounce (no deadline at all);
- **heavy parallel DSP**: partitioned convolution with long impulse responses, spectral
  work (time-stretch, the §3c pitch editor, denoise), the §7 AI models (stem separation),
  additive or physical-model synthesis with thousands of partials;
- **mixed CPU + GPU through anticipative processing** (§4b): tracks not being recorded or
  played live are computed ahead in large buffers, where the GPU's latency stops mattering;
  what is armed or played live stays on the CPU;
- **the interface**: waveforms, spectrograms, meters.

Rules: always a CPU fallback (a block the GPU misses is computed on the CPU, never a
dropout); a setting (*Off* / *Offline only* / *Mixed*); the GPU path in its own crate
(`wgpu`: Vulkan, DX12, Metal) so `cantus-engine` stays `forbid(unsafe_code)`; the
bit-exactness guarantee either kept with deterministic kernels or the GPU path declared
"equivalent within −120 dB" and excluded from it, documented per process. An integrated
GPU with shared memory (the owner's Ryzen 7 7735HS) has no PCIe trip — a good first
bench. **Order**: after anticipative processing and plug-in sandboxing; first a measured
prototype (GPU partitioned convolution vs. the CPU one: latency, CPU saved, misses), then
spectral and AI work if the numbers hold. *Done when:* a GPU-accelerated process is
measurably cheaper than its CPU twin at 256 frames with zero misses, and switching the GPU
off changes nothing but the CPU load.


## 4e. The engine against the market — to evaluate and research (asked 2026-10-10)

From an honest assessment the owner asked for (*how does our engine compare with the
market?*). **Already first-rank by design**: bit-exact determinism (offline = real time,
parallel = serial, any block size — tested, fuzzed; no commercial DAW is known to promise
it), real-time discipline enforced by tests (no allocation, lock or I/O on the audio
thread), a multicore pool that also splits one heavy instrument's voices (round 24,
ADR 0038), delay compensation, one build with ASIO and WASAPI. **Not yet proved against
anyone**: no number has been measured beside another DAW. Each item below is to be
evaluated and researched, with the section that holds it where one exists:

1. **A measured comparison** (the first step, so every other claim rests on data):
   DAWbench-style DSP and VI tests (§0.1, §4b "Proof") run on the same machine in Cantus,
   REAPER and FL Studio — most tracks/plug-ins at 64/128/256 frames, round-trip latency,
   CPU at a fixed load; published with the method.
2. **Plug-in crash isolation** (§4b "Plug-in hosting at scale"): a hosted plug-in that
   crashes takes Cantus with it today; Bitwig and REAPER's bridge isolate them. The most
   important gap for real users with third-party plug-ins.
3. **Anticipative processing** (§4b): REAPER-style rendering ahead of tracks not armed or
   played live, in large buffers — huge sessions at low latency; also the base for §4d's
   mixed CPU + GPU mode.
4. **Scale proved, not assumed**: sessions of 200 tracks with 50 third-party plug-ins
   (commercial synths and effects, not only ours) at 128/256 frames, a 10-minute live run
   on a real interface with zero overruns (BETA blocker 2's second half), and the UMC in
   duplex.
5. **Voice splitting for the remaining instruments** (ADR 0038 covers Calor, Unda,
   Exemplar): Modulus, Lamina, Organum, Gravis and Pulsus II; and the **CLAP
   `thread-pool` extension**, so hosted plug-ins can use the same pool (the same
   `cantus_rt::fork` could serve it).
6. **Audio features the market takes for granted, on the engine's side**: warp /
   time-stretch and pitch-shift of audio clips (§3), **ARA 2** so Melodyne and similar
   editors work inside a clip (new; weigh it against our own §3c editor), surround and
   Dolby Atmos (§0.2).
7. **Platforms** (§2): Linux first, then macOS.
8. **Proof of the concurrency core**: a loom model of `cantus_rt::fork` and the pool's
   queues and cells (only stress tests and the fuzz hold them today), and Miri where it
   applies.
9. **The GPU as an optional accelerator** (§4d).

*Done when:* item 1 is published and every item above has either landed with its own
measurement or been decided against with the reason written down.

## 5. Content

- A **factory sound library** (round 20 builds the content manager): CC0 drum kits,
  Cantus's own rendered kits (an original 808/909 family from Pulsus), instruments from
  CC0 sample libraries (Osiris Piano, VSCO 2 CE, VCSL, Karoryfer) loaded in Exemplar.
- **Presets**: a shared preset library across instruments with tags and audition through
  the user's own clip (research/instruments/family.md §6); dozens of musical presets per
  instrument and effect, made by ear.
- **Instrument gain staging**: Lamina and Exemplar sit 8–9 LU under the others at unity
  (docs/round19/fixes-mix.md) — calibrate with a migration so old projects keep their sound.
- Demo songs by real musicians, and template projects per genre.
- **CC BY content (ADR 0042, the owner, 2026-10-10)**: CC0 *and* CC BY 3.0/4.0 packs are
  accepted, with the credit always shown (NC, ND, SA still refused). To do in
  `cantus-content`: the manifest test accepts `CC-BY-3.0`/`CC-BY-4.0` and requires a credit
  line and the licence file; *Sound packs…* and *About ▸ Credits* show the credits;
  `xtask notices` generates a *Content* section. First packs: Salamander Grand (for
  Exemplar, ahead of the modelled piano), Naked Drums, the DrumGizmo kits (not
  *Aasimonster*), Dustyroom's acoustic kit; VocalSet for the voice work.

## 6. Product

- **Installer** (MSI/NSIS on Windows, DMG, AppImage/Flatpak), **code signing** (Windows
  SmartScreen and macOS notarisation), **auto-update** with release notes, file
  associations for `.cantus`, a portable mode.
- **Manual and tutorials**: a user manual (EN/ES), first-run onboarding, contextual help
  (the hint bar exists), video tutorials, keyboard-shortcut cheat sheet.
- **Licensing and legal**: the decisions in `research/content/README.md` and
  `research/ai/README.md`; the trademark search for "Cantus"; credits/attribution screen.
- An opt-in crash reporter; a website (§6a: cantusdaw.com); a community (forum/Discord).

## 6a. The website and its backend — cantusdaw.com, cantusdaw.studio (requested 2026-10-07)

*The owner bought **cantusdaw.com** and **cantusdaw.studio**: "a page to download — maybe a
backend too — with manuals, sound packs, a count of users, maybe serving the updates."*

- **The site** (cantusdaw.com; cantusdaw.studio redirects to it, or hosts the community):
  home with what Cantus is and its values, **download** (the setup and the portable zip,
  SHA-256 sums and the signature shown, the detected OS and CPU), release notes and
  changelog, the **manual** in EN/ES (`xtask manual` already renders it to HTML), the
  plug-in pages (`docs/plugins/`), tutorials and videos, the scorecard (§0.1) when it
  exists, privacy and licences, the source on GitHub, donations.
- **Sound packs**: host and mirror the packs of `crates/cantus-content/manifest/content.json`
  (today they download from their authors' own servers — a mirror with the same pinned
  SHA-256s keeps them available), then Cantus's own packs and community packs.
- **Updates**: serve the update manifest from our domain (GitHub Releases stays the
  binary store or a fallback); signed manifests, staged roll-outs, a beta channel. Changes
  `crates/cantus-update` and ADR 0032.
- **Counting users without telemetry** — `PRIVACY.md` promises no telemetry, so: download
  counts on the site; the daily anonymous update check counted as a number of requests per
  version (no identifier, no IP kept); an **opt-in** account or registration for those who
  want news, cloud presets or the forum. Any change is written in `PRIVACY.md` first.
- **Backend** (small, open source like the rest): the manifest and pack mirror behind a
  CDN, download and update counters, opt-in accounts later (preset/pack sharing, the
  community's uploads with moderation), an opt-in crash-report endpoint (§6), a status
  page. Hosting with a free tier first (static site + object storage + a small API).
- **Legal and security**: HTTPS everywhere, the trademark check for "Cantus" (BETA.md)
  before the site goes public, the RVR1960 attribution, GDPR/LFPDPPP-compliant if accounts
  exist.

- **Landing page, in detail (2026-10-09)** — see also `docs/monetize.md` §7. **cantusdaw.com**
  is the front door: what Cantus is, a demo video/real-project music, downloads + release
  notes + system requirements, the manuals, a bug-report link, FAQ, privacy, a donation page,
  and a plain "what is free and what is optional" policy. **cantusdaw.studio** is the
  ecosystem: the catalogue of Cantus's own instruments and effects (also as stand-alone
  plug-ins when they exist), **extra audio packs** (free first, paid ones later), presets,
  the API/MCP and developer docs, creators' collaborations, licences. Start static (a static
  site + object storage behind a CDN) and add subdomains (`downloads.`, `docs.`, `status.`,
  `api.`) only when needed. The page uses the new white-and-blue look and logo (§6d.4).

**Done when** a user on a clean PC goes from a search to cantusdaw.com to a running Cantus
in under two minutes, the packs and updates come from our domain with their hashes, and the
user count on the site matches the published method.

## 6b. Audio setup that is direct (requested 2026-10-05)

Today the audio device lives in Settings ▸ Audio (round 19 made it complete); the owner
wants configuring audio — interface, driver, inputs/outputs, rate, buffer — to be easier
and more direct.

- **An audio chip always visible** in the transport or status bar: the interface's name,
  driver, rate, buffer and round-trip latency (e.g. `UMC204HD · ASIO · 48 k · 128 · 6.1 ms`),
  a light that turns amber on dropouts and red when the device is lost. One click opens a
  compact popover to switch interface, driver, rate and buffer *in place*, with the
  latency shown for each buffer size before choosing.
- **First-run setup**: pick the interface from pictures/names of what is connected,
  play a test tone on the chosen outputs, show live input meters ("speak or play — the
  meter should move"), suggest the best driver (ASIO when the interface has one) and a
  buffer, and offer the loopback calibration.
- **Plug-and-play**: detect an interface being connected or removed and offer to switch
  (and back) without restarting; remember settings **per interface**; recover cleanly
  when a device disappears mid-session (keep editing, say so, reconnect when it returns).
- **Inputs and outputs named and routed simply**: name the device's channels ("Mic 1",
  "Guitar", "Monitors L/R"), a routing matrix page, output to a second pair for a
  headphone cue mix.
- **Problems explained in words with a fix**: "the input runs at 44.1 kHz and the output
  at 48 kHz — use one interface or set both to 48 kHz [Fix]", "another app holds the
  device exclusively", "the ASIO driver stopped responding — [Restart audio]".
- *Done when:* a musician who has never opened Cantus records a guitar through a new
  interface in under a minute without opening Settings.

## 6c. A verse at every start — Cantus's Christian values (requested 2026-10-05)

The owner: *"every time it starts, show a verse from the Reina-Valera in the configured
language — our DAW has Christian values."* (The project's motto is Colossians 3:23.)

- On the splash screen (round 18 made one) and in the welcome/start view, a verse with its
  reference, in the interface language, chosen at random (or one per day), from a curated
  list — verses of encouragement, work, creativity, praise and music (Psalms 33:3, 96:1,
  150; Colossians 3:23; Ephesians 5:19; Exodus 35:31–35; 1 Chronicles 15:16…).
- **Translations and licences (decide before shipping):** Spanish **Reina-Valera 1909 is
  public domain** and can be bundled; **RVR1960 and RVR1995 are under copyright
  (Sociedades Bíblicas Unidas / American Bible Society)** and need written permission or
  their licence terms (usually a limit on how many verses may be quoted without
  permission — a curated list of a few hundred verses may fall inside a quotation
  allowance; check). English: the **King James Version** is public domain outside the UK
  (Crown patent in the UK); the World English Bible (WEB) is public domain everywhere.
  Other languages: public-domain translations per locale (e.g. Almeida for Portuguese,
  Louis Segond 1910 for French, Luther 1912 for German).
- Bundled as a small text file per locale (`locales/verses/<lang>.txt`), the reference
  shown in that language's book names; the translation's name and licence credited in
  About.
- **The owner's choice of translation (2026-10-05): RVR1960 with a short list if its
  terms allow it; otherwise the public-domain RV1909.** What was found (search on
  2026-10-05, American Bible Society rights page,
  https://bibles.com/pages/american-bible-society-rights-and-permissions): the RVR1960 may
  be quoted, in any form including electronic, **up to 500 verses without written
  permission**, provided the verses are not 50 % of a book of the Bible, are under 25 %
  of the total text of the work, **the work is available for non-commercial use**, and it
  carries the acknowledgement *"Reina-Valera 1960 © Sociedades Bíblicas en América
  Latina, 1960. Renovado © Sociedades Bíblicas Unidas, 1988. Utilizado con permiso."*
  **To research before shipping:** (1) whether a free GPL app counts as non-commercial
  when the GPL lets others sell copies — ask SBU/ABS in writing, or keep the verse file as
  separately licensed data that downstream sellers must drop; (2) the same terms for the
  other locales' modern translations; (3) read the terms on the publisher's own page, not
  only a search excerpt. **Decision (owner, 2026-10-05, later the same day): ship the RVR1960** under its quotation allowance — under 500 verses (aim 200–300), the acknowledgement in About and *RVR1960* after each reference — with the public-domain RV1909 kept as a selectable fallback. A written confirmation from SBU/ABS about the non-commercial clause is still worth asking for.
- **On by default, with an On/Off switch** (decided by the owner, 2026-10-05: *"so someone
  who does not share our beliefs and is bothered by it can turn it off — but ON by
  default"*): Settings ▸ General ▸ *Show a verse at start*, plus a small *don't show* link
  on the verse itself; remembered per user. A setting to choose the translation where
  several are available.
- *Done when:* every start shows a verse in the current language (unless switched off),
  from a translation whose licence allows it, credited in About.

## 6d. Editing, mixer and look — the owner's requests (2026-10-07)

Found while trying the round-22 build. Each one gets an ADR before code; every mutation is a
`Command` with an inverse and an arm in the undo fuzz (ADR 0004).

1. **Copy and paste (Ctrl+C / Ctrl+X / Ctrl+V) in the piano roll and the arrangement** —
   notes, clips and time selections; paste at the playhead or the selected time/track, with
   the grid snap; across projects (a clipboard format that survives a restart); the same
   keys in the channel rack. Cut/copy/paste/duplicate appear in the menus with their keys.
2. **Linked clips (FL's pattern clips, Ableton's linked clips).** A copied instrument clip
   (piano-roll content) in the arrangement is **the same clip**: editing one changes every
   copy. A **"Make unique"** command (context menu, toolbar, a key) detaches one copy into
   its own clip; and the inverse ("Link to…") where it makes sense. **Audio clips too**: copies
   share their settings (gain, fades, pitch, stretch/warp, reverse, per-clip effects) until
   made unique, so changing a setting once changes every copy. Shared clips show it on
   screen (a link mark in the header; a "×N" count; the selection highlights the siblings).
   Model change → schema bump + migration + `fixtures/vN.cantus`; old projects keep loading
   (every existing clip stays unique). Paste's default (linked or unique) is a setting.
3. **A better mixer — cabling between channels, as in FL Studio.** Routing drawn as cables
   between strips (send/route lines, with their level), made by dragging from one strip to
   another; a routing view/matrix; sidechain sources shown in the same way; the
   "Nothing feeds this strip yet" hint (seen on the Reverb/Delay strips) turns into a
   direct "route here" action. **Remove the channel's effect slots from the *Extra large*
   view** (the inserts live in the Inspector; the strip shows level, pan, sends and routing).
   Take the best of FL Studio's mixer and of Ableton, Bitwig, Logic, Studio One and
   Reaper's routing; study them in `research/` before designing (feature by feature, as
   for the rest of the mixer).
4. **A new default theme: white with blue details, and a logo to match.** A light theme
   built on white surfaces with blue accents, from the owner's reference screenshot
   (2026-10-09: the arrangement + extra-large mixer + inspector on near-white surfaces, cool
   grey-blue panel headers, saturated blue for the active/selected state — the transport
   toggles, the selected strip and fader, the focus rings — soft pastel track colours,
   dark-slate meters; the **current light look is the base**, to be unified and sharpened,
   not replaced), a new **logo** (the app icon, the title bar
   mark, the installer, the splash) drawn to go with it, and it becomes the **default**.
   The dark theme is redone as a **dark theme with blue details** (same accent family), so
   the two are one design in two lights. Tokens only (the kit's colour roles, ADR 0024; no
   literal colours in views); the old themes stay selectable; `shoot` PNGs of every view in
   both themes are the check; the logo ships as a vector source plus the `.ico`/PNG sizes.
   *Round 25 (look, ADR 0046):* shipped as **Daylight** and **Blue Hour**, two of eight new
   themes — but **not the default**: on 2026-10-10 the owner asked for the default to go
   with the website, so it is **Cantus Night** (copper and amber on night) with **Cantus
   Paper** as its twin. The icon became the website's tile-less mark, its colour a setting.

## 6e. The top bar — a transport zone with more in it (requested 2026-10-09)

The owner's reference is FL Studio's top strip (two screenshots, FL 12 and the current one):
a dense, well-grouped toolbar. Ours (metronome, loop, record, BPM, position, undo/redo,
view toggles, master meter) is sparse and the pieces float apart.

- **Regroup and densify** the zone around the metronome: transport (play, stop, record,
  loop), tempo block (BPM, tap, time signature), metronome with its count-in/pre-roll
  and volume in the same group, the song/pattern mode switch, snap/grid, the position as bars
  and time, the CPU / RAM / voices / dropout readout (today in the status bar), the master
  meter with a peak/LUFS toggle, and the project name/title; one visual group per function,
  separated by the kit's dividers, nothing that collides at any width.
- **More functions in reach**: tap tempo, metronome options (sound, accent, count-in bars),
  a pattern/arrangement selector, a mini-oscilloscope or spectrum, MIDI-input and
  record-arm indicators, the audio-device chip (§6b), a quick-search/command box, the
  tool/skin/zoom shortcuts; the owner can **show/hide and reorder** groups.
- **Responsive**: groups collapse into a menu in order of importance when the window is
  narrow (never overlap or clip), and the bar reads well at every UI zoom.
- Look: first-rank, in the new white-and-blue theme and its dark twin (§6d.4); judged
  against FL's top strip with `shoot` PNGs at several widths. Touches `crates/cantus-ui`
  only (a new area locale file; strings in EN and ES).

## 6f. How Cantus earns money — the owner's plan (`docs/monetize.md`, 2026-10-09)

The plan is in `docs/monetize.md` (Spanish). Its rule: **monetise around the free product,
never by restricting it** — the DAW and its essential functions stay free, no mandatory
account, no telemetry, privacy by default. Streams, in the order the plan proposes:

1. **Voluntary donations** (recurring tiers $3/$5/$10/$25 and one-offs; they unlock no
   musical feature; public thanks if the donor allows; periodic reports of the use of funds).
2. **Optional paid sound packs** (a few high-quality packs first, free ones to attract users;
   each states its content, format, licence; ADR 0030: packs never in the repository).
3. **Free instruments and effects** as the funnel, also as standalone plug-ins (CLAP/VST3,
   only announced once implemented and tested) to reach people who keep their DAW.
4. **Corporate sponsorship** (interface/controller makers, schools), with no control over
   the roadmap or the users' data.
5. **Grants and funds** (open source, culture, accessibility, low-latency audio; check the
   legal entity each requires).
6. **Professional services** (integration, support contracts, training, funded features).
7. **Optional hosted AI** only later and only if sustainable; bring-your-own-key and local
   models first (`docs/BACKLOG-excellence.md` §7).

Targets and phases are in the document (a planning goal of about US$14–16k gross a month for
$10k net; phases: reliable beta → public launch → API/MCP → paid packs → institutional
funding → hosted services). **Work this backlog implies**: a donation page and a
transparent "what is free" policy on cantusdaw.com (§6a); a pack store only when a pack
exists (digital delivery, receipts, tax handling); the **licence decision** (the repo says
GPL-3.0-or-later; check dependencies, sample/preset/brand licences separately); the
trademark search; the legal entity and the payment provider (Open Collective / GitHub Sponsors /
Patreon / Stripe-Paddle — compare fees and countries); the metrics of §10 of the plan, taken
without telemetry (opt-in surveys, downloads, anonymous update checks — §6a).

## 6g. Double-click to open, click the name to rename — FL's quick gestures (requested 2026-10-14) — **done in round 27** (ADR 0051)

The owner, during round 26: the arrangement and the inspector should open things the way FL
Studio does, with no hunting through menus. Each gesture gets a test that drives it in the
whole window, and Esc never loses work. **The owner's clarification of 2026-10-14 replaced
item 2's first wording ("rename by typing"): a track is renamed only by clicking exactly on
its name's letters, never by typing with a track selected.** All four landed in round 27
(`r27/gestures`, `crates/cantus-ui/tests/quick_gestures.rs`):

1. ✓ **Double-click a track header in the arrangement → open its instrument** (a native
   instrument's editor, Pulsus's, or a CLAP/VST3 plug-in's own window — brought to the front
   when it is open already). Firmus (edited in the inspector) and an audio track (its
   instrument never plays) select the track and show the inspector.
2. ✓ **Rename by clicking on the name's letters** (only there): the click also selects the
   track; the kit field opens in place with the name selected; Enter commits as one undo
   step (`SetTrackName`), **Esc cancels and closes** leaving the old name, a click away
   commits; no shortcut or the typing piano reads the keys while it is open. The letters
   show an I-beam and the hint *Click: rename*. Typing never starts a rename. (The mixer's
   strips and the rack's pads keep their double-click rename — ADR 0051 §2 says why.)
3. ✓ **Every *Open editor* brings the window to the front** — restored if minimised (from
   the tray, or the taskbar for a plug-in's window, which stays maximised if it was), its
   tab in front, its window raised, focused — for native instruments, Pulsus, effects (the
   mixer menu, the inspector's inserts) and hosted plug-in instruments and effects. The
   inspector's hosted row and a hosted effect's card read *Show* while the window is open
   (they used to close it); a plug-in that floats a window of its own keeps *Close* (Cantus
   cannot bring it forward). F8 and the rack's channel button bring an editor that is not
   in front forward and close only the one in front.
4. ✓ **Double-click a clip in the arrangement → open its editor, on it**: the piano roll
   (fitted to the clip, focused), or the channel rack when the rack shows the clip **with
   nothing lost** — decided from the content, not a stored hint (no schema change): every
   note a hit on a step of the clip's own grid, one pitch per channel on a melodic
   instrument (a melody opens the piano roll, as in FL; a kit may use all its pads); the
   rack opens at that grid, scrolled to the clip's track. Automation clips keep their
   editor; an audio clip (no audio editor exists) shows in the inspector; an empty lane
   still makes a clip.

## 7. AI (research done in round 20: `research/ai/`)

Phase A tools foundation → B an MCP server → C the assistant panel → D local audio
intelligence (stems, audio-to-MIDI, mastering assistant) → E generative audio.

## 8. Polish

- Accessibility (screen-reader semantics for the painted views, keyboard-only operation,
  colour-blind-safe meters), UI zoom tested at every step, high-DPI on every monitor.
- The visual details only use reveals: alignment, wording, empty states, error messages
  that tell you what to do, consistent icons; a design review of every view against FL.
- Localisation beyond EN/ES (PT, FR, DE, JA) once strings settle.
