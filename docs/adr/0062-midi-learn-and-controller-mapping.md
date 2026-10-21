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

## Amended after review 1 (round 29)

An adversarial review confirmed 57 findings; the decisions above stand, with these changes
(the fixes, one per finding, are tabled in `docs/round29/controllers.md`):

- **Waking without a lock.** The MIDI callback no longer calls egui's `request_repaint`
  (which takes a lock): each tap owns a parked *waker* thread; the callback sets an atomic
  flag and unparks it, and the thread wakes the window. The tap's drop stops the thread.
- **Releases always reach the engine.** Each port keeps a `PortState` (notes held per
  channel, the sustain pedal): a note-off or pedal-up for something the engine holds goes to
  the engine even while learning or when a new mapping now takes that control — no stuck
  notes.
- **Reading a message is the mapping's job** (`Mapping::read` over `DriveState`): a button
  without a release (a program change, a CC that only sends 127) is a new press every
  time; an encoder whose value is a jump in the learnt encoding is read in the encoding
  that makes it a step; a relative control keeps its unrounded position, so an encoder
  walks a stepped target and flips a switch; a button control (a pad) has no takeover.
  Learning a CC button that sent one value (no release) makes it absolute, not a toggle.
- **The tempo target** reads and writes the governing point (`tempo::edit::point_at`), not
  the ramp's interpolated tempo; a gesture's target is the target *and* that point.
- **A gesture** keeps each target's last command and re-sends only what moved this frame;
  it is sealed by the messages' own time (`CommandBus::end_gesture_if` — only its own
  step), and the window is woken at its end (`workspace_ticker::wake_after`), also for the
  learn window, a 15-second learn timeout and feedback's quiet. A target joining a running
  gesture widens the same step (`CommandBus::widen_merging`); `SetTrackArmed` and the new
  `SetModulatorAmount` supersede per target.
- **`Command::SetModulatorAmount`** replaces the modulator's amount alone (exact inverse,
  fuzz arm 45, `freeze_guard` → the modulator's routes); a modulator amount is read back
  like every other target, so it records. A controller records its own target and position
  at the message's tick (`record_point_at`), the loop's wrap respected.
- **A hosted mapping remembers its plug-in** (`Mapping::plugin`: format and id, schema 35
  field, skipped when empty): under another plug-in, or while its parameter is not loaded,
  it is dormant (`Mapping::alive`, `mapping_alive`) — it takes no MIDI and looks missing.
- **VST3 program change** goes to the parameter flagged `kIsProgramChange` (the root
  unit's first), not to IMidiMapping controller 130; the change is reported as the
  plug-in's own parameter change, so the project keeps it.
- **14-bit pairs**: the coarse half alone is scaled to the full range until a fine half is
  seen; then the pair moves on its fine half only (no half-made values).
- **Feedback** is one channel message per send (a 14-bit pair is two), recorded as sent only
  when every send went; quiet while the control moves; feedback heard back on the input
  within 150 ms (a loopback) is dropped; a global mapping shadowed by the project sends
  nothing; transport actions feed back their state (playing, recording, loop, metronome,
  pattern mode) and actions without a state offer no *Send*.
- **Files**: a project's map is repaired on read (`ControllerMap::repair`: invalid or
  duplicate ids dropped, the limit kept); the global file is written through a file beside
  it, synced and renamed; a damaged one is kept as `.json.bad` and said in the status bar.
