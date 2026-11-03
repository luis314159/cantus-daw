# ADR 0094 — External units and the automatic latency ping

**Status:** accepted (round 36) · **Date:** 2026-11-02 · **Extends:** ADR 0004 (commands),
ADR 0005 (real time), ADR 0018 (delay compensation), ADR 0022 (what is heard), ADR 0025 §7
(the loopback measurement), ADR 0064 (freeze and the edit lock), ADR 0079 (MIDI out and its
clock)

## Context

BACKLOG §0.4 item 6 asks for *external gear with an automatic latency ping*: a hardware synth
or a rack processor is part of a track, sounds on time, and the user does not measure
anything with a ruler. Before this round nothing left the machine except the master to the
device, nothing but the master clock and time code went out of a MIDI port, and the only
measurement was the loopback of ADR 0025 (a noise train on every output, found on any input).
References: Ableton's External Instrument/Effect (a device with a hardware latency
compensation and a *Detect* button), Logic's External Instrument, Cubase's external
instruments with a ping, Reaper's ReaInsert, Studio One's *Pipeline*, Bitwig's Hardware
Instrument/FX.

## Decisions

### 1. A unit is project data on a track (schema 51)

`cantus_core::external_unit::ExternalUnit` (MPL), kept in `Project::external_units`:

- **Instrument**: the track's notes go out of a MIDI port (name + ordinal + channel, never an
  index, as controller maps do — ADR 0062); what the synth plays comes back on a pair of
  interface inputs (`ret`).
- **Effect**: the track's audio goes out of a pair of outputs (`send`) and returns on a pair
  of inputs (`ret`).
- Both keep the interface's name (`device`, empty: any), the **measured latency**
  (`Measured { frames, rate, confidence }` — the rate it was counted at; the engine converts),
  a manual **trim** in ms (−500…500: a slow attack, a cable chain), a return level
  (−60…24 dB; 0 touches no sample) and an on/off switch.
- One unit of each kind per track. A removed track leaves its units inert (undoing the removal
  brings them back); `Availability` says why a unit cannot play (no device, another interface,
  channels or port missing, switched off, track gone) and **the engine never opens or loads
  anything for a unit that is not available** — it is said on screen.

Commands: `InsertExternalUnit`, `RemoveExternalUnit`, `SetExternalUnit` (whole replacement:
routing, name, trim, level, switch), `SetExternalLatency` (the ping's result, its own command
so a re-measure is one undo step and the edit lock sees it touches no sound). A unit whose track is gone is inert, not invalid: the undo of its removal puts
it back whether its track is there yet or not (the shared undo fuzz found the order). Each has an
exact inverse, a `cmd.external.*` string, an arm in `freeze_guard::touches` (a unit touches
its track's source; a measurement touches nothing) and an undo-fuzz test in a stream of its
own (`tests/external_units.rs`) and a stream in the shared `undo_invariants.rs`. Schema 51 writes nothing (`external_units` is absent in
every older file); an older build refuses a v51 file.

### 2. The return is the track's input; the trip is the track's latency

An instrument's audio returns through the **track's own input** (`TrackInput`, monitor On —
the path armed audio tracks already use): no new mixing path. The unit adds one thing to the
track: `TrackSlot::external`, the measured trip plus the trim in frames at the engine's rate,
**reported as the track's own latency** (`TrackSlot::latency`, the number delay compensation
reads for a hosted instrument's lookahead, ADR 0028) and **delaying nothing itself** — the
audio arrives that late on its own, so everything else waits for it (`pdc.rs` is unchanged).
Not measured, or off, reports none. Tests: `tests/external_units.rs` — two monitored inputs
whose hits meet exactly once the unit says how late one comes (and not before), the trim moves
the wait, off and unmeasured are the old bits.

*Offline renders and bounces use the stored number*, never a fresh measurement: the number is
in the project, the snapshot carries it (`TrackSnapshot::external`), so an offline render is
repeatable and equals real time (the same code path).

*Not chosen:* a delay line on the track's source (the `Lateness` of ADR 0064 §6 delays what
was recorded with latency baked in; the return of hardware is already late, so a line would
double it).

### 3. Notes out: the track's outbox, stamped by sample, sent by the clock's own thread

A track whose unit sends MIDI is **listened to** like a track an effect hears (ADR 0022):
the notes it plays — a clip's, a live key's, an all-notes-off — already land in its outbox
with the absolute engine sample they were played at. At the end of each block
(`processor_ext.rs`, one marked call) the outbox is drained into the MIDI-out ring as
`Stamped::note(sample, route, bytes)`; the thread of ADR 0079 (`sync_out.rs`) turns the
sample into a host time through the audio thread's own line plus the device's output latency
and writes it to the port its **route** names (`MidiOut::set_unit_routes`; the table is
derived from the project by `Project::midi_routes`, so the engine and the interface agree
with no message between them). The track's own instrument is not played (`outboard`): the
synth is in the rack. Ports come from the shared pool (one connection per port: Windows'
class driver serves one client). Nothing here allocates, locks or does I/O on the audio
thread (`rt_memory.rs`: `external_notes_and_the_ping_never_allocate`).

