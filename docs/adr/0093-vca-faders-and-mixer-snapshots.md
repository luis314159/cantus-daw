# ADR 0093 — VCA faders, mixer snapshots with cues, and the console's safe switches

**Status:** accepted (round 36, mix builder) · **Date:** 2026-11-02 · **Extends:** ADR 0004
(every mutation is a command), ADR 0014 (strips and buses), ADR 0027 (automation on the audio
thread), ADR 0057 (modulators), ADR 0064 (freeze and its edit lock) · **Schema 50**

## Context

The ask (BACKLOG §0.4 item 6, the professional studio): a VCA moves its group with each
channel's automation intact; a snapshot recalls exactly; both usable by an MCP agent. FL's mixer
has linked faders but no VCA and no snapshots; every console and Logic/Pro Tools/Studio One have
VCAs and scenes. `StripGroup` is organisational only (a separator that folds); `snapshot.rs` is
the audio thread's compiled view of the project, not a user snapshot.

## Decision

### 1. A VCA is a strip with no audio; its fader adds decibels at the gain stage

`MixerStrip::kind` (`StripKind::Audio` by default, `Vca`), `MixerStrip::vcas` (the VCAs a strip
follows) and the VCA of a group (`Project::mix.group_vcas`, kept beside the snapshots so
`StripGroup` is unchanged). A VCA is a strip so that everything a fader has comes free: mute,
solo, the fader's own `AutoTarget::StripGain` (so "the VCA's own automation is a gain curve"
with no new target), modulators, MIDI learn, undo. It cannot hold audio: no track, output, send
or insert (the commands refuse), it is never run by the engine (`rest`), and a VCA somebody
follows cannot be removed.

**What a member hears.** The set of VCAs that reach it by any path (its own list, its group's
VCA, and recursively the same for each VCA found), **each counted once**; their decibels add to
the member's fader:

| Case | Behaviour | Test |
|---|---|---|
| Nested VCAs (C follows B follows A) | A strip following C hears C, B and A | `a_strip_hears_every_vca_that_reaches_it_once` |
| A member in two VCAs | Both add | same |
| A VCA reached twice (directly and through a nested one) | Counted once | same |
| A cycle | Refused by the command, the model restored; a hand-edited file with one is read safely | `follow_lists_are_checked_and_undo_exactly` |
| Mute / solo | A muted VCA mutes everything it reaches; a soloed VCA solos everything it reaches, with their routes (FL's solo); the member's own mute still wins | `a_muted_vca_mutes_its_members…`, engine `a_muted_vca_silences_its_members` |
| VCA at 0 dB, or none | Not one sample touched (the golden hashes do not move) | `a_vca_at_unity_is_a_null_to_the_bit` (engine) |

**Where it acts (the gain stage, not the fader value).** In dB, so it composes exactly with a
ridden or modulated fader: the member's own fader, automation and modulators are not rewritten
and its fader still reads where the member is (a −3 dB strip under a −6 dB VCA plays at −9 dB and
shows −3 dB).

- *Static*: `Snapshot::compile` adds the decibels of the reaching VCAs that have no lane to the
  member's `StripSnapshot::gain` (`vca::compile`).
- *Ridden*: when a reaching VCA has a fader lane (an automation clip or a modulator), or the
  member has a lane of its own, the compile gives the member a lane (holding its fader when it
  had none) and a `VcaLink { lane, static_db, from }`; at every 16-sample control point the engine
  evaluates the VCA lanes with the member's lane and ramps the **sum** to the next point
  (`cantus-engine/src/vca_rt.rs`, two marked lines in `automation.rs`). That is the same grid,
  ramps and clock as any fader ride: block-size independence at tolerance 0, offline equals live
  and parallel equals serial are consequences, and are tested (`a_ridden_vca_is_the_ride_drawn_on_each_member…`:
  the same samples, to rounding, as the ride drawn on each member, bit-identical across block
  sizes 16–1024).
- **The floor.** Decibels add, but the fader's bottom (`MIN_GAIN_DB`, silence) wins: a member at
  the bottom under a raised VCA, or under a VCA at the bottom, is silent (`vca::sum_db`, in the
  static fold and on the audio thread; review 1).
- **Freeze** (ADR 0064). A frozen strip's render is taken before its own width, pan and fader, so
  its *own* VCAs act after the render and stay live. A strip summed **into** a frozen bus is
  another matter: its fader stage is inside the bus's render, so the VCAs that reach it are part
  of the bus's key (`vca::with_vcas`: each reaching VCA's id, fader and mute, added to that strip's
  key part only when it has any — every key from before VCAs is unchanged), a VCA's automation and
  modulators reach the region (`target_in_strip_region`), and the commands that move them touch
  the mix of every strip the VCA reaches: `SetStripGain/Mute/Solo` on a VCA, `SetStripVcas`,
  `SetGroupVca`, and `SetStripGroup` into or out of a group with a VCA (review 1). The
  `vcas`/`recall_safe`/`solo_safe` fields themselves stay scrubbed from the strip's raw part.
