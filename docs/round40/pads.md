# Round 40 — `pads`: a pad's own mixer track, Hear the key, Record before/after FX, L5

Builder: `pads` (phase 2) · branch `r40/pads` · ADRs 0117, 0118 · schema 62 (ADR 0118).

## 1. A pad's own mixer track, chosen per pad (ADR 0117) — first delivery

**What I found first.** The model the BACKLOG thought missing has existed since round 37:
`DrumPad::output` (`None` the kit's strip, a strip the pad's own), saved, routed by the engine
as a bus of the kit carried by a tap track (ADR 0100), with `SetPadOutput` and its exact
inverse in the undo fuzz. What was missing was the way in: only *Split to strips* (all pads, to
new strips) and a pad's chain ever set it, and the rack drew no well on a pad. So this feature
needs **no schema bump**; schema 62 goes to *Record: Before FX · After FX* (§3), which does
change the model. I said so in ADR 0117 rather than inventing a second field for the same thing.

**What changed.**
- `cantus-core/src/padout_track.rs`: `pad_track` (what a pad's well shows: its own strip or
  the kit's), `has_own_track`, `can_carry_pad` (an audio strip, not a VCA nor a chain), and
  `set_pad_track` — one command, one undo step (`cmd.set_pad_track`): `SetPadOutput` for a pad
  without a chain, the chain's `SetStripOutput` for one with (the chain stays the pad's).
  `None` or the kit's own strip = the kit. A free, unnamed strip takes the pad's name in the
  same step (as FL names the track it routes a channel to). New refusal `Refusal::Unfit`.
- `routing::is_fed` now counts a pad playing on a strip: a pad's own strip (also round 37's
  split strips) read "Nothing feeds this strip — route here" in the inspector's Receives.
- The rack (`channel_rack_paint.rs`, `channel_rack.rs`, new `channel_rack_pad_track.rs`): every
  pad row has the mixer track's well (des-views' 24 × 18 at 72), reading the strip the pad plays
  on — the kit's number until one is chosen, as the sheet draws it. A click opens a kit menu:
  *The kit's strip (n)*, *First free strip (n)*, then every strip as `n · name`, scrolling in
  the window. `CantusApp::set_pad_track` is what it calls; a refusal is said on the status line.
- Strings in `areas/pads.{en,es}.txt`; the manual's *Drum kits* chapter (both languages) has a
  section *One pad on a mixer track of its own*.

**How it is tested.**
- `cantus-core` `padout::track::tests` (6): the kit's track until chosen; to a free strip and
  back (named after the pad), undo/redo exact; a strip a pad plays on is fed; sharing another channel's strip; a chained pad keeps its chain; every
  refusal.
- Undo fuzz `a_pads_own_track_undoes_and_redoes_among_the_others` (`// r40:pads`).
- `cantus-engine/tests/pad_track.rs`: a pad sent to its own track sounds **there and only
  there** (its strip's stem = the pad alone, the kit's strip's = the kit without it, to the bit,
  three pads); block-size independence at 0; sent back = the song to the bit; a pad sharing a
  strip follows its fader.
- `cantus-ui/tests/pad_track_e2e.rs` through `CantusApp::assemble`: click the kick's well, click
  *First free strip (n)*; the well reads it; stems through the engine's compile, to the bit;
  saved and reloaded; one undo exact.

**Shots** (`docs/round40/pads/`): `rack-pads-kit-*` (every pad on the kit: "3" under "3"),
`rack-pads-own-*` (the open hat on a strip of its own), `rack-pads-list-*` (the list open),
Slate Night and Paper, 1920 × 1080 at 125 %.

**Looked at only on the CPU renderer**; nothing here needs hardware.

Also for agents: `set_pad_track` (strip_id, free, or the kit's), answered through the MCP
dispatcher (`padout_agent.rs`); the tools' golden list gains it.

## 2. Hear the key (ADR 0118)

- `EngineMessage::SetHearKey(Option<EffectId>)`, `EngineHandle::set_hear_key`/`hear_key`; the
  pass's context carries it; `graph/strip_run.rs` plays the heard insert's key in place of the
  strip's signal from that insert on: a compressor its detector's input
  (`Comprimo::set_hear_key`, = its own *Listen: Key*), any other keyed insert (or a bypassed
  one) the key as it arrives. A strip whose insert is heard never rests. Never saved, never in
  a render (a render is another processor).
- Inspector: *Hear the key*, a des-mixer switch under each keyed insert's *From* row
  (`views/inspector_taps.rs`, one call site in `inspector_sections.rs`, an `Act::HearKey`
  arm in `inspector.rs`).
- Tests: `cantus-engine/tests/hear_key.rs` (off touches no sample — never set, an insert not
  there, on then off: before and after to the bit; on a compressor = *Listen: Key* to the bit;
  on an EQ keyed from the bass = the key as it arrives, to the bit); e2e `taps_e2e.rs` (the
  Sidechain section opened and the row clicked in an app made by `assemble`; the live engine
  plays the unit's *Listen: Key* to the bit; a second click gives the song back).
- Not covered: the master's keyed inserts (the Sidechain section is a strip's).

## 3. Record: Before FX · After FX (ADR 0118, schema 62)

- `Track::record_tap: RecordTap` (`BeforeFx` default, skipped when default), `SetTrackRecordTap`
  with its inverse, `migrate_61_to_62` (nothing to write), `fixtures/v62.cantus` (the second
  track after FX), `freeze`'s cosmetic list (where a take is tapped changes no frozen key — the
  undo fuzz found it: an *After FX* set on a frozen track made its unfreeze impossible to undo).
