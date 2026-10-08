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
learn, crossfades, congelar pistas, marcadores, un editor de audio; **(3b)** más efectos: pedales de distorsión y fuzz, chorus, flanger, un multiefecto tipo Distructor, corrección de afinación (tipo Newtone), manipulación de tiempo (tipo Gross Beat/NewTime), controlador de automatización por fórmula, clipper, analizador tipo Wave Candy, controlador X-Y, separador de transitorios, procesador de transitorios, y el resto de la familia que tienen las DAW más profesionales; **(3c)** un editor de afinación y tiempo para grabaciones, nivel Melodyne (notas como bloques, deriva, vibrato, formantes, timing; no destructivo; luego polifónico y ARA 2); **(3d)** un afinador automático en tiempo real (tonalidad y escala, notas a elegir o desde MIDI, velocidad de corrección, formantes, baja latencia para monitorear al grabar); **(4)** rendimiento con
sesiones grandes y un rendimiento por encima del mercado (procesamiento anticipado, motor multinúcleo, congelado automático en segundo plano, SIMD, benchmark publicado contra las demás DAW); **(5)** contenido (librería de sonidos, presets);
**(6)** configuración de audio directa (un chip siempre visible con interfaz, driver, frecuencia y buffer; asistente de primer uso; conectar y usar); un versículo de la Reina-Valera (1909, dominio público) en el idioma configurado en cada arranque (activado por defecto, con opción de apagarlo); producto: instalador, actualizaciones, firma de código, manual y tutoriales,
macOS/Linux; **(6d)** pedidos del dueño (2026-10-07): copiar/pegar (Ctrl+C/X/V) en el piano roll y el arreglo; clips enlazados (copiar un clip de instrumento o de audio da *el mismo clip*: editar uno cambia todos) con la opción "Hacer único"; mezclador con cableado entre canales como FL Studio y sin efectos del canal en la vista extra grande; un tema nuevo blanco con detalles azules (por defecto, con logo nuevo) y un tema oscuro con detalles azules; **(7)** pulido: nivel de los instrumentos, accesibilidad, detalles visuales.

## 1. Real use — the test that matters most

- **Make complete songs in Cantus**, start to finish (record a real instrument and voice,
  program drums, arrange, mix, master, export), and log every moment of friction, however
  small. *Done when:* three songs of different styles were finished in Cantus alone and
  the friction log is empty or scheduled.
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
  three OSes pass the suite.

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
  for vocals), slicing to a sampler, consolidate / **bounce in place**, **freeze** tracks,
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
   built on white surfaces with blue accents, from the reference screenshot the owner is
   sharing (**pending: the owner sends it**), a new **logo** (the app icon, the title bar
   mark, the installer, the splash) drawn to go with it, and it becomes the **default**.
   The dark theme is redone as a **dark theme with blue details** (same accent family), so
   the two are one design in two lights. Tokens only (the kit's colour roles, ADR 0024; no
   literal colours in views); the old themes stay selectable; `shoot` PNGs of every view in
   both themes are the check; the logo ships as a vector source plus the `.ico`/PNG sizes.

## 7. AI (research done in round 20: `research/ai/`)

Phase A tools foundation → B an MCP server → C the assistant panel → D local audio
intelligence (stems, audio-to-MIDI, mastering assistant) → E generative audio.

## 8. Polish

- Accessibility (screen-reader semantics for the painted views, keyboard-only operation,
  colour-blind-safe meters), UI zoom tested at every step, high-DPI on every monitor.
- The visual details only use reveals: alignment, wording, empty states, error messages
  that tell you what to do, consistent icons; a design review of every view against FL.
- Localisation beyond EN/ES (PT, FR, DE, JA) once strings settle.
