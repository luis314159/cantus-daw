# ADR 0062 — MIDI learn and controller mapping

**Status:** accepted (round 29) · **Date:** 2026-10-19 · **Extends:** ADR 0004 (commands),
ADR 0005 (real time: nothing new on the audio thread allocates or locks), ADR 0027
(automation targets and recording), ADR 0052 (the top bar's MIDI lamp), ADR 0057
(modulators, hosted parameters, recording from a plug-in's window), ADR 0060 §2 (the tempo
a controller sets)

## Context

The owner's ask (2026-10-16, item 2): *any parameter (native, hosted plug-in, mixer,
transport) to a knob, fader, button or pad; absolute / relative / toggle, range and curve,
soft takeover; maps saved per project and globally; a view of the mappings; a move records
automation while recording.* BACKLOG §0.2 (*Hardware integration*) puts it as: a generic
controller maps in two clicks.

Before this round: MIDI input fed one ring to the audio thread from every port (omni, port
identity and timestamps dropped); channel pressure, key pressure and program change were
parsed and dropped; nothing in the interface heard MIDI at all (the top bar's lamp read a
counter); there was no learn code. References: FL Studio (*Link to controller*,
*Multilink*, *Last tweaked*, takeover), Ableton (MIDI Map mode, min/max, the mapping
browser), Bitwig (learn, ranges, relative encoders), Reaper (relative modes, soft takeover).

## Decisions

### 1. The input: a tap beside the engine's ring, a lock-free filter before it

