# The scorecard — measured

BACKLOG §0's question — *is Cantus measurably the best?* — answered with numbers taken the same
way for every program, on one machine, the same day. The method is ADR 0102; the feature
dimension is [`features.md`](features.md) (97 rows over twelve DAWs). **A row Cantus loses is
published as plainly as one it wins**, and "better" is never claimed without a number here.

## Start-up on a quiet machine (0.1.0-beta.14, 2026-11-11)

Round 39's release build (`81f88171`) measured on the same machine, **quiet** (no builds
running; 13 % of the CPU busy before the runs, 0.6–0.9 of a core of other load during each
start), with the same script, rule and settings as below (`IdleShare 0.10`, `AlsoShare 0.35`,
`RestAt 30`, `RestFor 10`; five measured starts after one unmeasured start), fresh copies of
round 38's data folders and round 38's `empty.cantus` and `beat.cantus`. REAPER and FL Studio
were not measured again: their rows, and beta.13's, are 2026-11-09's (the next section), on the
same machine.

| Program — what it opens | Window ready (median · best) | Quiet < 0.10 | Quiet < 0.35 | Private memory | At rest (ticks · cycles) |
|---|---:|---:|---:|---:|---:|
| **Cantus beta.14 — an empty song** | **0.76 s** · 0.73 s | **1.5 s** | **1.5 s** | 140 MB | **1.1 % · 1.5 %** |
| Cantus beta.13 — an empty song (2026-11-09) | 0.74 s · 0.72 s | 1.5 s | 1.5 s | 237 MB | 1.6 % · 1.6 % |
| REAPER 7.85 — an empty project (2026-11-09) | 1.76 s · 1.74 s | 2.0 s | 1.8 s | **62 MB** | 2.3 % · 3.7 % |
| **Cantus beta.14 — the *Beat* starter** | **0.77 s** · 0.76 s | **1.5 s** | **1.5 s** | 143 MB | **2.0 % · 1.7 %** |
| Cantus beta.13 — the *Beat* starter (2026-11-09) | 0.76 s · 0.75 s | 1.6 s | 1.5 s | 239 MB | 2.0 % · 1.8 % |
| FL Studio 24.1.2 — its template (2026-11-09) | 2.49 s · 2.40 s | 5.8 s | 3.2 s | **133 MB** | 6.6 % · 6.7 % |

**What it says.**

- **Memory: 97 MB less than beta.13 on an empty song (−41 %), 96 MB less on the *Beat*
  starter** — the engine's parts are made as the song needs them (ADR 0107). At rest (30–40 s)
  the private bytes are 135.7 MB and 138.7 MB; the working set 171 MB and 178 MB. Every start's
  private memory falls within 1.5 MB of its median.
- **Memory is still lost**: 140 MB on an empty song against REAPER's 62 (by 78 MB), 143 MB on
  the *Beat* starter against FL Studio's 133 (by 10 MB). Cantus's own live heap on an empty
  song is about 25 MB; the rest is the next memory item (`docs/round39/memory-measured.md`).
- **Window ready, quiet and at rest: the same as beta.13, within the noise**, and still first
  in both pairs — the window 2.3× sooner than REAPER and 3.2× sooner than FL Studio; quiet at
  1.5 s against 2.0 s and 5.8 s; at rest 1.1–1.5 % of a core against REAPER's 2.3–3.7 %, and
  1.7–2.0 % against FL Studio's 6.6 %.

## Start-up on a quiet machine (0.1.0-beta.13, 2026-11-09)

Round 38's rows measured again **with the machine quiet** (the owner closed everything; WSL
shut down; under 4 % of the CPU busy before each program, 0.3 of a core of other load during
each start), on the release build of round 38's integration — the same script, rule and
data folders as below (five measured starts after a warm-up; *at rest* from 30 s to 40 s
after spawn). The rows that compare like with like are **Cantus on an empty song against
REAPER's empty project** and **Cantus on its *Beat* starter against FL Studio's template**.
Raw results: [`results/2026-11-09/`](results/2026-11-09/).

