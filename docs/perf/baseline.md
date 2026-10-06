# Baseline — where Cantus stands, 2026-10-06

*The starting line for the "better than the market" work (`docs/BACKLOG-excellence.md`
§4b). Method: `docs/perf/benchmark.md`. Raw reports: `perf/baselines/` (the gate's quick
suite) and the runs quoted below. Commit `d6d5c22` (`r20/bench`), release profile.*

## The machine, and how much to trust these numbers

| | |
|---|---|
| CPU | Intel Core i7-12700H — 6 performance cores (logical CPUs 0–11) and 8 efficiency cores (12–19), 20 threads |
| Memory | 64 GB |
| OS | Windows 11 Enterprise 23H2 (10.0.22631), power plan *Balanced*, on mains |
| Toolchain | rustc 1.97.1, `release` profile (thin LTO, opt-level 3) |
| Machine class | `windows-12th-gen-intel-core-i7-12700h` |

**Two sessions, two conditions — read the spread.**

- **Loaded** (2026-10-05 night): ten other builders compiling on the same laptop (machine
  load ~70 %), the suite pinned to logical CPU 4 (a performance core) with
  `start /AFFINITY`. The thread got 30–70 % of its core (`….share`), and the core's
  hyperthread sibling was busy, so even CPU time ran **2–3× slower** than on a quiet core.
  Those numbers are kept in this file only where nothing better exists, marked *loaded*.
- **Quiet** (2026-10-06 morning): the other builders done (load ~1 %), the same pinning.
  These are the baseline. The gate's baseline (`perf/baselines/…12700h.json`) is a quiet
  quick-suite run.

**Pinning matters on this CPU.** With the machine quiet, the same kernel measured on two
performance cores and two efficiency cores (3 runs each):

| Core | Comprimo, default | Spatium, default |
|---|---:|---:|
| logical 0 (P) | 132 ns/frame | 816 ns/frame |
| logical 2 (P) | 138 | 819 |
| logical 12 (E) | 209 | 1 163 |
| logical 16 (E) | 181 | 1 228 |

An unpinned run that lands on an efficiency core reads 1.4–1.5× slower; that alone would
fail a 25 % gate. Pin (`cargo run -p xtask -- perf --pin 2`).

## 1. DSP kernels (quiet, P-core, median of 3 runs, min–max)

