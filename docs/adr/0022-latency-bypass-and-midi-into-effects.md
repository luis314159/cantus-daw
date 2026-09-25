# ADR 0022 — Memory on request, soft bypass, what is heard, and notes into effects

**Status:** Accepted · **Date:** 2026-09-24 · **Extends:** ADR 0005 (the audio thread),
ADR 0018 (delay compensation and the plug-in contract)

## Context

Round 16 finished the effect family and delay compensation, and left the host-side items
listed in `docs/STATUS.md` (*The next session*, item 3):

- every Aequalis allocated about 4 MB up front so it could switch to linear phase without
  allocating on the audio thread — and so did every Spatium, whose post EQ is an Aequalis;
- a bypassed latent insert stopped counting, so compensation re-planned on every bypass
  and everything in parallel jumped by the insert's latency;
- the output lagged the position shown by `EngineStats::latency_samples`: the playhead,
  recorded notes, the click and a strip's take were all early by it;
- Porta's MIDI trigger (Pro-G has one) needed the engine to route a track's notes to an
  effect;
- Spatium's "measured" decay read short in the low octaves (3.46 s against 4.46 s at
  63 Hz in the demo), thought to be because the captured tail was shorter than the decay
  (decision 5 found another cause).

## Decisions

### 1. Memory an effect needs arrives by message, the way delay lines do

`Aequalis` holds its linear-phase convolver in an `Option`, sized for the resolution it
runs (`LinearPhase::new(sample_rate, resolution)`), not for the largest. Two constructors
say who may allocate: `Aequalis::new` makes the buffers itself when a mode needs them (tests,
the editor's pictures, offline tools); `Aequalis::realtime` — what the engine uses — never
allocates after construction. When linear phase is asked for and the buffers are missing
or too small, the realtime EQ **keeps running its previous path** and reports the need
(`linear_need`); the mode in force is `running_mode`, and `latency()` reports that one.

The engine side is generic, so the next effect that needs memory for a setting uses the
same road: `EffectUnit::memory_need` → the audio thread asks once per need
(`EngineEvent::EffectNeed { effect, need }`) → the UI thread builds it
(`EffectMemory::build`) and sends it (`EngineMessage::EffectMemory { effect, memory }`) →
the unit takes it (`EffectUnit::supply`), switches, and whatever it replaced goes back as
`EngineEvent::GarbageMemory` to be freed off the audio thread. Compensation is planned
again on arrival, since the latency changed then. `EffectUnit::new` runs on the UI thread,
so a unit built for a slot already in linear phase (a project being opened, an offline
render) is built with its buffers and never waits; an offline render also answers any
request in its settle loop before the first block (`Processor::settle`), like `PdcNeed`.

Buffers are kept when the EQ leaves linear phase — switching back is instant — and larger
ones replace smaller ones; they are never shrunk on the audio thread. The natural-phase
correction (about 20 KB) is always held, so switching to it never waits.

### 2. Soft bypass: a bypass keeps the latency

A bypassed insert with latency — and every latent insert of a strip whose chain is
switched off — keeps counting in compensation, and its slot passes its input delayed by
exactly its latency. Every latent insert has a dry line of its latency from the pool
(`pdc::Edge::Dry`), running at **every** setting, so the slot mix, the bypass and the
chain switch always meet a dry signal already in step with the effect. A bypassed latent
effect **keeps processing** (its output is discarded): its state follows the signal, so
switching it back in is as seamless as switching it out. That costs its CPU while
bypassed, which is the price of nothing jumping in either direction. A bypassed insert
with no latency is unchanged: a wire, and it does not run.

### 3. What is heard: the engine reports it

The correction lives in the engine, not in the UI: the engine is the only place that knows
the latency at each block, the tempo, the loop and when the playhead jumped. Following
the coordination note of this round, the transport advances as before and the engine
reports the heard position **alongside** its own: `EngineEvent::Position { tick, heard,
playing }`. `heard` is exactly the position the engine was at `latency` samples ago
(`Transport::position_back`, integer sample arithmetic, so it is the same number the
position had then); after a start or a seek it stays at the start point until the start
has come out; across a loop wrap it counts back into the loop's end. Stopped, it is the
engine's position (the lines drain to it). `EngineHandle::position()` returns the heard
position — the transport readout, the playhead, the piano roll, anything placed "at the
playhead" — and `engine_position()` the engine's, for the few views that time something
on an effect's input (the gate's captured hits, exact for a gate that is the first insert
of a track's strip).

- **Recorded notes** are stamped with the heard position: a note played while hearing
  beat 2 lands on beat 2.
- **The click** goes through a line of the output's latency (`pdc::Edge::Click`), so a
  beat's click comes out with that beat's notes. It is still added after the master strip
  and never recorded.
- **A strip's take** is delayed by the strip's path to the master (`pdc::Edge::Capture`),
  so every take is in time with what is heard; the master's take is the output itself, as
  before. An offline render is what was heard from the start of playback, latency
  included, and a take started with playback is that render to the sample. Trimming the
  latency from the front of a bounce is an export decision, left to the interop work.

### 4. Notes into effects

An insert slot can hear a track's notes: `EffectSlot::midi_from: Option<TrackId>` (schema
14, migration writes nothing), set by `Command::SetEffectMidiSource` (refused for a
missing track and for a kind that takes no notes — `EffectKind::takes_notes`, the gate
today), chosen in the effect's editor. It is not an audio route: it orders nothing and
cannot loop. A removed track simply stops sending (undoing the removal brings it back);
muting a track silences its instrument, not its notes — a trigger track stays silent and
still plays the gate.