The stamp is the sample the note was *played*: the thread adds the device's output latency, the
synth answers, the interface returns it, and the processor pops it `L` frames later — the
quantity the ping measures (§4), so the same `L` that aligns the return is the delay between
a note's stamp and its sound. Tests: a clip's notes leave with their exact samples, route and
channel at every block size (37…1024), a project with no unit stamps nothing and renders the
same bits, a unit switched off is as if absent.

### 4. The ping: two measurements, both inside the audio callback, evaluated off it

`cantus_engine::external_ping` (MPL). The processor runs a **`PingJob`** block by block
(`EngineMessage::StartPing` → `EngineEvent::PingDone`; the job is built and evaluated on the
interface thread; the audio thread only copies into buffers made in `new` — counted by
`rt_memory`). The transport must be stopped (a running ping is refused with `Busy` and comes
back); while an audio ping runs the output is the ping's alone.

- **Audio ping** (an effect, or the interface's own trip): the burst train of ADR 0025 §7 goes
  out of the send pair and is found on the return pair by cross-correlation. The number is the
  round trip *as the engine sees it* — output frame written to the frame the processor pops
  back — exactly what compensation must apply. Each of the eight bursts is also located on
  its own around the global lag; the answer is their **median**, and the **jitter** (the
  widest disagreement, frames) is said and refused above 8 frames (`Unstable`). The return's
  polarity is reported (`inverted`), never corrected.
- **Onset ping** (an instrument): a note-on is stamped at a known sample (the same
  `Stamped::note` path as live notes, so the MIDI thread's timing is *inside* the number), the
  first frame the return rises above four times the lead-in's noise floor (at least 0.003) is
  found, four times; the answer is the median, and the repetitions must agree within 2 ms.
  This includes what no audio loopback can measure — the MIDI path, the cable, the synth's
  own response — and its attack: the trim covers a slow one.
- Errors are said, never guessed: `Silent`, `NoInput`, `NotFound` (too few bursts agreed, or
  hum), `Dropped` (the input lost a block while the ping ran: `stats.input_repeated` rose),
  `Unstable`, `NoNote`, `Busy` (each has an `external.ping.*` string).
- The result is `Measured { frames, rate, confidence }`, stored with `SetExternalLatency`; the
  screen shows it in frames and ms and *Re-measure*.

Tests: with a virtual interface (`Wire`: delay, gain, polarity, noise, channel) the audio ping
returns the delay plus one block **to the sample** at block sizes 32…1024 and delays 0…24 000;
polarity, a quiet return, hum, silence, no input, a dropped block and a jittering return are
each said; the onset ping equals the synth's latency plus a block to the sample through noise;
and the same through the real `Processor` (`tests/external_units.rs`): ring in, ring out,
notes stamped, a refused second ping, the master silent while the ping runs.

*Honest limit:* the ping's physics is proved on a virtual wire; what only hardware shows —
a driver's own jitter between blocks, a synth's MIDI input buffering, the real round trip of an
ASIO device — is for the owner's interface (`docs/round36/external.md`, *Owner-only*). Never
load an ASIO driver whose interface is unplugged: a unit whose interface is not present is
`Availability::OtherInterface`/`NoDevice` and the ping is not offered.

### 5. External effects: the source goes out of the send pair, the return is the track's input

An effect unit has its model, routing, audio ping and compensation (§1, §4) **and its send**.
At the point of `track_segment` where the track's source (its instrument, its clips) is complete
and the monitored input has not joined yet, a track with an enabled effect unit writes that source
into its own send buffer (`TrackSlot::send_l/r`, one per track: nothing shared between threads
of the pass) and stops hearing it; after the pass the processor mixes each send into the
device's output at the unit's pair (`ext_send_mix`, one marked call after `run_pass`, adding
into the pass's slice; mono is the mean of left and right) and clears it. What comes back is the
**track's input** (the unit's creation sets it to the return pair, monitor On), joined and
levelled as any monitored input: the return's level (`return_db`) multiplies the input's trim
(`live_install`, unity exactly 1.0), the strip's inserts, fader and sends follow. The stored trip
is the track's reported latency (§2), so the rest of the mix waits for the hardware.

A send on outputs 1–2 would play on the master and is refused (`Availability::SendOnMaster`; the
engine makes no send there); the default is outputs 3–4 when the interface has them. Tests
(`tests/external_units.rs`, `mod send`): the source is on the send pair and the master is
silent while the return is silent; the send equals the plain render's source; through a virtual
wire (delays 1…2000, blocks 64…480) the master is the plain master again after the trip, to
1e-6; a send on the master's outputs is not made; a project with a unit on a track that is
not there is the same bits.

*Not chosen:* a send as a mixer insert slot (it would add an `EffectKind`, its dispatch in the
effects, the catalogue and every insert test for a thing that is not DSP); a send after the
level (hardware inserts take the source, the strip's fader is for what returns).

### 6. What makes it hold in the application (the integrator's review)

- **Routes.** A unit's notes are stamped with its route: the unit's id (never reused) in the low
  32 bits and a hash of its port (name, ordinal) in the high 32 (`external_unit::route_for`). Each
  frame the interface registers every enabled instrument's `(route, port, ordinal)` with the
  MIDI-out thread `assemble` started (`SyncService::set_unit_routes`, re-sent to a new out thread
  after an audio restart); the second port of a name is opened by its ordinal (`pool_key`). A ping
  registers its unit's route while it runs, even for a unit that is off — it never borrows another
  route. Tested through `CantusApp::assemble` (`tests/external_boot.rs`) and at the out thread
  (`sync_midi.rs`).
- **No stuck notes.** The processor keeps the keys each slot holds on its synth. A unit removed,
  switched off, moved to another channel or port (another port is another route) first releases
  them: note-offs and an all-notes-off on the old route and channel. A route that leaves the out
  thread's table is retired, not closed, for 3 s and while anything waits for it, so those
  note-offs reach the synth that holds the notes. A note-on is only sent while 256 places of the
  out ring are free; a note-off that still finds it full waits in a queue of its own and goes first
  in the next block. Tested: removal, off, channel, port, and a stalled ring that never leaves a
  note on.
- **Availability gates the engine.** The interface gives the handle the units that can play now
  and the interface's round trip (`external_presence`); a unit not available (another interface,
  its channels or port missing, no device) gives the snapshot no link — its track plays as without
  it; a change sends the project again. A send or a ping pair past the device's channels (an
  offline render runs two) is never written: nothing lands on another channel.
- **Re-based trips.** A measurement keeps the interface's reported round trip (input + output
  latency + one buffer), buffer and name. On the same interface at another buffer the trip is
  re-based — stored trip − round trip then + round trip now — and the screen says "measured at
  buffer 256, adjusted to 512"; on another interface it is used as it is and the screen says
  "measured on another interface: measure again".
