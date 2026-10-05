# Round 19 — master: effects on the master channel (ADR 0026)

FL's master track with its own insert slots: everything that works on a strip's slot
works on the master's, addressed as `StripId::MASTER` through one resolver.

## What is done

**Core (`cantus-core`)**

- One resolver for every effect host: `Project::effects_of(host) / effects_of_mut`,
  `Project::effect(host, id) / effect_mut`, `Project::fx_enabled(host)`,
  `Project::effect_host(id)` (which chain holds an effect) and `Project::all_effects()` /
  `all_effects_mut()` (every insert with its host, the master's last).
  `Snapshot::effect_slots()` is the same for a snapshot (the strips', then the master's).
- Every effect command (`InsertEffect`, `RemoveEffect`, `MoveEffect`, `SetEffectBypass`,
  `SetEffectParam`, `ClearEffectParam`, `SetEffectSidechain`, `SetEffectMix`,
  `SetPluginParam`, `SetPluginState`, `SetEffectMidiSource`) goes through
  `chain_mut / slot_mut` in `command.rs`, so `StripId::MASTER` acts on
  `Project::master.effects` with no special case per command.
- `Command::SetMasterFxEnabled { enabled }` (`cmd.set_master_fx_enabled`), merging like
  the other master controls.
- Sidechains: a key **from** the master is refused (`RoutingCycle`) for any insert; a key
  **into** a master insert from any strip is allowed (the master is processed last).
  `RemoveStrip` is refused while a master insert is keyed from the strip.
- Undo fuzz: a quarter of the inserts land on the master, a quarter of the effect
  commands address the master's chain, `SetMasterFxEnabled`, and keys from the master
  (always refused).

**Engine (`cantus-engine`)**

- `processor_master.rs` (a child module of `processor.rs`, so the shared file changed by
  a few small hunks): `MasterChain` — units, bypass, mix, keys, notes, the chain switch,
  dry lines — built and disposed exactly as a strip's (`install_master` pulls from the
  incoming pool after the strips, before unclaimed units are thrown away; dropped units
  go back through `GarbageEffect`).
- `run_master` runs the chain on the summed mix **before** the master's width, gain,
  balance and polarity. Empty, switched off, or every insert a bypassed wire: it returns
  before reading a sample (bit-exact). Otherwise the mix is lifted out of the interleaved
  output and written back (copies). A mono output gives the chain half the sum on each
  side and takes back their sum (exact at unity).
- Soft bypass (ADR 0022) for latent master inserts: `pdc::Edge::MasterDry(i)`.
- Latency: the chain's latency is added to the output's (`EngineStats::latency_samples`,
  `pdc.total`, so the heard position), to the click's line and to a strip take's line.
  Nothing inside the mix moves.
- Keys into master inserts read the key strip's post-fader buffer of the same segment;
  notes into master inserts (`midi_from`) are due at the master's arrival plus the
  latency ahead of the slot.
- The analyser, telemetry (`Meters::fx`, `comp`, `eq_deltas`) and a hosted plug-in's
  parameter gestures work for a master insert. `Panic`, `EffectMemory`
  (linear-phase EQ on the master), `ReplaceEffect` (a restarted plug-in) and
  `request_memory` include the master's units.
- `Meters::master_gr`: the deepest reduction of the chain's working dynamics over the
  block. New `FxUnit::gain_reduction_db()` (default `None`), implemented by the limiter
  (deeper of now and the last 10 ms column); `EffectUnit::gain_reduction_db()` covers
  Comprimo too.
- The handle (`set_snapshot`), `render`, `stems` and the tests' rigs build units for the
  master through `Snapshot::effect_slots()`.
- **Stems — the decision**: stems do *not* pass through the master chain by default
  (most DAWs; a strip's stem is what it sends on, so the stems still add up to the
  master's input). The option `StemOptions::through_master` passes each stem but the
  master's through its own copy of the master's inserts (bypass, mix, the switch and
  soft bypass honoured), trimmed by the chain's latency so sample 0 is still the song's
  first tick; master fader/width not applied; a keyed master insert hears no key there.
  The stems dialog offers it (*Through the master chain*) when the master has inserts.
  See `stems_master.rs`.
- The mix recorder's master take is post-chain (it always was the output).

**Host (`cantus-host`)**: the plug-in session (`sync`, `find_slot`, `project_for_save`,
`render_units`) walks `Project::all_effects()`, so CLAP/VST3 plug-ins on the master load,
save their state, render offline and open their windows.

**UI (`cantus-ui`)**

- Every `project.strip(id).and_then(|s| s.effect(e))` (≈40 sites: device windows,
  presets, A/B, Comprimo/Aequalis editors, hosted plug-ins, inspector menus) goes through
  `Project::effect(host, e)` / `effects_of(host)`.
- The desk: the master strip has its FX lamp (click: `SetMasterFxEnabled`) beside the
  record dot, and its insert slots in Extra large — the same `inserts()` code as a strip
  (add from an empty slot, lamp = bypass, name = open the editor, drag = move it to
  another slot — new, for every strip — right-click = the slot's menu). The inserts row's
  height counts the master's chain.
- The master's number and name: a click shows the master in the inspector
  (`CantusApp::select_master`, `Subject::Master` followed), outlined in the selection
  colour; a right-click opens the master's menu (*Effects*: Add effect ▸ with every kind,
  Effects on; Pan law ▸).
- A strip's menu gained *Effects*: Add effect ▸ (every kind, the three new ones
  included) and Effects on.
