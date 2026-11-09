# Round 38 — the scorecard's start-up rows (builder: scorecard)

Branch `r38/scorecard`, worktree `F:\projects\daw-r38\scorecard`. The brief: make Cantus win,
honestly, the start-up rows of the measured scorecard (`docs/scorecard/README.md`, ADR 0102)
against FL Studio and REAPER, and make sure the comparison is fair. Decisions: ADR 0106
(amending 0031 §5.1, 0059 §6, 0091 and 0102).

## 1. Was the comparison fair?

Mostly — but in both directions, and the round-37 table could not say which.

**Against Cantus**

- **Different first screens.** Cantus opened its 10-track demo song (the house drop: nine
  strips with inserts, a mastering chain, a reverb and a delay return, a sampler, three
  synths and the dancer), FL Studio its default template, REAPER an empty project. The
  at-rest row compared a desk of 20+ effects against an empty one. The like-for-like rows are
  **Cantus on an empty song against REAPER**, and **Cantus on its *Beat* starter (drums,
  bass, keys, a reverb return) against FL Studio's template**; this round measures Cantus on
  all three (`examples/scorecard_projects.rs` writes the files; Cantus is given the file on
  its command line, which also keeps the start page shut, as FL and REAPER open with none).
- **The quiet threshold.** Round 37 raised the *quiet* threshold to 0.35 of a core because the
  dancer breathes; Cantus then sat at 0.29–0.33 at rest, so "1.5 s in a row under 0.35" was a
  coin toss on 250 ms samples: the 6–42 s "tail" was mostly that, not start-up work (§2).
  This round reports both thresholds from the same samples, 0.10 (the ADR's) and 0.35.

**For Cantus**

- **No plug-in scanner.** The round-37 Cantus ran from a build folder without
  `cantus-scan.exe`: its plug-in scan at start failed at once (53 entries "cannot run the
  scanner" in its `plugins.json`). An installed copy has the scanner. This round's build folder
  has it (the first, unmeasured start scans; the measured ones find the cache).
- **A data folder of its own** (`CANTUS_DATA_DIR`, the first-run audio setup marked done).
  FL Studio and REAPER started from their own long-used settings. Both are "a configured copy";
  kept, and said.

**The same for all three, checked**

- **Audio device**: Cantus used the system default through WASAPI (shared): *Altavoces
  (Realtek)* at 48 kHz, a 480-frame period (10 ms), and the default input (the laptop's
  microphone array) open. FL Studio used its own *FL Studio ASIO* (a wrapper over the same
  WASAPI endpoint) at 44.1 kHz, 512 frames (11.6 ms) — read from its registry settings, not
  changed. REAPER used WASAPI at 48 kHz, 512 frames. Periods within 16 % of each other; all
  three run their audio engine at rest.
- **The machine**: the same as round 37 (Ryzen 7 7735HS, 31 GB, High performance plan, mains).
  The screen is drawn by the Radeon (the GL driver loaded is AMD's `atio6axx.dll`). The
  owner's installed Cantus was open the whole day, in the background, using 0.14 of one core
  (1 % of the machine); every row was measured with it there (the script prints the machine's
  load before each program: 3–7 %).
- **Window ready** is the main window with the project's title, answering — after the splash
  for Cantus, after its splash for FL Studio — the same rule.

## 2. What was found (traced, not guessed)

`tools/scorecard/profile.ps1` — new: a sampling profiler that runs from outside, with no
administrator rights: it suspends each thread that used CPU, walks its stack with dbghelp and
the program's PDB, and records every thread's exact CPU time every 250 ms.
`profile_report.py` folds it per second, per thread, per function. On beta.12:

| What | Where it went |
|---|---|
| **At rest on the demo: 32.8 % of a core** | the audio callback 13.9 points (every strip with an insert, the master chain and the master's loudness meter running over silence: Spatium, Sibilus, Multiplex, Comprimo, Culmen, Terminus, the K-weighting and 4× true peak); the graphics driver's thread 11.3 and the main thread 6.1 — the dancer's twelve breaths a second, each a whole-window frame. Hidden, the demo rests at 16.1 %. |
| **The quiet tail** | Chorda's reference keyboard designed in the 3 s after the window on ten threads (≈ 7.5 s of CPU) for a piano the demo does not have; then the at-rest load against a 0.35 threshold (above). |
| **The *Beat* starter at rest: 44.5 %** | the mixer asking egui for a repaint 16 ms away directly: egui shortens that by its predicted frame (round 37's finding), so the window drew at 144 Hz while a meter moved. The effect editors, the pinned inspector meter and the dancer editor did the same. |
| **An empty song at rest: 2.0 %** | a MIDI-out thread sleeping 1 ms a thousand times a second with no port (1.1 points); the device watcher listing every audio endpoint every 1.5 s (0.5). |
| **Memory: 229 MB private on an empty song, 267 MB on the demo** | see §4. |

## 3. What changed (ADR 0106)

1. **Every strip with inserts, the master chain and Spatium rest in silence**, exactly
   (twins to the bit; `rest_blockers()` names what keeps an insert running); the master's
   loudness meter counts silence through clear filters as columns.
2. **The dancer holds still** 10 s after the last thing that happened (stopped), and the
   window then asks for no frame; the pointer, a key, Play, a drag or a hover wakes it from
   the pose it held. (And then, by the owner's decision, nothing dances at start: §6.)
3. **No view asks egui for screen-rate frames**: the mixer's meters, the effect editors, the
   pinned meter and the dancer editor go through the ticker; the first-run audio setup left
   open on a silent input looks four times a second instead of drawing 60 (it was 78 % of a
   core).
4. **Chorda's reference keyboard is designed when the instrument picker opens**, not at start.
5. **Idle threads sleep**: MIDI out every 20 ms with nothing to serve; the device watcher
   waits for Windows' endpoint notification (`cantus_rt::endpoints`), rescanning at least
   every 10 s.
6. **Tools**: `profile.ps1`/`Sampler.cs`/`profile_report.py`, `memmap.ps1`/`MemMap.cs`,
   `startup.ps1 -AlsoShare` and its at-rest measure, `CANTUS_REPAINT_LOG`, the memory map test.

## 4. Memory: what the 267 MB is

Measured two ways. From outside, `tools/scorecard/memmap.ps1` reads the address space: on an
empty song 229 MB private — about 140 MB of it in the process heap's segments, the rest in
large private regions outside it (the AMD OpenGL driver's buffers, thread stacks, the audio
and MIDI drivers' own heaps). From inside, `crates/cantus-ui/tests/memory_e2e.rs` counts every
allocation on the boot path (`--nocapture` prints the map):

| Step of a start | Live heap | Of which |
|---|---:|---|
| The processor, before any song | **96 MB** | the engine's event queue 15.9 MB (4 096 events of 3 880 B: an event is as large as its largest kind, the block's meters — 256 track and 256 strip peaks, 256 gain reductions, two telemetry blocks); 256 track slots each holding a Pulsus drum machine whether the track is a drum track or not, 120 KB apiece, **31 MB**; 4 338 pass buffers of 4 KB for 256 tracks' and 256 strips' outputs, keys, sends and edges, 17 MB; the delay-compensation pool, 192 lines of 2 × 32 KB, 12.4 MB; the pool's scratch, 4.4 MB |
| The app on an empty song | +3 MB | |
| Its first frames | +8 MB | the fonts' atlas and the text caches |
| The demo opened | +39 MB | its reverbs' and delay's lines, wavetables, the sampler's choir, the mastering chain's look-ahead |

**What is waste, and what was cut.** The waste is the engine's preallocation for the largest
session it can hold (256 tracks, 256 strips) paid by every session: ≈ 60 MB of the 96 MB is
capacity an empty song never touches (the drum machines in every slot, the event queue sized
by its largest message, the delay-line pool). None of it is cut in this round: each is an
audio-thread contract — nothing may be allocated there, so what a snapshot may need is made
in advance — and changing it means moving the Pulsus kit to the UI-built units the native
instruments already use, meters into a mailbox of their own, and a delay-line pool that grows
from small. Those are ADR-sized engine changes in the hot files; they are §4b's next memory
items (with the measured sizes above), not done here. What this round removed from memory is
small: Chorda's reference keyboard is no longer designed at start (its registry stays empty
until the piano is wanted).

## 5. Before and after

`tools/scorecard/startup.ps1`, five measured starts after a warm-up each, the release build of
this branch and beta.12 built from `master` at 25b565a7 (both with `cantus-scan.exe`, both on
a finished plug-in cache), FL Studio and REAPER the same evening. Medians; the table with
every column is the README's.

| | Window | Quiet < 0.10 | Quiet < 0.35 | Private | At rest (ticks · cycles) |
|---|---:|---:|---:|---:|---:|
| Cantus beta.12, its start (the drop with Trino) | 1.35 s | never in 60 s | 21.7 s | 268 MB | 30.2 % · 30.7 % |
| **Cantus r38, its start (the drop, no dancer)** | **1.32 s** · 1.18 s | **2.3 s** | **2.1 s** | 272 MB | 2.6 % · 2.2 % |
| Cantus r38, the drop with Trino (a user's dancer) | 1.24 s | 11.3 s | 2.2 s | 266 MB | 1.1 % · 2.3 % |
| Cantus beta.12 → r38, an empty song | 1.02 → 1.02 s | 4.7 → **2.0 s** | 2.8 → 1.8 s | 233 → 237 MB | 1.4 · 2.2 → **1.2 · 1.1 %** |
| Cantus beta.12 → r38, the *Beat* starter | 1.12 → 1.04 s | 3.4 → **2.0 s** | 2.9 → 2.0 s | 239 → 239 MB | 6.1 · 5.4 → **0.5 · 1.2 %** |
| FL Studio 24.1.2, its template | 2.86 s · 2.80 s | 6.2 s | 3.6 s | **133 MB** | 7.3 % · 6.4 % |
| REAPER 7.85, an empty project | 1.98 s · 1.94 s | 3.0 s | 2.2 s | **61 MB** | 5.1 % · 4.9 % |

(Round 37's published Cantus row — 1.31 s, 10.1 s at 0.35, 267 MB, 29 % — was beta.12 with no
plug-in scanner beside it; the beta.12 rows here have one.)

## 6. The dancer at start — the owner's decision

Measured first with the dancer kept and made cheap (item 2 of §3: at rest it then costs
nothing), the demo still lost the *quiet* row at 0.10: its ten seconds of breathing are
≈ 17 % of a core (a whole-window frame twelve times a second: the main thread ≈ 4 ms and the
graphics driver's thread ≈ 8 ms each). The owner then decided (2026-11-08): *el dancer por
default cada vez me gusta menos, quítalo*. The song Cantus opens on is now the house drop
without Trino (ADR 0106 §9); the whole demo, the picker, the palette and saved songs keep the
dancer as before, and a dancer the user adds still holds still after 10 s. Held by
`a_fresh_start_shows_no_dancer_and_spends_no_frames_on_one` (through `assemble`: no dancer
track, no texture, no redraw, no frame asked at rest). No setting or schema changed. The
README's and the website's screenshots of the start (`shoot_readme`, `shoot_site`,
`shoot_showcase`) will show no dancer when next shot; `shoot_stage` uses the excerpt with
Trino.

## 7. What is still lost, and why

- **Private memory**: 237 MB on an empty song against FL Studio's 133 MB and REAPER's 62 MB
  (§4). Not cut this round: the waste is the engine's preallocation for its largest session
  (≈ 60 MB of the 96 MB heap an empty song never touches) and the graphics driver's buffers
  outside the heap. The next steps, measured: Pulsus kits built with the units the UI already
  builds for native instruments (−31 MB), the meters out of the event queue into a mailbox of
  their own (−15 MB), a delay-line pool that starts small (−9 MB). Each changes an
  audio-thread contract; each wants its ADR.
- **At rest against REAPER**: tonight Cantus (2.2 % on its start, 1.1 % on an empty song, in cycles) was below REAPER (4.9 %), but the machine carried 2–4 cores of another build; round 37 measured REAPER at 0.3 % on a quiet one. Until it is measured quiet, the row is not claimed. What remains in Cantus at rest is the audio callback's per-pass work for a resting desk and the open input stream.
- **A frame's cost** (BACKLOG §4b): a whole-window frame costs the main thread ≈ 4 ms and the
  AMD OpenGL driver's thread ≈ 8 ms — not traced further this round (the driver's stacks show
  only waits; vsync is the first suspect).

## 8. Verified only here, or not at all

- The device watcher's registry notification was checked to arm and time out on this
  machine; an interface plugged and unplugged is the owner's to try
  (`docs/beta-checklist.md`, *En reposo*). The 10 s safety rescan bounds the worst case.
- The Linux check (`tools/wsl/check.sh`) was **not run**: C: had 25 GB free (the WSL image
  lives there; the brief keeps 30 GB). The Windows-only code added (`cantus_rt::endpoints`)
  has a non-Windows path that only sleeps; the integrator should run it.
- Tests: the four crates' suites (`cantus-rt`, `cantus-dsp`, `cantus-engine`, `cantus-ui`)
  ran in full once (4 474 passed; the one failure, `listen.rs`'s block-of-1 test, found the
  follows-song bug fixed in `db02cf90`), then the affected binaries again after each fix;
  clippy `-D warnings` on the four crates' targets, `cargo fmt --check` per package.

## 9. Commits

`9ab86d2f`, `4890b758`, (rustfmt), `db02cf90`, `30abdc1d` and this document's.
