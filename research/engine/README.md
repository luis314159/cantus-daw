# El motor de Cantus frente al estado del arte

## Resumen para el dueño (español)

Revisé la arquitectura de rendimiento del arquitecto (ADR 0031 y `docs/perf/plan.md`)
contra los motores de hoy (REAPER, Cubase, Pro Tools, Studio One, Logic, Bitwig, Live,
FL Studio, y el código abierto de Ardour, Tracktion Graph, JACK2, PipeWire, Supernova y
Faust) y contra los artículos de investigación. **El diseño es bueno, y en varias cosas va
por delante de todos**: ningún motor estudiado promete que el render en paralelo sea
idéntico bit a bit al serial, ninguno tiene una caché de render automática que se invalide
sola, ninguno suspende plug-ins con un criterio exacto (todos adivinan con un umbral) y
ninguno ordena el trabajo por el camino crítico. Eso se queda.

Lo que encontré está en los cimientos, no en la forma:

1. **Renderizar por adelantado (anticipación) antes que la exportación paralela.** Todos
   los líderes lo tienen (REAPER, Cubase, Studio One, Pro Tools, Logic). Es la palanca para
   "más pistas con un búfer más bajo", y es donde Cantus va atrás. Propongo adelantarlo una
   fase.
2. **El hilo de audio de Cantus hoy no es de tiempo real en Windows.** Con WASAPI (el modo
   por defecto) corre a prioridad normal, porque cpal sólo lo promueve con una función que
   no activamos. Además Windows puede mandar el programa a los núcleos lentos (EcoQoS):
   Steinberg tuvo que publicar un artículo porque a Cubase le pasaba justo eso con los
   hilos que procesan por adelantado.
3. **La cola de trabajo del prototipo no es "lock-free"** (su propio autor, Vyukov, lo dice):
   un hilo interrumpido en el momento justo puede detener a los demás. Hay una alternativa
   probada (una cola fija por hilo, como Faust) que además mantiene cada plug-in en el mismo
   núcleo, bloque tras bloque.
4. **Los bits no son iguales entre máquinas.** La librería matemática de Windows elige
   código distinto según el procesador, y Microsoft documenta que los resultados cambian.
   Para una caché de render compartida eso importa: propongo matemáticas propias y
   deterministas, y un dither sin estado para que el master pueda "descansar".
5. Y medir y probar como lo hace el estado del arte: pruebas de concurrencia PCT,
   reproducción exacta de sesiones, RTSan, un benchmark que imite al dispositivo real y un
   detector de cortes objetivo.

Las diez ideas, ordenadas, están abajo (en inglés, como el resto de la documentación
técnica). Las enmiendas concretas al ADR 0031 son **R1–R16**, en su última sección
("Review against the state of the art (2026-10-06)"), y `docs/perf/plan.md` tiene el nuevo
orden de fases. Hay tres preguntas nuevas para ti al final del plan: una regla de
monitoreo de baja latencia (como *Constrain Delay Compensation* de Cubase), aceptar el
nuevo orden, y aceptar un cambio de `DSP_VERSION` para que los renders sean idénticos en
todas las máquinas.

---

## The ten ideas that matter most for Cantus (English)

