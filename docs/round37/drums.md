# Round 37 — drums: Pulsus III

ADR 0100 · schema 56 · fixture `v56.cantus` · manual `docs/manual/{en,es}/drum-kits.md` ·
brief items 1–5, the integrator's three requests and the seven review findings.

## Done (item 1)

- **Kit model 3 — each pad on its own lane** (`cantus-dsp/src/pulsus.rs`,
  `Pulsus::process_buses`): every stretch of up to 256 frames, each pad's voices into a lane
  from +0.0, the lanes handed out in pad order. Unsplit they are added into the kit's buffer;
  split, each goes to its pad's bus. Same lanes, same order: the split kit is the kit to the
  bit. Models 1 and 2 keep their voice-order sum — old songs render as they did, no golden
  moved, `DSP_VERSION` stays 5. `PULSUS_MODEL` is 3: every kit made now. A pad's ringing voices
  follow it to a new output.
- **The multi-output contract**: `InstrumentSpec::output_strips` (bus 0 = the track's own; bus
  `k` = the `k`-th strip the spec names; for Pulsus the pads' distinct `output`s in pad order)
  and `InstrumentUnit::process_buses` (default: all on bus 0) for native units — an Exemplar
  kit's microphones are the next user, not built.
- **Taps**: `Snapshot::compile` appends one `TrackSnapshot` per bus after the project's tracks
  (`tap: Some(Tap { source, bus })`; the source lists them in `buses`). On the audio thread the
  source renders every bus in one sweep into its taps' preallocated buffers and mirrors its
  level, balance, audibility and ramps to them; the plan makes a tap wait for its source. To
  everything else a tap is an ordinary track: compensation, sends, VCAs, solo/mute, stems,
  freeze and the recording mode need no second path. A tap's peak folds into its source's
  meter (the kit's own meter still shows the whole kit).
- **A strip's own pan law** (`MixerStrip::pan_law`): a pad strip gets the 0 dB law (a balance),
  so under the default −3 dB law the split kit is not 3 dB down — and is exact.
- **Commands**: `SetPadOutput` (refuses a missing strip or a VCA), `padout::split_with` /
  `split` (*Split to strips*, `cmd.split_kit`) and `padout::merge` (*Merge back*,
  `cmd.merge_kit`), one batch each; `RemoveStrip` refuses a strip a pad plays on;
  `first_free_strip` skips one; `tracks_on_strip` and `Track::destinations` know pad outputs.
- **Freeze / recording mode**: `CannotFreeze::SplitKit` (a split kit's track cannot be frozen;
  *Freeze* on the kit freezes its strip, which holds the pads); `Why::SplitKit` for a split kit
  straight to the master (its pad strips are baked); the dormant rule wakes a kit when any of
  its outputs is heard; `settle_taps` after `settle`, `isolate`, `rest_for_master`.
- **UI**: *Split to strips* / *Merge back* in the kit's ⋯ menu (greyed when there is nothing to
  do) and in the mixer's strip menu of a kit strip or a pad strip (`CantusApp::split_kit`,
  `merge_kit`, `kit_on_strip` in `cantus-ui/src/padout_ui.rs`); notices for refusals; the kit
  chip says *Model 3*; model 2 kits are no longer shown as model 1 (the upgrade is offered to
  model 1 only).
- Strings, both locales, `padout` area: `cmd.*`, menus, notices, the freeze refusal, the
  recording-mode reason.
- Schema 56: migration (writes nothing), 54 and 55 as empty stubs for piano's numbers,
  `fixtures/v56.cantus` (the fixture's Drums kit split, one pad fader at −3 dB).

## Done (items 2–5, the requests, the review)

- **A chain per pad** (ADR 0100 §6): a strip of kind `Chain` the pad plays into, hidden from
  the desk, shown on the pad's **Chain** card (inserts with switch, *Edit*, *Remove*, *Add
  insert ▾*, and *More…* — the device picker, plug-ins included — once the pad has one);
  made with its first insert, gone with its last (`padout::chain_add` / `chain_remove`, one
  undo step each, `cmd.pad_chain_*`); split moves a chain's output, merge keeps the chain.
  **At rest it costs nothing**: a chain strip with nothing in, quiet EQ and lines, no key or
  note and every insert at rest skips its inserts (`skip`, ADR 0031 §5.1) —
  `EngineStats::chain_rests`; measured 0.027 s against 0.018 s with no chain (13 resting,
  1 running), exact against the same chains as ordinary strips.
