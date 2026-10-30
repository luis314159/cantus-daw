# Round 33 — integration: Ableton's core

**The ask** (the owner, 2026-10-28, `docs/next-session.md` of round 32): Ableton's core — the
**session view** (a clip grid per track × scenes, quantised launch, follow actions, recording
into slots, legato, launch modes, stop per track and all, recording the session into the
arrangement as one undo step, arrangement → session, shared tracks and mixer, per-clip
automation, keyboard, MIDI learn, Launchpad/APC grids); **Ableton Link** (licence checked
first); **MIDI clock and MTC** in and out; **MPE** end to end (MIDI 2.0 if it fits, else
measured and noted). During the round the owner added BACKLOG §6i (a character that dances
on screen, driven by automation — round 35 by the owner's leave, 2026-10-29).

## How

- **Item 0 had nothing**: `macos.yml` on public 809644f8 green with no FLAKY lines, no issues
  on `cantus-daw`, the Mac session not done (beta.8 = Windows and Linux).
- **The licence decision at the contract**: Link's SDK is GPLv2+ or proprietary, so Link's
  protocol is written in Rust in a new GPL-3.0-or-later crate `cantus-link`; the MPL engine
  sees only an abstract timeline seam (`cantus-engine/src/sync.rs`) shared by Link, MIDI clock
  in and MTC in (ADR 0078).
- Contract `e0853ec1` on `r33/integration` (`docs/round33/BUILDING.md`: ownership, marked
  lines `// r33:<name>`, briefs). Builders in `F:\projects\daw-r33\{session,launcher,sync,mpe}`;
  `launcher` started from session's first delivery (model, commands, message API), merged as
  `20505f04`.
- **Reviews, one agent each, read-only**: session's first delivery (11 findings), sync (12),
  session's engine (a design verdict and 12), mpe (10), launcher (11 and the shots); one
  reviewer for every fix round (sync 7 new, mpe 8 new, session 8 new, launcher 6 new); one on
  the integrated tree (10 seams). Every finding fixed with a test that fails without it (where
  a test could not fail first — a new API, a measurement — the builder said so).
- **The design verdict on the session engine**: the audio thread runs nothing of the session.
  A pure planner on the interface's side plays the performance as a *projection* — the song
  with what plays written into its arrangement, compiled into ordinary snapshots — so live,
  recorded and offline are one render by construction. Accepted; its timing policy was not:
  the fixed 80 ms lead became two device blocks + the 95th percentile of recent compiles + a
  2 ms floor, presses that miss their boundary by the lead are caught up on it, compiles are
  per track (`Snapshot::compile_reusing`), cut notes are worked out off the audio thread.
- Merges into `r33/integration`, in order: session's first delivery; session's engine
  (`5eba37bd`); sync after two fix rounds (`d658ee43`); mpe after two (`a851e2ef` — schema
  chained 45 → 46 → 47, `fixtures/v47.cantus` regenerated, MPE's port mark made a column of
  sync's Settings ▸ MIDI table); launcher after two (`1df68be4` — the palette's *Toggle
  Ableton Link* wired, the grid column in the table); session's second round (`ce23e294`);
  then the integrated review's fixes.
- **The integrated review's ten seams**, each fixed by its owner with a test: a launch from
  stop while following waits for the source's start (Link's quantum, a master's Start) and a
  relock is not a locate; the lead and the catch-up use the followed tempo; a scene's tempo
  reaches Link from the audio thread (`Snapshot::scene_tempos`, proposed once at its bar);
  MPE expression kept in slot recordings and overdubs; MPE's per-port mark and its zones in
  Settings ▸ MIDI's table; tempo proposed to Link only on explicit edits; a held note
  released when a port's *Notes* is turned off; the palette's *Toggle Ableton Link* wired;
  the dense-install timing test measured over nine pairs' median.

## What each part brought

| Part | ADR | What |
|---|---|---|
| session | 0076 | `Track::slots`, `Project::session` (scenes, settings), `SessionClip` (launch, follow, start, envelopes), `Command::Session`, schema 46; the planner `cantus_core::session::play` (bar-line launches, four modes, legato, follow actions by seeded hash, scene tempo and metre, stops, back to arrangement); the conductor `cantus_ui::session_play` (a derived lead, late presses caught up, plan from the heard state, open-ended loops through stalls); recording into slots (count-in, fixed length, overdub, audio and MIDI laid ahead so the replay is seamless, crash recovery into the slot) and into the arrangement (one undo step); session audio and warp renders preloaded; cut notes listed off the audio thread |
| launcher | 0077 | The Session pane (`Tab`): columns, the master column of scenes, stop row, header, slots with mini views, clip and scene panels, envelopes; editing (drags both ways, copy/cut/paste, duplicate, one undo step each); keymap 14 (13 actions); MIDI learn of slots, scenes, stops and quantise; Launchpad Mini MK3/X/Pro MK3, APC mini mk2, APC40 mkII (programmer modes, colours, the session ring, given back on quit); Settings ▸ MIDI's inputs table that fits and scrolls |
| sync | 0078, 0079 | `crates/cantus-link` (Link's protocol in Rust, every up IPv4 interface rescanned, hostile peers capped); the engine's seam; following sample-accurately (the beat matched when heard); MIDI clock and MTC out stamped per sample, sent at speaker time by an out thread, one connection per output port shared with controller feedback; clock in (an α-β-γ tracker) and MTC chase; per-port flags; the top bar's Sync group; tempo proposed to Link only when asked |
| mpe | 0080 | `Note::expr` (bend, pressure, timbre at MIDI 2.0 resolution), `SetNoteExpr`, `SetInstrumentExpression`, schema 47; compiled beside the notes; Unda, Calor, Modulus, Gravis follow per note; live MPE from marked ports (zones from the MCM and RPN 0); recording per member channel; hosted routes (CLAP note expressions, VST3 note expression, MPE over MIDI; sandbox PROTOCOL 7); MPE MIDI files and DAWproject both ways; three lanes under the piano roll; MIDI 2.0's translation layer (the transport sized in ADR 0080 and BACKLOG §0.2) |

## Measured

- **Link**: two Cantus processes over loopback, worst phase error 0.002–0.012 ms; the engine
  following Link (a device 50 ppm fast, ±1 ms callbacks) worst 0.19 ms.
- **MIDI clock out**: 0 samples of jitter at any block size, within 1 sample through a ramp;
  **MTC out**: every quarter frame within 1 sample, four rates. **Clock in** ±1 ms jitter:
  worst 0.52 ms; **MTC chase**: 0.000 ms with lost or bunched quarter frames.
- **Session**: the lead at 48 kHz before compiles 4.7 / 12.7 / 44.7 ms at 64 / 256 / 1024
  frames; one track launched while a 128-track scene plays compiles in 3.5 ms (5.2 worst); an
  install cutting notes on 32 dense tracks costs +0.03 ms per 64-frame block (the first
  version +6.8 ms).
- **MPE**: a +7-semitone curve measured from the sound — Unda, Modulus, Gravis 700.0 cents,
  Calor 698.2; null to the bit without expression; block sizes 1/37/64/256/1024 identical.
- **The Session pane**: a 16 × 32 session playing, 18 draw calls and 49 160 triangles a
  frame, median 2.0 ms (measured, not asserted); a pad's press becomes a message in 3 µs.

## Causes found, not waits

- F: filled twice while three builders linked at once (62 GB of one target); targets were
  deleted as each builder finished.
- A lib test installed the light palette while another read the process-wide palette (Linux
  only, by timing); the light case moved to a test binary of its own.
- Timing tests under load (`an_install_cutting_32_dense_tracks_is_quick`, the MIDI-out
  thread's 90th percentile, `crash_recovery`'s autosave) failed only with builds running; made
  robust (best of several) or bounded for load, and the final suites ran on a quiet machine.

## Results

On `6582f587` (every branch merged), on a quiet machine, with `RUSTFLAGS=-D warnings`:

- **Windows**: `cargo test --workspace` 5 137 passed, 0 failed, 61 ignored; the engine fuzz
  (`--features fuzz`, `CANTUS_WORKERS=3`) 963 passed; `cargo clippy --workspace
  --all-targets` clean; `cargo fmt -p` clean for every package.
- **Linux** (WSL Ubuntu 24.04, `tools/wsl/check.sh`, a clone of its own): clippy clean; 5 120
  passed, 0 failed, 61 ignored.
- **macOS**: by the runner at publish (`macos.yml`).

## What only the owner can do

- **Hardware**: a Launchpad or an APC (the Pro MK3's side buttons and Stop Clip, the APCs'
  blink rates and colours); a real Link app (Live, a phone app, another PC — LinkHut could
  not be built here); MIDI clock and MTC hardware in and out, or loopMIDI / Windows MIDI
  Services for a loopback; an MPE controller and an MPE plug-in.
- **Ears**: a performance — launching, legato, follow actions on real material.
- Everything rounds 30–32 left (`docs/STATUS.md`).