- A VCA in its own group follows nothing through it: the cycle check uses the same edges as the
  reach (review 1).

### 2. Snapshots: state, never structure

`Project::mix.snapshots` (`MixSnapshot { id, name, state: MixState }`). `MixState` is each
channel's fader, pan, width, polarity, mute, solo, swap, inserts switch, EQ, **each existing
send's** level, switch and tap, and each insert's **bypass, dry/wet and native parameters**
(sparse kinds such as the EQ restore to their defaults for keys not stored), and optionally the
master's. Left out, deliberately: routing, which inserts and sends *exist*, and a hosted plug-in's
own state (a blob owned by the plug-in's project, not the desk's: the insert's bypass and dry/wet
are still recalled). Hence **a recall never creates, removes or re-routes anything**: an insert
or send made since is left as it is, one that is gone is skipped (`a_recall_never_creates_removes_or_reroutes`).

- **Recall** (`MixCommand::Recall { id, scope, master, include_safe }`): all channels, or the
  `scope`; the master when asked; a strip marked **recall-safe** (`MixerStrip::recall_safe`) sits
  out a recall of everything unless named in `scope` or `include_safe`. It is one command, one undo
  step; its inverse is `Apply`, carrying exactly the state it replaced, so undo restores the model
  to the bit (`a_recall_restores_the_state_exactly_and_undoes_exactly`, the undo fuzz's own stream).
- **Compare / A-B** is the desk swapped with the snapshot and back, each an undoable `Recall`/
  `Apply` (the desk before the first press is held by the view).
- Take, update (take again), rename, remove: commands with exact inverses; a snapshot a cue plays
  cannot be removed.
- The freeze guard classifies a recall by what it would change (a dry run on a clone): chain and
  mix touches per strip, the master as global; a recall of what is already on the desk touches
  nothing.

### 3. Cues: snapshots on the timeline ("automatable as scenes")

`Project::mix.cues` (`MixCue { id, at, snapshot, scope }`). A cue is a recall at a bar, compiled
— the engine is never asked to change the model — in two halves, both read by **tick** at the
engine's 16-sample control points, so a seek, a loop, a block size, an offline render or a
parallel run hears the same:

- **Levels** (`mix_snap::cue_lanes`): fader, pan, width, send levels, native insert parameters
  and the master's three become *holds* in their automation lanes from the cue's tick **until the
  target's next automation clip starts** (or a later cue). A clip playing across the cue's bar is
  silenced from the cue on — a scene recall overrides a ride until the next one is written, as a
  console's does (review 1: the earlier rule let the clip's next point take over, unlike these
  words). Only what changes is emitted: a value the lane already holds, unmoving, over the span
  makes no lane, so a cue of a whole desk costs only its differences (64 strips, every fader
  moved: 64 lanes instead of 6 400-odd; `full_desk_cue_cost`, numbers in `docs/round36/mix.md`).