`cantus_engine::midi`: each port's midir callback runs `route(event, port, tap, engine,
ui_ring)`:

- a lock-free **filter** (`MidiFilter`: atomics, one bit per port × channel × address —
  128 controllers, 128 notes, 128 key pressures, 128 programs, the wheel, channel pressure;
  32 ports) says whether the controller map takes the message. **A taken message is
  consumed**: it goes to the interface only — a knob mapped to a filter no longer bends
  the instrument, a mapped pad no longer plays a note. While a control is being learnt,
  every message is taken (the knob the user turns to teach Cantus does not play
  meanwhile).
- what is taken is pushed to a **ring of its own port** (`rtrb`, single producer — the
  callback — single consumer — the interface; no lock, no allocation) as `Heard { port,
  event, at: Instant }`, and **wakes the interface** once until it drains (an atomic flag
  and egui's repaint, set at boot).
- what is not taken goes to the engine's ring as before. The top bar's lamp counts both.

Port identity is **name plus ordinal** (`PortRef`: the second "nanoKONTROL2" is ordinal
1), never the index: a mapping survives a device plugged in before it, a rescan, an audio
restart. The interface republishes the filter when the project's revision, the global map
or the ports change (dormant mappings take nothing).

The tap's shared half (`MidiTap`) is made in `boot.rs` (`Stage::Audio`, with the window's
wake), the ports are opened with it (`MidiInputs::open_with`), `CantusApp::assemble` keeps
it (or makes one when there are no ports), and `restart_audio` / `rescan_midi` reopen the
ports with the same tap. Tests: `controllers_boot.rs`.

What was dropped is routed (`processor_msg.rs`): channel pressure → `Controller::Pressure`;
key pressure → the live key's note as `NoteExpression::Pressure`; program change → a new
`InstrumentUnit::program` (default nothing; a hosted instrument hears it as MIDI — CLAP
directly, VST3 through `IMidiMapping`'s `kCtrlProgramChange`, as `program / steps` of its
program-list parameter, which stays visible). No allocation (`rt_memory`
`live_pressure_and_program_changes_never_allocate`).

**Not chosen:** an `EngineMessage::SetMidiFilter` checked on the audio thread (makes
`processor_msg.rs` hot and puts map logic in real time); one shared tap ring behind a
mutex (the callbacks would contend; per-port SPSC rings need none).

### 2. The mapping: a target, a source, a mode, a range, a bend, a takeover

`cantus_core::controller` (MPL):

- **`MappingTarget`**: `Param(AutoTarget)` — every automatable parameter, hosted ones
  included, by the same stable address automation uses; `TrackMute/Solo/Arm`,
  `StripMute/Solo`, `EffectBypass`, `EffectMix` (the switches that have no `AutoTarget`);
  `Tempo` (ADR 0060 §2's rule: the song's tempo, or the governing point's, at the
  playhead, over a 40–240 BPM travel); `Action(id)` — any keymap action by its stable id,
  run through `run_action` (play, stop, record, loop, metronome, tap tempo, previous/next
  marker, PAT/SONG…); and the **relative** targets a global map needs: `SelectedTrack(…)`,
  `SelectedStrip(…)` (volume, pan, mute, solo, arm), `DeviceParam(0…7)` (the focused
  device's first eight parameters — the device window clicked last, else the open effect
  editor, else the selected track's instrument; continuous parameters first, then stepped,
  the mix last).
- **`AutoTarget::command(project, value)`** (`automation/set.rs`) is the inverse of
  `value_of_command`, round-trip tested over every kind of target
  (`tests/controller_targets.rs`); `MappingTarget::command(project, pos, at)` builds on
  it. A hosted parameter's value goes through the plug-in's range in the interface (the
  project does not know it).
- **`MidiSource`**: port, channel (or any), `Control` — `Cc`, `Cc14` (coarse `n`, fine
  `n + 32`), `Note` (velocity, or pressed/released), `PitchBend`, `Program(n)` (a button),
  `ChannelPressure`, `PolyPressure(n)`.
- **`MapMode`**: `Absolute`, `Relative(TwosComplement | BinaryOffset | SignBit)`,
  `Toggle`, `Momentary`, `Trigger` (an action fires on the press edge, once).
- **Range and bend** in **position space** (0…1 of the target's own travel, ADR 0027's
  `AutoRange`), so a fader's taper, a log frequency and a stepped switch come with the
  target: `pos = min + (max − min) · Shape::Curve.at(curve, x)`; `max < min` inverts.
  `unshape` (bisection, deterministic) inverts it for relative steps and feedback.
- **`Takeover`**: `Jump`; `Pickup` (default — FL's and Ableton's: nothing moves until the
  control reaches the value or passes it between two messages); `Scale` (Reaper's and
  Ableton's value scaling: the target covers what is left of its way in the share of what
  is left of the control's, converging at the end). The current value is `base_pos`
  (automation where it plays, else the project's; a latched parameter's own); a target
  moved by anything else since the mapping last set it is taken over again.
- **Learning** (`controller::learn::detect`): a key, pad or program change is learnt on its
  first press; a knob after a 300 ms window (or eight messages) that tells a 14-bit pair
  (its fine twin came), an endless encoder (the same small value repeated, in one
  encoding's band — an absolute knob never repeats while it moves) and a two-valued button.
  The plain reading (absolute) wins where the hardware does not make it clear; the pane
  changes it.

**One control on two targets is allowed** (FL's multilink; Bitwig allows it): learning a
control already mapped adds a mapping, the status line names what else it drives, and the
pane flags both rows *Shared*.

### 3. Applying a move: commands, one undo step per gesture, recording, the latch

Mapping stays entirely on the interface thread: a move becomes the same command a mouse
drag sends, at frame rate, through a new snapshot — no new real-time mechanism (§5 of the
map: a `SetParam` fast path was weighed and not needed).

- **One command per target per frame** (the newest value of a burst), merged under a key
  made of the gesture's targets; a gesture ends after **500 ms** without a message
  (`end_gesture`). Two knobs turned together are **one** batch (`cmd.controllers.gesture`):
  when a new target joins, the step so far is sealed and a wider one begins (a batch of
  another shape would compose and grow). Test: `two_knobs_turned_together_are_one_step…`.
- **Recording**: a mapped move while a pass records automation is written into the pass at
  **the message's own time** — the playhead less the message's age, through the tempo map
  (`tick_of_message`) — via `record_tweak_at`; the pass ends as one automation clip in one
  undo step, as a mouse's moves do.
- **A parameter under an automation clip** follows the clip in playback (FL's rule; the
  mouse's too) — **unless a pass records it**: from its first move in the pass it is
  **latched** (Pro Tools' and Cubase's *latch*, Ableton's override while recording): its
  lane in the snapshot holds the project's value, i.e. the controller's newest, until the
  pass ends and the recorded clip takes over. Ableton's permanent override (automation
  disabled until re-enabled) was not taken: it leaves a project silently not playing its
  clips.
- **Feedback** (per mapping, off by default): the target's value goes back on the mapped
  control whenever it changes by anything — 7 or 14 bits, a note's velocity for a pad's
  light, the wheel for a Mackie-style motor fader — on the output port of the same device
  (the same name, or Windows' `MIDIIN2 (X)` → `MIDIOUT2 (X)`, or *In* → *Out*), opened on
  demand; never while the mapping's own control is moving (250 ms), so a motor fader does
  not fight the hand.

### 4. Where maps live: per project and global, the project first

- **Per project**: `Project::controllers` (**schema 35**, skipped when empty: a project
  without mappings saves the same bytes). Commands `InsertMapping { index, mapping }`,
  `RemoveMapping { id }`, `SetMapping { mapping }` (merged per mapping while a pane cell is
  dragged), validated (channel, numbers, range, bend, sensitivity, ids), exact inverses;
  undo-fuzz arm 45 with its counter. A mapping whose target vanished is **kept dormant**:
  shown *Missing*, consuming nothing, alive again when undo brings the target back.
- **Global**: `controllers.json` beside `keymap.toml`, read in the boot's Files stage;
  only **portable** targets (the transport's actions, the tempo, the master,
  `SelectedTrack`, `SelectedStrip`, `DeviceParam`); a malformed or non-portable entry is
  dropped on read, never the whole file. Settings, not commands (as the keymap).
- **Learnt mappings go** to the global map when their target is portable (play/stop then
  works in every song), to the project otherwise.
- **Precedence**: a message a live project mapping takes is not looked up in the global
  map.

### 5. The interface

- `kit/automate.rs`: `Choice::Learn` / `Choice::Forget`; `ModOffer::learn: LearnOffer
  { mapped, mode, armed }` (set by `CantusApp::mod_offer`, so every control that offers
  *Modulate…* offers learn — the probe test `learn_everywhere.rs` fails on one that does
  not); *Learn MIDI controller* / *Change MIDI controller* / *Forget MIDI controller* in
  the menu; the mapped mark (a dot at the lower right); `learn_spot` — in learn mode an
  outline on a sublayer above the control, a cover that takes the click (the value does
  not move) and answers `Choice::Learn`. Buttons that are not parameters (mute, solo, arm
  lamps; the top bar's transport keys, PAT/SONG, metronome, tap, the tempo well) call it
  themselves. Chorda's tuning keys (never automated) are not learnt.
- The mixer's strip, master and insert menus and the track header's menu: *Learn* rows
  (checked when mapped) and *Forget these controllers*.
- Keymap 9 (`NEW_IN_V9`): **`MidiLearnMode`** `Ctrl+Shift+M` (Live's Cmd+M is our
  metronome), **`LinkLastTweaked`** `Alt+Shift+A` (beside `Alt+A`'s automation clip — the
  way into a hosted plug-in's own window), **`ViewControllers`** `Ctrl+Shift+K`. The top
  bar's *MIDI* cell is Live's *MIDI* key: a click turns the learn mode on and off (the cell
  lit while on), a right-click opens the MIDI settings — no extra width (the default bar
  still shows every group at 1920 px in Spanish, `topbar_layout`).
- **`Pane::Controllers`** (lower zone, View menu): the header (learn, *Add a mapping* — every
  portable target, armed on a click —, filter, the takeover new mappings get, the ports),
  two sections (*Everywhere*, *This project*), a row per mapping: activity dot, control,
  target, mode, from/to in the target's units (drag, double-click to type, right-click to
  reset), curve, takeover, feedback, learn again, remove; *Shared* and *Missing* flags;
  both ways scrollable; a two-row header when narrow.

## Consequences

- The audio thread is unchanged but for three routing arms (no allocation, tested); the
  MIDI callbacks gained a filter read and an SPSC push (lock-free).
- A project without mappings renders and saves exactly as before (no golden moved; the
  field is skipped when empty).
- Every round from now on that adds a parameter control gets learn for free through
  `mod_offer`; a control drawn another way fails `learn_everywhere.rs`.
- Not built here (BACKLOG §0.2): Mackie Control / HUI and controller scripts (a surface's
  protocol, banks and displays) — the mapping layer is what they will build on.

## What the tests hold

Core (`controller_tests.rs`, `tests/controller_targets.rs`, `undo_invariants` arm 45): the
encodings; shape/unshape inverse with range, bend, inversion; jump, pick up (never jumps,
crossing between messages, taken over again after an outside move), scale (converges at the
end); relative steps within the range and sensitivity; toggle once per press; momentary;
an action fires once per press; a pad's velocity; feedback values; learning (knob, 14-bit,
three encoders, button, pad); ports by name and ordinal; overlap; relative targets; switch
and tempo commands; the commands' inverses and refusals; serialization skipped when empty;
`command(value_of_command(c)) == c` and `base_value(apply(command(t, v))) == v` over every
target. Project: `schema_35_keeps_the_controller_mappings`. Engine: the filter per port and
channel, `route` tees and wakes once, the output's name; a mapped CC and pad never reach
the instrument (through a real `Processor`), pressure and program change do; `rt_memory`.
Host: a program change's normalised value. Interface (`controllers.rs`,
`learn_everywhere.rs`, `controllers_boot.rs`): learn from the menu, learn again, forget;
what each control is learnt as; a transport key to the global file; the global file's
lenient read; pick up and one undo step per gesture; two knobs one batch; encoder, pad,
action; the project first, dormant mappings; the filter follows maps, undo and ports;
recording one clip at the right ticks; a message's tick through the map; the latch; feedback
once per change; the keymap's actions and a v8 file; learn mode on a kit knob (a click
learns, a drag moves nothing); the pane's words, flags, filter and keys; learn offered on
every control of every native editor, the inspector's rows and sends; Chorda's tuning keys
refused; the boot path.