`FxUnit` gains a default no-op input: `fn note(&mut self, offset: usize, note: FxNote)`,
with `FxNote::{On, Off, AllOff}` — a note `offset` samples into the next `process` call,
always inside it. The engine keeps the notes in a preallocated queue (1024), filled by a
small hook where a track's events fire, where live notes are played and where every note
is released (stop, seek, loop wrap); it delivers them in the strip stage, to the effect,
just before it processes the segment the note falls in. Each note waits for the audio:
its due time is the strip's arrival plus the latency of the inserts ahead of the slot, so
a note and its sound meet at the effect even behind a linear-phase EQ. The offset makes
delivery independent of the segment structure — a whole-block strip stage (planned
after this round) keeps it sample-accurate with no change to the contract.

**Porta's MIDI trigger** (`trig` = Notes): a note-on opens the gate, the last note-off lets
it hold and release, at the attack, hold and release set; the key is not listened to.
`vel` (0–60 dB) is how much less the softest note opens it than the hardest; 0 opens every
note fully. Played by notes, the gate starts shut, at once. Notes act on the key's
timeline, so the lookahead opens it early exactly as it does for a level.

### 5. The measured decay: what the octave hears, and a cut tail

The brief blamed the demo's short 63 Hz reading on a tail shorter than the decay. Measured
(the demo's settings, its hit, captures of 2.9, 6 and 13 s): the 63 Hz octave reads
**3.43 s whatever the length**, and its level had fallen about 46 dB by the end of the
2.9 s the editor captured.
The tail was not cut. The demo's hit has almost nothing below 100 Hz and its post EQ cuts
at 140 Hz, so the 63 Hz octave holds little energy of its own, and a 24 dB/octave band
measured its louder neighbours' faster decay through its skirts: 77 % of that band's
energy was leakage. The same capture through 48 dB/octave skirts reads 4.30 s, through
96 dB/octave **4.57 s** (the engine's curve: 4.46 s at the band's centre, longer below
it). So the editor measures through `measure::band_decay`, whose octave filters have
96 dB/octave skirts (`DECAY_BAND_SLOPE`); the DSP's own tests keep `band_t60` and
`expected_band_t60` at 24 dB/octave, which measure a flat excitation and agree with each
other. A band whose energy sits at one edge (the demo's 125 Hz octave, where the post EQ
leaves only the part near the hit's 190 Hz body) still reads the decay of that edge: the
number is honest about the signal, and the curve is where the intent is drawn.

A tail can still be cut — the editor measures once the tail has fallen 35 dB or after
4 s, and a 10 s hall is cut at 4 s. Schroeder's backward integration of a cut tail misses
the energy after the cut, so its curve bends down early and T20 reads short.
`band_decay` adds that energy back — ISO 3382-1's correction for truncation, iterated
until the rate agrees: for a decay of power rate k, what follows the cut is the energy of
the capture's last quarter over e^(kW) − 1 (a quarter of the capture is many cycles even
of the lowest octave, where one short window's level swings by decibels) — and reports
how far the band fell from where the fit starts (the decay curve's −5 dB point, after
any early burst that is not the decay) to the end of the capture. Below 30 dB (the fit's
20 dB and a 10 dB margin) the tail was cut and the number is an estimate: the octave
table prints it as `≈9.42`, dimmer, or the word *cut* where too little was heard to
estimate at all, the table explains both on hover, and the curve's square for that octave
is dimmer (`shoot … space-cut` renders a 9 s hall measured after 4.3 s: the 500 Hz and
1 kHz octaves read ≈9.42 s and ≈8.50 s against the engine's 9.32 s and 8.44 s).

## What the tests hold

- `tests/rt_memory.rs` (a counting global allocator, per thread): at 48 kHz a
  zero-latency EQ holds **153 232 B** (bound 192 KiB; 96 KiB of it is the key's analyser),
  natural phase the same, linear phase Medium **1 179 648 B** and Very High **4 132 608 B**
  — each only what it needs; in round 16 every EQ, and every Spatium's post EQ, held the
  Very High buffers, about 4.3 MB. Switching to linear phase keeps the zero-latency
  output to the bit until the buffers arrive, and from the switch on is bit-identical to
  an EQ made in linear phase.
  The processor makes **zero** heap allocations or frees while it plays through a switch
  to linear phase, a larger resolution (old buffers handed back), soft bypass, the chain
  switch, a gate played by notes, the delayed click, a strip take, a seek and a stop. The
  harness found one old breach at once: the EQ's `configure` built its band keys
  (`b3.gain`) as `String`s — about 1 100 allocations and frees per snapshot on the audio
  thread since round 15. `eq_bands` now reads them through the parameter table's
  `&'static str`s, as `comp_settings` already did.
- `tests/pdc.rs`: toggling bypass on a latent insert (and the chain switch) four times
  keeps a track and its inverted twin cancelled to **exact silence**, with
  `latency_samples` unchanged at every toggle; a bypassed insert with no latency is a wire
  to the bit in a project with latency elsewhere.
- `tests/heard.rs`: with latency L = 100, the heard position is the start for the first L
  samples and the engine's position L samples ago after; a note played while beat 2 is
  heard is recorded on beat 2 (and on beat 2 without latency); a click and a note on the
  same beat both come out exactly L later than without latency; a strip's take equals, to
  the bit, the offline render of that strip alone, and the master's take is the output.
- `tests/midi_fx.rs`: a gate played by notes is exact silence until the note-on's sample
  and open on it; velocity 40 with `vel` 24 dB opens it 16.44 dB less (within 0.05 dB);
  blocks of 1, 7, 64, 1000 and 4096 give identical output (lookahead on); a gate that
  listens to its key renders to the bit the same whether or not its slot hears a track;
  behind a latent insert the gate opens where the note's audio arrives.
- `cantus-core`: `SetEffectMidiSource` undoes exactly and refuses a missing track, a
  missing effect and a kind without notes, leaving nothing behind; it is in the undo fuzz.
- `spatium_measure`: a quiet 63 Hz octave (4.82 s measured alone) beside a loud 190 Hz
  tone decaying over 3.2 s reads 3.51 s through 24 dB/octave skirts and **4.82 s** through
  the editor's (within 2 % asserted). A 63 Hz octave decaying over 4.46 s under a louder
  1 kHz octave over 1.5 s, cut after 2 s (the low octave fell 21.4 dB past where the fit
  starts): plain Schroeder reads **3.84 s**; the corrected measure reads **4.13 s** — within 3 % of the **4.21 s** the
  whole 12 s tail of the same noise reads (one realisation of a narrow octave reads a few
  per cent off its nominal decay whatever the method) — and says the tail was cut; the
  whole tail reads complete, and the mid octave, which fell 67.3 dB, reads 1.41 s and
  complete.
- `porta_tests`: in MIDI mode the gate is shut until a note's sample and opens on it —
  with lookahead, exactly the lookahead before the note's audio — at blocks of 7, 64 and
  1000; a second held note keeps it open through the first one's note-off; after the last
  one it holds and releases to exact silence.

## Consequences

- New: `EffectMemory`, `MemoryNeed`, `EngineMessage::EffectMemory`,
  `EngineEvent::{EffectNeed, GarbageMemory}`, `Aequalis::{realtime, running_mode,
  linear_need, supply_linear, linear_capacity}`, `LinearPhase::{capacity, fits}` (and
  `new` takes a resolution), `pdc::Edge::{Click, Capture}`, `Transport::position_back`,
  `Processor::heard_position`, `EngineEvent::Position::heard`,
  `EngineHandle::engine_position`, `FxNote`, `FxUnit::note`, `EffectUnit::note`,
  `EffectSlot::midi_from`, `Command::SetEffectMidiSource`, `EffectKind::takes_notes`,
  Porta's `trig` and `vel` (and `porta::Trigger`, `Porta::note`), `measure::{band_decay,
  decay, Decay, COMPLETE_DB, DECAY_BAND_SLOPE, octave_band_sloped}`; schema 14; the
  `shoot` kinds `gate-midi` and `space-cut`.
- A bypassed latent effect costs its CPU. A strip's chain switched off keeps its latency.
- Every latent insert holds one pool line for its dry path, bypassed or not.
- Left: the other effects' editors that time telemetry in song time (the compressor's and
  the limiter's histories) read the heard position; exact for an effect at the end of the
  path, early by the rest of the path otherwise — the engine would need to report each
  analysed effect's input lag to make them exact. Recorded notes are stamped at the block
  they arrive in (sub-block MIDI timestamps are still backlog). A `midi()` all-notes-off
  controller reaches the instrument but not the effects listening to it.
