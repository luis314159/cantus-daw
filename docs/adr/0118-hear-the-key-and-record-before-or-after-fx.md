# ADR 0118 — Hear the key; Record: Before FX · After FX

**Status:** Accepted (round 40, builder *pads*) · **Date:** 2026-11-12 · **Round:** 40 ·
**Schema:** 62 (a track's `record_tap`) · **Extends:** ADR 0004 (commands), ADR 0005 (real
time), ADR 0018 (delay compensation), ADR 0022 (what is heard), ADR 0025 (recording), ADR 0031
(the graph) · **Design:** des-mixer's inspector sheet (*Sidechain*: "Hear the key"; *Recording*:
"Before FX · After FX") · **Owner:** round 39's decision 11, answered 2026-11-12: *both*. ·
**Amended:** round 40's mid-round review (`fix-a`, A-M1) — §1 *never in a take*

## Context

Round 39 built the inspector to des-mixer's sheet without two rows the engine could not back
(BACKLOG §6j):

- **Hear the key** — FabFilter Pro-C's and Ableton's "listen to the sidechain": the keyed
  insert's key played in place of the strip, to tune the key by ear. Cantus's compressor
  already had *Listen: Key* as a saved setting of the unit; nothing let the inspector audition
  any keyed insert without changing the song.
- **Record: Before FX · After FX** — Logic's and Studio One's "record with effects": a take
  of the input as heard through the strip's inserts. Cantus recorded only the input, after
  its trim (ADR 0025 §4).

## Decision

### 1. Hear the key — a monitor state of the live engine

- `EngineMessage::SetHearKey(Option<EffectId>)`, sent by `EngineHandle::set_hear_key` (which
  remembers it: `hear_key()`); the processor copies it into the pass's context. One insert at
  a time. **Never saved** (not in the project), **never in a render or an export** (a render
  is another processor, which is never told).
