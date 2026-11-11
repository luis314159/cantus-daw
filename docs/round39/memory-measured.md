# Round 39 — private memory measured (2026-11-10)

## How it was measured

- **Builds.**
  - Round 39: the integrated release at `r39/stack` b578fc23, i.e. r39/integration fc96e017 plus the start-up stack fix.
  - Control: beta.13's release (`F:\projects\daw-r38\runs\b13\cantus.exe`), measured in the same session.
- **Method.** The scorecard's own script and settings: `tools/scorecard/startup.ps1` with `IdleShare 0.10, AlsoShare 0.35, RestAt 30, RestFor 10`, five measured starts after one unmeasured start.
- **Data folders and projects.** Each build ran with a fresh copy of round 38's configured data folder (`data-m-cantus-empty` / `-beat`) and round 38's `empty.cantus` / `beat.cantus`.
- **What counts here.** The machine was **not quiet**: builds ran beside it. The window, quiet and at-rest columns are therefore not valid here and are not reported. Private memory does not depend on load, so that column is valid.

## Results

| Build — what it opens | Private memory (median of 5) | At rest, 30–40 s | Working set |
|---|---:|---:|---:|
| beta.13 — an empty song | 236.5 MB | 227.5 MB | 248.8 MB |
| **round 39 — an empty song** | **141.2 MB** | **137.2 MB** | 176.0 MB |
| beta.13 — the *Beat* starter | 237.9 MB | 228.6 MB | 251.6 MB |
| **round 39 — the *Beat* starter** | **144.6 MB** | **140.6 MB** | 180.0 MB |

Every one of the five starts falls within ±2 MB of its median. Round 39 holds **−95 MB (−40 %)** on an empty song and −93 MB on the starter.

## Against the others

The comparisons use the scorecard's measurements of the other programs, taken on round 38's quiet evening on the same machine:

- **Empty song:** 141 MB against REAPER's 62. Still lost, by 79 MB.
- **Beat starter:** 145 MB against FL Studio's 133. Still lost, by 12 MB.

## What is left

Cantus's own live heap on an empty song is about 25 MB (`memory_e2e`). The rest of the 141 MB lies outside it:
- the heap's reserved segments;
- the graphics driver's buffers;
- thread stacks;
- the audio and MIDI drivers' heaps.

`tools/scorecard/memmap.ps1` maps these regions. Mapping them is the next memory item.

## Before the release

At the round's close, re-measure the final release on a quiet machine, every column, before the scorecard README and the README change their rows. The website's memory card already shows these numbers (r39/web e7a90ed9).

## The final release, quiet (2026-11-11)

- **Build.** The release build at `r39/integration` 81f88171 (`cantus.exe` 0.1.0-beta.14).
- **The machine was quiet**: no builds; 13 % of the CPU busy before the runs; other programs' load during the runs 0.6–0.9 of a core. Every column is valid.
- **Method.** As above: `tools/scorecard/startup.ps1` with `IdleShare 0.10, AlsoShare 0.35, RestAt 30, RestFor 10`, five measured starts after one unmeasured start, fresh copies of round 38's data folders and round 38's `empty.cantus` / `beat.cantus`.

| beta.14 (median of 5) | Window | Quiet (settled) | Private | At rest (share of a core, 30–40 s) | Private at rest | Working set |
|---|---:|---:|---:|---:|---:|---:|
| An empty song | 755 ms (best 728) | 1 485 ms | **139.7 MB** | 1.1 % (cycles 1.5 %) | 135.7 MB | 171 MB |
| The *Beat* starter | 770 ms (best 760) | 1 515 ms | **142.7 MB** | 2.0 % (cycles 1.7 %) | 138.7 MB | 178 MB |

- **Against beta.13** (round 38's quiet evening, 237 and 239 MB): −97 MB (−41 %) on an empty song, −96 MB on the starter. The window, quiet and at-rest columns are beta.13's within the noise (0.74 s, 1.5 s, 1.6 % on an empty song).
- **Against the others** (the same evening's measurements): 140 MB against REAPER's 62, **still lost, by 78 MB**; 143 MB against FL Studio's 133, **still lost, by 10 MB**. Window, quiet and at rest still won against both.
- The loaded run above agrees: 141.2 and 144.6 MB there, 139.7 and 142.7 MB here.

The scorecard README, the README, beta.14's notes and the website show these numbers.
