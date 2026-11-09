# Round 38 — integration: at rest costs nothing, and Chorda's picture follows its controls

**The ask** (the owner, 2026-11-08, after beta.12): make Cantus win the scorecard's
start-up rows honestly against FL Studio and REAPER (BACKLOG §0.1, ADR 0102), and fix
Chorda's scene, which pixelated while *Width*, *Lid* or *Room* moved. On the way the owner
decided that **nothing dances at start** (*"el dancer por default cada vez me gusta menos,
quítalo"*) and moved the next round to UX and graphics (BACKLOG §6j, round 39); the third
voicing pass of Chorda, half done, goes to round 40.

## How

Two branches from master `709ee14c`, merged into `r38/integration`
(`F:\projects\daw-r38\integration`) with no conflict (only `BACKLOG-excellence.md` was
touched by both, in different sections):

| Branch | By | Up to | What |
|---|---|---|---|
| `r38/scorecard` | a builder agent | `264716fb` | ADR 0106 (at rest costs nothing) and amendments to 0031, 0059, 0091, 0102; the profiler and memory tools; the start-up rows measured again. Report: `docs/round38/scorecard.md`. |
| `r38/chorda` | the integrator (an earlier session) | `2f05056a` | The scene's baker ~4× faster, the previews whole, voicing by register (flat), BACKLOG §6j with the owner's crops. Report: `docs/round38/chorda.md`. |

No ADR or schema numbers collided (the round adds ADR 0106 and no schema). ADR 0104 is
amended with the new baker. Version **0.1.0-beta.13**.

## What each part brought

- **At rest costs nothing** (ADR 0106): every strip with inserts, the master chain and
  Spatium stop running over exact silence and wake to the sample, held by twins to the bit;
  the master's loudness meter counts silence through clear filters; no view asks egui for
  screen-rate frames (the mixer's meters, the effect editors, the pinned meter, the dancer
  editor go through the ticker); the first-run audio setup looks four times a second;
  Chorda's reference keyboard is designed when the instrument picker opens, not at start;
  the MIDI-out thread and the device watcher sleep (the watcher waits for Windows' endpoint
  notification).
- **Nothing dances at start**: the song Cantus opens on is the house drop without Trino;
  a dancer the user adds holds still ten seconds after the last thing that happened.
- **Chorda's scene**: span rasterization, rows in bands taken by free threads and the
  once-serial passes on every thread — the full bake 597 → ~140 ms, the previews whole. The
  owner confirmed on 2026-11-09 (`builds\chorda-scene2`) that *Width*, *Lid* and *Room* no
  longer pixelate.
- **Voicing by register**: four peaking bands per register bus, in the code and flat (the
  default skips the bus path, to the bit); the third voicing pass is round 40's.

## The reviews (one agent each, read-only, no builds)

**r38/chorda: no correctness finding.** One claim was too strong: the span walk is
bit-identical to the old box walk for the scenes Cantus draws (24 bakes hash the same) and
across thread counts by construction, but a sample decided only by f32 rounding noise, more
than two samples outside the f64 interval, would now be skipped — ADR 0104's amendment says
so plainly. A doc comment contradicted the code (fixed). The baker's per-pass thread spawns
and dummy buffers are a performance item (BACKLOG §8).

**r38/scorecard: the engine's rest held on every path the reviewer tried** (sends, keys
before and after, the master, hosted units and timed automation, glides, stems and freeze
taps, the meter's silence path, Spatium's skip). Found and fixed here:

1. A dancer played live from a MIDI keyboard while stopped froze once it held still — notes
   wake the window but were not *something happening*. A note on a dancer's feed now wakes
   it (`at_rest_e2e::a_note_played_live_wakes_a_dancer_that_holds_still`).
2. The import re-check ran on two clocks, re-asking an imported character every frame while
   its card showed; it runs on the wall clock.
3. The dancer editor's stage card asked for 60 frames a second while its dancer held still.
4. The endpoint watch is armed again as soon as it fires, and a watch the registry gives up
   falls back to the 1.5 s poll (not a rescan every 0.4 s).
5. Two tests could not fail: `memory_e2e` now bounds each step of a start (its two tests run
   one at a time), and `a_knob_turned_while_the_desk_rests_changes_nothing_heard` turns a
   knob on every chain while the desk rests against a twin that never rests.