- **Switches** (`mix_scene::timeline`, `cantus-engine/src/scene_rt.rs`): mute and solo — as the
  audibility they resolve to, worked out once per cue region with the VCAs and solo-safe —
  polarity, swap, the inserts' switch, each insert's bypass, the strip's own EQ, and the master's
  polarity, inserts' switch and bypasses. Region 0 is the project as it is; each cue's region is
  the previous one with the cue's snapshot applied as a recall would (scope, safe strips sitting
  out a cue of everything). At a control point the engine finds the region of the playhead; what
  changes is faded out over the control segment before the grid point where the cue's tick is
  reached, switched there, and faded in over the 16 samples after; a seek or a wrap switches at
  once and fades in; a snapshot installed mid-song puts its region on at once (an install is a cut
  anyway). The fade is on what leaves the strip's chain (and on the master's output), so nothing
  inside a chain is cut; nothing allocates (the timeline is the snapshot's, the switches are the
  strips' own fields). A project whose cues switch nothing compiles to an empty timeline: the
  engine runs exactly as before (`a_cue_mutes_at_its_grid_point_with_a_fade`: the samples before
  the fade are the project's to the bit, after it the muted project's, in between each nearer the
  muted one, the last within 1/16; block sizes 16–1024 bit-identical).

**Placed by the bar, not by a clip or a scene — and why that is complete.** A cue is an
arrangement object: the timeline is where a mix's sections live, and a cue placed at a bar (at the
playhead, moved with `SetCue`) is what Pro Tools' and Studio One's scene automation are. A *clip*
that recalls a snapshot would be a second, overlapping way to say the same thing (an automation
clip already is "this lane from here"), with no new power. The **session view** is different in
kind: a scene launch happens on the audio thread when its quantisation comes round, by a person
performing, not at a tick of the arrangement — recalling a snapshot there would either change the
model from the audio thread (refused by ADR 0005 and ADR 0004: every mutation is an undoable
command from the UI thread) or record performance gestures into the undo history. A performer who
wants the desk to change with a scene uses the scene's clips' own envelopes (ADR 0076), which are
the session's automation. So the timeline cue is the whole feature for the arrangement, and the
session already has its own.

### 4. Console feel (decided against FL's mixer and the consoles' lists)

Done: **solo-safe** (`MixerStrip::solo_safe`: a solo elsewhere does not silence it, unless it is
muted — a click, a reference, a talkback; works with VCA solos), **recall-safe** (above), **VCA
and group-VCA**, the dB tick scale of the fader (already in the kit) with Shift fine-drag and
double-click reset (already the house rules). Not done, with the reason: **input trim** (the
track's own gain already is the trim: it sits before the strip's inserts, and a second input gain
on the strip would be one more place to look for a level); **channel link of two faders** (a
group with a VCA is the link, and moves the members' decibels without rewriting them, which a
link of two faders cannot do); **fader-law switch** (the taper is one, shared by the fader, the
automation range and the hint bar; a second would make a recorded ride read differently).

### 5. Agents

`create_vca`, `assign_vca`, `take_snapshot`, `recall_snapshot`, `get_mix_snapshots`,
`remove_snapshot`, `add_mix_cue`, `remove_mix_cue`, `set_strip_safe`
(`cantus-ui/src/ai_tools_mix.rs`, one marked line in `ai_tools.rs`): each is one command or one
batch (one undo step), the same the buttons run; the strip entries of `get_mixer` carry
`vca`/`vcas` only when set.

## Consequences

- Schema 50: no migration writes anything (`kind`, `vcas`, `recall_safe`, `solo_safe` and
  `Project::mix` are skipped when default); fixture `v50.cantus` holds a VCA the Keys strip
  follows, a snapshot, a cue at bar 3, and both safe switches; every older fixture loads. An older
  build refuses a v50 file.
- A strip's own VCAs are live after its own freeze; the VCAs of a frozen bus's inputs are in the
  bus's key (§1).
- A project with VCAs and the audio engine: `rt_memory` counts zero allocations through a ridden
  VCA, a member's own ride and a cue (`vcas_and_cues_never_allocate`).
- Cost: a lane's VCA link is found by index (`Snapshot::vca_by_lane`), never by a search; a cue
  costs its differences only (§3).

## What the tests hold

`cantus-core`: `vca_tests.rs` (sets, nesting, groups, mute/solo, commands, compile, the static and
laned folds, solo-safe), `mix_snap_tests.rs` (recall exact and undone exact, structure kept, scope
and safe, exact inverses, cues compile, the edit lock, JSON round trip and the unchanged file of a
project without any; the review-1 regressions: own-group VCA, the floor, a frozen bus and its
inputs' VCAs, a refused group VCA's order, exact undo of added keys, partial update, cues of
differences only, a cue holding until the next clip, the switch timeline, routing into a VCA),
the undo fuzz stream `xrng` (16 kinds incl. `SetCue` and groups, applied and refused; its own floor
of applied commands; the main stream draws among audio strips only) ·
`cantus-engine/tests/vca.rs` (null to the bit, the members' gain stage, mute, a ridden VCA = the
ride on each member and block-size independent, a member's ride plus the VCA, a cue in a render,
the floor on a ride, a cue's mute with its fade, a cue's polarity) ·
`rt_memory` · `cantus-project` (fixture v50) · the mixer's shoots in both themes.
