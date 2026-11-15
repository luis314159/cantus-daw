# Round 40 phase 3 — `tymp-ui`: Tympanum's views, the instrument mixer, the bands on the desk

**Status: delivered after the merge** (checkpoint D shown to the owner; the integrated tree merged in at
637943ca, `r40/tymp-engine` at 777e55a5). Branch `r40/tymp-ui`, worktree `F:\projects\daw-r40\tymp-ui`.
Schema 70 (ADR 0016 and ADR 0123 amended).

## What is built

| Piece | Where | What |
|---|---|---|
| Kit view | `views/inst/tympanum.rs`, `tympanum_kits.rs`, `tympanum_stage.rs`, `tympanum_cards.rs`, `tympanum_piece.rs` | the kits list (*Modelled* with its counts read from the kit / *Recorded* with *Get · size*, the bar and cancel while it downloads, *Downloaded*, *from the author's site*, *Manual* opening the author's page / *Your own kits* with *Install from a file…*, *Kits on disk*, *Manage…*); each recorded kit once (deduped by pack and kit); the picture through `scene::paint`/`scene::pick` (a press plays and selects a piece, Alt-click walks its articulations, a drag sets the strength, the tags, *Show mics*); the room card (below); the row of pieces with their keys; *Feel* (`timing` — only ever late —, `humanize`, `vary`), *Hi-hat pedal* (controller 4, `hat.chick`), *Kit* (`tune`, `ring`, level); the selected piece's panel (size, *Hear*, articulations, *Where it is struck* on `p{n}.pos` — an offset from the piece's own spot, −1 the centre … +1 the edge, a click on the head sets it and plays there — and `p{n}.spread`; *Sound*; *Heard by*; *Open the mixer ›*). Piece names are the kit's locale keys (`t(&piece.label)`), the pack's own name otherwise |
| Room card | `tympanum_stage.rs` | **Room** is picture and sound in one undo step (`cmd.tympanum_room`): *Stage* writes `look.room = stage` and acoustic room 0 (the kit's own room — a recorded kit as recorded, the modelled one dry); *Booth / Studio / Hall* write the look and rooms 1/2/3. **Floor, Walls, Rug** are the scene's text keys (`look.floor/walls/rug`, schema 69), picture only — the tooltip says so. The meta line says which room you hear ("Stage — the kit's own room, as recorded") |
| Mixer view | `tympanum_mics.rs`, `tympanum_strips.rs` | the band of mics (pattern, distance, aim, bleed as one batch); *What it hears* is the engine's own `tympanum_kit::heard_db`, *Every mic* the matrix; the routes row; the strips with each mic's meter from the engine's telemetry (`METER_TEL + n`, the engine's constant) |
| Shared instrument mixer | `views/inst/inst_mixer*.rs` | as at checkpoint D (Chorda's mixer uses it too), plus a **Delay** cell on every output strip under ø and pan: `o{n}.delay` in ms (0 = as recorded, up to 50), dragged sideways 0.05 ms a point (Shift 0.01), a double-click types it (Enter keeps, Esc leaves); the help line "time-aligns a mic; 0 = as recorded" in both languages. Shown in Tympanum's and Chorda's mixers (the engine's lines are `tymp-engine`'s `OutDelay` and `chorda-src`'s Exemplar lines) |
| Header | `kit/shell.rs`, `kit/segmented.rs` | the view segments measure their labels (`Header::views_w`, `segmented::label_need`, never under the old 58 pt): *Mezclador* whole in every instrument and effect header. Tympanum's well reads *Studio Maple · preset* |
| **Bands on the desk** (owner-approved, `design-p3/groups/`) | `views/mixer_group.rs`, `mixer_group_paint.rs`, `mixer_desk.rs` (+ marked call sites) | a user group is a band over its strips, first to last member, a gap and a seam before and after, its count at the end, a chevron that folds it; folded it is one strip with its members named, their meters side by side and *Unfold* (never hidden); dragging the band's right edge takes in the next strip or gives the last back (one undo step, Esc leaves it); docked right it keeps its band. The two kinds read apart: an instrument's family (solid bar, glyph, *Drums · Tympanum · 3 mics*) and a user group (thin top line, name, count). A band's head is **sticky**: when a band is partly scrolled off, its chevron, the family's glyph and its name stand at the visible part's left and a group's count at its right — the name with its count, alone when only it fits, and never a bare chevron (all or none; the tooltip and the strips name it meanwhile) |
| One band per strip (rule 6) | `mixer_group.rs`, `app.rs` | a strip is in at most one family; a user group either sits inside one family (all its members from it — drawn as a sub-band, one level, folding on its own; a strip leaving the family leaves the sub-group) or outside families (taking whole families: `whole_families` in `create_strip_group` / `add_strips_to_group`). *Each mic to its own strip* makes a sub-group per output family (Kick, Snare, Toms, Cymbals, Overheads, Room) in the same undo step as the split; *All to Main* removes them |
| Strips added to a family (rule 7) | `model.rs` (`MixerStrip::attached`), `command.rs` (`SetStripAttached`), `mixer_menu.rs` | a strip fed only by the family's outputs (a bus, parallel compression) can be attached from its menu: drawn at the band's end, marked *added*. If its routing stops coming from the family it leaves the band and the status line says why. Schema 70 (migration writes nothing, `fixtures/v70.cantus` with a *Kit Bus* attached) |
| Words | `locales/areas/tympanum.*`, `inst_mixer.*` | English and Spanish, every key twinned |