- **Kits from outside** (§7): `pads::import::kit_from_files` (EN/ES names → General MIDI,
  numbered takes → round robin, dynamics → velocity layers, the rest by order, what does
  not fit said) and `kit_from_zones` (an SFZ kit through Exemplar's reader: layers, round
  robin, `off_by` groups joined into choke groups). A folder or an `.sfz` dropped on a kit's
  pads loads onto it (outputs carried), elsewhere a new kit track — one undo step. Tested
  with Virtuosity Drums' published `01-basic-kit.sfz` and its 122 includes (CC0, byte for
  byte, `crates/cantus-ui/tests/fixtures/sfz/virtuosity_drums`, README with source and
  commit).
- **Original kits** (§8): sixteen factory kits (house, lo-fi, rock, pop, R&B, reggaeton,
  cinematic and jazz brushes added, all synthesised by Pulsus), every one tagged
  `preset.style.*` and `preset.role.drums` for the library's browser (the kit floor of 16
  met); *Browse presets…* in the kit's ⋯ menu, tested through its row with a kit loaded in
  one undo step.
- **Agents**: `split_kit_outputs` (with `merge`), `set_pad_chain`, `make_kit_from_folder`
  (`cantus-ui/src/ai_tools_drums.rs`, one marked registration line); `tools/list` golden
  regenerated.
- **Bounce in place of a split kit's track** (request 1, review 1): bounces the whole kit,
  bit-equal to the unsplit kit, into the kit's strip, with a notice that the pads' strips are
  not in it (`padout_bounce.rs`). **A pad strip's bounce** (review 2) refuses and points to
  the kit's strip.
- **DAWproject** (request 2, review 6): a split kit is written whole and valid against the
  official XSDs, the report says the split and the pads' strips are lost
  (`x_split_kit`, Dropped); the kit comes back unsplit, every pad as it was; the reader
  clears a pad's output (a stale strip id could name another strip); a strip's own pan law
  is said lost (`x_pan_law`).
- **Stop then play** (request 3): investigated — tails ring by design (stop and seek release,
  ADR 0028); one defect fixed in my area: Pulsus's `all_notes_off` did nothing, so a pad that
  chokes on its note-off rang on past a stop; the contract's `reset` doc corrected.
- **Orphans** (review 3): another kit carries the outputs by pitch, then voice; another
  instrument or the track deleted removes the pad strips and chains in the same undo step
  (`padout::with_orphans_removed`, app's `set_instrument`, `delete_track`, bounce's remove);
  merge removes orphans left before; `first_free_strip` skips strips made for pads.
- **Model readout** (review 4): Model 1 / 2 · Pulsus II / 3 · Pulsus III; strings name
  Pulsus III; models 1 and 2 are offered the upgrade to 3 (2 → 3 says the measured
  difference, −135 dBFS, one undo step).
- **The condition stated** (review 5): ADR §1 and the manual; tested at −4.5 dB / balance
  0.3 and under a level ride (1e-6).
- **A strip's own law visible** (review 6): the strip menu's *Pan law: its own ▸ / the
  project's ▸* with every law (`SetStripPanLaw`, `cmd.set_strip_pan_law`, fuzz arm).
- **Menus tested as a click drives them** (review 7): the kit's ⋯ rows come from one
  function (`more_rows`/`more`), tested; the mixer's strip menu entries applied in a unit
  test; `padout.cannot_freeze_master` for a kit straight to the master.

## Done (the finishing items and the second review)

- **Kit gain stage** (ADR 0100 §11, schema 57): `KitSettings::gain_db`, *Kit level* on the kit
  card, applied per lane before the pad-order sum (split = unsplit to the bit), exactly 1.0
  at 0 dB (no golden moved).
