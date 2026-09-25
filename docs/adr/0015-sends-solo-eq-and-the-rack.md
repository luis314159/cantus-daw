# ADR 0015 — Sends, solo, EQ and the effects rack on a strip

**Status:** Accepted · **Date:** 2026-09-20 · **Extends:** ADR 0014 (mixer strips), ADR 0012 (pan law)

## Context

ADR 0014 gave the mixer strips of its own and buses, and left four things for later:
sends, solo on a strip, a way to group strips, and the rest of what FL's strip carries.
The user's direction for this round was FL's mixer, in this order: **sends and linking
between channels, groups of channels, the per-channel effects rack, a base channel EQ**,
and the link from the channel rack. [FL's mixer manual][fl-mixer] and its strip diagram
were the reference.

[fl-mixer]: https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/mixer.htm

Each of these is a decision about the *signal path* before it is a control, and a wrong
one is audible in every project afterwards. So they are written down.

## Decisions

**A send is a post-fader copy, and a route.** `MixerStrip::sends` is a list of
`Send { to, level, enabled }`, one per destination. The copy is taken *after* the strip's
gain, pan and mute — FL's default, and what a reverb return wants: turn the source down
and the reverb follows. A send is an edge in the routing graph exactly as `output` is:
`strip_order` sorts over both, and `routing_would_cycle` walks both, so a send can never
close a loop and the audio thread never meets a return before its source. A switched-off
send keeps its level and stays an edge, so switching it back on cannot change the
processing order under the audio thread. Sends are resolved to `(index, level)` in the
snapshot with the disabled and zero ones dropped, and the processor adds each into its
destination's buffer with one loop after the strip's own output.

**Solo is FL's *solo with connected routes*.** Soloing a strip keeps audible: the strip,
everything that feeds it (through outputs and sends, transitively) and everything it
feeds. Anything else is silenced; mute always wins. This is the only solo the strip has,
because a plain solo on a bus that silenced the bus's inputs, or on a channel that silenced
its bus, would be a solo you cannot hear. It is resolved in `Snapshot::compile`
(`Project::strip_audibility`) into the snapshot's `muted`, so the audio thread reads one
boolean and the offline render agrees with playback to the bit.

**A group is a submix, in the one tree.** ADR 0014 said track groups must be the mixer's
tree; *create submix* is that tree's operation: a new strip after the last member, every
member's output set to it, the new strip to the master, all in one `Batch`. FL's visual
"groups" (coloured separators, docks) are not modelled: a bus is the grouping that changes
what is heard, and a second grouping that only changes what is seen is the two-of-
everything trap.

**The multi-selection is the app's, not the project's.** `CantusApp::selected_strips`
holds FL's `Ctrl`+click selection. Everything that acts on it — *route selected here*,
*send selected here*, *create submix*, the ganged faders — builds a `Batch`, so it is one
undo step. Ganged fader moves merge across frames because `Command::supersedes` now says a
batch supersedes a batch of the same shape, pair by pair.

**The channel EQ is three bands after the inserts: low shelf, bell, high shelf.** FL's is
three parametric bands; a console's is shelf–bell–shelf, and that shape does a tone
correction in three moves rather than six, so it is the one taken. It sits after the
inserts and before width and pan, where a console's channel EQ sits and where FL's is.
The three sections are RBJ biquads (`cantus_dsp::eq`), and a band at 0 dB is the identity
*without arithmetic*: `b0 = 1`, everything else 0. The processor skips a flat EQ entirely,
and an EQ that has signal in it keeps running over silence until its state is exactly
zero — otherwise its state would freeze at a different sample for every block size, and
the next note would meet a different filter. That was found by the block-size test, not by
reasoning, and is the reason `Biquad::is_quiet` exists.

**Every slot has its own dry/wet, outside the effect.** `EffectSlot::mix` crossfades the
slot's input with the effect's output; at 1 it is skipped, at 0 the slot is a wire but the
effect still runs so its tail is where it would be when the knob comes back. This is FL's
slot knob and it is what makes any insert a parallel one without a second strip. The
insert count is capped at **ten** (`MAX_INSERTS`), FL's number, enforced by the command.

**Swap and the FX switch are strip switches.** Swap is an input-stage switch beside the
polarity; `fx_enabled` takes the whole chain out without touching each slot's bypass.

**Schema 10 writes nothing.** Every new field has a serde default that is the identity —
no sends, not soloed, not swapped, FX on, EQ flat, mix at unity — and the null tests hold
that a strip with all of them at their defaults renders exactly as a schema-9 strip did.
The fixture `v10.cantus` uses every one of them.

## What the tests hold

- A unity send into an empty strip is a second copy of the post-fader signal; at 50 % it
  is one and a half; off or at zero it changes no sample; a muted source sends nothing; the
  destination's insert is heard on the copy and not on the direct path.
- A flat EQ, whatever its frequencies, changes no sample; a shelf cut halves the RMS under
  it; the EQ is block-size independent (block 7 against block 512, to the bit).
- A slot at 0 % is a wire to the bit, at 50 % the crossfade; the FX switch off is the dry
  strip to the bit.
- Swap on a bus is the mirror image of panning the source the other way.
- Solo on a channel is bit-identical to muting its sibling; solo on the bus is everything.
- The undo fuzz runs every new command, refused sends and cycles included. It found one
  pre-existing fault on the way: two clips at the same start were kept in insertion order,
  so undo could restore a project that compared unequal. Clips sort by `(start, id)` now.

## Consequences

- New commands: `SetStripSolo`, `SetStripSwapChannels`, `SetStripFxEnabled`,
  `SetStripEq`, `SetStripSend`, `RemoveStripSend`, `MoveStrip`, `SetEffectMix`.
  `RemoveStrip` also refuses a strip something sends to.
- The mixer strip: `S`, `⇄`, `FX`, three EQ bars, a peak readout in dB, a sends block
  with `+`, and a menu on the header (rename, colour, solo/mute, route/send selected here,
  create submix, move, remove). `Alt+←/→` move the selected strip. The inspector draws the
  EQ's response from the same coefficients the audio thread runs.
- The route picker offers *first free strip* — FL's `TRACK` auto-assign.
- **Not in this round, and why:** external audio input (there is no audio input path in
  the engine yet; it arrives with audio clips and recording, not with the mixer), sidechain
  sends (no insert takes a sidechain input yet; a sidechain is a send with a destination
  that is an effect's second input, and it should wait for that effect), plugin delay
  compensation (no insert has latency), and FL's docks and separators (see *a group is a
  submix*).