| Program — what it opens | Window ready (median · best) | Quiet < 0.10 | Quiet < 0.35 | Private memory | At rest (ticks · cycles) |
|---|---:|---:|---:|---:|---:|
| **Cantus beta.13 — an empty song** | **0.74 s** · 0.72 s | **1.5 s** | **1.5 s** | 237 MB | **1.6 % · 1.6 %** |
| REAPER 7.85 — an empty project | 1.76 s · 1.74 s | 2.0 s | 1.8 s | **62 MB** | 2.3 % · 3.7 % |
| **Cantus beta.13 — the *Beat* starter** | **0.76 s** · 0.75 s | **1.6 s** | **1.5 s** | 239 MB | **2.0 % · 1.8 %** |
| FL Studio 24.1.2 — its template | 2.49 s · 2.40 s | 5.8 s | 3.2 s | **133 MB** | 6.6 % · 6.7 % |
| Cantus beta.13 — its start (the house drop, no dancer) | 0.95 s · 0.94 s | 1.7 s | 1.7 s | 273 MB | 3.0 % · 2.9 % |

**What it says.**

- **Window ready:** Cantus 2.4× sooner than REAPER on the same empty song, 3.3× sooner than
  FL Studio on a comparable starter.
- **Quiet:** Cantus first in both pairs (1.5 s against REAPER's 2.0 s; 1.6 s against FL
  Studio's 5.8 s).
- **At rest: Cantus first in both pairs**, on a quiet machine: 1.6 % of a core on an empty
  song against REAPER's 2.3–3.7 %, 1.8–2.0 % on the *Beat* starter against FL Studio's
  6.6 %. (Round 37 had REAPER at 0.3 %; tonight, quiet, it measured 2.3 % by ticks and
  3.7 % by cycles in all five starts — what differed on 2026-11-06 is not known. Both are
  published.)
- **Memory: lost, as plainly** — 237 MB on an empty song against REAPER's 62 MB, and 239 MB
  against FL Studio's 133 MB. Most of it is the engine reserving in advance everything its
  largest session could need, so that the audio thread never asks for memory while it plays
  (`docs/round38/scorecard.md` §4); making that reservation grow with the song is planned,
  one ADR per part.
- While FL Studio ran, the rest of the machine carried 1.4 cores (0.3 for every other
  program); the script measures `FL64.exe` alone.

## Start-up, measured again (round 38, 2026-11-08)

The same machine and script as below, with what round 37 could not say made explicit
(`docs/round38/scorecard.md`): Cantus measured **on what it opens with and on what the
others open with**; *quiet* read at the ADR's 0.10 of a core and at round 37's 0.35 from the
same samples; *at rest* measured by the script itself (one core's share from 30 s to 40 s
after spawn), from the thread times (counted in 15.6 ms ticks) and from the cycles (exact,
over the rated 3.2 GHz clock); every Cantus data folder holding a finished plug-in scan, as
a configured copy has. Raw results: [`results/2026-11-08/`](results/2026-11-08/) (FL Studio and REAPER twice: the table has the later run, with the Cantus start beside it; `*-first-run.json` the earlier, which agrees within its noise).

| Program — what it opens | Window ready (median · best) | Quiet < 0.10 | Quiet < 0.35 | Private memory | At rest (ticks · cycles) |
|---|---:|---:|---:|---:|---:|
| **Cantus r38 — its start: the house drop, no dancer** | **1.32 s** · 1.18 s | **2.3 s** | **2.1 s** | 272 MB | 2.6 % · 2.2 % |
| **Cantus r38 — an empty song** (≈ REAPER's start) | **1.02 s** · 0.99 s | **2.0 s** | **1.8 s** | 237 MB | 1.2 % · 1.1 % |
| **Cantus r38 — the *Beat* starter** (≈ FL Studio's template) | **1.04 s** · 0.99 s | **2.0 s** | **2.0 s** | 239 MB | 0.5 % · 1.2 % |
| Cantus r38 — the drop with its dancer (holds still after 10 s) | 1.24 s · 1.23 s | 11.3 s | 2.2 s | 266 MB | 1.1 % · 2.3 % |
| Cantus beta.12 — the drop with its dancer (its start) | 1.35 s · 1.33 s | never in 60 s | 21.7 s | 268 MB | 30.2 % · 30.7 % |
| Cantus beta.12 — an empty song | 1.02 s · 1.01 s | 4.7 s | 2.8 s | 233 MB | 1.4 % · 2.2 % |
| Cantus beta.12 — the *Beat* starter | 1.12 s · 1.00 s | 3.4 s | 2.9 s | 239 MB | 6.1 % · 5.4 % |
| FL Studio 24.1.2 — its template | 2.86 s · 2.80 s | 6.2 s | 3.6 s | **133 MB** | 7.3 % · 6.4 % |
| REAPER 7.85 — an empty project | 1.98 s · 1.94 s | 3.0 s | 2.2 s | **61 MB** | 5.1 % · 4.9 % |

**What it says.**

- **Window ready: Cantus first in every row.**
- **Quiet: Cantus first** on its own start, on an empty song and on the *Beat* starter. The
  start-up work after the window was Chorda's keyboard, designed for a piano nobody had
  asked for (now designed when the instrument picker opens). A dancer the user adds breathes
  for 10 s after the last thing that happened, which keeps a start above 0.10 that long.
- **At rest: from 30 % of a core to about 2 %** on the demo (1 % on an empty song) — every strip, the master and the
  reverb rest exactly in silence; no dancer at start (the owner's decision); nothing asks for
  frames at the screen's rate. Tonight FL Studio rested at 6.4 % and REAPER at 4.9 % (cycles) under the same other load; on round 37's quiet machine REAPER measured 0.3 %, so the at-rest row against REAPER was measured again on a quiet machine on 2026-11-09 (above): won.
- **Memory: still lost** — 237 MB on an empty song against FL Studio's 133 and REAPER's 62.
  Most of it is the engine's preallocation for the largest session it can hold (a drum
  machine in each of 256 track slots, an event queue sized by its largest message, 192 delay
  lines: ≈ 60 MB an empty song never touches) and, outside the heap, the graphics driver's
  buffers. Mapped in `docs/round38/scorecard.md` §4; cutting it changes the audio thread's
  contracts and is not done in this round.
- **The machine was not quiet**: another build ran through these measurements (the script
  now records it per start — 2 to 3.6 cores of other load). Round 37's quieter run had REAPER
  at rest at 0.3 % and FL Studio at 6.2 %.

## The first measurements (2026-11-06)

**Machine:** AMD Ryzen 7 7735HS (8 cores, 16 threads), 31 GB, NVIDIA GeForce RTX 4060 Laptop
GPU, a 144 Hz display, Windows 11 Home (build 26200), the *High performance* power plan, mains
power, nothing else running (under 5 % of the CPU busy before each program). Raw results:
[`results/2026-11-06/`](results/2026-11-06/).

**Programs:** Cantus 0.1.0-beta.12 (the release build of this round), FL Studio 24.1.2.4394
(the owner's licensed install, its own *FL Studio ASIO* device), REAPER 7.85 (the 60-day
evaluation, a portable install, its own default device: WASAPI, 48 kHz, 512 samples; the
system's plug-in folders not scanned).

### Start-up — `tools/scorecard/startup.ps1`

Five measured starts after one unmeasured warm-up start, each closed politely. *Window*: spawn
→ the main window (not a splash, not a dialog) is visible and answers. *Quiet*: spawn → the
program has used under 0.35 of one core for 1.5 s after its window appeared (the threshold is
the same for all three; 0.35 rather than the script's default 0.10 because Cantus's dancer
breathes at rest, see below). *At rest*: one core's share over 10 s, after 30 s.

| | Window ready (median · best) | Quiet (median) | Private memory | At rest |
|---|---:|---:|---:|---:|
| **Cantus 0.1.0-beta.12** | **1.31 s** · 1.29 s | 10.1 s (5.9–41.6 s) | 267 MB | 29 % of a core |
| FL Studio 24.1.2 | 2.82 s · 2.81 s | 3.6 s | 133 MB | 6.2 % |
| REAPER 7.85 | 1.94 s · 1.92 s | 1.9 s | 62 MB | 0.3 % |

**What it says.**

- **Cantus shows its main window first:** 2.1× sooner than FL Studio, 1.5× sooner than REAPER.
- **Cantus loses the other three rows:**
  - It keeps working for 6–42 s after its window is up (background start-up work, not yet
    traced).
  - It holds twice FL Studio's private memory and four times REAPER's.
  - At rest it uses 29 % of a core: about 13 points are the audio engine playing silence
    through the 16-track demo it opens with, about 16 are the demo's dancer breathing 12 times
    a second, each breath a redraw of the whole window.
- **Each program opened its own start:** Cantus its demo song, FL Studio its template, REAPER
  an empty project. So the at-rest figures are the cost of each program's first screen, not of
  the same content.

All four are round 38's (`docs/next-session.md`, BACKLOG §4b).

**Found on the way, and fixed in this release:**
- **The repaint ticker.** egui takes its predicted frame (1/60 s) off every delayed repaint, so
  the ticker's "next frame in 17 ms" became 0.3 ms, and the window drew at the monitor's rate:
  144 frames a second on this screen, 1.6 cores with the first-run audio setup open.
- **Unconditional repaints.** The first-run setup, the routing meters, the VU needle and every
  open instrument editor asked egui for an immediate repaint outside the ticker.
- **The fix.** The ticker now gives egui's predicted frame back, and those four views go
  through it. With the first-run setup open, Cantus went from 144 to 48 frames a second and
  from 162 % to 78 % of a core. An end-to-end test fails without either fix.

### Capacity — DAWbench-style (`docs/perf/benchmark.md` §3–§4)

Cantus's offline estimate (`cantus-bench --only dawbench`): one core, the most tracks whose
median block stays under half its budget, three searches each.

| Session | 64 samples | 128 samples |
|---|---:|---:|
| DSP: a held saw → 8 Comprimo instances per track | 120 instances (15 tracks) | 120 instances |
| VI: Calor, an 8-note chord every half note | 40 voices (5 tracks) | 40 voices |

These are **estimates, not the published comparison**. The live DAWbench needs:
- the same session rebuilt in each host;
- the CLAP/VST3 build of Comprimo (bit-identical, ADR 0020) and one free third-party
  compressor and synth in every host;
- a real interface;
- tracks added by hand until the host's own dropout counter moves.

That is the owner's run (`docs/beta-checklist.md`). Until it is made, **no capacity
comparison with FL Studio or REAPER is claimed**.

## Re-running

```powershell
# Start-up (each program once by hand first, to close its first-run dialogs). Round 38's rule:
# quiet at 0.10 and 0.35 from the same samples, at rest from 30 s for 10 s.
$m = @{ IdleShare = 0.10; AlsoShare = 0.35; RestAt = 30; RestFor = 10; TimeoutSeconds = 60 }
cargo run --release -p cantus-ui --example scorecard_projects -- projects   # empty, beat, demo
./tools/scorecard/startup.ps1 @m -Exe target\release\cantus.exe -Title "— Cantus" -Name cantus-demo -Out cantus-demo.json
./tools/scorecard/startup.ps1 @m -Exe target\release\cantus.exe -Arguments projects\empty.cantus -Title "— Cantus" -Name cantus-empty -Out cantus-empty.json
./tools/scorecard/startup.ps1 @m -Exe "C:\Program Files\Image-Line\FL Studio 2024\FL64.exe" -Title "FL Studio 2024" -Name fl -Out fl.json
./tools/scorecard/startup.ps1 @m -Exe <reaper>\reaper.exe -Title "REAPER v7" -Name reaper -Out reaper.json
# What a row is made of (no administrator rights): CPU by thread and function, memory by region.
./tools/scorecard/profile.ps1 -Exe target\release\cantus.exe -Seconds 45 -Out cantus.samples
python tools/scorecard/profile_report.py cantus.samples --from 30000 --to 40000
./tools/scorecard/memmap.ps1 -Exe target\release\cantus.exe -After 20
# Capacity (release):
cargo run --release -p cantus-bench -- --only dawbench --json dawbench.json
```

**Cantus's data folder.** Cantus is started with a data folder of its own
(`CANTUS_DATA_DIR`) that holds one setting — `{"audio_setup":{"setup_done":true}}` — so it
starts as a configured copy does, without the first-run audio setup, as the other two were —
and, since round 38, the `plugins.json` of a finished scan (a fresh folder scans its plug-ins
in its first starts, which is not a configured copy's start).

**The quiet time.** The script's *quiet* time is the start of the quiet stretch, so it can fall
just before the window. The table shows the later of the two.