## Files outside the owners table (each line marked `// r40:tymp-ui`)

`views/inst/frame.rs` (no bands for Tympanum; its header words), `kit/shell.rs` and `kit/segmented.rs` (the
measured segments; `views_w: 0.0` in each `Header` literal — fx frame, Pulsus editor, tests, proof example),
`kit/skin/instruments.rs` (`TYMPANUM`), `views/mixer.rs`, `mixer_desk.rs`, `mixer_strip.rs`, `mixer_paint.rs`,
`mixer_cables.rs`, `mixer_drag.rs`, `mixer_parts.rs` (the separator replaced by bands and gaps),
`mixer_menu.rs` (*Add to the family / Take out*), `app.rs` (`attach_strip`, the one-band rule),
`cantus-core` `model.rs`/`command.rs`/`inst/tympanum.rs` (`room_look` keys are text keys),
`cantus-core/tests/undo_invariants.rs` (one arm), `cantus-project/src/format.rs` and `examples/write_fixture.rs`.

## How it is tested

- `tests/mixer_bands_e2e.rs` (through `CantusApp::assemble`): a group is a band that folds into one strip
  and unfolds (one undo step); the edge drag takes in the next strip, Esc leaves it as it was (egui ends a
  drag on Esc itself: the stop frame counts), undo gives it back; the one-band rule (a family joins a group
  whole); *Each mic to its own strip* makes the family sub-groups and *All to Main* removes them, one undo
  step each; a strip attached to a family, and leaving it with the status line's reason.
- `tests/inst_mixer_delay_e2e.rs` (through `CantusApp::assemble`, Tympanum's Mixer view drawn): the snare
  mic soloed, its *Delay* cell double-clicked and typed "1" — the master is the same sound 48 samples later
  at 48 kHz, to the bit, silent before; typed "0" it is the master before, to the bit; each typed value one
  undo step; a sideways drag grows the delay and one undo takes it back.
- `tests/mixer_bands_e2e.rs` also: a group whose first strip the desk scrolls in at the right edge (the rest
  past it) still shows its name over its visible part.
- `tests/tympanum_views_e2e.rs`: a press on the snare plays and selects it; *Each mic to its own strip*
  through the real `plan`; the family band folds and unfolds; `full_name`.
- `tests/tympanum_words.rs`: Kit, Mixer and the grouped desk at 100/150 %, Night/Paper, English/Spanish
  with the text probe — nothing cut (a long pack name like *CrocellKit, every microphone* keeps its head
  and moves what follows its comma to the line under it) — and no egui id clash warning in Tympanum's views.
- `cantus-core` undo fuzz: `a_strip_attached_to_a_family_undoes_and_redoes_among_the_others`;
  `cantus-project`: `schema_70_keeps_a_strip_added_to_a_family`, every fixture loads.
- The lib's tests (545 pass), `app_flows`, `mixer_routing`, `locale_isolation`; clippy `--all-targets`
  `-D warnings` clean on my files (the other owners' findings are listed in the report).

Instrument header goldens: the measured segments keep the 58 pt minimum, so no English header moved and no
golden was re-blessed.

## What the owner should look at

`F:\projects\daw-r40\compartir\p3\tymp-ui\`: `side-{kit,mixer,desk}-*.png` (the design left, Cantus right, 100
and 150 %, English and Spanish). `F:\projects\daw-r40\compartir\p3\groups\`: `side-grp-{band,folded,drag,docked,both}-*.png`
beside the designer's `grp-*` sheets. `docs/round40/tymp-ui/` keeps the 100 % side-by-sides. Made by
`cargo run -p cantus-ui --example shoot_tympanum -- <dir> [--es]` and `--example shoot_bands -- <dir> [--es]`
(the bands on the design's own desk: Neon Tide's tracks, Reverb and Delay docked right, no strip beyond them,
the arrangement closed, the inspector at the design's width showing Vox's strip)
(the scene bakes on the painting thread for shots, `scene::set_blocking`; the live window bakes off-thread).

Left for the owner: the shared device chrome in `kit/shell.rs` (the ▸ and the logo) — the coordinator takes it.

## Verified only in emulation

The shots are the software renderer; the kit's sound is `tymp-engine`'s, proved by the e2e tests as notes and
telemetry, not heard here. DRSKit (1.89 GB) and CrocellKit (5.91 GB) were not downloaded from this worktree;
the list shows each once with its size from the manifest.
