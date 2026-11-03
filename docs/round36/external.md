# Round 36 — external (ADR 0094 external units and the ping, ADR 0095 Mackie Control and HUI)

Branch `r36/external`, schema **51** (`fixtures/v51.cantus`). Manual: `docs/manual/{en,es}/external-gear.md`.

## Done

**External units (ADR 0094).**
- Model: `cantus_core::external_unit` — `ExternalUnit` (instrument: MIDI route + return pair;
  effect: send pair + return pair), `Measured`, `Availability`, `Present`; `Project::external_units`
  (schema 51, nothing written when empty). Commands `InsertExternalUnit`, `RemoveExternalUnit`,
  `SetExternalUnit`, `SetExternalLatency` (exact inverses, `cmd.external.*`, freeze-guard arms,
  undo fuzz in `tests/external_units.rs` in its own stream: 60 seeds × 90 steps, undo-all and
  redo-all exact).
- The effect's send: the track's source goes out of the send pair (past the master's 1-2) and the
  return is the track's input at its own level; bit-exact to the plain render after the wire's
  delay (tests `mod send`).
- Latency into PDC: the measured trip + trim is the track's own reported latency
  (`TrackSlot::external`) — tested with two monitored inputs that meet exactly once the unit says
  how late it is (`cantus-engine/tests/external_units.rs`).
- Notes out: a unit's track is listened to through its outbox; notes leave stamped with their
  sample, route and channel through the MIDI-out thread of ADR 0079 (`Stamped::note`,
  `OutKind::Note`, `MidiOut::set_unit_routes`); the outboard track's own instrument is silent.
- The ping (`cantus_engine::external_ping`): audio burst train out of a pair, correlation on the
  return, per-burst median and jitter, polarity; onset ping for instruments (a note, four times,
  the first sound above the noise). Run inside the processor (`StartPing`/`PingDone`,
  `EngineHandle::start_ping/take_pings`), evaluated off the audio thread, stored with
  `SetExternalLatency`. Proved **to the sample** on a virtual wire at block sizes 32…1024, with gain,
  inverted polarity, noise, wrong channel, silence, hum, no input, a dropped block and a jittering
  return each said, and through the real `Processor` with its input ring and its MIDI-out ring.
- `rt_memory`: `external_notes_and_the_ping_never_allocate`.
- Interface: Settings ▸ MIDI ▸ *External gear* (add instrument/effect for the selected track, routing
  steppers, state, *Ping*/*Measure again*, trim, return level, on, remove), `external_service.rs`,
  `views/external_bands.rs`, the agents' tools `add_external_unit`, `ping_external_unit`,
  `list_external_units`, `list_surfaces` (`ai_tools_external.rs`).

**Mackie Control and HUI (ADR 0095).**
- `external_mcu.rs`, `external_hui.rs` (byte-exact codecs), `external_surface.rs` (the state machine:
  banks, extenders, flip, name/value, shift; motor faders in the console scale, never against a
  hand), `external_emulation.rs` (`VirtualMcu`, `VirtualHui`). `tests/external_surface.rs`: 18 tests,
  every message of both protocols as literal bytes, the handshake accepted by the unit, the whole
  surface through the virtual units, only-what-changed.
- Interface: `surface/` (view, apply as ordinary commands with a gesture per touch, service), the
  surface's port taken whole in the controller filter (`FilterSet::take_port`), Settings ▸ MIDI ▸
  *Control surfaces* (kind, ports, virtual unit, log), `settings.surfaces`.

## The integrator review (12 items), fixed - each with a test that fails without the fix

1. **Notes reach the synth in the app.** Routes are `route_for(unit, port)` (the unit id and a
   hash of its port); `external_service::poll` registers every enabled instrument route, port
   and ordinal with the MIDI-out thread that `assemble` started (`SyncService::set_unit_routes`,
   sent again after an audio restart); a ping registers its own unit route, also for a unit that
   is off. Tests: `cantus-ui/tests/external_boot.rs` (through `CantusApp::assemble` and the running
   thread), `sync_midi.rs` (a note reaches the registered port; the second port of a name by its
   ordinal).
2. **No leaks, no gear in renders.** Sends and the audio ping write only a pair inside the device
   channels; the handle drops the links of units that are not available (`external_presence`);
   freeze (`CannotFreeze::External` in `freeze_commands`, marked), bounce and export are refused
   with what to do (ADR 0094 section 6 argues against a real-time bounce inside the export).
   Tests: `review::*` in the engine, `a_track_with_outboard_gear_cannot_be_frozen...`,
   `export_and_bounce_say_why...`.
3. **Re-based trips.** `Measured` keeps `rtt`, `buffer`, `device`; the same interface is
   re-based, another one says "measure again". Tests in core and engine.
4. **No stuck notes.** Held keys per slot; note-offs and CC 123 to the old route and channel on
   removal, off, channel or port change; retired routes on the out thread; 256 places kept for
   note-offs and a queue for the rest. Tests: `stuck::*`, `external_out` tests, the out-thread test.
5. **MCU 7-segment** per TouchMCU (0x40 the rightmost timecode digit ... 0x49; assignment
   0x4A/0x4B).
6-8. **HUI** strip ports, zones 08/0a/0d/0e/0f, the V-pot and jog encoding and the ping, each from
   the theageman notes (URL and pages in ADR 0095 section 4 and in `external_hui.rs`); the tests
   quote the bytes the notes print (the solo example of page 9 and the table of page 14).
9. **Keep-alive:** a HUI is pinged every second, a loss after 3 s is said in the log, it is
   refreshed on return; lit MCU meters are sent again every 150 ms.
10. **SysEx** from a surface port reaches the surface (`external_sysex`,
    `MidiInputs::drain_sysex`); the host connection query is answered (core, engine, ui tests).
11. **Ping:** a 15 s timeout, given up on a device change or an audio restart, aborted by the
    engine when the transport starts (its note released).
12. The track menu has External instrument/effect; removal restores the track input
    (`restore_input`); the onset confidence is the notes level over the noise divided by one plus
    the spread; `rt_memory` covers the effect send. Faders without touch sensing: an option
    (motors held 0.6 s), said on the page.

## Second fix round (after the five merges)

- One choke point for offline renders: the processor offline ignores external links (notes,
  sends, latency), and the handle strips them from render compiles; engine tests `offline::*`
  (a snapshot with links rendered offline is the bits of the project without units).
- Video export and modulator bake refuse while a unit plays; listen renders (AI, mastering
  assistant) render without the gear and say so (`outboard_not_heard`). UI test
  `every_user_facing_offline_render_refuses...`.
- Record mode: record adds `Why::External` (units stay live); its bakes run offline, so they
  could not include the gear in any case.

## Third review (integrated tree)

- One undo step per surface gesture: every fader and V-pot moved from the first touch to the last
  release (simultaneous faders included) is one step, widened as targets join; only that step is
  sealed (never the mouse drag); moves without touch end after 600 ms still. The recording mode
  lock is asked before each move (`locked_against`): a refused target is skipped quietly, the
  others keep moving. Tests: `two_faders_ridden_at_once_are_one_undo_step`,
  `a_locked_target_is_skipped_quietly_and_the_others_keep_moving`.
- The external stream in the shared undo fuzz (`undo_invariants.rs`, `urng`, its own counters and
  assert). It found that a unit removal could not be undone after its track was removed: a unit of
  a gone track is now inert, not refused (`a_units_removal_after_its_track_is_gone_undoes_exactly`).
- Fixture: the units sit on tracks of their own outside every frozen region; v51-v53 regenerated;
  `every_fixture_is_consistent_and_no_frozen_region_holds_outboard_gear`.
- No brand names: the gear in fixtures, tests and shots is "Hardware Synth" / "Bass synth" on a
  "Studio Interface"; the shots re-taken.

## Checks (after the review and the merge of r36/integration with r36/mix)

Windows: clippy `-D warnings --all-targets` on core, engine, project and ui clean; fmt clean;
`cargo test -p cantus-core -p cantus-project -p cantus-engine` 1 902 passed, 0 failed; cantus-ui
(`--lib` and 15 binaries: external_ui, external_boot, ai_mcp_boot/ui, agent_tools, controllers x4,
freeze, freeze_boot, export_dialog, export_files, mixer_vca, mixer_desk) 698 passed, 0 failed.
Linux (WSL, `-D warnings`): workspace clippy and plug-ins-without-editor clean; core, project,
engine tests and the ui external_ui, external_boot, ai_mcp_boot, controllers_review2 tests pass.
`tools/list` golden regenerated after the merge. Fixture v51 regenerated over mix's v50.

## Marked lines (`// r36:external`)

`cantus-core`: `lib.rs` (modules, re-export), `model.rs` (`Project::external_units`),
`command.rs` (4 variants, labels, 4 apply arms), `freeze_guard.rs` (3 arms), `snapshot.rs`
(`TrackSnapshot::external`). `cantus-engine`: `lib.rs`, `loopback.rs` (3 items `pub(crate)`),
`sync.rs` (`OutKind::Note`, `Stamped::route/note`), `sync_out.rs` (routes, `NOTE_PORT`),
`midi.rs` (`FilterSet::take_port`), `messages.rs` (`StartPing`, `PingDone`), `backend.rs` (pings,
`start_ping`, `take_pings`), `processor.rs` (module, field, 2 calls, `fire`), `processor_msg.rs`
(1 arm), `processor_pass.rs` (1 call), `processor_snap.rs` (`external`, `ext_plan`, `ext_listen`),
`processor_sync.rs` (`push_ext`), `processor_rec.rs` (`channel` `pub(crate)`),
`listen_fork.rs` (3 lines), `graph/track.rs` (`external`, `outboard`, 2 early returns, `latency`),
`graph/notes.rs` (`drain_from`). `cantus-project`: `format.rs` (`SCHEMA_VERSION` 51,
`migrate_49_to_50` — a stub for mix's schema 50 that the integrator drops for mix's own —,
`migrate_50_to_51`), `write_fixture.rs`. `cantus-ui`: `lib.rs`, `app.rs` (2 fields), `settings.rs`
(`surfaces`), `controllers_input.rs` (3 places), `views/workspace_settings.rs` (module + call),
`ai_tools.rs` (module + registry).

## Merge notes for the integrator

- `tools/list` golden (`crates/cantus-ui/tests/golden/mcp-tools-list.json`): regenerate after
  merging (`CANTUS_TOOLS_DUMP=1 cargo test -p cantus-ui --test ai_mcp_boot`).
- Schema: 51 over mix's 50 (merged).
- `Command` variants are added beside `SetGroove` (conflicts with mix/record/editor are additive).

## Not done, and why

Nothing of the brief or the review is left that does not need the owner hardware.

## Owner-only (hardware)

Listed in `docs/beta-checklist.md` (section 1, *Equipo externo y superficies*):
a real synth's ping and its alignment; an effect through a real loopback cable against the
settings page's round trip; an unplugged interface; a Mackie Control (or X-Touch/Icon/FaderPort in
MCU mode) with its motors, touch and displays; a HUI (its zone/port table is from the published theageman notes, not yet held against a real
unit); the handshake on a real MCU. No ASIO driver of an unplugged
interface is ever loaded by any of this.