- Engine: the strip writes what leaves its inserts into a recorder's tap (`Nodes::rec_taps`,
  eight preallocated `PASS_FRAMES` pairs, 64 KB) while recording; the recorder's call moved from
  before the strips ran to after (the input it reads is the same); an after-FX take is stereo
  and drops the strip's lateness (`strip_post`) at the pass's start, so it lands where it was
  played. A mono input recorded into a pair writes the input on both sides.
- Inspector: *Take from: Before FX · After FX* under *Monitor* (not *Record*: the *Arm* row is labelled so), dimmed on a track with no input; its
  tip says After FX needs monitoring (Auto or On).
- Tests: `cantus-engine/tests/record_tap.rs` (before FX = the input to the bit whatever the
  inserts do; after FX = what was heard, stereo, to the bit; with a 5-ms lookahead the take is
  the input through the chain, frame for frame); `recording_rt.rs` records a track after FX
  (nothing allocates); every recording test green (17 + latency + pre-roll + rt);
  `cantus-project/tests/schema62.rs`; `rec_tap.rs` (inverse, no freeze key change); the undo
  fuzz's arm; e2e `taps_e2e.rs` (the Recording section opened, *After FX* clicked: one undo step,
  saved and read back, and a take through the app's own engine handle that is what was heard,
  to the bit).
- Known limit, written in the ADR: over a loop, each pass after the first starts with the
  strip's lateness of the pass before (zero without latent inserts or compensation).

## 4. L5 — the application's engine starts with no compensation line (ADR 0120 §7)

- `Processor::with_lines` / `without_midi_lines` (`new`/`without_midi` keep the 32 start lines
  for what drives a processor without sending lines: tests, the bench, renders' copies); the
  device engine (`EngineHandle` on a real device) starts with **none** and its handle with
  `pdc_lines = 0`, so `lines_needed` is sent ahead of every snapshot (ADR 0107 §3) — already the
  path for anything past 32. `EngineHandle::manual_with_lines` makes a hand-driven one either
  way. `hold_parts_for` now also holds `lines_needed` (a processor started with none, driven by
  hand). Nothing allocates on the audio thread: lines arrive as `PdcLines`, as before.
- Tests: `cantus-engine/tests/pdc_lazy.rs` — a song gaining latency mid-session, and one with it
  from the start, play **to the bit** as an engine with the old 32 lines (and live = offline);
  the lines sent are exactly `lines_needed`; a hand-driven processor with none holds what its
  snapshot needs through `hold_parts_for`. Green: `rt_memory` (39), `pdc` (10), `sched` (the
  schedule fuzz, 15), `pool`, `listen` (the fork keeps its 32: it copies the plan's lines over
  its own), `null_tests`.
- The 2.25 MB is not measured here (the scorecard's measurement is the integrator's, on a quiet
  machine).

## What the owner should look at

- `docs/round40/pads/rack-pads-*` — the rack's well on every pad, a pad on its own strip, the
  list.
- `docs/round40/pads/insp-hear-key-*`, `insp-record-tap-*` — the two inspector rows (the window
  drawn tall so the folded sections show).
- His decision: an own-track pad's number in the accent colour, or the sheet's ink (kept).
- His ears and hardware (`docs/beta-checklist.md`, *Ronda 40 — el canal propio de un pad…*):
  hearing the key on a real song; a take after FX through his interface in duplex.

## Files outside my list (all marked `// r40:pads`)

`cantus-core/src/routing.rs` (`is_fed`), `freeze.rs`, `freeze_guard.rs`, `model.rs`,
`snapshot.rs`, `command.rs`, `lib.rs`; `cantus-dsp/src/comprimo.rs` (`set_hear_key`);
`cantus-engine/src/graph/{strip_run,strip,exec,ctx}.rs`, `effects.rs`, `messages.rs`,
`backend.rs`, `listen_fork.rs`, `recorder.rs`; `cantus-project/src/midi/import.rs` (a `Track`
literal); `cantus-ui/src/views/inspector.rs` (module, `Act::HearKey`), `inspector_sections.rs`
and `inspector_record.rs` (one call site each), `ai_tools_drums.rs`, the tools' golden list.

**Machine note (shared `target-pd`).** cargo's dep-info lists paths relative to the workspace
root, so a crate the `delay` worktree built in the shared target dir looked fresh here and
`cantus-ui` linked `delay`'s `cantus-core` (my new functions "not found"). I touch this
branch's changed files before each build (`F:/projects/daw-r40/tmp/pads-touch.sh`); the
same can hand `delay` a crate built from this worktree. Worth knowing when either of us sees
an impossible error.

## Suites run (Windows)

`cantus-core` (all), `cantus-project` (all), `cantus-engine` (all, `--no-fail-fast`),
`cantus-dsp` (Comprimo's), clippy `--all-targets -D warnings` on core, dsp, engine, project and
ui: green. `cantus-ui`: the tests of what changed (`pad_track_e2e`, `taps_e2e`,
`padout_agent`, `padout_bounce`, `ai_mcp_boot` with the new golden, i18n) green; **the whole
`cantus-ui` suite did not run** — F: filled (21 MB free, `target-pd` 18 GB) and two test
binaries failed to link (LNK1180). The integrator's run of the merged tree is the one to trust
for it.