*Round 20, engine-research, 2026-10-06. Each idea: where it comes from, the evidence, and
what it changes in ADR 0031 (amendment numbers refer to the ADR's review section). Full
citations, with fetched/excerpt markers, are in the topic files:*

| File | Covers |
|---|---|
| [`engines.md`](engines.md) | REAPER, Cubase/Nuendo, Pro Tools, Studio One, Logic, Bitwig, Live, FL Studio, JUCE, the benchmarks |
| [`realtime-systems.md`](realtime-systems.md) | Bencina's rules, MMCSS, RTWQ, QoS/EcoQoS, Thread Director, audio workgroups, Linux RT, caches, page faults, denormals, the Rust crates |
| [`scheduling-papers.md`](scheduling-papers.md) | Ardour, Tracktion Graph, JACK2, PipeWire, Supernova, Faust, CLAP/VST3, HEFT, work stealing and locality, SDF, Time Warp, build systems |
| [`dsp-quality.md`](dsp-quality.md) | SRC, time-stretch, ADAA/oversampling, ZDF filters, SIMD voices, smoothing, dither, loudness/true peak, float determinism |
| [`verification.md`](verification.md) | loom, Shuttle, PCT, CHESS, GenMC, Miri, deterministic simulation, record/replay, latency and glitch measurement, DAWbench, RTSan, CI benchmarking |

### 1. Render ahead — and build it before the parallel export

- **From:** REAPER's anticipative FX; Cubase's ASIO-Guard; Studio One's Dropout Protection; Pro
  Tools 11's dual buffer; Logic's process buffer; and, in one-cycle form, JACK2's
  asynchronous mode and PipeWire's async nodes.
- **Evidence:** every leading DAW has it, and the reference benchmark (DAWbench) runs in REAPER
  with anticipation on all tracks. REAPER reaches "over 95 percent utilisation of all cores"
  because cores rarely synchronise (*SOS*, 2011). Cubase's own meter separates the real-time
  path from the ASIO-Guard path.
- **ADR change (R7c, R6):** §3 is right in design and ahead of the market in exactness. Move it
  from Phase 4 to right after the pool (plan: P2'), ahead of the parallel export and the
  cache, which are where Cantus is first rather than behind. Eligibility also reads VST3
  `IPrefetchableSupport` and keeps disk-streaming hosted instruments live, as the VST3 header
  asks.

### 2. Own the real-time chain end to end

- **From:** Microsoft's MMCSS, QoS and power-throttling documentation; Steinberg's
  hybrid-CPU knowledge base; Apple's WWDC20 *Meet Audio Workgroups*; the cpal and Mozilla
  `audio_thread_priority` sources.
- **Evidence:**
  - Cantus's WASAPI callback runs at **normal priority** today (cpal's `realtime` feature is
    off), and that feature would ask for "Audio", not "Pro Audio".
  - Windows' EcoQoS moves a background window's process to E-cores at reduced frequency.
    Steinberg's fix for the resulting prefetch dropouts is a `powercfg` command per
    executable.
  - Apple gives the right thread count (`os_workgroup_max_parallel_threads`).
  - Microsoft protects only Real-Time Work Queue items for WASAPI applications in its
    low-latency mode.
- **ADR change (R1, R3, R4, R5):**
  - Promote the callback ourselves on WASAPI.
  - Opt the process out of EcoQoS.
  - Put ahead workers on spare P-cores first, not E-cores.
  - Size the macOS pool by the workgroup, keeping the time constraint before the join.
  - Keep the live pool within one last-level cache.
  - Measure RTWQ against MMCSS.
  - State the driver path the targets assume (ASIO or `IAudioClient3`).

### 3. A ready queue with a real progress guarantee, and locality

- **From:** Vyukov ("not lockfree in the official meaning"); the stall documented in
  Boost.Lockfree; Faust's work-stealing scheduler (LAC 2010, fixed per-thread arrays); Chase–Lev
  (SPAA 2005) and Lê et al. (PPoPP 2013); Acar, Blelloch and Blumofe on the locality of work
  stealing (SPAA 2000).
- **Evidence:**
  - A worker preempted between claiming and publishing a cell makes the callback see an empty
    queue. Ardour's and Tracktion's queues share this property.
  - Because each task is pushed at most once per block, a Chase–Lev deque sized to the task
    count never grows. That removes the objections that rightly disqualified crossbeam-deque.
  - Affinity mailboxes match static partitioning on a quiet machine and beat it under load.
- **ADR change (R2):** per-worker fixed-capacity deques with home-worker seeding. The *sched*
  builder builds both queues and the bench decides. This also gives risk 3's "home worker" for
  free.

### 4. The same bits on every machine — what a shared render cache needs

- **From:** Microsoft's `_set_FMA3_enable` documentation; Rust's RFC 3514 and std float docs;
  B. Dawson on floating-point determinism; counter-based random generators.
- **Evidence:**
  - The UCRT picks FMA3 or non-FMA3 transcendental code by CPU, and "differences may also be
    observable between computers".
  - `cantus-dsp` calls std's `sin`/`exp`/`powf` about 450 times.
  - The v2-vs-v3 CI job cannot see this.
  - The master's `Ditherer` is a sequential generator, so a dithering master never comes to
    rest. That means no settle points for the cache or time-parallel export.
- **ADR change (R9, R10):**
  - A deterministic math module, with std's transcendentals banned by lint.
  - `_set_FMA3_enable(0)` until it lands.
  - No approximate reciprocal instructions.
  - Cross-machine CI.
  - Counter-based dither.
  - The FTZ guard's basis stated honestly (it is outside Rust's model, so the guarantee is
    empirical) and re-asserted after every hosted plug-in call.