- **Kit levels** (the integrator's decision: single-pad rule kept, drums window −19 ±2 LUFS):
  measured pad by pad, the groove's loudness is the kick's, so lowering the kick lost level;
  the factory kits instead get a driven kick (same peak, more body), the snare +2…+3 dB and the
  hats +3 dB toward it, and gains recalibrated. Every kit now sits −18.3…−20.6 LUFS and the
  known list holds no kit; the dense riff stays under full scale (tightest −1.0 dBFS). The
  window constant is set to −19 in `preset_render.rs` (marked line; library's constant).
- `picker.dev.pulsus.name` is "Pulsus III" in both locales (marked).
- BACKLOG §4b: every strip could rest like a pad's chain (with the measurement); §3e: the
  transcriber misses the jazz kit's brush snare.
- Review 2, all eleven: (1) SFZ `group=` is `zf::EXCL`, `off_by` one-way unioned, clamped
  groups skipped and logged, GM hats joined — Virtuosity's hats choke from the real file;
  (2) `make_kit_from_folder` is a *Files* tool through `resolve_touch`/`may_touch` (every file
  and include), refuses a track that is not a kit, answers file names only (the import's log
  names files, not folders); (3) `padout::is_split` everywhere, *chains* wording for freeze and
  the recording mode, the bounce keeps chains (`kit_sum_strip`), DAWproject's `x_pad_chains`;
  (4) orphan cleanup in the bus (`may_orphan` → `with_orphans_removed`), every caller plain,
  and the agent's `apply_preset` carries a split kit's outputs; (5) `StripKind::Pad` marks pad
  strips (the v56 migration sets it), no per-strip law offered on a VCA; (6) `on_desk` filters
  cables/matrix/route menus/stems, the mixer JSON says `kind`; (7) `carry_outputs` pitch first,
  then voice; (8) e2e: the chain card clicked through `assemble`, the drop path through its
  thread, the three tools through the MCP dispatcher, a latent chain compensated; (9) folders
  read bounded (512 sounds, 8 192 entries, 4 GB) on a thread, a folder naming no drum refused
  off a kit; (10) numbers that name the drum kept (Tom 1/2/3, Crash 1/2, 808/909 Kick);
  (11) `kit_made_short` counts notes, the chain card's tips name the insert, `pulsus.pad.fx_chip`.
- The undo fuzz's two routing arms that read strips with outputs or sends now build their
  own when none is left (pad strips, which were those, now go with their kits).

## Tests

| Where | What |
|---|---|
| `cantus-dsp` `pulsus_lanes_tests.rs` (4) | model 3 split to buses = the kit to the bit; lanes block-size independent at 0 (1…4096); model 2 = its voice-order sum, model 3 within 1.8e-7 (−135 dBFS) of it; a moved pad takes its ringing voices |
| `cantus-core` `padout_tests.rs` (6) | split: strips in pad order after the kit's, named, into the kit, 0 dB law, unity; buses; processing order; one undo step back exactly; merge keeps a strip a send uses, removes the rest; `RemoveStrip`/`first_free_strip`; taps in the snapshot (ids, strips, levels, the kit's mute); taps' rest |
| `cantus-core` `undo_invariants.rs` | its own stream (`krng`, `round37_padout_command`): kit, split, merge, pad to a strip (refused to one that is not there), pad back — each applied, the refusal seen |
| `cantus-engine` `tests/padout.rs` (10) | split at unity = unsplit to the bit (512 and 1, 64, 333, 1024, 4096 frames; under the default −3 dB law); merged back = the kit; real time (irregular blocks) = offline, pool 1 and 3 workers = serial, no borrow failed; a pad fader moves only its pad; mute/solo of the kit and of a pad strip, the kit strip soloed; a VCA over a pad strip = its fader to the bit; a send carries that pad alone; compensation around a latent (linear-phase) pad strip to the bit; freezing the kit's strip or a pad strip = the live mix to the bit (64, 512, 1024), the kit track refused, *Freeze* on the kit → its strip; the recording mode bakes a split kit to the bit, `Why::SplitKit` straight to the master; stems (pad strips; the kit's track stem = every pad within 1e-6; the master) |
| `cantus-engine` `tests/rt_memory.rs` | `a_split_kit_never_allocates`: ride on the kit's level, merged and split while ringing, seek, stop — serial and on 3 workers |
| `cantus-project` | `schema_56_keeps_a_kit_split_to_strips` (v56 split, v53 whole, model kept, no law) |
| `cantus-dsp` `pulsus_lanes_tests.rs` (5th) | a stop lets go of a pad that chokes on its note-off; a one-shot rings on |
| `cantus-core` `padout_tests.rs` (+3) | a pad's chain made with its first insert and gone with its last; split and merge move a chain and keep it; another kit carries outputs by pitch, another instrument or the track deleted takes the pad strips in the same step, merge removes old orphans |
| `cantus-core` `pads_import_tests.rs` (5) | EN/ES names → roles; a folder → a kit on the GM map (takes → round robin, dynamics → velocity layers, hats choke, the rest by order); velocity layers each cycling their takes; what does not fit said; SFZ zones → pads, layers, `off_by` groups joined into one choke group |
| `cantus-core` undo fuzz | its stream now also adds and removes chain inserts and sets a strip's pan law (refused on a strip that is not there) |
| `cantus-engine` `tests/padout.rs` (+4, 14) | sixteen chained pads split to sixteen strips = the unsplit kit to the bit (512, 333, live on 3 workers); a chain at rest rests (counted) and costs 0.027 s against 0.018 s with no chain; resting chains = the same chains as ordinary strips, to the bit; a kit level of −4.5 dB / balance 0.3 and a level ride agree within 1e-6 |
| `cantus-ui` `tests/padout_bounce.rs` | a split kit's track bounced in place = the unsplit kit to the bit, the notice said; a pad strip's fader is not in the bounce |
| `cantus-ui` `tests/padout_kits.rs` | Virtuosity Drums' `01-basic-kit.sfz` (CC0, from outside) → a mapped kit, one microphone position per key, every velocity zone, round robin; a folder of our own WAVs → a kit through the app, one undo step, each named pad heard through the engine |
| `cantus-ui` `mixer_menu.rs` unit tests (2) | the strip menu's *Split to strips* / *Merge back* applied as a click applies them, from the kit's strip and a pad's; *Pan law ▸* sets and clears a strip's own law |
| `cantus-project` `tests/dawproject_padout.rs` (2) | a split kit exported valid against Project.xsd and MetaData.xsd, the loss said, back whole with every pad as it was; a strip's own law said lost |
| `cantus-ui` `tests/padout_e2e.rs` | through `CantusApp::assemble` and the manual engine's processor: the app's *Split to strips* (one undo step, 14 strips into the kit) played live = the unsplit kit's export to the bit; a pad strip's fader at −12 dB live = its export to the bit, and the change is that pad × (g − 1) within 2e-6; *Merge back* = the kit to the bit; undo gives the song as opened | Plus: the kit's ⋯ menu rows split and merge; *Browse presets…* opens the library's browser, 16 kits there with style and role, one loaded in one undo step.

