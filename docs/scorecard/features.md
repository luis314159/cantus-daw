# The scorecard — features (round 37, researched 2026-11-04)

BACKLOG §0.1's *Features* dimension: every feature the leaders have, and for each DAW
**have**, **partial** (what is missing), **missing**, or a number. The rows are in
[`rows.md`](rows.md) (97 of them). The cells are in [`matrix/`](matrix/):

- [`matrix/cantus.md`](matrix/cantus.md) — Cantus, each cell with the ADR, STATUS round or
  BACKLOG note that establishes it (the repository at `c95ee8a9`).
- [`matrix/leaders-a.md`](matrix/leaders-a.md) — FL Studio 2026 (26.1.7.5), Ableton Live
  12.4.16, Bitwig Studio 6.1.4, Logic Pro 12.4.
- [`matrix/leaders-b.md`](matrix/leaders-b.md) — Cubase Pro 16.0.0, Pro Tools 2026.11, Fender
  Studio Pro 8.2 (formerly Studio One), REAPER 7.84; Ardour 9.8, LMMS 1.2.2, Zrythm 1.0.0,
  Waveform Free 14.1.

## Method

- **Sources**: every other DAW's cell rests on its vendor's current manual, release notes or
  product page (versions and URLs under each file's *Sources*); nothing from memory. A cell no
  vendor page confirmed either way says `unverified` and why. `missing` means absent from the
  vendor's current documentation for that area — a documentation finding, not a hands-on test.
- **Cantus's cells** cite the repository; a feature only planned or done in part is `partial`
  or `missing`. **"Better" is never claimed without a measurement against another DAW.**
- **Unverified**: 23 of 388 cells in part A, 149 of 776 in part B — most where vendors' pages
  refuse automated reading (avid.com and fender.com answer 403; steinberg.net renders by
  script) and for Zrythm and Waveform Free, whose manuals do not list their bundled devices. A
  hands-on pass is needed before any of those cells is quoted.
- **Re-run every release**: features change; the versions above date the cells.

## What it shows (against the eight commercial leaders)

**Rows where Cantus is missing or partial and at least one leader has it (22):**

| ID | Feature | Cantus | Leaders that have it |
|---|---|---|---|
| I3 | Slicer | missing — **round 37 (piano builder: Sectio)** | FL, Live, Bitwig, Logic, Cubase |
| I4 | Drum machine with per-pad outputs and chains | partial — **round 37 (drums: Pulsus III)** | Live, Bitwig, Logic, Cubase |
| W5 | Browser with previews (samples, presets) | partial — **round 37 (library: audition)** | FL, Live, Bitwig, Logic, Cubase, Pro Tools, REAPER |
| F4 | Convolution reverb | partial (algorithmic only) | FL, Live, Bitwig, Logic, Cubase, Studio Pro, REAPER |
| E11 | Insert/delete time, ripple edit | missing | FL, Live, Logic, Cubase, Pro Tools, REAPER |
| E12 | Clip groups | partial (linked clips only) | FL, Bitwig, Logic, Cubase, Studio Pro, REAPER |
| P3 | AU hosting (macOS) | missing | FL, Live, Logic, Studio Pro, REAPER |
| P8 | ARA 2 hosting | partial (out-of-process slice open) | Logic, Cubase, Pro Tools, Studio Pro, REAPER |
| W9 | Video track | missing | Logic, Cubase, Pro Tools, Studio Pro, REAPER |
| X7 | Surround / Dolby Atmos | missing | Logic, Cubase, Pro Tools, Studio Pro |
| I8 | Granular / spectral sampler | partial | FL, Live, Bitwig, Logic |
| M4 | Step sequencer per melodic channel | partial | FL, Logic, Cubase, Studio Pro |
| M1 | Ghost notes in the piano roll | partial | FL, Bitwig, REAPER |
| I9 | Physical-model instruments | partial | FL, Live, Logic |
| W7 | Scripting API | missing (MCP tools only) | Live, Pro Tools, REAPER |
| W8 | Project versions / snapshots | missing | Logic, Studio Pro |
| X11 | AAF/OMF | missing | Cubase, Pro Tools |
| W4 | Multiple arrangements | missing | FL |
| M6 | MIDI 2.0 transport | partial | Logic |
| X8 | AAC export | partial | Studio Pro |
| I10 | Vocal synthesis | missing | Cubase (Omnivocal, beta) |
| AI4 | Chord-following session players | missing | Logic |

**Rows only Cantus has, as documented (3):** an agent interface over MCP (AI5 — FL's and Pro
Tools's assistants are partial; Ardour 9.5, a free DAW, ships an experimental MCP server),
open source among the commercial-class DAWs (O1), and **bit-exact determinism — offline equals
real time, the same hashes on three systems** (O4: no vendor states it; Cantus tests it).

The next rounds' order should follow this table: the rows most leaders have first
(convolution reverb, ripple edit, clip groups, AU, video, surround), then the rest. Recorded in
BACKLOG §0.4.