- **The interface**: learn spots outline and take clicks only inside their view's clip;
  every control the review found unlearnable (the channel rack's knobs and lamp, the route
  menu's send level, stepped device controls, desk modules) offers learn and shows the
  mapped mark; the clip menu has *Link to controller*; *Add a mapping* offers every keymap
  action by its group; forgetting several controllers is one step.

## Amended after review 2 (round 29)

A second review confirmed 55 findings (the fixes are tabled in `docs/round29/controllers.md`).
What changed in the decisions:

- **A gesture follows the history, and its key is its own.** A gesture remembers the
  project's revision after its own step; when anything else changed the history under it
  (an undo, a redo, a mouse edit, a load), its step is sealed and only what moves next is
  sent — a target's stored command never overwrites what was done since. Each gesture's
  undo key carries a serial, and `CommandBus::end_gesture_if` seals the key's step wherever
  it is in the history (buried under another step, an undo bringing it back to the top
  never merges a later turn into it).
- **Recording stays inside the pass.** A point is written between the pass's start (the
  record start, the loop's start for later passes, the punch-in) and the punch-out: a move
  during the count-in or a moment before tick 0 is where the pass begins, never before the
  song (a point there failed the whole pass's commit). A message heard before the loop's
  start lands at its end only when a wrap was seen since it came.
- **Learning** waits for a held controller button's release (`HOLD_WINDOW`, 1.5 s): pressed
  and released it is a toggle, never released a latching button (absolute). A program
  change after bank select is learnt as the program change, at once. Feedback's echo is
  never learnt. Learning again a hosted parameter makes the mapping the plug-in's there
  now; a mapping learnt on a swapped-out plug-in is left for an undo of the swap and is not
  what the mark, learning or *Forget* act on (`ControllerMap::of_target_in`,
  `Mapping::fits`, allocation-free).
- **Feedback's echo** is judged by the message's own arrival time, within 30 ms of the
  send — a loopback answers in a millisecond, no hand presses again so soon; the list of
  what was sent is pruned and bounded. Every toggle the Add menu offers that has a state
  feeds it back (punch, count-in, low latency, the tempo track, the typing keyboard, the
  meters' wave, the selected track's arm, the selected strip's mute and solo).
- **14-bit pairs**: a coarse half alone, after the fine half has been heard, is read with a
  fine 0 at the end of the burst (MIDI's rule); a controller that sends its fine half first
  is recognised and read whole.
- **Scale** takeover keeps its own unrounded position (as an encoder does), so it moves a
  stepped target; a switch target takes no takeover.
- **Frozen sounds.** A target a frozen sound refuses (`freeze::refuses`, read off the
  project without applying) is left out of the gesture — the gesture's other targets keep
  moving — and the refusal is said once, in words. A hosted mapping whose plug-in rests
  because its sound is frozen stays the map's (its control reaches no other instrument).
  The pane tells why a row does nothing: *Missing* (the target is gone, the danger colour),
  *Other plug-in*, *Not loaded*, *Frozen* (dim).
- **The tap** (engine): the rearm is a swap; learning is published with release/acquire;
  a pedal released while its messages are taken reaches the engine fully up, sostenuto
  is kept like the sustain and reset-all-controllers lets both go; a full ring keeps the
  newest value of each address aside (a stalled interface still gets a fader's last value
  and a pad's release).
- **VST3 program change** is the state change it is: no parameter step besides it — the
  next idle says the plug-in's state changed, and the project records one state step.
- **The global file** is kept beside (`.json.bad`, an earlier one kept with the time in its
  name) whenever anything in it is lost: not text, unreadable, from a newer Cantus, a
  mapping this build cannot read.
- **Learn everywhere**: every stepped parameter drawn by a plain control — a segmented
  choice, an LED, a menu field, a stepper, a card's lamp or selector, a row of value
  buttons, a drawbar, the scale keyboard — goes through its parameter's offer (menu, mark,
  learn mode): `controllers::stepped_param` / `stepped_inst` for instruments,
  `fx::controls::stepped_offer` and its keyed variants for effects; a test pins every
  stepped key the editors draw. A modulator's amount moved with the mouse sends
  `SetModulatorAmount`, so it records and is *last tweaked* as the controller's does.
- **An idle frame allocates nothing** with hosted mappings (a hosted parameter is looked
  up, not copied) and with no feedback mapping.

## Amended after review 3 (round 29)

A third review confirmed 60 findings (tabled in `docs/round29/controllers.md`). What
changed in the decisions:

- **A target's steps are part of driving it.** The interface computes each target's shape
  (`controller::Steps`: a switch when it is stepped with two states — `max − min == 1` —
  and its number of steps when it is a choice) and passes it to `Mapping::read_in`,
  `drive_in` and `controller::detect_in`. A two-state parameter is a switch everywhere a
  switch target is: no takeover for a held or latching button, one relative detent sets
  it, a pad learnt on it toggles, the pane reads and types it *Off* / *On*. A relative
  encoder on a choice moves one choice per detent (the sensitivity divided by the steps,
  not by 128).
- **Choices are shown and typed by name.** The From and To cells of a choice say its
  names, the words its editor draws: an effect's from the same table the editor uses
  (`controllers_words`, in the application — never in the MPL core), an instrument's from
  what its editor drew (noted once per kind and key, cleared on a language change), a
  hosted plug-in's from its own text for each step (64 steps at most); a name typed is
  read back to its step.
- **14-bit halves wait for each other, a moment.** A half whose partner is expected is
  held, never driven: a coarse half after a fine half has been heard waits up to
  `PAIR_GAP` (5 ms), across frames (the controllers' wake asks for the frame), and only
  then is read alone with a fine 0; a fine-first controller's fine half waits the same
  way. Fine-first is recognised only after `FINE_FIRST` (3) fine-then-coarse pairs in a
  row, and a coarse-then-fine pair clears it. Halves left waiting are resolved before a
  burst's own messages, so a burst's halves are new pairs.
- **The tap's overflow keeps order and one slot per address**: pitch bend and channel
  pressure have one slot per channel (not one per value); while anything is parked, newer
  messages park too, and the take hands them over by arrival with their own times.
- **Pedals**: sustain, sostenuto and soft are tracked whenever above 0 (a half-pedal too)
  and their release is never swallowed; reset-all-controllers (CC 121) no longer counts as
  a release (the native instruments ignore it).
- **Feedback's echo** is stamped before the send, with 2 ms of skew allowed, so a loopback
  answering before `send` returns is still the echo.
- **VST3 program change**: the state change is said only for a MIDI program change heard
  from an input, not for the host's own writes of the program parameter (automation, an
  LFO, a mapped knob).
- **Recording's bounds**: the pass is bounded by its record start, the loop's start only
  once a wrap was seen in this pass, and the punch; the clip ends at the punch-out and the
  latch lets go there; a pass stopped before its start writes nothing. While audio records,
  a tempo knob does nothing (ADR 0060 §4: the status says so; the tempo is changed before
  or after the take), and a gesture that turned the tempo before the take sends no tempo
  command in it.
- **A new project starts the controllers again**: an armed learn is cancelled and the
  controllers' states, sends and gesture are cleared when the project is replaced.
- **The global file** is read as whole, kept (the copy beside named in the status), or
  unsafe (no copy could be made: the session does not save over it); an identical copy is
  never made twice.
- **Ports** are named without ALSA's `client:port` numbers, so a mapping matches after a
  replug; a mapping whose port is not open reads *Not connected*.
- **NRPN and RPN** are not learnt: the learn says so instead of mapping a plain CC 99.
- **The selected strip** may be the master; **a strip's effects switch and record arm**
  are targets (`MappingTarget::StripFx`, `StripArm`; schema 40 — numbered 39 on the branch, renumbered at the merge after warp's 39), learnable on the strip
  and from its menu.
- **The pane** says *Frozen* for any target a frozen sound refuses (native ones too) and
  says the refusal again after a pause; *Other plug-in* rows are named after the plug-in
  they were learnt on and are not counted as shared; an unknown action is *Missing*; a
  crashed plug-in's mappings are *Not loaded*. The learn mode, the panes' show/hide, the
  instrument window and the takes feed their state back.
- **An insert swapped** for another keeps its controllers: bypass and mix follow the new
  slot, a native parameter with the same key follows, a hosted one stays its plug-in's.
- **Offers cost nothing idle**: with the learn mode off, nothing mapped and no menu open a
  stepped control's offer builds no target, and a card's lamp hands its response back
  instead of keeping it in egui's memory (`frame_budget.rs` holds a Compressor and a
  Wavetable editor's warm frame).
- The Tonus keyboard outlines and covers each key only where it shows; a typed curve is
  kept as typed; a hosted value's words are asked again until the plug-in answers, per
  effect.

The tests of review 3: core `controller_tests.rs` (switches, a pad on two states, a detent
per choice, port names, sharing, the master as the selected strip, the strip switches),
`undo_invariants` (the strip switches as targets); project
`schema_40_keeps_the_strip_switch_mappings`; engine
`a_full_ring_keeps_order_and_the_last_bend_and_pressure` and
`a_pedal_let_go_while_taken_reaches_the_engine_fully_up`; host
`a_program_change_reaches_the_program_list_and_the_project`; interface
`controllers_review3.rs`, `learn_everywhere.rs` (every stepped key of every editor, offered
or said why not drawn), `frame_budget.rs` (`an_editors_idle_stepped_controls_cost_no_allocation`).

## Amended after review 4 (round 29)

A fourth review confirmed 64 findings (tabled in `docs/round29/controllers.md`). What
changed in the decisions:

- **Names before Off and On.** A target's choice names are looked up first, whatever its
  number of steps — two named choices read and are typed by their names (a De-esser's
  mode, a Saturator's phase, a modulation effect's sync). Off and On are the words of two
  states only when they have no names: a mute, an LED, a unitless 0…1. A two-step number
  (1…2 oct) keeps its number and unit.
- **A hosted plug-in's own steps.** `ParamInfo` carries the plug-in's step count (VST3
  `stepCount`, CLAP `max − min`): a VST3's plain values need not be step numbers (JUCE's
  and the SDK's base parameter give a four-way choice as 0…1; another plug-in 0, 10…40).
  A target's shape, its words (asked at each step, the pane asking one step only) and the
  values a controller and automation send (`ParamInfo::snap`, `snap_to_steps` in the
  host's parameter sync) are counted in those steps, never in whole plain units.
- **An encoder steps in the target's own steps inside the mapping's range**: one choice a
  detent however *From*, *To* and the bend narrow or bend it, up to 128 steps
  (`MAX_DETENT_STEPS`); a stepped number with more (a seed, a plug-in's milliseconds)
  crosses its travel in 128 detents, as a range does.
- **The instruments' choices** come from a static table (`cantus_ui::inst_choices`), the
  editors' labels by instrument and field — never from what an editor drew this session.
  A test paints every editor and holds what it draws to the table; another holds the
  effects' table (`controllers_words`) to the exported plug-ins' (`cantus_plugins::params`).
- **14-bit halves are told by time.** A coarse half held after a fine half alone takes as
  its own the closer fine half — the one before it (a fine-first pair; the next fine half
  then belongs to the next pair) or the one after it (a coarse-first pair after a
  fine-only update). Pairs a millisecond or two apart are read whole, a known fine-first
  controller stays known at speed, and the last pair is read whole. A half left waiting
  is judged against the burst's earliest message (bursts come port by port).
- **NRPN and RPN** — data entry included (CC 6/38, 96/97), which many controllers send
  alone — are passed over while learning, said, and the learn stays armed for the next
  control moved.
- **Recording.** A false start is judged against the start of the pass that ran (a loop's
  later lap stopped before the record start keeps its automation). A loop's wrap is seen
  once a frame, before anything heard in it is recorded, by the recorder's rule (back by
  more than half the loop, playing): a stale position from before a count-in's seek is no
  wrap. The latch is sent to the engine whenever it turns on or off — the pass beginning,
  the punch-out crossed — so a parameter follows its clip again past the punch-out.
- **One value of a choice.** A control that stands for one value of a choice is learnt
  over it: a band's solo LED over none ↔ that band (its mark shows on that LED only), a
  scale key over as the scale ↔ its click, a lamp lit at 0 (Organum's rotary) the lamp's
  way. The learnt mapping is read as two states.
- **A strip's record arm** is its tracks' arms in a gesture (the newest press of a strip's
  or a track's arm wins); with no track to arm it reads *No input* and takes nothing.
- **Learn everywhere, the inspector too**: an insert tile's lamp (bypass) and ring (mix),
  the inserts' switch, the mini view's mix, the arm LED, the slot menu's rows, and a device
  header's bypass lamp.
- **Names that last.** A hosted mapping keeps the plug-in's and the parameter's names it
  was learnt with (schema 41); a row whose plug-in was swapped out — for another plug-in
  or one of Cantus's own — reads *Other plug-in* by those names, its range in percent,
  sharing nothing, and the filter finds it by them.
- **Smaller things**: an unknown action takes nothing; feedback is stamped after its
  output opened; the master's switch is named as the master; a switch shows no takeover;
  idle rows are all dimmed; global edits a session cannot keep are said each time; a new
  project lets a learn go silently; the clips list and the device keyboard feed back; an
  in-process plug-in's "no words" is its answer (only an isolated one is asked again).

The tests of review 4: core `controller_tests.rs` (a narrowed choice, a 2 000-step number,
generic ALSA ports, a dormant record arm, a plug-in's names); project
`schema_41_keeps_a_hosted_mappings_names`; engine `a_messages_moment_is_after_its_output_opened`;
host `a_stepped_parameter_lands_on_its_own_steps`, `values_land_on_the_plug_ins_own_steps`,
`a_program_change_is_said_only_once_its_value_is_on_its_way` (two threads); interface
`controllers_review4.rs`, `controllers_boot.rs` (a lossy and an unreadable global file),
`learn_everywhere.rs` (`the_choice_table_names_choices_as_the_editors_draw_them`),
`frame_budget.rs` (`a_hosted_choices_word_asks_for_one_step`); plug-ins `pane_words.rs`.
