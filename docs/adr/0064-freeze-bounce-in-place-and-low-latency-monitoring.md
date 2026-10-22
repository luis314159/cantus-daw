# ADR 0064 — Freeze, bounce in place, and low-latency monitoring

**Status:** accepted (round 29; amended after its two reviews — schema 38 for a bounce's
lateness) · **Date:** 2026-10-19 · **Amends:** ADR 0031 §4 (the render
cache: built here as freeze, keyed by content), ADR 0025 §7 (a take's latency is the one at
its start, not at its finish), ADR 0022 (a second kind of bypass: hard, for monitoring) ·
**Extends:** ADR 0050 (a frozen render streams on its pool and reader), ADR 0063 (the
plug-ins of a frozen chain are unloaded), ADR 0004 (an edit lock in `Command::apply`)

## Context

The owner's ask (2026-10-16, BACKLOG §4c steps 1 and 2): *freeze tracks, bounce in place,
and low-latency monitoring while recording — freezing frees CPU and unfreezes without
loss; bounce in place to audio with or without effects; an armed track is monitored
without the latency of other tracks' plug-ins.* §4c's own design: the render taken **after
the inserts, sends and effect automation and before the fader and pan**, keyed by a hash of
everything that makes the sound, streamed from disk, an edit that would change it blocked
with a notice, *every `Command` classified … and a new one fails the build until it is*.
It is the first two layers of §4c's recording mode (step 3, later), and the cache §4b's
automatic background freeze will share.

The references: Ableton (*Freeze* / *Flatten*, frozen tracks still mixable), Bitwig
(*Bounce in place*, with or without the chain), FL Studio (*Render to audio*, *Freeze* in
the mixer), Cubase (*Freeze* with tail; *Constrain Delay Compensation*), Logic (*Freeze*
pre / source only; *Low Latency Mode*), Studio One (*Transform to audio*; *Low latency
monitoring*), Pro Tools (*Commit*).

## Decisions

### 1. What freezes (schema 37)

- **A mixer strip** (`MixerStrip::frozen`): everything it gathers — the tracks into it, the
  strips sending or outputting into it, recursively — its polarity, swap, inserts and EQ.
  The render is the strip's pair **just before its width** (`graph/strip_run.rs`): width,
  pan, fader, mute, solo, the sends' levels and taps, where it goes, and every automation
  and modulation of those stay **live**. Pre-fader sends tap after the width, so they stay
  correct.
- **A track's source** (`Track::frozen`): its instrument and clips, before its own level
  and balance (Logic's *source only*); level, balance, mute, solo and its strip stay live.
- **Freeze on a track** freezes the strip it feeds when that strip is its own (no other
  track feeds it — Ableton's track freeze: instrument and devices), otherwise its source
  (`Project::freeze_target_of_track`). The strip menu freezes the strip.

`Frozen { content }` stores the target's **content key** and nothing else; the model change
is schema 37 (`migrate_36_to_37` writes nothing: a v36 strip or track plays live; an older
build refuses v37). A loaded file whose stored key is not what the target holds (a hand
edit) is **re-keyed**, never unfrozen (`freeze::repair`, after the links' repair).

### 2. What makes the sound: the subject and its keys

`cantus_core::freeze::region` is a target's upstream to a fixed point: the strips and tracks
gathered (by output or send, enabled or not — enabling one would change the sound), the
**strips its inserts are keyed from** (and theirs), the **tracks whose notes its inserts
hear**, and what the **modulators driving anything in it** listen to (a follower's strip,
an envelope's notes; a follower of the master puts the whole mix, master included, in). A
frozen **track**'s region grows the same way: a follower driving its instrument brings in
the strip it follows *and everything that strip gathers* (review 1). Two flags ride with a
region: `hears_root` — a modulator in it follows the frozen strip itself (or the track's own
strip), or a strip the root's sound reaches (a follower of the bus the frozen strip plays
into, review 2): then the root's fader, pan, width and mute (a track's level and balance) are part
of the sound, in the key and locked — and `own`, a frozen strip's only input (one track, no
other strip into it), whose **mute and solo stay live** (the strip is silenced with them,
`freeze::silenced`).

The **subject** (`freeze::subject`, `SUBJECT_VERSION` 3 after review 2) is the canonical
JSON of the region: the frozen strip on its own key without its live controls (where it
sits on the desk is nothing to hear), the other strips (in the desk's order: their sums are
made in it) and tracks whole — the tracks grouped by the strip they feed (by its id) and in
the desk's order only within one, so a track moved among others that feed other strips
moves no key (review 2) — but for what is never heard
(names, colours, heights, docks, groups, panels shown, solo, where a plug-in runs — ADR
0063 isolation is bit-identical), the automation clips and modulators that reach it (wherever
they sit; the clips in no order of tracks — only which of two starting together wins is
kept), the audio sources its clips play (path and SHA-256 when known, by id), the tempo
map, and the pan law when a non-root strip's pan uses it (the root's too when it is heard:
`hears_root`, or a follower of the master — review 2). A clip's warp (ADR 0061) is part
of its `AudioRegion`, so it is in the subject; a freeze's or a bounce's render compiles
through `compile_for_render`, which waits for the warp renders. Objects are written with their
keys **sorted** (a removed key must not reorder the rest — the undo fuzz found that one).
The **content key** is SHA-256 of it, lower-case hex (`sha2` and `serde_json` join
`cantus-core`'s dependencies; both MIT/Apache-2.0, already in the workspace's notices).
`SUBJECT_VERSION` changes it on purpose.