- The inspector's master sheet: Input "The whole mix" → the master's inserts as tiles
  (mini views, mix rings, LEDs, drag to reorder, the band's power = the chain switch,
  right-click menus, add from an empty tile) → EQ and sends dimmed in their places →
  routing (output latency, output "Audio output").
- Device windows: the frame's context names "Master" (`capture_label` maps
  `Strip(StripId::MASTER)` to the master), so presets, A/B, the analyser and the hint
  line work unchanged.

## Tests

- `cantus-core/tests/master_chain.rs` (6): the resolver; every effect command on the
  master undoes and redoes exactly; hosted plug-in commands; keys from the master
  refused and a keyed strip kept; the master's insert limit; the snapshot.
- `cantus-core/tests/undo_invariants.rs`: master arms in the fuzz.
- `cantus-engine/tests/master_chain.rs` (12): `an_empty_or_bypassed_master_chain_is_bit_exact`
  (empty and switched off, every zero-latency kind bypassed, switched off, at 0 % mix);
  `a_bypassed_latent_master_insert_delays_the_mix_exactly`;
  `the_chain_runs_before_the_master_fader` (a limiter at −6 dB, the master +6 dB: peaks at
  0 dBFS, not −6); `a_master_chain_renders_the_same_at_any_block_size` (7, 64, 1000,
  4096); `latency_on_the_master_moves_the_heard_position_by_exactly_that` (output = mix
  delayed by exactly L, the engine's own position unmoved, heard = where it was L samples
  ago; strip and master latency add); `the_click_waits_for_the_master_chain`;
  `takes_line_up_with_a_latent_master_chain` (strip take = the strip alone through the
  master, to the bit; master take = output, post-chain);
  `a_strip_keys_a_master_compressor` (block-size independent);
  `the_master_reports_its_gain_reduction`; `master_units_come_and_go_like_a_strips`
  (through the handle); `a_unit_behind_the_contract_runs_on_the_master`;
  `stems_skip_the_master_chain_unless_asked`.
- `cantus-engine/tests/rt_memory.rs`: `the_master_chain_never_allocates` (limiter, EQ
  switched to linear phase on the master, keyed compressor at 50 %, soft bypass, the
  switch, an insert removed, the master take, seek, stop: zero heap events).
- `cantus-project/tests/master_chain.rs` (3): the chain survives save/open; a bare master
  writes no `effects`; the v18 fixture opens with its limiter on the master.
- `cantus-ui/tests/master_chain.rs` (4): every kind with an editor opens from the master
  in the shared frame, which names the master; A/B (one undo step) and presets on a
  master insert; on the desk the master's slot shows its insert, its FX lamp switches the
  chain (and undoes), a click on its head makes the inspector follow the master; the
  inspector's master sheet paints the mix input, the chain and the audio output.
- Renders (`shoot -- <dir> 1 master`, `shoot_mixer -- <dir> slate-1920x1080-16-master`):
  the desk at Extra large with Aequalis / Comprimo (bypassed) / Terminus on the master and
  the inspector's master sheet with Aequalis's mini view open; the master's menu; the
  limiter's window opened from the master, titled *Terminus limiter (Insert 2 · Master)*.

## Files touched outside my area

- `crates/cantus-engine/src/processor.rs` — only: the `mod master` line, one field and its
  initialiser, `install_master` call after the strips' keys, `run_master` call before the
  master strip, `master_gr` into the meters, the master's units in `Panic` /
  `supply_effect` / `request_memory` / `replace_effect`, and in `compensate` the
  `heard = master + chain` line used for the click, a strip take and the total. Nothing in
  the block start or the track loop.
- `crates/cantus-engine/src/{pdc.rs (Edge::MasterDry), effects.rs, fx/mod.rs
  (FxUnit::gain_reduction_db), fx/limiter.rs, backend.rs, render.rs, stems.rs}`.
- `crates/cantus-engine/tests/{stems.rs (..Default in opts), rt_memory.rs (effect_slots in
  the rig, a new test)}`.
- `crates/cantus-host/src/session.rs` (all_effects; the loop bodies are dedented).
- `crates/cantus-ui/src/views/interop.rs` (the stems option).
- The ≈20 UI files whose effect lookups now go through the resolver (one-line hunks).

## Notes for the other builders and the integrator

- **culmen**: implement `FxUnit::gain_reduction_db()` for the Maximizer so the master's
  GR meter (`Meters::master_gr`) shows it; the limiter's is the model.
- **fervor / latitudo / culmen**: nothing else needed — your kinds appear in every *Add
  effect* list (`EffectKind::ALL`) and work on the master through the same paths.
- **demos**: `Master::effects` plays now; `app.add_effect(StripId::MASTER, kind)` or push
  slots into `project.master.effects` in a builder.
- **engine-rec**: if you add fields to `StemOptions`, use `..Default::default()` at the
  literals (I added `through_master`). The master section of the processor is in
  `processor_master.rs`.
- **ui-rec / ui-audio**: if you add an effect lookup, use `project.effect(host, id)` /
  `effects_of(host)`; `project.strip(id)` never reaches the master's chain.