### 5. Be honest about the live path

- **From:** REAPER ("or dependent on a track that is"); Logic's live chain; Cubase's
  *Constrain Delay Compensation*; Logic's *Low Latency Mode*; Live's *Reduced latency when
  monitoring*; Studio One's < 3 ms rule; Cubase 12–14's Audio Performance Monitor.
- **Evidence:**
  - With anything live, the master is live, and Cantus's master holds the mastering chain.
  - All four leaders bypass or switch latent plug-ins while monitoring.
  - Cubase shows the real-time peak and the prefetch average as two meters, because only the
    peak predicts a dropout.
- **ADR change (R7a, R7b, R13.5):**
  - Phase 2's live-track target counts the serial master chain.
  - A low-latency monitoring rule is proposed (a new ADR beside 0022).
  - The readout shows the live peak and the ahead average separately.

### 6. Test concurrency the way the literature proves it

- **From:** PCT (Burckhardt et al., ASPLOS 2010); CHESS (OSDI 2008); Shuttle (SOSP 2021);
  loom and its stated blind spots; FoundationDB's seed self-check and BUGGIFY; RealtimeSanitizer
  (Clang 20, and rustc nightly since PR #147935).
- **Evidence:**
  - Uniform random scheduling makes a specific bad interleaving exponentially unlikely. PCT
    gives a probability bound per run.
  - loom treats `SeqCst` as `AcqRel` and skips load buffering.
  - The counting allocator sees no syscalls or locks. That matters now that ADR 0005's rule 3
    is amended.
- **ADR change (R13.1–R13.3):**
  - A PCT-shaped schedule fuzz with replay tokens and a twice-per-seed check.
  - RTSan on Linux CI with the permitted wakes scoped.
  - A session recorder, so a user's bug replays offline bit for bit.

### 7. Measure like the stage, not like a loop

- **From:** DAWbench (Walker 2006/2008; Curigliano 2026: serial and parallel bus variants, and
  a plain sine track "used for detecting glitching"); OboeTester's glitch test; Fuchsia's slope
  checker; Gil Tene on coordinated omission; Speed Shift ramp times (5–17 ms); a DAFx 2026 paper
  showing an isolated real-time factor of 0.21 becoming 158 % p99 utilisation under contention.
- **Evidence:**
  - The bench runs blocks back to back. Its own method notes that live blocks see cold caches
    and lowered clocks.
  - Driver xrun counters are unreliable.
- **ADR change (R13.4):** a paced bench mode, a glitch-detector node for the live figure, missed
  callbacks counted from the device position, and the serial-chain variant (the case where a
  parallel engine gains nothing — §7's pipelining is for it).

### 8. Sandbox the way JACK2 and PipeWire already do

- **From:** JACK2 (activation counters in shared memory, asynchronous mode as default);
  PipeWire (activation records, direct eventfd wake, async nodes since 1.2); Supernova on the
  cost of spinning; the ADR's own prototype (spinners = cores ⇒ p99 of 97 ms).
- **Evidence:**
  - Sandboxes that spin "while the transport plays" are spinners the host does not count. With
    per-plug-in hosting they oversubscribe the machine.
  - Both servers trade one cycle of latency for immunity to late clients.
- **ADR change (R15):**
  - A machine-wide spinner budget.
  - A direct, PipeWire-style wake.
  - An async hosting option whose extra block of latency is reported to delay compensation.

### 9. Memory and topology the audio thread never trips on

- **From:** mlock(2) and `VirtualLock` documentation; Supernova (mlocked, pre-touched stacks);
  crossbeam's `CachePadded` (128 bytes on x86-64 because the spatial prefetcher pairs lines);
  DAWbench 2025 (L3 size and inter-CCD latency decide 64-sample results); Rowland's ADC 2022
  measurement (per-node buffers beat run-time reuse).
- **Evidence:**
  - Slabs and rings allocated on the UI thread fault on first touch, and that touch happens on
    the audio thread.
  - §5.4's padding is half what x86 needs.
- **ADR change (R11, R12, R4):**
  - Pre-fault and best-effort lock every pool.
  - 128-byte padding everywhere.
  - One cache domain for the live pool.
  - No buffer reuse until measured, the ahead rings included.

### 10. Roll back instead of waiting: edits on ahead nodes

- **From:** D. Jefferson, "Virtual time" (*ACM TOPLAS*, 1985), the Time Warp mechanism of
  optimistic simulation.
- **Evidence:** render-ahead's user-visible cost is late edits. REAPER users turn anticipation
  off per track to avoid it, and Cubase's fader latency grows with the ASIO-Guard level.
  Cantus's native units are plain Rust values, so a small one can be checkpointed per chunk and
  re-rendered from the checkpoint. The bits stay exact, because the state *is* what it was.
- **ADR change (R8, research-gated):** after anticipation holds, and only if §3.2's promotion
  leaves audible latency, an opt-in `checkpoint_size()` lets an edit on an ahead native node be
  heard within two blocks.

### Also worth doing (smaller)

- **ADAA before SIMD** (Parker–Zavalishin–Le Bivic, DAFx 2016; Bilbao et al., IEEE SPL 2017;
  Chowdhury): first-order ADAA at 2× beats either technique alone, which halves Fervor's
  oversampled work. Also a fixed internal sub-block for coefficient updates (R14).
- **Lints** that keep the rules true: no `AtomicRefCell::borrow`, no std transcendentals in the
  DSP crates (R16).
- **When warping lands** (ADR 0025's "later"): Rubber Band R3 is the quality reference. Cantus
  is GPL, so it is usable. Signalsmith Stretch (MIT) is the light default. Warped clips are
  rendered into the cache, and only the edited clip stretches live (`dsp-quality.md` §2).
- **Already state of the art, keep:**
  - the de Soras-style sampler resampler;
  - histogram-gated loudness, finer than libebur128;
  - true peak with parabolic refinement, which beats BS.1770 Annex 2's 0.688 dB under-read.
  - Update the references to BS.1770-5 (2023) and EBU R 128 v5.0 (2023).

### Where Cantus is already ahead of everything surveyed

- Bit-identical parallel = serial = offline (§1.3, §3.6).
- An automatic, invalidating render cache, with build-system theory behind it (constructive
  traces and early cutoff; Mokhov, Mitchell, Peyton Jones, ICFP 2018).
- Exact silence suspension instead of a threshold (§5.1).
- A dropout blamed on the plug-in, with OS preemption told apart (§2.9).
- Critical-path ordering of ready work (HEFT's upward rank, unused in audio engines).

### Method and limits

Five research passes ran in parallel: market engines; real-time systems; open-source schedulers
and papers; DSP; verification. Source code was read directly: Ardour, Tracktion Graph, JACK2,
PipeWire, Supernova, Faust, CLAP, VST3, crossbeam, cpal and Mozilla's `audio_thread_priority`.

The Zscaler proxy blocked steinberg.help in part, the REAPER user guide and wiki, rossbencina.com,
timur.audio and 1024cores.net. Mirrors and original copies were used where they existed, and
nothing was bypassed. Claims seen only in search excerpts are marked in the topic files, and no
amendment rests on an excerpt alone.

Not reached this round:
- the Supernova thesis full text;
- ffitch's Csound papers in full;
- Puckette's ICMC 2008 paper;
- the VST3 developer portal;
- Justin Frankel's 2024 DAWbench interview on anticipative FX (the best primary source on
  REAPER, worth an hour).