- On the strip (`graph/strip_run.rs`), at the insert heard, when it is keyed:
  - a unit with a **detector of its own** plays the detector's input — the compressor
    (`Comprimo::set_hear_key`): its key after its level, high-pass and EQ, delayed with the
    audio by its lookahead, exactly as its own `Listen::Key`. The slot's dry/wet does not
    blend it back.
  - any other keyed insert, or one bypassed: the key **as it arrives** (the key strip's
    post-fader output, silence where that strip made none this pass).
  - What follows the insert (the rest of the chain, the strip's EQ, pan and fader) acts on it
    as on the strip's own signal: the switch replaces the strip's signal at that insert.
- A strip whose insert is heard never rests (`may_rest`).
- **Never in a take** (amended, `fix-a` A-M1). An *After FX* take is what leaves the strip's
  inserts, and the heard key replaced the strip's signal at the heard insert — so a vocal
  through a keyed compressor, the key heard, recorded the kick. A stateful chain cannot give
  both the heard signal and the unheard one, so while a strip's take is being written after
  FX (recording, the strip holding a recorder's tap: an armed *After FX* track on it) **no
  key is heard on that strip**: it plays what it records. Every other strip, and a take
  *Before FX*, hear the key as before. The compressor's state followed the signal
  throughout, so a take is the take with nothing heard, to the bit, even when the key was
  heard up to the moment recording began; an insert after it that keeps a memory (a delay,
  a reverb) still carries what it heard before recording began into the take's first
  moments (the switch is a monitor state; turn it off before recording for a clean start).
- Master inserts are not covered: the inspector's Sidechain section is a strip's.

**Null when off, to the bit:** with no insert heard, the only work is a compare per insert
(`ctx.hear_key == Some(e.id)`) and, for a compressor that was heard, one call that turns it
off. The compressor's state follows the signal throughout (only its output is chosen), so
after the switch goes off the song is exactly the song.

### 2. Record: Before FX · After FX — a choice per track, in the song (schema 62)

- `Track::record_tap: RecordTap` (`BeforeFx`, the default and every older track; `AfterFx`),
  `#[serde(default, skip_serializing_if = "is_before")]`; `Command::SetTrackRecordTap` with
  its exact inverse; `migrate_61_to_62` writes nothing; `fixtures/v62.cantus` has its second
  track after FX. The freeze guard ignores it (where a take is tapped, not a sound).
- **Before FX** is ADR 0025's take, unchanged: the input after its trim, to the bit.
- **After FX**: the strip writes what leaves its **inserts** — before its EQ, width, pan and
  fader — into a recorder's tap (`Nodes::rec_taps`, eight preallocated pairs of
  `PASS_FRAMES`, 64 KB) while recording; the recorder reads it after the strips ran (the
  recorder's call moved from before the pass to after it; the input it reads is the same).
  The take is **stereo** (the strip's pair; `begin_recording` opens a two-channel file).
- **Where it lands:** the tap is late against the input by the strip's lateness after its
  inserts (`strip_post`: the compensation it waits for plus its inserts' latency). The take
  drops that many frames at the pass's start, so its frame *f* is the input's frame *f*
  through the chain, placed by the same `Started` mark as a take before FX. The inserts' last
  frames are not played when the pass stops: an after-FX take is that much shorter.
- **What it holds:** what the inserts make of everything the strip carries — the monitored
  input, and anything else playing through the strip then. The inserts hear the input only
  while it is monitored: **After FX needs monitoring Auto or On** (the row's tip says so).
- Up to eight tracks record after FX at once (two tracks on one strip share its tap); past
  that a track records its input, into a two-channel file (both sides the input).
- *Known limit, written down:* recording over a loop, each pass after the first starts with
  the strip's lateness of the pass before (zero without latent inserts or compensation).

### 3. The inspector

Both rows live in `views/inspector_taps.rs` (the inspector is `look`'s), called once each:
*Hear the key*, a des-mixer switch under each keyed insert's *From* row; *Take from*, a
segmented *Before FX · After FX* under *Monitor* (not labelled *Record*: the *Arm* row is), dimmed (never hidden) on a track with no input.

## Consequences

- Real time (ADR 0005): nothing allocates, locks or blocks; the taps are preallocated, the
  switch is a value in the pass's context (`recording_rt.rs` records a track after FX).
- The recorder's call now follows the strips; every recording test holds (before FX to the
  bit, latency, pre-roll, overflow, real time).

## What the tests hold

- `cantus-engine/tests/hear_key.rs`: off touches no sample (never set; an insert not there;
  on for a while, then off — the song before and after it, to the bit); on a compressor, the
  output is the unit's own *Listen: Key* to the bit; on an EQ keyed from the bass, the key as
  it arrives, to the bit.
- `cantus-engine/tests/record_tap.rs`: before FX = the input to the bit whatever the inserts
  do; after FX = the strip after its inserts, stereo, to the bit what was heard; with a 5-ms
  lookahead, the take moved back by exactly its latency (frame *f* = input frame *f*).
  Amended (`fix-a`): a voice recorded after FX through a compressor keyed from a kick, the
  key heard from before recording on — the take is the take with nothing heard, to the bit
  (`after_fx_a_take_never_holds_the_key_heard`); before FX the take is the input and the key
  is heard meanwhile (`before_fx_a_take_is_the_input_while_a_key_is_heard`).
- `cantus-project/tests/schema62.rs`: every older fixture reads before FX; v62's second
  track after FX, kept by a save, written only for the track that chose.
- Undo fuzz (`// r40:pads`): `SetTrackRecordTap` among the others.
- `cantus-ui/tests/taps_e2e.rs`, through `CantusApp::assemble`: the Sidechain section opened
  and *Hear the key* clicked — the live engine plays the unit's *Listen: Key*, to the bit — and
  clicked again; the Recording section opened and *After FX* clicked — one undo step, saved,
  and a take through the app's own engine handle that is what was heard, to the bit.