Numbers on Windows (the end of items 2–5, `1a5d99ec` and after): cantus-dsp 775, cantus-core
737, cantus-engine 1 098, cantus-project 173, cantus-ui 2 369 passed — 0 failed. Clippy
`-D warnings --all-targets` on the five crates: clean. Linux (WSL, workspace clippy and the
plug-ins without editor): clean at `15d834bf`. The round-34 drum transcription benchmark
(`transcribe_drums.rs`) now scores a class only on kits with a pad for it (marked line): a
kit with no toms plays silence where the pattern has toms; it also shows the transcriber does
not take the jazz kit's brush snare for a snare (0 of 28) — the transcriber's, noted.

## Marked lines in shared files (`// r37:drums`)

`cantus-core`: `command.rs` (SetPadOutput, its label, RemoveStrip's refusal), `freeze.rs`
(SplitKit, sounds_in, the dormant wake, settle/isolate → settle_taps), `freeze_guard.rs`
(SetPadOutput's touches), `instrument.rs` (`output_strips`), `model.rs` (`pan_law`,
`destinations`, `feeds_through_bus`, `tracks_on_strip`, `first_free_strip`), `record_mode.rs`
(Why::SplitKit, rest_for_master), `snapshot.rs` (buses, tap, pan_law, append_taps), `lib.rs`,
`tests/undo_invariants.rs` (own stream). `cantus-engine`: `graph/{plan,plan_compile,strip,
strip_run,track}.rs`, `inst/mod.rs` (the contract), `listen_fork.rs`, `processor.rs`,
`processor_snap.rs`, `stems.rs` (`track_add`), `lib.rs`, `tests/rt_memory.rs`. `cantus-project`:
`format.rs` (56, and 54/55 stubs), `examples/write_fixture.rs`. `cantus-ui`: `freeze.rs`,
`record_bar.rs`, `ai_tools_record.rs` (the new arms), `views/mixer_menu.rs` (the strip menu's
entries), `views/pulsus_editor{,_panel}.rs`, `lib.rs`. No `kit/` file and no mixer drawing file
(finish's) touched.

## Owner items (and two for the integrator)

- **Ears**: the eight new factory kits (house, lo-fi, rock, pop, R&B, reggaeton, cinematic,
  jazz brushes) are tuned by numbers (headroom, decays, filters), not yet by ear.
- **Ears**: the rebalanced kicks (drive) and the lifted snares and hats are set by numbers,
  not yet by ear.
- *Integrator*: the manual chapter says "0.1.0-beta.12" for the next release.
- *Integrator*: generalising the chain's rest to every strip (ADR 0031 §5.1) is the next
  performance step — measured exact here for chains only.

Verified only in emulation: nothing here needs hardware; DAWproject validity is checked
against the official XSDs, not opened in another application.