One instance, 128-frame stereo blocks at 48 kHz. *ns/frame* is per stereo sample;
the budget at 48 kHz is **20 833 ns/frame** on one core, so an effect at 1 000 ns/frame
uses 4.8 % of a core. *Fit* is how many instances one core could run at 128 samples
(budget over one instance's block — the ceiling, not a promise).

| Effect | Default ns/frame | Fit | Heavy ns/frame | Fit | Heavy is |
|---|---:|---:|---:|---:|---|
| Aequalis (EQ) | 0.8 (no band in use: a wire) | — | **71** (69–72) | 293 | 8 bands, 2 dynamic, linear phase finest |
| Comprimo (compressor) | **143** (143–153) | 145 | **450** (387–571) | 46 | 4× oversampling, −30 dB, 8:1 |
| Terminus (limiter) | **730** (682–810) | 28 | **776** (775–832) | 26 | true peak, 5 ms lookahead, dither |
| Spatium (reverb) | **1 239** (1 013–1 297) | 16 | **872** (854–876) | 23 | 8 s decay, 120 ms pre-delay |
| Multiplex (multiband) | 5.5 (no band in use) | — | **1 001** (997–1 220) | 20 | 4 bands, linear-phase 48 dB/oct |
| Sibilus (de-esser) | **273** (263–273) | 76 | **415** (400–611) | 50 | split mode, −50 dB |
| Porta (gate) | **40** (39–44) | 522 | **109** (81–110) | 190 | style 5 |
| Fervor (saturation) | **13** (12–19) | 1 616 | **484** (480–730) | 43 | 8× oversampling |
| Latitudo (stereo) | **40** (40–56) | 522 | 40 | 522 | (no heavier setting) |
| Culmen (maximizer) | **998** (996–1 008) | 20 | **990** (988–1 058) | 21 | 3 bands, true peak, 5 ms |
| Delay (legacy) | 31 | 666 | 31 | 666 | 1.5 s, 0.9 feedback |
| Reverb (legacy) | 46 | 451 | 45 | 467 | size 1 |

| Instrument | 1 voice | 8 voices | 32 voices | Note |
|---|---:|---:|---:|---|
| Firmus | 196 | 1 129 | 2 205 | ns/frame for the whole voice set |
| Pulsus (hits per 16th) | 1 160 | 1 399 | 5 032 | a kit, not voices |
| Unda (wavetable) | 183 | 1 062 | 2 039 | |
| Calor (analog) | **488** | **3 500** | 3 373 | caps at 8 voices: 32 ≈ 8 |
| Modulus (FM) | 76 | 324 | 604 | |
| Exemplar (sampler) | 154 | **2 261** | **11 920** | 1.5 µs per voice per frame at 32 |
| Gravis (bass) | 169 | 186 | 230 | monophonic |
| Lamina (e-piano) | 251 | 648 | 1 920 | |
| Organum (organ) | 903 | 876 | 870 | the tonewheels run whatever is held |

## 2. The processor at scale (the demo songs scaled up, 128-frame blocks)

Median block, µs (budget **2 667 µs**); `.cpu` is the thread's own CPU time per block.
s10/s32: the gate's quiet quick run (3 runs). s64/s100 and 64 frames: the *loaded* full
run (5 runs) — the quiet re-run of the full suite was stopped unfinished.

| Session | Block (wall) | CPU / block | Load (CPU ÷ budget) | Share of core | Voices | Condition |
|---|---:|---:|---:|---:|---:|---|
| s10, 128 | 3 174 (2 751–6 422) | **2 750** (2 625–5 667) | **103 %** | 78 % | ~28 | quiet |
| s32, 128 | 16 617 (12 868–18 925) | **11 958** (10 417–12 500) | **448 %** | 58 % | ~77 | quiet |
| s64, 128 | 53 724 (18 589–108 070) | 30 594 (14 792–34 792) | 1 147 % | 42 % | 192 | loaded |
| s100, 128 (engine plays 64) | 46 419 | 24 427 | 916 % | 35 % | 192 | loaded |
| s64, 64 | 34 387 | 15 250 (budget 1 333) | 1 144 % | 31 % | 192 | loaded |

An earlier single unpinned probe on a quieter core read s10 **896 µs** CPU per block (34 %)
and s64 12 042 µs (452 %): the same sessions vary ~3× with the core they land on and its
hyperthread sibling. Either way the conclusion is the same: **one core cannot play the
32- or 64-track demo session in real time; the 10-track one sits at or past the edge.**

**Profiler overhead** (on/off alternate blocks, s32, quiet): **+0.1 %** median
(−1.9 to +8.1 % over 3 runs) — within noise.

**Hot spots — 64-track session** (the engine's own profiler, loaded run; shares of the
block):

| Node | Share | Node | Share |
|---|---:|---|---:|
| Unda on Pad | 13.3 % | Calor on Strings (×2) | 2.3 + 2.2 % |
| Calor on Chords (×2) | 9.0 + 8.9 % | Exemplar on Pad | 2.2 % |
| Calor on Pad | 5.9 % | Unda on Lead (×2) | 2.2 + 2.2 % |
| Unda on Pad (×2 more) | 4.4 + 4.4 % | Exemplar on Choir | 1.9 % |
| Calor on Arp | 2.5 % | Unda on Bells | 1.6 % |
| Pulsus on Cymbals | 2.4 % | | |

The fifteen heaviest nodes are all instruments; the heaviest insert (Sibilus on Hats) is
~2 %. The engine's own work (event dispatch, meters, mixing) never reaches the top 15.

## 3. Offline render (export path, 512-frame blocks; × real time, higher is better)

| Song | × RT wall | × RT per CPU second | Condition |
|---|---:|---:|---|
| s10 loop | **0.83** (0.23–0.84) | **1.07** (0.43–1.09) | quiet, 3 runs |
| house | 0.31 | 0.67 | loaded |
| ballad | 0.23 | 0.53 | loaded |
| funk | 0.93 | 1.36 | loaded |
| lofi | 1.10 | 1.60 | loaded |
| ambient | 0.92 | 1.11 | loaded |
| trap | 0.58 | 0.92 | loaded |
| s64 loop | 0.08 | 0.12 | loaded |

The demos export at **about real time or slower** on one core. The goal is 50–200×.

## 4. Opening a project (quiet, 3 runs, ms)

| | Read + parse + integrity | Compile | Units (instruments, effects) | Total |
|---|---:|---:|---:|---:|
| 10 tracks | 55.8 | 0.27 | 22.9 | **79** (76–89) |
| 64 tracks | 102 | 0.76 | 96.1 | **182** (130–213) |

(The loaded run read 24 ms and 133 ms totals: file-cache luck. Both are well under the
"100 tracks in under a second" goal.)

## 5. UI frames (whole window 1920×1080, build + tessellate, quiet, ms)

| Case | Median | p95 |
|---|---:|---:|
| 64 tracks, mixer | **6.42** (6.37–6.43) | 12.2 |
| 64 tracks, piano roll + arrangement | **2.56** | 3.64 |
| Startup project, piano roll | **2.29** | 5.23 |

ADR 0024's budget is ≤ 4 ms p50 for the 64-strip mixer: **over by 60 %** on the scaled
demo session (the frame_budget test's synthetic 64 strips measure ~3 ms).

## 6. Start-up (real executable, fresh data folder, warm file cache, unpinned, 5 runs)

| | Median | Min–max |
|---|---:|---:|
| Splash's first frame | **0.70 s** | 0.69–2.5 s |
| Application assembled (first frame) | **5.7 s** | 4.6–8.1 s |
| Spawn → second frame | **6.2 s** | 5.0–9.6 s |

(An earlier run under load: 3.4 s / 3.75 s; the spread is the audio device and MIDI
opening.) Goal: under one second to a playable project.

## 7. DAWbench-style capacity

Measured only on the loaded machine with the first criterion (p99 ≤ 80 %), whose tail
was the neighbours' preemptions: DSP 0–8 Comprimo instances, VI 0–8 Calor voices — **not a
result**. The criterion is now the median ≤ 50 % of budget; the run with it was stopped.
From the kernels, the estimate is ~70 Comprimo instances (8 tracks) and ~24 Calor voices
(3 tracks) at 128 samples on one P-core; to be measured.

## 8. Hot spots for the next round, ranked

1. **One thread.** The scaled demo sessions need 1–11 cores' worth of work; the
   processor uses one. A multicore graph scheduler (§4b) is the first lever, and offline
   export (≈1× RT today) should render independent tracks in parallel.
2. **The 64-track / 64-strip ceiling** (`MAX_TRACKS`, `MAX_STRIPS`): a 100-track session
   plays 64 tracks, silently. The 100-track goal needs it raised (memory: two 8192-frame
   stereo buffers per strip).
3. **Calor** — 440 ns per voice per frame (3.5 µs/frame at 8 voices, the unison-saw
   presets in the demos heavier still): the biggest node in every demo. SIMD voices.
4. **Unda pads** — one pad 13 % of a 64-track block.
5. **Exemplar** — 1.5 µs per voice per frame at 32 voices; a sampler should be ~10×
   cheaper (interpolation and per-voice filters per sample).
6. **Pulsus** — 1.2 µs/frame for a single hit stream (modal synthesis per hit).
7. **Spatium, Culmen, Terminus, Multiplex (linear)** — 0.7–1.2 µs/frame each: 16–28
   instances per core. Terminus's default true-peak oversampling is most of its cost.
8. **Inserts on a silent strip always run** (`ringing = !effects.is_empty()`): no
   silence/tail detection per node.
9. **Start-up 5–6 s** to the app; the splash is up in 0.7 s. Break down Assemble (audio,
   MIDI, the demo build) next.
10. **UI: the 64-strip mixer at 6.4 ms p50** on the demo session, over its 4 ms budget.
