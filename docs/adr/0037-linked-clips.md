# ADR 0037 — Linked clips: copies that are the same clip

**Status:** Accepted · **Date:** 2026-10-09 · **Round:** 24 (linked) · **Extends:** ADR 0004 (commands), ADR 0006 (schema), ADR 0025 (audio clips), ADR 0027 (automation clips), ADR 0036 (clipboard)

> Numbering: written as 0037 and schema 21 on `r24/linked`; schema 22 at the merge, as
> the instrument calibration (ADR 0039, `r24/beta-rest`) took 21.

## Context

BACKLOG-excellence §6d.2, the owner's request (2026-10-07): *a copied instrument clip in
the arrangement is **the same clip**: editing one changes every copy (FL Studio's pattern
clips, Ableton's linked clips). "Make unique" detaches one copy; "Link to…" where it makes
sense. Audio clips too: copies share their settings (gain, fades, pitch, stretch/warp,
reverse, per-clip effects) until made unique. Shared clips show it on screen (a link mark,
a "×N" count, the selection highlights the siblings). Model change → schema bump; old
projects keep loading, every clip unique. Paste's default (linked or unique) is a
setting.*

How the model stores a clip today (`cantus-core/src/model.rs`): a `Clip` is a **window
onto its content** — `start`, `length`, `offset`, `content_length` (the loop brace) say
where it sits and what part of the content it shows; the content is `notes` (in content
ticks), or `audio: Option<AudioRegion>` (the file, clip gain, fades and their curves,
reverse, the sub-tick slip), or `automation: Option<Automation>` (target and curve). Every
reader — the snapshot compiler, the arrangement and piano roll, the clipboard, the MIDI and
DAWproject exporters, glue, split, the time-range crop, recording — reads `clip.notes` /
`clip.audio` / `clip.automation` directly, in about forty places across five crates. Split
already makes two clips that share their whole note list *and note ids* (that is what lets
glue put them back), so a note id is not unique across clips today.

Two ways to make copies share content were weighed:

**A. A content pool keyed by an id.** `Project.patterns: Vec<Pattern { id, notes, audio,
automation }>`, and a clip names its pattern. Content exists once; sharing cannot drift.
But every reader above must resolve the pattern — forty call sites, several hot (the
snapshot, the arrangement's per-frame miniatures) — and every command that edits notes
must address a pattern rather than a clip, while the piano roll, the clipboard, the
recorder and the undo history all speak in clips. The structural edits that are per clip
(split an audio clip and its fades move; glue two copies into one) would need a
copy-on-write pattern fork in each of them. A large, risky rewrite in a round where two
other builders share the tree.

**B. A link-group id on the clip, content replicated.** `Clip.link: Option<LinkId>`; every
member of a group holds the same content. Readers do not change at all. The cost moves to
the writers: an edit must reach every member, and nothing may let them drift.

## Decision

### 1. B: a link group, with the content replicated, kept equal by `Command::apply`

`Clip::link: Option<LinkId>` (`#[serde(default, skip_serializing_if = "Option::is_none")]`),
`LinkId` a new id type from `Project::alloc_id`. A group is every clip with the same id,
on any tracks and lanes.

**What is shared** (`cantus_core::links::ClipContent`): `notes`, `audio` (the file, gain,
fades and curves, reverse, slip), `automation` (target and curve), `name`, `color`. **What
stays each copy's own**: `id`, `start`, `length`, `offset`, `content_length`, `muted`,
`lane` — where it sits, which part of the content it shows, whether it loops, whether it
sounds. This is FL's split: a pattern's notes, name and colour are the pattern's; each clip
in the playlist is trimmed, slipped and muted on its own. Pitch, stretch/warp and per-clip
effects do not exist on audio clips yet; when they arrive as fields of `AudioRegion` they
are shared with no further work.

**Propagation lives in `Command::apply`, not in the callers.** Every command that changes a
shared field — `InsertNotes`, `RemoveNotes`, `SetNotes`, `SetClipAudio`,
`SetClipAutomation`, `SetClipName`, `SetClipColor`, and the new `SetClipContent` — runs its
edit on the clip it names, then the *same* edit on every other member (`edit_shared` in
`command.rs`). The inverse is computed from the named clip and reaches the siblings the
same way, so undo is exact by construction: before the edit all members were equal, the
edit does the same thing to each, its inverse does the same thing to each. No view, no
script, no AI tool can forget a sibling, because none of them propagates anything. A clip
in no group pays one `Option` check. A member that would refuse the edit (only possible in
a project that already broke the invariant) takes the named clip's content instead.

**Membership is guarded.** `InsertClip`, `InsertTrack` and the new `SetClipLink` refuse a
clip that names a group whose members it does not match (`CommandError::InvalidValue`);
`InsertTrack` also compares the inserted track's own clips with each other, since the
project does not hold them yet. So
the invariant — *every group's members have equal content, to the bit* — holds after any
sequence of commands; `links::check` states it and the undo fuzz runs it after every step
and every undo.

### 2. Two new commands, everything else is a batch of existing ones

- `SetClipLink { track, clip, link: Option<LinkId> }` — join a group (refused unless the
  content already matches) or leave one (keeps the content). `cmd.make_unique` /
  `cmd.link_clip`.
- `SetClipContent { track, clip, content: Box<ClipContent> }` — replace every shared field
  at once, on the clip and its siblings; inverse: the old content. `cmd.set_clip_content`.
  It does not check that an audio source is in the pool, like `InsertClip`: an inverse must
  always apply, and a missing file plays silence.

Builders (`cantus_core::links`), each a plain command list for one `Command::Batch`:

- **A linked copy** (`Linker::copy`): a fresh clip id, the same content — the same note ids,
  as the halves of a split share them — in the original's group; for an original in no
  group, a `SetClipLink` puts it in a new one first. A `Linker` remembers the groups it
  made, so one clip copied twice in a batch makes one group, not two.
- **Make unique** (`make_unique`): `SetClipLink None` for each selected linked clip. The
  clip keeps its note ids (re-iding them would change how a seeded instrument plays it,
  §7); the engine tells such clips apart without it.
- **Link to** (`link_to`): each selected clip of the same kind as the target (notes ↔
  notes, audio ↔ audio, automation of the same target) leaves its old group (so its old
  siblings keep their content), takes the target's content (`SetClipContent`, when it
  differs) and joins the target's group (a new one if the target is in none).

### 3. Structural edits decide for themselves

Propagation is right for *edits* (a note moved, a fade dragged, a curve redrawn, a rename,
a recording overdubbed into a pattern) and wrong for *structural* operations that change
one clip's content as a side effect:

- **Split** keeps both halves in the group when the split leaves the content as it was (a
  note clip: the halves are two windows onto the same notes, exactly as before). An audio
  clip with fades is different — its fades stay at the outer edges, so each half's region
  changes — and both halves leave the group first, so the cut never reaches a sibling.
- **Glue** of clips whose glued result changes the kept clip's content takes the kept clip
  out of its group first (halves of a split glued back change nothing and stay linked).
  The glued clip plays over the later clips what they played: only the notes a later clip
  *sounds* (inside its window, or on its loop) come in — a copy trimmed from the left no
  longer drops its hidden notes onto the first clip's own. A note whose id is already there
  at the same place is the same note (split halves); one the first clip holds only hidden
  while the later clip plays another version of it (an edited split) is replaced by the
  version heard (`SetNotes`); one both play (linked copies) comes in under a fresh id —
  before this, gluing two linked copies dropped the second's notes. The first clip's own
  hidden notes past its old end are kept as before and become audible when the window
  grows (unchanged behaviour, noted in the round report).
- **A content edit that grows the edited copy** — a note paste or a piano-roll Duplicate
  past its end, a MIDI overdub past it — changes that copy's window or loop so the new
  notes are heard. Windows are per copy, so `links::follow_window` adds the same change
  (moved to where each sits) for every sibling that showed exactly what the edited copy
  showed (same offset, length and loop: every copy nobody trimmed), in the same batch; a
  sibling trimmed by hand keeps its window, as FL keeps a resized pattern clip. Without it
  the notes reached every copy but sounded in one.
- **The time-range crop** (ADR 0036) writes a looping note clip's notes out once when a
  piece does not start on a repeat: that piece leaves the group. Recording's
  `record_pass::piece` (the same content through another window) keeps it.

### 4. Copies follow a setting, linked by default

`Settings::link_copies` (default **on**: the owner's words are "a copied clip *is* the same
clip"): **paste, Duplicate and the Alt-drag copy** all follow it, so the three ways of
copying never disagree. *Make unique* detaches any copy afterwards; with the setting off,
every copy is a clip of its own as before. The switch is a checked row of the arrangement's
menu (*Copies are linked*), remembered in `settings.json`.

### 5. The clipboard becomes `cantus-clip/2`

The payload's clips are the live model, so version 2 is version 1 with `Clip::link`. The
reader takes `cantus-clip/1` and `/2` (`clipboard::READS`; a v1 clip has no link and
pastes as a clip of its own) and refuses anything later, as before. A **linked paste**
joins a group only when it can honestly be the same clip: copied from this project in this
run (`ClipOrigin`), and the clip it was copied from is still here with the same content
(or, if that clip is gone, a member of its group is). Otherwise — another project, an
original edited since the copy, a source remapped — the paste is a clip of its own with
fresh ids, whatever the setting says.

### 6. What the screen shows

A linked clip's title bar shows a chain mark before its name and **×N** (the group's size)
at its right end. The other members of a selected clip's groups are outlined in dashes in
the selection's colour. The arrangement toolbar's link key is lit while the selection is
linked; pressing it makes the selection unique. The clip's right-click menu offers *Make
unique* and *Select linked copies* on a linked clip, and *Link selection to this clip* when
several are selected; the menu bar's Clip menu lists all three. Keys: **Alt+U** make unique,
**Alt+L** link the selection to the edited clip, **Shift+Alt+L** select linked copies (Alt
and a letter, so the typing piano never takes them).

### 7. The engine: correct and as cheap as copies

The snapshot compiles a linked copy exactly as an unlinked copy (each clip's own window over
its own replicated notes): no new code on the audio thread, nothing to look up. One thing
had to change. The voice tag (`NoteEvent::note`) is the note's id; it pairs a note-off with
its note-on, and it is also the seed of every seeded instrument (Pulsus II's noise and
variations, Calor, Gravis, Lamina, Modulus, Organum, Exemplar). Two linked copies that
overlap on one track share their note ids, so the first copy's note-offs released the
second copy's notes (found by a test, below).

The tag must therefore tell copies apart, and it must **not** depend on whether a clip is
linked: otherwise linking a clip (which every Duplicate does, by default) or *Make unique*
would change how an untouched clip sounds. The rule (`snapshot::tell_copies_apart`): on
each track, for each note id played by more than one clip, the clip with the lowest id —
the one that held the notes first, since ids only grow — keeps the note's own id, and every
other clip gets `voice_tag(note, clip)`: the id mixed with a SplitMix64 hash of the clip's
id, with bits 30 and 63 set, so it never equals a note's own id (ids count up from 1),
neither as Cantus's 64-bit tag nor cut to the 31 bits a CLAP or VST3 note id keeps
(`plugin_note_id`). Consequences: linking, unlinking or copying a clip changes no event of
the clip it was copied from; copies made unique, or taken out of a group on load, are still
told apart; a copy on another track plays its notes' own ids, exactly as the original. A
track with one heard clip, or whose clips share no note id — every project before this
round — compiles to the same events as before (the check is one sort of the played ids,
only on a track with two or more clips), so no golden hash or `DSP_VERSION` moves. The only
old render that can change is one that already had two clips playing the same note id on
one track (split halves both re-exposing a note), whose note-offs were already mispaired.

### 8. Schema 22

`SCHEMA_VERSION` 20 → 21, `migrate_20_to_21` writes nothing (a clip without the field is a
clip of its own — what every older clip was); the number is what makes an older build
refuse a project whose copies it would silently cut apart. `fixtures/v22.cantus` holds a
note pattern linked three times across two tracks (one copy trimmed and slipped) and two
linked audio clips. A loaded file whose group members differ (edited by hand) is repaired
on load: the odd member leaves the group with its content (`links::repair`,
`LoadReport::links_repaired`).

## Consequences

- Readers are untouched; old projects open with every clip unique and render to the bit as
  before.
- Memory and file size grow with the copies (each holds its notes), as they did with
  unlinked copies; a project file stores each copy's notes. A file format that stores a
  group's content once is possible later without a model change.
- An edit through a member costs one pass per member: moving one note in a 2 000-note
  pattern takes 3–5 µs alone and 190–285 µs with 63 linked copies (release build, a shared
  machine; 10 µs and 380 µs in the dev profile) — inside a frame, even mid-drag. Telling
  copies apart in the snapshot adds about 0.5 ms to a compile with 63 copies of that
  pattern on one track (1.07 ms in all).
- Note ids are shared across a group, as across split halves. Anything that needs a note
  id unique in the project must pair it with its clip (the engine's voice tag does).
- Seeded instruments draw from the tag: a later copy on the same track as the original
  chooses differently from the original (as an unlinked duplicate with fresh ids always
  did); a copy on another track chooses exactly as the original. Deleting the original
  hands the ownership of its note ids to the next copy, whose seeded choices then change.
- Two 31-bit plug-in note ids of different copies can collide with a probability of about
  one in 2^30 per overlapping pair; a 64-bit Cantus tag essentially never.
- Existing installs get the new keys: the keymap file goes to version 4, and a version-3
  file is filled in with Alt+U, Alt+L and Shift+Alt+L where those keys are free.
- A file whose group no longer matches is repaired on load, and the status line says how
  many clips were unlinked.
- Automation clips can be linked too (same target only), by the same mechanism.
- Left for later: an inspector row naming the group; a "Link to…" picker by group name;
  dragging a pattern from a browser of groups (FL's picker panel).

## What the tests hold

- `cantus-core/tests/links.rs` (17 + 1 measurement): a linked copy shares content and
  group; notes inserted, moved, removed, a rename and a colour through either copy reach
  the other, and undo walks every step back on both; each copy's window, loop, mute and
  lane stay its own; audio settings (gain, fades, curve, reverse) are shared and the
  snapshot plays both reversed; Make unique detaches one copy, its edits reach nobody, the
  others' do not reach it, and undo links it again; Link to gives a clip the target's
  content, keeps its window, leaves its old sibling with the old content, undoes exactly; a
  clip that does not match a group is refused into it (`InsertClip`, `SetClipLink`) and a
  removed sibling comes back linked on undo; `SetClipContent` replaces everything on every
  sibling; the snapshot plays a linked copy as an unlinked one, and an edit through the
  copy is heard in the original; one clip copied twice in a batch makes one group;
  `repair` takes the odd member out; a split keeps a note group and takes an audio clip
  with fades out, its sibling's fades unchanged; a linked paste joins the original's group
  (a new group for a unique original), pasting again joins the same group, one undo per
  paste; a paste with linking off, from another origin, or of a clip edited since the copy
  is unique; a linked paste finds the group when the original is gone; the voice tags do
  not depend on the link (the original's events unchanged by a linked copy laid over it,
  Make unique of both changes no event, a copy on another track plays the raw ids) and the
  copies' tags are distinct in 31 bits; an `InsertTrack` whose own clips disagree on a
  group is refused; a note paste past the end of a linked copy grows the siblings that
  showed what it showed, not one trimmed by hand, and undoes exactly. Watched failing with
  the propagation switched off (6 of 11 at the time), and the three new ones before their
  fixes.
- `undo_invariants.rs`: six new arms — linked copies (on their track or the next; one in
  eight spoiled so the batch must be refused), Make unique, Link to (and a bare
  `SetClipLink` into a group the clip does not match, refused), an edit through a sibling
  of a real group (notes, colour, audio, curve, whole content, name), linked pastes — and
  a track brought in whole with a link group of its own (one in three disagreeing, refused);
  the note-paste arm prefers linked clips half the time and adds `follow_window` — and
  `links::check` after every step and every undo. The coverage check asserts each case
  really applied across the seeds (a grown linked paste and a linked track included); with
  the `InsertTrack` guard removed, the fuzz breaks the invariant.
- `cantus-project` (`linked_clip_tests`): every fixture v1–v20 opens with every clip
  unique; v22 keeps its two groups (sizes 3 and 2, one of audio) through a round trip; a
  broken group is repaired on load.
- `cantus-engine/tests/linked_clips.rs`: a linked copy renders as an unlinked copy to the
  bit, and at block sizes 64, 512 and 997 identically; two linked copies overlapping on one
  track render as two unlinked ones (failed before the tag change: the first copy cut the
  second's notes); an edit through one copy renders as the same edit made by hand in each;
  on Pulsus II and Calor, a linked duplicate leaves the original's render bit-identical and
  Make unique changes no sample, also for copies laid over each other; overlapping copies
  made unique still render as two independent copies.
- `cantus-ui/tests/links.rs` (14): Duplicate makes a linked copy and a transpose through it
  changes the original; with linking off it is a clip of its own; Make unique is one undo
  step named for it and its edits stay its own; Link to makes the other selected clip the
  edited one, one undo; Select linked selects every copy; glue of two linked copies keeps
  both copies' notes with unique ids, leaves the group and does not touch the third copy
  (watched failing without the glue fix); paste follows the setting, through the events
  egui-winit sends, and the text is `cantus-clip/2`; the keys (Alt+U, Alt+L, no conflict)
  and every label in both locales; the menu's switch flips the setting and keeps it; glue
  of edited split halves and of a left-trimmed linked copy plays exactly what was heard;
  the piano roll's Duplicate past a linked copy's end grows every copy, one undo; the clip
  menu's *Link selection to this clip* and its Alt+L reach the clip right-clicked; a
  project with a repaired group opens with a status that says so. `keymap`: a version-3
  file gets the three link keys. `arrangement_paint`: only the dashes on screen are drawn.
  `app_flows`: the Alt-drag copy is linked with the setting on and independent with it off.
- `cantus-ui` clipboard codec: a version-1 text is read (no link), a version-2 text keeps a
  clip's link, version 3 is refused.
- Shots: `cargo run -p cantus-ui --example shoot_links -- <dir> [es]` — the arrangement
  with linked clips, the clip menu on a linked copy, the timeline menu's switch.
