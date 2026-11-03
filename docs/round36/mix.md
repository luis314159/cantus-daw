# Round 36 — mix: VCA faders, mixer snapshots with cues, the console's safe switches

ADR 0093 · schema 50 · fixture `v50.cantus` · manual `docs/manual/{en,es}/mixing-studio.md`
(plus a row in each `ai-tools.md` and a link in each `README.md`).

## Done

- **VCA faders** (`cantus-core/src/vca.rs`): a strip with `kind: Vca`, no audio (the commands
  refuse a track, output, send or insert on it; the engine never runs it); the strips that follow
  it (`MixerStrip::vcas`, and a group's VCA in `Project::mix.group_vcas`) get its decibels added
  to their own at the gain stage, each reaching VCA counted once (nested, in two VCAs, reached
  twice: each case defined in the ADR and tested); a muted/soloed VCA is its members' mute/solo.
  Members' own faders, automation and modulators are not rewritten. Static decibels fold into the
  strip's gain in `Snapshot::compile`; a VCA (or member) that rides a lane becomes a `VcaLink`
  the engine sums at the 16-sample control point (`cantus-engine/src/vca_rt.rs`).
- **Snapshots** (`mix_snap.rs`, `mix_cmd.rs`): state, never structure — fader, pan, width,
  polarity, mute, solo, swap, inserts switch, EQ, each existing send's level/switch/tap, each
  insert's bypass/dry-wet/native parameters, the master; capture all or some channels; recall all,
  named channels, or with the safe ones; one undo step, exact inverse (`Apply` carries what was
  replaced); update, rename, remove; A/B compare; **recall-safe** strips.
- **Cues** (snapshots as scenes on the timeline): `MixCue { at, snapshot, scope }` compiled into
  automation holds (`cue_lanes`) — through the normal engine path, so block-size independent and
  offline = live; a cue of nothing is a null. One undo step to add/remove.
- **Console feel**: solo-safe (new), recall-safe, group VCAs; trim, two-fader link and a second
  fader law decided against, with reasons, in the ADR §4.
- **UI** (hand-drawn kit, no new widgets): VCA strips (plate, fader, mute, solo, none of the
  audio rows), a bar of the VCA's colour under the number of every strip it reaches (hover names
  the VCAs), the title row's *Snapshots* field (the kit's menu field, with the count) opening the
  take/recall/compare/update/rename/cue/remove menu and the cue list, the strip menu's *Follow
  VCA ▸* / *Safe from recall* / *Safe from solo*, the group menu's *VCA of the group*, *Add VCA*
  in the desk menu. Shoots (Slate and Paper, English and Spanish): `docs/round36/mix/`.
- **Agents**: `create_vca`, `assign_vca`, `take_snapshot`, `recall_snapshot`, `get_mix_snapshots`,
  `remove_snapshot`, `add_mix_cue`, `remove_mix_cue`, `set_strip_safe`
  (`cantus-ui/src/ai_tools_mix.rs`); `tools/list` golden regenerated (`CANTUS_TOOLS_DUMP=1`).
- Strings in both locales (`mix` area): menus, hints, history names (`cmd.mix.*`), the agents'
  sentences (`ai.tools.mix_*`).

## Tests

| Where | What |
|---|---|
| `cantus-core` `vca_tests.rs` (13) | the sets (nesting, two VCAs, diamond once, groups), mute/solo follow, commands refuse and undo exactly, a VCA has no audio, compile folds (unity = same bits), the laned links, solo-safe |
| `cantus-core` `mix_snap_tests.rs` (9) | recall restores exactly and undoes exactly; structure kept; scope and safe set; inverses; cued snapshot not removable; cues compile (hold, clip after the cue takes over); the edit lock sees a recall; JSON round trip; a project without any writes none of it |
| `undo_invariants` | its own stream `xrng` (seed multiplier `0x3C6E_F35F`, +50): 14 kinds, each applied and the checked ones refused; never changed an existing `below(N)` |
| `cantus-engine/tests/vca.rs` (6) | a VCA at 0 dB is a null to the bit; a VCA's fader = the members' faders moved (to the bit); mute follows (to the bit); a ridden VCA = the ride drawn on each member (to rounding) and bit-identical at block sizes 16/64/333/1024; a member's own ride adds the VCA; a cue in a render = the ride, block-size independent, and no cue = null |
| `rt_memory` | `vcas_and_cues_never_allocate`: a ridden VCA, a member's ride, a cue, new snapshot, seek, stop |
| `cantus-project` | fixture `v50.cantus` (a VCA, a follower marked safe, a snapshot, a cue); every older fixture loads |
| `cantus-ui/tests/mixer_vca.rs` | a VCA strip has a fader, mute, solo and none of the audio rows (its followers keep theirs); the Snapshots menu lists snapshots and cues; a recall is one undo step |

