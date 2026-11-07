# The scorecard — measured

BACKLOG §0's question — *is Cantus measurably the best?* — answered with numbers taken the same
way for every program, on one machine, the same day. The method is ADR 0102; the feature
dimension is [`features.md`](features.md) (97 rows over twelve DAWs). **A row Cantus loses is
published as plainly as one it wins**, and "better" is never claimed without a number here.

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
# Start-up (each program once by hand first, to close its first-run dialogs):
./tools/scorecard/startup.ps1 -Exe target\release\cantus.exe -Title "— Cantus" -Name cantus -IdleShare 0.35 -Out cantus.json
./tools/scorecard/startup.ps1 -Exe "C:\Program Files\Image-Line\FL Studio 2024\FL64.exe" -Title "FL Studio 2024" -Name fl -IdleShare 0.35 -Out fl.json
./tools/scorecard/startup.ps1 -Exe <reaper>\reaper.exe -Title "REAPER v7" -Name reaper -IdleShare 0.35 -Out reaper.json
# Capacity (release):
cargo run --release -p cantus-bench -- --only dawbench --json dawbench.json
```

**Cantus's data folder.** Cantus is started with a data folder of its own
(`CANTUS_DATA_DIR`) that holds one setting — `{"audio_setup":{"setup_done":true}}` — so it
starts as a configured copy does, without the first-run audio setup, as the other two were.

**The quiet time.** The script's *quiet* time is the start of the quiet stretch, so it can fall
just before the window. The table shows the later of the two.