The **render key** (`cantus_engine::freeze::render_key`) adds the rate, `DSP_VERSION` and
the file format: the same sound at the same rate from the same build is the same file,
for any project. Files are `<render key>.cfrz` — ADR 0050's cache-file layout (every block
checksummed; a damaged block is never played), labelled with the key and the latency the
frames arrive after — in the **project's own folder of renders** (`Freeze/<file stem>`
beside its file: `Freeze/Song` for `Song.cantus`; review 3: one `Freeze` folder served
every project in a directory, and saving one pruned the others' renders — a project's
renders in that older shared folder are still found, never pruned) when it has one, else
**Cantus's cache** (`<data>/cache/freeze`), and looked up in both. Looked up, never
written to, too (review 4): **the other projects' folders beside it** — a render's name is
its sound, so a project renamed or copied outside Cantus finds its renders in its old
name's folder, and its next save brings them into its own (then the copies in the folder of
a project no longer there go, and the folder when emptied; a folder whose project is there
is never touched) — and the folders its own notes it is still copying from
(`copying-from.txt`, below). A copy that fails is set aside alone: another copy of the same
render may be good (`FrozenStore::damaged` by copy, review 4).
They are **regenerable**: *Collect* does not copy them; a project opened without them (on
another computer, at another rate, after the format changed) plays **live** while they are
made again in the background (`freeze::scan`, one at a time), then plays them.

