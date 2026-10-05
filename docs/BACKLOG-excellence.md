# What stands between Cantus and excellent

*Requested by the owner on 2026-10-05 after round 19 ("agrega al backlog lo que debemos
para ser excelente"). Written from an honest assessment: the engineering is first-rank
(real-time safety, bit-identical renders, 1 400+ tests, a FabFilter-class effect family,
eight native instruments, professional recording); what is missing is mostly what only
**use** reveals, plus a set of features every top DAW has. Ordered by what pays most.
Each item says how we will know it is done.*

## Resumen (español)

Lo que nos separa de "excelente", en orden: **(1)** uso real — hacer canciones completas
con Cantus y corregir cada fricción; **(2)** probar con hardware real (tu Behringer UMC,
otras interfaces, ASIO dúplex); **(3)** las funciones que toda DAW top tiene y aún no:
warp y pista de tempo, automatización (ronda 20), instrumentos externos (ronda 20), MIDI
learn, crossfades, congelar pistas, marcadores, un editor de audio; **(4)** rendimiento con
sesiones grandes y un rendimiento por encima del mercado (procesamiento anticipado, motor multinúcleo, congelado automático en segundo plano, SIMD, benchmark publicado contra las demás DAW); **(5)** contenido (librería de sonidos, presets);
**(6)** producto: instalador, actualizaciones, firma de código, manual y tutoriales,
macOS/Linux; **(7)** pulido: nivel de los instrumentos, accesibilidad, detalles visuales.

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
- An opt-in crash reporter; a website; a community (forum/Discord).

## 7. AI (research done in round 20: `research/ai/`)

Phase A tools foundation → B an MCP server → C the assistant panel → D local audio
intelligence (stems, audio-to-MIDI, mastering assistant) → E generative audio.

## 8. Polish

- Accessibility (screen-reader semantics for the painted views, keyboard-only operation,
  colour-blind-safe meters), UI zoom tested at every step, high-DPI on every monitor.
- The visual details only use reveals: alignment, wording, empty states, error messages
  that tell you what to do, consistent icons; a design review of every view against FL.
- Localisation beyond EN/ES (PT, FR, DE, JA) once strings settle.