## The integrator's own fixes

- `fx_lint` failed on both systems: the scene's dissolve tint was written
  `Color32::from_rgba_premultiplied(a, a, a, a)`, which the kit's lint reads as a spelled-out
  colour; it is `Color32::from_white_alpha(a)`, the same four bytes.

## Measured

On a quiet machine (2026-11-09: the owner closed everything, WSL shut down, under 4 % of the
CPU busy before each program), the release build of this tree, `tools/scorecard/startup.ps1`
through `F:\projects\daw-r38\runs\measure-quiet.ps1`; raw results in
`docs/scorecard/results/2026-11-09/`, the table and what it says in `docs/scorecard/README.md`.

| Program — what it opens | Window | Quiet < 0.10 | Private | At rest (ticks · cycles) |
|---|---:|---:|---:|---:|
| Cantus — an empty song | **0.74 s** | **1.5 s** | 237 MB | **1.6 % · 1.6 %** |
| REAPER 7.85 — an empty project | 1.76 s | 2.0 s | **62 MB** | 2.3 % · 3.7 % |
| Cantus — the *Beat* starter | **0.76 s** | **1.6 s** | 239 MB | **2.0 % · 1.8 %** |
| FL Studio 24.1.2 — its template | 2.49 s | 5.8 s | **133 MB** | 6.6 % · 6.7 % |
| Cantus — its start (the drop, no dancer) | 0.95 s | 1.7 s | 273 MB | 3.0 % · 2.9 % |

Cantus wins window, quiet and at rest in both like-for-like pairs and **loses memory** in
both. REAPER at rest measured 2.3–3.7 % in all five starts, not round 37's 0.3 %; both are
published. The website (`web/src/measured.rs`), the README and the release notes carry these
rows.

## Checks

- **Windows**, the full suite on `8e374c98`: 6 550 passed, 2 failed, 102 ignored —
  `fx_lint` (fixed above) and `clap_window::its_own_popup_or_a_topmost_window_over_it_leaves_it_in_front`,
  which opens real windows on the owner's desktop while it was in use and passed 3 of 3 run
  alone. After the review's fixes (`8689da54`) the affected binaries again (`at_rest_e2e`
  with its two new tests, `memory_e2e`, `fx_lint`, `dancer_demo`, the Chorda scene's three,
  `inst_chorda_boot`, `dance_live`, `cantus-rt`, `hotplug`, `clap_window`): all pass — **6 554**
  with the two new tests. The engine fuzz (`CANTUS_WORKERS=3`): **1 106** passed, 16 ignored.
  Clippy `--workspace --all-targets -D warnings`: clean. No test binary was blocked by UAC
  this round.
- **Linux** (WSL Ubuntu 24.04, `tools/wsl/check.sh` in its own clone `~/ci-r38-integ`, the
  warm `target/` of round 37's so the disk image on C: did not grow): on `8689da54`
  **6 515 passed, 0 failed, 102 ignored**; clippy and the plug-ins without their editor
  clean; `cargo fmt --all -- --check` clean. (On `8e374c98` the same suite had `fx_lint` as
  its one failure.)
- macOS: at the tag, on GitHub's runners.

## What was verified only by measurement, tests or emulation

- The device watcher's notification was checked to arm and time out on this machine; an
  interface plugged and unplugged is the owner's (`docs/beta-checklist.md`, *En reposo*).
- A dancer woken by a MIDI keyboard is tested with notes sent to the engine, not a real
  keyboard.
- macOS is built, tested and packaged on GitHub's runners at the tag, not on a real Mac.

## What is left

- **Round 39, UX and graphics** (BACKLOG §6j, `docs/next-session.md`): a design agent who
  draws every element by hand from a blank sheet, a kit builder, a UX builder; the owner
  accepts on whole windows.
- **Memory** (ADR 0106 §4, the scorecard): the engine's preallocation for its largest
  session — Pulsus kits built as the native instruments are (−31 MB), the meters out of the
  event queue (−15 MB), a delay-line pool that grows (−9 MB); each its own ADR.
- **Round 40** (proposed): Chorda's third voicing pass and ABX 4, a convolution reverb,
  ripple edit / insert and delete time.
- The owner's own items in `docs/beta-checklist.md`.