An audio source changed on disk *outside* Cantus after freezing is not noticed while it
stays frozen (the key holds its path and hash): the frozen render plays what was frozen — as
in the other DAWs. Freezing again makes the render again when a file the region plays is
newer than it (review 1). **Where a file lives** is in the key, so saving — which moves a
recorded take into `Audio/` — and finding a missing file again run **through** the frozen
targets they move (`freeze::rekeyed`): unfrozen and frozen again with the new key in the
same undo step; saving copies each render under its new key beside the project (it is the
same file's sound) — on the save's thread, named as the new key inside (`freeze::copy_as`,
the cache header relabelled: review 4 found a plain copy kept the old key and never
opened), the old render playing under the new key meanwhile (`FrozenStore::alias`);
finding a file makes the render again. A refused collection puts the
files back (`collect::undo`).

**A file missing at render time** (a pack still downloading, a drive unplugged) is never
rendered as silence under the true key (review 2): *Freeze* is refused naming the file; a
background refresh waits until every file of the region is there (it is not marked failed);
the engine's `prepare` refuses a missing file of its own (`missing_file`); a file back on
disk is read again (`SampleCache::has_failed` is asked, the cache's failure forgotten).
Only a file that plays counts (review 3): a take on a comp lane below, a muted clip or a
muted pad refuses nothing — in the engine's check too, a kit's layers included (review 4:
the engine refused a muted pad's file, and never looked at a kit with layers); the waiting
is said once (`freeze.waits_for_file`).

**Saving** (reviews 2 and 3): the project's folder of renders gets the renders its frozen
targets play — saved somewhere else (*Save as*), they come along, copied, instead of being
made again — and keeps no other `.cfrz` but those an undo or a redo would want
(`freeze::keys_in` over both sides of every step of `CommandBus::history` and
`redo_history`, at the usual rates and the session's — review 3: the undo of a removed
frozen track or strip, and a replaced project's frozen targets, were missed) and those a
freeze running or waiting to land will want (`wanted_by_jobs`, review 3: a save while a
freeze was landing pruned its render): before, every freeze, edit and freeze again left a
whole render there. The copies and the prune run on a thread of their own
(`cantus-freeze-save`; each copy written as a part file and renamed when whole, its wires
note with it), never on the frame (review 3: a first save copied gigabytes there); no
folder is made when there is nothing to copy. Review 4 made it safe: **one such thread at
a time** (a later save's waits behind the one before); its prune removes only renders the
folder held when its save began — a render landing while it copies is never among them —
and none the session wants *now* (`FreezeState::protect`, renewed by each save and by
each render started); a queued freeze is kept by the key it will be frozen with
(`as_frozen`: disarmed, the plug-ins' states in). Until its copies are in, the folder notes
where they come from (`copying-from.txt`), so a session cut short looks there; and the
session's close waits for them (`freeze::finish_saving`, up to two minutes). A
project opened at start (the boot path) has its path and its folders set before the first
compile (`Files::project_path`, `CantusApp::from_parts_at`): its renders are found at once,
no frozen plug-in loaded meanwhile.

### 3. The edit lock

`Command::apply` runs every command through `freeze::guard` (`freeze_guard.rs`). Each
command is **classified by what it touches** in one `match` with **no wildcard arm** — a
new command does not build until it is classified (§4c's rule, enforced by the compiler):

- `TrackSource` (instrument, clips, notes, input, arming), `TrackMix` (level, balance, mute,
  solo, place), `StripChain` (polarity, swap, inserts, EQ), `StripMix` (width, pan, fader,
  mute, solo, sends, output, place), `StripInput` (a route into a strip made or taken), an
  automation or modulation `Target` (mapped to the above), or `Global`.
- The **locks** of the frozen targets: a frozen strip locks its region's chains and the
  mixes of every region strip but its own (its own too when `hears_root`); its tracks'
  sources and mixes — but its only input's mute and solo (`TrackAudibility`, review 1: the
  header's M and S after *Freeze track*). A frozen track locks its source, and the sources
  and mixes of what its modulators follow. **Moving a strip or a track** is applied and
  checked like a global command (the order of a bus's sums is the desk's).
- A touch inside a lock is refused with `CommandError::Frozen(target)` before anything is
  applied. **Global** commands (the tempo and the metre, the pan law, a track or strip
  inserted or removed, a whole project replaced, the master — in a region only through a
  follower) are applied and **checked**: if a frozen key moved, the inverse is applied and
  the command refused.

The invariant — *after any command, applied or refused, every frozen target's stored key is
its content key* — is what the undo fuzz checks after every one of its commands (arm 47
freezes, unfreezes and tries wrong keys; the other 47 arms run against the frozen
regions). Freezing itself is validated: a `SetStripFrozen`/`SetTrackFrozen` with a key that
is not the target's is refused. Freezing **disarms** the region's tracks in the same undo
step (a take could not land on a frozen track); a track that monitors its input *On*
cannot be frozen (a render cannot hold a live input; the notice says which track).

The interface says a refusal on the status line and in the corner card with **Unfreeze**
(`views::freeze_view::card`); its words say what stays live (the fader, or only the track's
level when the fader is heard — `live_words`, review 2).

**A plug-in's own window** is no command, so the lock meets it in the host (review 2): a
change made there to a locked unit — an insert in a frozen region, a frozen track's
instrument — is given back (`PluginHost::refused_by_freeze`,
`instrument_refused_by_freeze`: the project's state and values sent to the plug-in again)
and said as a refused command is (`HostNotice::Frozen` → the card with **Unfreeze**). A
render is made from the states the project stores (`render_units_stored`,
`render_instruments_stored`), never from what a window holds, and a save never writes a
window's state over a frozen region's (`project_for_save`, `instruments_for_save`).
Review 3: **a state of its own** that moves no value (a sample its browser loads:
`StateDirty`, the values unchanged) is refused, given back and said the same way, and
counted as an edit where nothing locks it; the give-back is made **once per frame, and at
the end of the plug-in's own gesture** while one is open (`give_back_now`: a drag reloaded
the whole state for every value); and a locked plug-in that loads again (it rested, its
render is gone) loads **the state the project holds**, never one parked when it was
unloaded (`load_for` / `load_instrument` with `locked`). Review 4: **the echo of the
host's own load is no edit** — a plug-in that says *dirty*, or asks for its values to be
read again, as it loads (JUCE's VST3 wrapper does) was given back, and the refusal said,
every frame. What it reports is nothing of its own when it is the project's state, what it
held just after the host's last load, what it said last, or its first report after a
give-back (`session::Echo`); and a whole-mix region's master plug-ins have their states
written into the project at the freeze like the region's own (`hosted_states`).

**Solo.** Solo is no part of a frozen sound: renders (and bounces) are made **unsoloed**
(`freeze::neutral`; review 1: a solo elsewhere rendered silence under the unsoloed key, and
a bounce under a solo muted the original for good). Live, a solo plays a frozen target as
live would (`freeze::solo_play`, review 2 — it replaces `silenced` — and asked of the root
first since review 3): **`Silenced`** when the solo or a mute silences the root — a frozen
strip not audible on the desk, or with no audible track or strip bringing it sound (a solo
elsewhere; its only input muted or another track soloed): the render muted, what live
would give but for its own tail, **nothing woken** (review 3: any solo elsewhere woke every
frozen bus with a key or a sub-strip, and each toggle loaded and unloaded its plug-ins); a
frozen track that is not heard plays its render all the same (its mute and solo are live
controls after it). **`Live`** only when the solo silences part of what is heard — a track
or strip inside whose sound reaches the render (a key or a note source silenced changes
nothing heard: `Render`): the render cannot follow that, so the region plays live until the
solo is lifted (`Snapshot::compile` and `FrozenStore::plays` take it as thawed; nothing
changes in the project). **`Render`** otherwise. While a target plays live its render
stays attached, its reader's board kept at the playhead (`FrozenPlay::live`, the slots'
`parked`), so going back to the render drops nothing (review 3: it was closed and opened
again at the playhead). Freezing a
region with a track or strip **soloed inside** is refused (*unsolo it first*: its render
holds all of it), and so is freezing a soloed track whose own sound a modulator of its
region hears (`hears_root`: its solo would be part of what is heard); after, those solos
are locked. Strip solo is the strips' own.

**Pattern mode** plays every chain live (`pattern::audition` thaws): a render is the song's;
the renders in use are held open meanwhile (`FrozenStore::hold`, review 3), so leaving
pattern mode drops nothing.

### 4. Playing a render

`EngineHandle::compile` attaches the renders at hand (`FrozenStore::attach`) and settles the
snapshot (`freeze::settle`):

- A frozen strip that plays its render **loses its chain in the snapshot** (inserts, EQ,
  polarity, swap — the render holds them) and **gathers nothing** (`graph::plan`: inputs
  into it are dropped). In `run_strip` it reads its render's frames at the transport into
  its pair; width, pan, fader and the rest follow — the same arithmetic as live.
- **What rests** (`freeze::dormant`): every node of a playing frozen strip's region whose
  sound reaches **nothing but** playing frozen strips (review 1: a key strip that also
  plays to the master, a strip that also sends into a frozen return, a note source that
  also plays through its own strip went silent). A node runs when its output or an enabled
  send reaches the master or a strip that runs and plays no render (a send that is off
  carries nothing: no way out — review 2), or when something live needs
  it — an insert keyed from it or hearing its notes on a strip that runs (or the master), or
  a modulator driving something live that follows it or hears its notes — and what a
  running node gathers runs too, to a fixed point. A track needed **only for its notes**
  (an insert or a modulator that runs hears them; its sound reaches nothing live) runs for
  its notes only (`Dormant::notes`, `TrackSnapshot::notes_only`, review 2): its events reach
  their listeners, its instrument is not played (no voice: review 3 counts them), it has no
  output, and it lets its reader board go (review 3: 32 blocks stayed pinned). A resting node is not
  run (`TaskMap::rest`, `run_node` returns at once), delays nothing (`compensate` skips it),
  and its meters read zero; a frozen node that starts resting releases its reader board.
  Its hosted plug-ins are **unloaded** (`PluginHost::sync` and the snapshot drop them; ADR 0063: an isolated one's helper ends when empty; the state is
  parked and the project holds it — the freeze writes the live states into the project in
  its own undo step).
- A frozen track plays its render in place of its instrument and clips; its notes still go
  to the inserts that hear them.
- **The latency is kept.** Frame *f* of a render is what reached the tap at the transport's
  frame *f* — the region's compensation and the chain's latency included — and the frozen
  strip (or track) reports that latency as its own. Every frame then meets its fader, its
  pan and their automation at the sample it did live, so the mix is the live mix **to the
  bit, before compensation too** (trimming it would have moved the fader's automation
  against the music by the chain's latency: the first design did, and the null test caught
  it). Freeze frees CPU, not latency; §4c's recording mode may trade that later. One render
  holds one latency: a region whose latency **moves during the song** (an automated
  look-ahead, a plug-in reporting a new figure, a delay line the plan could not serve)
  fails its render (`RenderError::LatencyMoved`, said on the status line:
  `freeze.latency_moves`, or for a bounce `bounce.latency_moves` with its track's name —
  review 4) — review 3: it kept the first figure, and played off after the move.
- **Streaming** (ADR 0050): a render is a stream (its head in memory, its blocks in the
  shared pool); a frozen node's board points the reader at the block it reads with the most
  a cursor asks ahead (sixteen blocks, about 1.4 s at 48 kHz) and keeps the loop's start
  ready in a second cell; stopped, it points where play will start; an offline read waiting
  for a block points at it again. An **offline render reads with a board of its own**
  (`compile_for_render`), never moving live playback's cursors (review 1). A node that lets
  a render go clears its board on the audio thread; the store lets go of renders the
  project no longer uses (their last owner is always a snapshot freed on the interface's
  thread); a render whose stream failed (a damaged file) is missing — made again, never
  opened again while the file is that one, and said nowhere as a damaged sample. A
  **passing read error** (`Io`: a network drive's hiccup, a file held by a scanner) is read
    again — four tries, waiting 0, 50, 200 and 1 000 ms (review 3: one immediate try) —
  before the render is given up, the error the copy's that was opened (review 4: the
  first copy found was blamed, and a good one set aside); given up, the next scan looks at it again as a file that
  may come back, never making it again; and the file is kept: only a damaged or vanished file
  fails it, the store never deletes a file it finds, and a render that fails asks for a new
  snapshot at once (`FrozenStore::take_lost`) so its target plays live meanwhile (review 2:
  one hiccup failed a render for good and deleted it). A node
  that starts or stops resting or playing a render is **reset** (voices, inserts, EQ):
  nothing from before sounds again when it thaws during playback; a frozen or resting
  track's instrument is never played, not even at a loop's start. The modulator trace
  (*Bake to automation*) reads renders offline too. Allocation- and lock-free on the
  audio thread and the workers (`rt_memory::frozen_playback_never_allocates`). In real
  time a block not there yet is silence, counted (`stream_late`); offline (export, stems,
  a freeze or bounce render: `Processor::set_offline`) the pass waits for it.
- Stems: a strip or track inside a frozen strip has no stem of its own while it is frozen —
  its sound is in the frozen strip's. A track's stem is trimmed by its track's latency (a
  bounce's kept delay, §6) and a *Direct* stem by the master's arrival: both on the grid
  (review 3: a bounced track's stem came out late).
- **A loop's wrap is not carried** (by decision, review 2): a render is one straight pass,
  so after a wrap the frozen target plays what led into the loop's start in that pass (the
  bars before it, their tail, the chain's latency), not the loop end's tail and delayed
  frames carried across the wrap as live does. Ableton's, Logic's and Cubase's freezes
  share it; unfrozen (or with the region thawed by a solo), the wrap is exact.

### 5. The render

`cantus_engine::freeze::render` drives the processor offline, as `render` and `stems` do,
with a tap where the frozen sound ends (`StemTap::with_pre`: a strip's pair before its
width, a track's source before its level) and **everything outside the region resting**
(`freeze::isolate`), so only what makes the sound is computed. It runs from tick 0 to the
song's end (or the loop's, if later), then on until the tap has been **exactly zero for one
second** (denormals are flushed: a tail ends in zeros) or for 30 s; the frames kept end at
the last one that was not zero. Frames go to a raw file as they come and the render file is
written from it in blocks — never whole in memory. Hosted plug-ins render through
`render_units` / `render_instruments` (fresh instances, where they run live; isolated ones
wait without a deadline, ADR 0063).

**Render first, freeze after.** *Freeze* renders on a worker (the corner card shows how
far, with **Stop**: nothing changes); when it is done the freeze is one undo step. An edit
made while it rendered moves the key: it is rendered again for what it holds now; one that
makes it unfreezable (a live input, a solo inside) is said. A plug-in turned in its own
window or browser while it rendered is rendered again when the render lands: the host
counts those edits (`PluginHost::edits_of`: values, presets, a state of its own) and
`commit_freeze` compares the count, never the states' bytes, which automation and
modulators move (review 3: a freeze of a plug-in so driven never landed, each try leaving a
render behind). An armed track is keyed and rendered as it will be frozen — disarmed
(`as_frozen`, review 2: it rendered twice). A bypassed insert, or one on a strip whose
inserts are off, is not heard: its plug-in is not required — the render keeps a wire in its
place, and notes it beside the render (`<render>.wires`, the plug-ins not installed then):
once one is installed, the render is made again (review 3: live and frozen differed by its
latency) — whichever copy's note says so, every copy then set aside (review 4: a save's
copy had no note, and was played for ever). A target resting inside
another's playing render is not refreshed until it wakes. A render waiting (behind an
export, a bake, files loading) shows its card, with **Stop** and how many more wait; Stop
pressed while its file is written commits nothing (the writers look within a block); and
nothing starts once the session closes (`FreezeState::closed`). One render runs at a time,
never beside an export or a modulator bake (their hosted instances are one set; an export
stops a background refresh, made again after it), never while a file it plays is still
loading. **Its plug-ins**: only those the render runs are instantiated; a render that needs
one that is not loaded is refused, naming it — a pass-through render is never written under
the true key (review 1). A render belongs to the project it was asked in
(`CantusApp::epoch`): another project opened drops it. A frozen target nested in another's
region renders with the outer one thawed. **Stop** stops background refreshes too, until
the user freezes or unfreezes. A stored key that is not the target's any more is frozen
again before its render is made. The progress counts the tail at its longest. Half-written
files are tidied at start and when the folder changes — the renders' folders, and a
bounce's in the project's `Audio` folder and the recordings' scratch folders (review 2) —
and on quitting the render is stopped; Cantus's cache keeps to 4 GB (the oldest renders
go).

### 6. Bounce in place

On a **new track right under the original**, at the same place (Ableton's, FL's), the
original **muted** — or removed, the user's choice, remembered (`Settings::bounce_removes`):

- *Bounce in place*: this track alone through its strip's chain up to the width (the
  strip's other inputs resting, its keys and note sources running but routed away from the
  tap); the new track feeds a **new strip** taking the old one's width, pan, fader, mute,
  solo, output and sends (Bitwig's *post-chain, pre-fader*). Refused when the strip keys
  something or a modulator follows it — unless the track is all it gathers and the whole
  track is bounced: then they follow the new strip.
- *Bounce in place (source only)*: before the track's level; the new track takes its level
  and balance and **feeds the same strip** — the song sounds the same, to the bit.
- Ranges: the whole track, the **selected clips** (only those render; they are the ones
  muted or removed; clips on several tracks: one bounce per track), or the **time
  selection** (its edges moved out to the notes held across them and past a comp seam's
  crossfade, to a fixed point — review 3; the clips it crosses cut
  there — at both edges, from the project at commit; the render plays the track cut at the
  edges and the song **plays on** while the tail rings out, its lanes moving as they do —
  review 2: the transport stopped at the end and the tail met still lanes; muted clips in it
  are kept). A clips or time range is refused while something hears the track's notes (its
  muted clips would send none: `bounce.notes_heard`; the whole track can be), and chosen
  clips that take one window of a comp seam and not the other (`bounce.seam`: its crossfade
  would become a cut — review 3). The keys take
  the time selection when it was chosen last, a track header chosen last takes the **whole
  track** (`SelKind::Track`, review 2: its first clip only), and a strip chosen on the desk
  takes the strip whole; a clip's own menu bounces that clip, whatever was chosen on the desk
  since (review 3); the master is never frozen nor bounced (`freeze.master`). The
  strip menu bounces a whole strip (everything it gathers, the new track placed after the
  first track whose sound reaches it; the original strip muted, its keys and followers
  moved to the new one); it is refused while a track or strip inside is soloed, and the new
  track takes only the strip's own solo. **What goes with it** (review
  1): the automation and modulators of what the bounce replaces (the track's level and
  balance; the strip's fader, pan, width, sends) are carried; a soloed original hands its
  solo over; what drives a carried or rescued modulator's amount comes with it (its
  `ModAmount` lanes and routes, review 3); removed, the original's other automation moves to
  the new track — and an
  original whose notes something hears is muted, not removed, and the result says why
  (`bounce.done_kept`). **A render whose sound changed since it started is made again**
  (`freeze::region_key` of what it renders, compared when it lands — review 2: any edit
  anywhere made it again). A muted track or strip is refused (a bounce already replaced
  it), so is a live input anywhere in what the render plays — the track's, a key's track, a
  note source (`bounce.live_input`: the render cannot hold it; review 3: only the tracks
  whose sound reached the strip were asked) — and a bounce running or waiting is not asked
  twice. Its refusals are **asked again when it lands** (review 3: a solo set while a strip
  rendered silenced the song). A file the render plays that is there but unreadable is
  refused by the engine too (`missing_file` at the render's start), and an offline compile
  waits for the files it asked to load in the background (`SampleCache::wait_audio`) —
  review 3: such a file was bounced as silence. Bounced unsoloed, its
  source with what its modulators follow, **played from tick 0** with the frames before the
  range dropped (`TapOptions::from`, review 2: a free LFO started in another phase). Written
  under a name of its own and renamed when whole. A time range cut inside a note's tail
  sums that tail and the bounce apart (float order: within −120 dB, from the source and
  through the inserts); a whole track or strip stays exact to the bit.
- **The audio on the grid, the delay on the track** (review 3; the first design trimmed
  the file and kept nothing, so the rides and keys carried over acted early by the path's
  latency; review 2 kept the frames late in the file, so the clip played late wherever it
  went — moved, copied, pasted, exported — and the track's other clips early): the file is
  **trimmed to the grid** (`TapOptions::bounce`), and the new track keeps the path's delay
  as **a real delay before its level** (`Track::lateness { frames, rate }`, **schema 38**:
  `migrate_37_to_38` writes nothing; scaled to the session's rate; in the engine a source
  delay line, `pdc::Edge::Source` / `TrackSlot::source_line`, reported as the track's
  latency and compensated as any; a monitored input joins after it). So its level, the new
  strip's fader, pan and sends and the keys it drives meet each frame at the sample they did
  before, to the bit; every clip on that track — the bounce's, a take put there — is
  delayed alike, and a clip taken elsewhere (moved, copied, pasted) or exported (DAWproject
  has no such delay) plays on the grid. The wait is **planned on the whole song** before the
  render (`freeze::render::strip_arrival`: the loaded hosted plug-ins' reported latencies,
  `EffectUnit::latency_stand_in` for the rest, an instrument's hint as its track's delay)
  and kept **before the chain** (`TapOptions::arrive`, `Processor::set_arrive_floor`) —
  review 3: it came from live meters, zero without a device, and was added after the
  chain, so the strip's automation met the bounce early; it does not depend on whether
  low-latency monitoring is on (the plan is made without it). The inspector shows the
  delay (*Delay*, with *Clear*: `Command::SetTrackLateness`, locked like the track's source
  in a frozen region). A delay no bounce could keep (a rate outside 8–768 kHz, more than
  10 s) is refused by the commands and dropped on load (`Project::repair_lateness`, said:
  `freeze.lateness_repaired`).
  The tail is kept as for a freeze; the audio clip is rounded up a tick so it never ends
  before its file. 32-bit float WAV in the project's `Audio` folder. One undo step. A
  track a freeze holds is refused (the notice says to unfreeze it first).
- *An instrument track becomes an audio track when asked*: bounce with *remove the
  original*.

### 7. Low-latency monitoring (§4c step 2)

A **session setting** (`Settings::low_latency`, `low_latency_ms` — no schema change: it
belongs to the machine and the moment, not the song), a switch where a recording musician
looks (the record options, the audio chip's popover, the inspector's recording rows) and a
key (`Alt+Shift+R`). Default limit **3 ms**: above Tonus's 2.7 ms (the voice's own tuner
stays), below every linear-phase or look-ahead mastering unit. Engine:
`EngineMessage::SetLowLatency(Option<u32>)`, planned in `Processor::compensate`
(`processor_monitor.rs`):

- **Monitored**: a track whose input monitors *On*, or *Auto* and armed — the rule that does
  not change when recording starts, so nothing re-plans under a take — and an **armed track
  with no audio input** (its notes played live into its instrument; review 1). The
  **monitored path**: its strip, what that strip feeds (outputs and sends), the master.
- **Hard bypass**: an insert on the path (the master's included) whose latency is above the
  limit is not run, has no latency and no dry line (`ll_off`) — unlike ADR 0022's soft
  bypass, on purpose: the point is the latency. They are named on screen (*Monitoring
  without: Aequalis, Culmen*, "and 3 more" past eight) from `EngineStats::low_latency`. One
  coming back from a hard bypass is reset (nothing from before it sounds), and inserts
  after one are dated without its latency. The flag goes with its unit, not its place: a
  chain edited while one is bypassed keeps it on that insert (`rebuild_chain`,
  `install_master`; review 2: the insert after it lost its tail).
- **Constrained compensation** (Cubase's): a strip that carries **monitored input only**
  (the singer's own strip, a bus of monitored strips) is not held back — its output and
  sends want no delay and do not set anyone's arrival — while the backing, aligned among
  itself, sets the master's. **A bus that mixes monitored input with backing keeps the
  backing aligned**: the monitored input waits there with it — a strip of monitored input
  only that plays into such a bus raises its arrival and waits on its line (review 1: it
  arrived early) — so to hear yourself soonest, monitor through a strip of your own. A strip
  of monitored input only still knows how late what it carries is (review 2): a monitored
  instrument's own latency raises its strip's arrival, and one such strip raises the next's
  — its line still wants nothing. The armed track's own clips
  go the monitored way too (heard early while monitoring, as Ableton accepts).
- **The take is placed to the sample**: what is recorded is the raw input, placed by the
  backing's latency — **captured when the pass starts**, not read at its finish (this
  amends ADR 0025 §7): a mode switched, or a bypass restored on stop, must not move the
  take. The mark carries the figure itself (`RecordMark::Started { tick, plugin, queue }`:
  the plan's heard latency and the input ring's wait at that block — review 2: it was the
  previous callback's), and a heard latency that moves **during** the pass (the mode
  switched, an automated look-ahead on the backing) moves what follows in the file with it
  — frames dropped, or silence written (`Live::shift`) — so every part of the take sits on
  the backing it was played to. Review 3: the move is applied **a round trip later**, when
  the musician hears it (`EngineMessage::SetDeviceRoundTrip`, sent as the devices are
  synced; a queue of moves due), never at the re-plan block; **a loop's wrap carries it**:
  each later pass is placed by the moves made before it (`RecordMark::Wrapped { shift }`,
  `FinishedTake::wrap_shifts`); and the recovery journal keeps the figures the engine
  marked for the pass — its start's latency and its wraps (`JournalPass::marked_wraps`) —
  not ones read later. The interface places takes by `EngineHandle::pass_latency`.
- **The audio chip** shows the monitored path's round trip while it is on: the driver's
  output, the monitored chain's (`LowLatency::monitored_samples`) and — for an audio input
  only (`LowLatency::audio`; review 2: notes played in come in no ring) — the driver's
  input and the input ring's wait; with both an audio input and notes monitored, the slower
  of the two ways (`LowLatency::monitored_note_samples`, review 3: the input's wait was
  added to the notes' way); measured through the planned delays (the instrument's,
  each chain's, each line's), not guessed (review 1: wrong through a bus).

## Measurements (Windows, the machine shared with other builds; best of five)

- **CPU freed**: a strip with a heavy Wavetable (32 voices, thick settings) and two inserts
  — 300 blocks of 512 frames played serially: **2.477 s live, 0.203 s frozen (8 %)**
  (`freeze::what_only_reaches_a_frozen_strip_is_not_run`, asserted below 60 %).
- **Low-latency monitoring**: a voice through Tonus with a linear-phase EQ on another
  strip: heard **128 samples (2.7 ms) late instead of 4 608 (96 ms)**; with an EQ on the
  voice's own strip too, it is bypassed and named, still 128.
- **Take placement**: identical, to the frame, with the mode on and off, and for what was
  played before the mode is switched off mid-pass (the master's look-ahead limiter
  returning) — without the capture at the start the take was 330 samples off (seen
  failing). Review 2: with the backing's heard latency moved mid-pass (a linear-phase EQ
  taken out), a click played on beat 2 before and one on beat 4 after both land where they
  were played, to the frame (without the shift the second was the EQ's latency off).
- **Bounce through latency** (review 2): Inserts and Strip bounces of a track through a
  linear-phase EQ, with a fader ride and a key carried, play the song to the bit (trimmed,
  the ride landed the EQ's latency early). Review 3: with the file on the grid and the delay
  on the track, the same song to the bit, and the bounced clip moved or pasted to a plain
  track the song to the bit too (it was the EQ's latency late); through DAWproject and
  back, within 1e-4 of the song; a frozen strip's note source running for its notes only
  plays **0 voices** (300 blocks).
- Real-time streaming at twice real time: no late block; the output equals the offline
  render.

## Tests (what each holds)

- `cantus-core/tests/freeze.rs` (34): the key's form and stability through the file; it
  changes for what changes the sound and for nothing else (fader, pan, width, mute, solo,
  output, names, colours, others); freeze/unfreeze one undo step each, exact; a wrong key
  refused; a frozen strip refuses inside its region and names itself, allows its live
  controls and everything outside; a frozen track keeps its level live; freezing disarms
  in the same step and refuses a live input; freeze on a track picks strip or source; what
  rests and what a live insert wakes; settling a snapshot; solo; pattern mode; modulators.
  Review 1: a frozen return or keyed bus never silences what also plays live; a live
  follower wakes a region strip and what it gathers; a frozen track's region takes in what
  its modulators follow; a follower of the root locks its fader; an only input keeps mute
  and solo live; a solo inside refuses the freeze; moves keep keys or are refused; a take
  moved by saving re-keys its freeze in the same step. Review 2: a follower of the bus the
  frozen strip plays into locks its fader; a frozen strip's only track moves among the
  others; a solo that silences part of a region plays it live; a soloed track its own
  follower hears cannot be frozen; the pan law in a heard root's key; an off send and a
  note source wake nothing. Review 3: a solo on an unrelated strip silences a keyed frozen
  bus without waking it; muting a keyed frozen strip's only input silences it; a frozen
  track not heard plays its render; a note source silenced wakes nothing; a target played
  live keeps its render in the snapshot; `keys_in` sees removed tracks, strips and replaced
  projects; a delay no bounce could keep is refused and repaired; a track's delay is locked
  inside a frozen region.
- Undo fuzz arm 47 and the driver's invariant check after every command (all seeds); since
  review 3 it also sets, clears and tries impossible track delays (`SetTrackLateness`,
  applied and refused counted), drawn from a generator of its own.
- `cantus-project`: `schema_37_keeps_frozen_strips_and_tracks` (the fixture's stored keys, as
  written, are the content keys), a hand-edited key repaired; `collect::undo`;
  `schema_38_keeps_a_tracks_lateness_and_its_keys_as_written` (fixture v38);
  `an_out_of_range_track_delay_is_dropped_on_load` (review 3).
- `cantus-engine/tests/freeze.rs` (44): the frozen mix is the live mix to the bit with its
  fader ride, pan, width, send level and mute moved after freezing; unfreezing gives the
  live chain back to the bit; block sizes 1, 37, 64, 512; a frozen track's source with its
  level and balance live; a latent insert frozen away (compensated and not); no render →
  live; the render key and streaming (only the head in memory); the tail rings out;
  cancelling writes nothing; pool = serial at W 1 and 3; real time = offline with the reader
  ahead; the CPU freed. Review 1: a frozen return and a keyed bus (and a note source)
  leave their sources playing, to the bit; a solo when it froze is not in the render; a
  nested strip's render made under the outer one; an offline render and live playback each
  with their own board; an offline start just before a block boundary; thawing while
  playing leaves no voice; a frozen track at a loop's start is not played; the modulator
  trace of a frozen strip; renders let go; a damaged render is missing. Review 2: a solo
  that silences a key plays the frozen bus live; a passing read error keeps a render and
  its file (`inject_io_fault`); thawing while playing resets the EQ, and a resting strip's
  echoes; a note source resting but for its notes keeps the mix; a missing file makes no
  render; a cancelled write leaves none; a damaged render is opened, fails and is made
  again. Review 3: a track's delay plays its source late and the mix on the grid; track
  and Direct stems on the grid with a kept delay; a latency that moves during the song is
  no render; a solo that thaws a frozen strip while it plays drops nothing when lifted; a
  read error that outlasts its first try keeps the render playing, and one that outlasts
  every try is looked at again, not made again; a notes-only track lets its blocks go and
    plays no voice; an offline compile waits for audio loading in the background. Review 4:
  a muted pad's missing file refuses no render, a layer's does; a read error on one copy
  never sets a good copy aside; a damaged copy is passed over for a good one; a render
  copied under a new key opens under it, and its alias plays meanwhile.
- `cantus-engine/tests/low_latency.rs` (14: review 3 adds a loop pass after a heard
  latency move landing where it was played, a move felt only a round trip later, the
  input's and the notes' monitored figures apart; review 2 adds an insert after a hard-bypassed
  one keeping its tail through a chain edit, a voice two pure buses down, a take that
  follows the backing when the heard latency moves mid-pass; review 1 adds the wait measured through a pure and a
  mixed bus, an insert back from a hard bypass, an armed instrument track, the dating after a
  bypassed insert, every bypass counted), `fx_tonus` (the old test kept for *off*, a new
  one for *on*), `rt_memory::{frozen_playback_never_allocates,
  low_latency_monitoring_never_allocates}` (serial and pool).
- `cantus-sandbox/tests/freeze.rs`: a frozen chain with the hosted test gain (in process and
  isolated) renders through `render_units`, is unloaded, plays the live mix to the bit, and
  comes back thawed; a change in a locked plug-in's window is given back and said. Review 3
  (9 more): a change the host never heard of is neither saved nor rendered (the state's
  bytes); a render's instances hold the project's states (`render_states_for_test`); a
  state of its own in a locked insert or instrument is given back and said, never saved nor
  rendered; a locked insert or instrument comes back with the project's state, not a parked
  one (a tag no value covers tells); a drag a freeze refuses gives back once, at its end;
    edits count what windows do, never what automation does; a solo elsewhere toggled never
  loads a frozen strip's plug-in. Review 4 (4 more, with the test plug-ins' *dirty* tag): an
  echo of the host's own load is no edit; a locked insert (in process and isolated) or
  instrument that echoes its give-back is given back once and said once; a locked one
  loaded again is left alone.
- `cantus-ui/tests/freeze.rs` (86): Freeze renders then freezes in one step, the mix equal;
  a refused edit's card with Unfreeze; Stop changes nothing; an edit during the render is
  rendered again; reopened elsewhere it plays live and remakes its render; both bounces
  lay the audio under the original and the song is the same to the bit; selected clips;
  low latency's key reaches the engine; both languages. Review 1 (22 more): a time range
  cut at both edges, notes held across its edges, muted clips kept; a latent chain; solo
  (elsewhere, and handed over); an edit while bouncing; a bounce asked twice; rides
  carried; a key handed over; another project; *Freeze track*'s header M/S; saving a
  frozen take; Stop and an export against a refresh; the key on what was chosen last; the
  time selection made last and every track's clips; a freeze that cannot land says why; a
  missing plug-in; a render older than its files; the folder before the first compile;
  tidy and prune; a bake waits; no leak into an Inserts bounce. Review 2 (23 more): a
  missing file refuses a freeze and a refresh waits; an armed track renders once; a nested
  strip is not refreshed while it rests; Save As brings the renders and prunes; half-written
  bounces tidied; a strip bounce's solos, live inputs and notes heard refused; a bounce
  through latency keeps its rides and keys in time (to the bit); a free LFO in phase; an
  edit elsewhere does not make a bounce again; *kept* said with the result; the keys take a
  chosen track or strip whole and never the master; Stop while writing; closing; a
  bypassed missing plug-in; a waiting card; the words of what stays live; the original's
  own rides and modulators; a damaged render made again; the chip for MIDI. Review 3 (21
  more): a bounced clip moved to a plain track, copied and pasted there, a take put on the
  bounced track, the bounced track through DAWproject — each on the grid (to the bit, or
  within 1e-6 / 1e-4 through the exchange); an Inserts bounce waits for a latent co-input
  before the chain; saving one project keeps another's renders beside it; a removed frozen
  track saved and undone finds its render; a save while a freeze lands keeps its render;
  Save As brings what an undo wants, off the frame; a take on a lane below with its file
  gone stops no freeze; a file there but unreadable refuses a bounce; a solo set while a
  strip bounce renders is refused when it lands; a live key source refuses a bounce; what
  drives a carried modulator's amount drives its copy; a comp seam's crossfade never cut;
  the clip menu's bounce takes its clip; the heard words name what is locked; one render
  waiting said in good Spanish, English naming the bake; a freeze lands though a plug-in's
  state moved without an edit; a render made with a wire is made again once the plug-in is
    installed; renders stay open while pattern mode plays. Review 4 (12 more): a freeze
  landing while a save copies keeps its render; two saves in a row keep what the later one
  wants; the close waits for the copy; a session cut short mid-copy finds the renders where
  they were; a renamed project finds its renders and its save brings them (the old folder
  tidied), a copied one leaves the original's alone; a Save As returns before its copies,
  the old folder looked in; a wired render saved elsewhere is made again once installed; a
  take moved by saving keeps its render under its new key, off the frame; a save keeps the
  render a waiting freeze of an armed track will land with; a state of its own changed
  while it renders renders it again (on a strip, and on the master of a whole-mix
  region); a bounce whose latency moves is said as a bounce. `tests/inspector.rs`: the
  *Delay* row drawn with every other (no collision, its figure whole at 220 pt) and
  allocating nothing of its own; `tests/final_look.rs`: its hints read whole in the hint bar.
  `tests/freeze_boot.rs` (2): the boot path (`assemble`) sets the folder and tells the
  engine; a frozen project opened at start finds its renders at once.

## Consequences

- Every new `Command` must say what it touches (`freeze_guard::touches`) — the build fails
  otherwise. Round 29's controllers builder's commands get theirs at the merge.
- `cantus-core` depends on `sha2` and `serde_json`.
- Schema 38 (`Track::lateness`): a bounce made through latency is a track whose source is
  delayed by it (review 3: the audio on the grid, the delay on the track — the same field,
  a new meaning before any release); an older build refuses the file.
- A loop's wrap through a frozen target plays the straight pass (§4); a solo that silences
  part of a region thaws it while it lasts (§3) — the CPU comes back when the solo goes.
- §4b's automatic background freeze can reuse the key, the store and the render as they
  are; §4c's recording mode is these two layers held as a mode.
- Not done, by decision: a frozen track's render does not follow a later tempo change
  (the tempo is locked while anything is frozen); hosted plug-ins that render differently
  offline than live would make a different render (none of ours do; ADR 0063's isolated
  renders are bit-identical).