- **Renders refuse outboard gear, and say why.** A freeze of a region with an enabled unit
  (`CannotFreeze::External`, in `freeze_commands`), a bounce in place of such a track and an export
  of a song in which one plays are refused with a sentence that says what to do: record the gear's
  return (or switch the unit off). *Not chosen now:* a real-time bounce through the gear from the
  export dialog — it is recording the return while the song plays, with the device, latency and
  monitoring the recorder already handles; offering it inside the export would duplicate the
  recorder under another name and make an export depend on the hardware being on. Recording the
  return is the honest path, and the message says so.
- **Every offline render plays no gear - one choke point.** Every offline render (export, stems,
  freeze, bounce, video, listening renders, bakes, the record mode bakes) runs a processor with
  `set_offline(true)`; offline, the processor ignores every external link: no note goes out, no
  send is made, the track plays its own source, and no gear latency is reported (also when it
  goes offline after the snapshot is installed). The handle strips the links from every render
  compile too (`compile_for_render`). Tested so that a future offline path cannot miss it: a render
  of a snapshot compiled straight from a project with units is the same bits as the project
  without them. User-facing renders refuse while a unit plays (export, video export, bounce,
  freeze, modulator bake), with what to do; an AI listen render (and the mastering assistant)
  renders without the gear and names the tracks it left out (`outboard_not_heard`). The record
  mode keeps units live (its plan, `Why::External`, record's).
- **A surface gesture is one undo step** (ADR 0095 §2).
- **The ping cannot hang.** It is given up after 15 s or when the device changes, and on an audio
  restart (`engine_replaced`); the transport starting under it aborts it in the engine
  (`PingError::Aborted`), releasing the note an onset ping holds.
- **The onset ping's confidence is measured:** the notes' level over the room's noise (their
  median), divided by one plus the repeats' spread in frames.
- **Taking a unit away gives the track its input back** (kept in `ExternalUnit::restore_input`
  when the unit made its return the input); one undo step either way. The track menu has
  *External instrument…* / *External effect…* (or their removal).

## Consequences

- A synth in a rack is one track: notes out, audio in, on time after one click.
- A freeze, a bounce or an export of outboard gear is refused with what to do instead (§6).
- `StartPing`/`PingDone` and `take_pings` are on the engine handle; the interface wraps them in
  a service (`external_service.rs`) so a ping never blocks a frame.

## What the tests hold

`tests/external_units.rs` (core: commands, inverses, refusals, the edit lock, the file,
snapshot, a 60-seed undo fuzz), `external_ping.rs` unit tests (the arithmetic on a virtual
wire), `tests/external_units.rs` (engine: PDC, notes out, the ping through the processor),
`rt_memory.rs` (no allocation), `crates/cantus-project/tests/schema51.rs` (the v51 fixture).