## Marked lines (`// r36:mix`)

`cantus-core`: `lib.rs` (3 `pub mod`), `model.rs` (fields `kind`, `vcas`, `recall_safe`,
`solo_safe` on `MixerStrip`; `Project::mix`; the literals; `first_free_strip` skips VCAs;
`strip_audibility` early return and the solo-safe term), `command.rs` (the `Mix` variant and its
four arms; five VCA guards; `InsertStrip` and `RemoveStrip` checks), `freeze_guard.rs` (the `Mix`
arm; a VCA's mute/solo), `freeze.rs` (`STRIP_COSMETIC` gets `vcas`, `recall_safe`, `solo_safe`),
`snapshot.rs` (`vca_links`, cue lanes, the compile call, the fold, `rest` for a VCA) ·
`cantus-project`: `format.rs` (schema 50, `migrate_49_to_50`), `write_fixture.rs` (the block) ·
`cantus-engine`: `automation.rs` (3 lines + the `#[path]` of `vca_rt.rs`), `tests/rt_memory.rs`
(a test) · `cantus-ui`: `ai_tools.rs` (2), `views/mixer.rs` (the `#[path]` of `mixer_vca.rs`, the
`Words` fields, `Renaming::Snapshot` and its arm, `snap_ab`, `open_snapshots`),
`views/mixer_desk.rs` (`vca_marks`), `views/mixer_strip.rs` (the plate, the skipped rows, the
bar, the hover), `views/mixer_menu.rs` (the `#[path]` of `mixer_snap.rs`, `Menu::Snapshots`,
`Sub::{Snap,Vca,GroupVca}`, `Act::Mix`, the entries, the apply arm), `views/mixer_paint.rs` (the
title field) · `tests/undo_invariants.rs` (the stream), `tests/golden/mcp-tools-list.json`
(regenerated: the integrator regenerates again after merging the other builders' tools).

## Needs the owner

- Their eyes on `docs/round36/mix/` (the VCA plate and the colour bar are my reading of "a console's
  VCA group colour"; a different look is one function in `mixer_vca.rs`).
- Their ears on a VCA ride in a real song (the tests prove the arithmetic, not the taste).
- Decisions I made that are theirs to overturn: trim, link and a second fader law left out (ADR
  0093 §4); a cue holds continuous values only (mute, solo, bypass are not automatable in Cantus —
  making them so is its own feature); cues are placed by bar, not by a clip or by a session scene.

## Verification (Windows, this machine)

- `cargo clippy -p cantus-core -p cantus-engine -p cantus-project -p cantus-ui --all-targets` with
  `RUSTFLAGS=-D warnings`: clean. `cargo fmt -p` for each.
- `cargo test -p cantus-core -p cantus-engine -p cantus-project`: 624 passed, 0 failed (the undo
  fuzz's own count of applied commands now includes its own stream's, a bound the new stream
  starved by one or two on seeds 31 and 99); `cantus-ui --lib` 438 passed; `cantus-ui` tests
  `ai_mcp_boot` (golden regenerated), `mixer_vca`, `mixer_desk`, `mixer_routing`, `frame_budget`
  pass.
- Linux (WSL clone `~/ci-r36-mix`, `tools/wsl/check.sh r36/mix clippy`): clippy exit 0, plug-ins
  without the editor exit 0; the clone's `target/` deleted. Linux tests were not run (workspace
  test is the integrator's).

## Review 1 (integrator) — what changed

| # | Fix | Test |
|---|---|---|
| 1 | A frozen bus's key holds the VCAs of the strips summed into it (`vca::with_vcas`: id, fader, mute — added only when a strip has VCAs, so older keys stand); a VCA's automation/modulators reach the region; `SetStripGain/Mute/Solo` on a VCA, `SetStripVcas`, `SetGroupVca` and `SetStripGroup` into/out of a VCA group touch the mix of every strip reached; cues touch what their recall would change | `vca_tests::a_frozen_bus_hears_its_inputs_vcas` |
| 2 | `vca_cycle` uses the self-filtered edges (`vca_direct`) | `a_vca_in_its_own_group_is_no_cycle_and_undo_stays_exact`; fuzz arm 15 (groups, VCAs among them) |
| 3 | `vca::sum_db`: either side at or below −96 dB is −96 dB (silence), in the static fold and on the audio thread | `the_floor_is_silence_whatever_the_other_fader_says` (core), `the_floor_holds_on_a_ride` (engine) |
| 4 | `Snapshot::vca_by_lane` (index, no search); `cue_lanes` emits only what differs from the lane/project over the cue's span, compares whole inserts first | `a_cue_makes_lanes_only_for_what_it_changes`; measurement below |
| 5 | Switch timeline (`mix_scene.rs`, `scene_rt.rs`, `graph/scene_master.rs`): mute/solo as audibility per region, polarity, swap, insert switch, bypasses, strip EQ, master polarity/switch/bypasses; tick-driven at the control points, 16-sample fade out before and in after; installs put the region on at once; empty timeline = engine unchanged. Bar-only placement argued complete in ADR 0093 §3 | `the_cues_switch_timeline` (core); `a_cue_mutes_at_its_grid_point_with_a_fade`, `a_cue_flips_polarity_with_a_fade_in` (engine, block sizes 16–1024, null when nothing switches); `rt_memory::vcas_and_cues_never_allocate` (mute, polarity, master polarity, a seek back across the cue) |
| 6 | `applied > 80` counts the main stream only; the mix stream has its own floor (≥ 3 per seed); the main generator draws among audio strips (no VCA: no change of draw); `SetCue` arm | the fuzz itself |
| 7 | A refused `SetGroupVca` restores the list as it was | `a_refused_group_vca_keeps_the_order` |
| 8 | *Update from the desk* = `mix_snap::recapture` (its own strips, the master only if it had one) | `mixer_snap::tests::an_update_keeps_a_partial_snapshot_partial` (UI), `an_update_keeps_a_partial_snapshot_partial` (core) |
| 9 | `apply_state(…, exact)`: an undo writes the taken state back verbatim (whole parameter maps) | `undoing_a_recall_takes_out_the_keys_it_added` |
| 10 | A cue's levels hold until the target's next clip starts (the clip's pieces inside the span removed), as the ADR and manual say | `a_cue_inside_a_clip_holds_until_the_next_clip` |
| 11 | `frozen` doc back on `frozen`; `InsertTrack` with a VCA strip, `InsertStrip` routed/sent/keyed into a VCA, `SetEffectSidechain`/`InsertEffect` keyed from a VCA refused; A/B's held desk is dropped when anything changed since its recall (an undo included) | `nothing_routes_audio_into_a_vca`; `mixer_snap::tests::an_undone_ab_starts_again` |

The tests were written to the old behaviour's failure (each asserts what the old code did
differently); they were not run against the old commit.

**Full-desk cue, 64 strips** (each with an EQ and a compressor; every fader moved and every mute
flipped; release, best of 5, `full_desk_cue_cost`): **64 lanes** where the first version wrote one
per value a whole-desk snapshot holds (31 168 here); 2 switch regions; `Snapshot::compile` 0.07 ms
(0.01 ms without the cue; 33 ms before the insert-level comparison); the song's offline render
789 ms with the cue against 749 ms without (the machine shared with three other builds).

**Verification after review 1.** Windows: clippy `-D warnings` clean on core, engine, project,
ui; `cargo test -p cantus-core -p cantus-engine -p cantus-project` all pass (the fuzz with the
main stream's `applied > 80` restored); `cantus-ui` lib and the `ai_mcp_boot` (golden
regenerated), `mixer_vca`, `mixer_desk`, `mixer_routing`, `mixer_cables_view`, `frame_budget`
tests pass (`frame_budget`'s stage test failed once while three other builds ran and passed alone —
an allocation count under load, not this round's code). Linux (WSL): clippy clean; the tests of
the four crates found one real fault — the title row's *Snapshots* field overlapped the cable keys
at 720 px and 200 % with Linux's font metrics — fixed (the field gives way like the options key)
and the row test now checks the field too.
