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
  dragging a pattern from a browser of groups (FL's picker panel) — all three done in
  round 26 (the amendment at the end).

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

## Amendment — 2026-10-13, round 26 (linked): linking by name, the inspector's link row

**Context.** Two items from "Left for later" (BACKLOG §6d.2): *an inspector row naming the
group; a "Link to…" picker by group name*. Until now a link could only be made to the
clip being edited (Alt+L), so linking to a pattern elsewhere in a long song meant finding
it, selecting it last, and pressing the key; and nothing outside the arrangement's ×N said
that a clip was shared.

**Decision.**

1. **The list, in `cantus_core::links`.** `groups(project)` returns every link group once —
   its shared name, its kind (`Kind::{Notes, Audio, Automation}`, what `compatible`
   compares), its size, its first member (by track index, then start, then clip id: the
   clip a link copies, whose track and bar tell two groups of one name apart) — and then
   every clip of its own. Groups of two or more come first, then the clips of their own
   (a group of one, left after the others were made unique, counts as a clip of its own);
   each part by name ignoring case, then exactly, then by place. The order depends only
   on what the project holds, never on how its clips are stored. **Clips of their own are
   listed** because the first link of a pattern is to a clip that is in no group yet: a
   picker of groups alone would be empty in every project until something was linked by
   other means. The filter keeps a long song's list short.
2. **What would happen, before it happens.** `would_link(project, sel, target)` answers
   `Ok(n)` — how many selected clips `link_to` would put in the target's group — or why
   none (`Refusal::Kind`: another kind of clip; `Target`: automation of another control;
   `Already`: the target itself or already its group, or nothing selected; `Gone`). It is
   written beside `link_to` and must agree with it exactly: a core test checks every target
   against every selection of one or two clips in a scene, and the undo fuzz asserts it on
   every *Link to* step (half of them now aim at an entry of `groups`, as the picker does).
3. **The picker** (`views/link_picker.rs`, `link_picker_paint.rs`): a floating panel in the
   kit — title, a line saying what is being linked ("“Beat B” becomes the clip you
   choose"), a filter field (name or track, case and accents ignored, as the device
   picker's search), the list in a well under two bands (*Linked clips*, *Clips of their
   own*), two-line rows (name; kind and ×N at the right; the track and bar under it), and
   a line with the keys. A row that would link nothing stays in the list, **dimmed, saying
   why** instead of where it sits, and cannot be chosen; ↑ ↓ walk only the rows that link,
   Enter chooses, a click chooses, Esc or a click outside closes. Choosing runs the existing
   `CantusApp::link_clips_to(Some(target))` — `links::link_to` in one `Batch`, one undo step,
   no new command shape. It opens at its anchor (the inspector row), else at the pointer,
   else centred near the top of the window, and does not move while one types. Rows are
   built when it opens and again only when the filter or the project revision changes.
4. **Where it is reached.** A new action `LinkTo` (*Link to…*, clip category) — in every
   clip's right-click menu (a clip of its own as much as a linked one), the menu bar's
   Clip menu, the inspector's link row, and **Alt+N** (*N for name*; Alt and a letter, as
   the other link keys, so the typing piano never takes them; every clip action has a key,
   which a test holds). The keymap file goes to **version 7**; a version-6 file gets Alt+N
   where it is free. Ctrl+Alt+L was considered and refused: Ctrl+Alt is AltGr on many
   European layouts.
5. **The inspector** (`views/inspector_clip_link.rs`, called from the clip section):
   - **Name** becomes editable in place: a click turns the well into a field with the
     whole name selected (the field opens the frame after the click, so the click does not
     place the cursor); Enter or a click away renames, Esc leaves it, an emptied name is an
     abandoned edit. A linked clip's name is its group's: the well ends with the chain mark
     and ×N, the hint says that renaming renames every copy, and the rename is one
     `SetClipName` that `Command::apply` already carries to every sibling — one undo step.
     One edit at a time, for the clip it names; shown another clip, the inspector drops an
     unfinished one.
   - **Link**: on a linked clip, "3 linked copies" as a menu field whose list holds
     *Select linked copies*, *Make unique* and *Link to…* (the inspector's own menu, a
     `MenuId::ClipLink`); on a clip of its own, *Link to…*, which opens the picker at once.
     A row's action runs on the selection when the clip is in it, else on the clip alone,
     as the arrangement's menu does. The words come from the sheet's text cache, so a still
     frame formats nothing.

**Consequences.**

- Linking is reachable by name from anywhere; two groups called "Verse" are told apart by
  their track and bar.
- No schema change, no new command, no golden hash: the name is the shared
  `ClipContent::name`, and the picker issues the batch `link_to` always did.
- A clip of its own appears in the picker's list, so a song with hundreds of clips has a
  long list; the filter and the list's scrolling (the kit's scrollbar) are how it stays
  usable (and *Linked only*, added after the review, below).
- The keymap's file version moved (7); another builder adding a key this round takes 8.

**What the tests hold (round 26).**

- `cantus-core/tests/links_groups.rs` (7): every group listed once with its name, kind,
  count, first member and place, clips of their own after, two entries of one name told
  apart; groups first then each part by name (case ignored) then place; the order the same
  whatever order the clips are stored in; a group of one is a clip of its own, an empty
  project lists nothing; `would_link` agrees with `link_to` for every target and every
  selection of one or two clips; each refusal names its reason (kind, target, already,
  gone; a mixed selection links what it can); a rename through any member renames the
  entry, and undo brings the old name back. Seen failing against stubs (7 of 7).
- `undo_invariants.rs` (additive, no new arm — no new command shape): the *Link to* arm
  aims half of its steps at an entry of `links::groups` and asserts `would_link` agrees
  with the batch. With `would_link` broken (an *already linked* clip counted), the fuzz
  fails at once.
- `cantus-ui/tests/links_picker.rs` (13): the rows' count, kind, hint ("Drums · 1.1") and
  reason; the filter ignores case and accents and finds a track; with nothing selected the
  status says so and nothing opens; typing a name and Enter links the selection, keeps its
  place, one undo step that restores the project; the arrows skip refused rows and a
  refused row cannot be clicked (seen failing with the refusals switched off); Esc and a
  click outside change nothing; the inspector names a linked clip's group (×3, "3 linked
  copies") and offers *Link to…* on a clip of its own; renaming from the inspector renames
  every copy in one undo step, Esc leaves it (seen failing before the field opened a frame
  late: the click put the cursor at the end, "BeatGroove"); the link menu selects the
  copies, makes unique and opens the picker; no two texts collide in the clip section
  (220/280/420 pt) or the picker, English and Spanish, 75 % and 200 %, and the picker stays
  on a 1280×720 screen; the clip's right-click menu offers *Link to…* and it opens the
  picker for the clip right-clicked (seen failing before the entry); a still frame of a
  linked clip allocates no more than one of its own (9 and 9; 14 with the count formatted
  every frame — seen failing); Alt+N, no conflict, a version-6 keymap gets it, the label in
  both languages (seen failing before the binding).
- `app_flows::every_clip_action_has_a_key_bound_to_it` failed with the new action unbound,
  which is what decided Alt+N.
- Shots: `cargo run -p cantus-ui --example shoot_links -- <dir> [es] [light] [z0.75|z2]
  [kind=…]` adds `links-picker`, `links-picker-filter` and `links-inspector`, in Cantus
  Night or Cantus Paper and at any scale.

### After the review, and the owner's additions (2026-10-14, round 26)

An adversarial review (four lenses, a refuter per finding) confirmed nine findings; the
owner asked to close the "Left" items in the same round. What changed:

1. **Already wins** (`would_link`): in a mixed selection that holds the target or a member
   of its group, the reason is *already the same clip*, not *another kind* — the reason
   that names what the user sees. Order of reasons: already, another control, another kind.
2. **One summary of the selection** (`links::Selection`): the selection is counted once —
   deduplicated in a `HashSet`, per kind, per group (`(kind, target, LinkId)`), automation per
   target — and every entry is answered in constant time (`Selection::would_link(&Group)`),
   exactly as `would_link` (a property test over every entry × thousands of selections,
   and the undo fuzz checks it on every *Link to* step). `Group` gained `target` (what an
   automation entry drives). The picker keeps the list (`ui::links::PickList`, every row's
   strings made then) per project revision and language; a keystroke only filters it.
   Measured (release, 20 tracks × 100 clips, all 2 000 selected, the whole window's frame):
   opening **1 400 ms → 5.7 ms**, a keystroke **806 ms → 2.1 ms**, a still frame
   **10.2 ms → 1.8 ms** (closed: 1.7–2.0 ms).
3. **Only the rows in view**: the list is virtualised (rows of one height, bands of
   another, so a row's place is arithmetic; `ScrollArea::show_viewport`), sensed first and
   painted with one brush. A still frame allocates the same with 2 000 rows as with 12
   (28; 4 040 before). The arrows scroll the highlight into view.
4. **Decomposed names**: the filter folds both sides the same way — lower case, the
   precomposed accents mapped, and the combining marks U+0300–U+036F dropped — so
   "Melodi\u{301}a" (a macOS file name) is found by "melodia" and "melodía".
5. **Untitled** in the lead line and the hint bar too, as in the row.
6. The module doc says Alt+N.
7. **One well**: the filter's text edits over the whole well, inset by the kit's margin, so
   the focus ring outlines the well; the placeholder is the kit's text. The same for the
   inspector's name field. The field is shared (`link_picker::search_field`).
8. **The verb stays**: the lead line cuts the clip's name (the longest start that fits,
   with "…"), never "becomes the clip you choose".
9. **Natural order**: names compare with runs of digits as numbers ("Groove 2" before
   "Groove 10"; "02" equals "2", then the exact name decides), then place.

**Linked only** (the owner's): a switch beside the filter lists only the link groups. It
is **kept while the program runs, not saved**: it is a way of looking at one list, like a
filter, and the right default for a first link is *all* (a pattern's first link is to a
clip of its own); a saved switch would hide the clips a new session needs.

**Keys in the inspector's menus** (the owner's): a row that runs a keymap action shows its
binding at the right, as the arrangement's menus do; the link menu's three rows show
Shift+Alt+L, Alt+U and Alt+N, and a menu with keys is at least 260 pt wide. Every other
inspector menu row is a choice (an instrument, a route, a wave), not an action, so it
shows none — the rule is the same everywhere.

**The clip browser** (the owner's; FL Studio's playlist *picker panel*): FL keeps, at the
left of the playlist, a column of tiles — every pattern and clip in its colour, with a
mini preview — to click and paint, or drag into the playlist. Cantus's equivalent is a
dock pane, **Clips** (`views::clip_browser`, Alt+F5 — F5 is the arrangement, FL's
playlist — and View ▸ *Clip browser*), placed at the left of the arrangement's area:

- It lists what *Link to…* lists (`PickList` with no selection): the link groups under
  *Linked clips*, then the clips of their own, in the same natural order, filtered by name
  or track with the same one-well field. Each **tile** is FL's: a title strip in the clip's
  colour (its own, else its track's) with the chain mark for a group, the name and ×N; on
  the colour's darker ground the kind and a miniature — its notes as dashes over its
  length (at most 128), its curve as a line, nothing drawn for audio. Only the tiles in
  view are laid out.
- **Dragging a tile onto the arrangement places a linked copy** where it is let go: on the
  row under the pointer, at the time under it snapped down to the grid as the arrangement
  snaps — `CantusApp::place_linked_copy`, one `Batch` (`cmd.place_linked_copy`) of what
  `Linker::copy` returns and an `InsertClip`: the same shape as a linked Duplicate, so no
  new command and no new fuzz arm. A clip of its own and its copy become a new group. An
  automation tile lands under the track it automates, as its Alt-drag copy does. The copy
  is selected. While dragging, a chip with the name follows the pointer and a ghost in the
  clip's colour shows where the copy would land (clipped to the timeline); let go off the
  rows, nothing happens.
- A click on a tile selects its clips (every copy of a group). *(Changed below: a click
  chooses the paint source, a double-click selects.)*
- The keymap's version 7 also brings Alt+F5 to a saved map.

Why a pane and not a column inside the arrangement: the workspace is a dock (ADR 0024
§5), and every panel that lists things to drag — the devices, the samples — is a pane one
can move, resize and close; a column inside the arrangement could not be.

**What the tests hold (the review and the additions).**

- `links_groups.rs` (+3): *already* wins in a mixed selection; natural order with equal
  names kept by place; the selection summary agrees with `would_link` for every entry over
  every selection of one to three clips (duplicates, a missing clip, automation of two
  targets). The first two seen failing on the old code, the third against a stub.
- `undo_invariants.rs`: the *Link to* arm also checks the summary against `would_link`
  for every entry.
- `links_picker.rs` (+6): a decomposed name found both ways; *Untitled* in the lead line;
  a long name cut and the verb whole, in English and Spanish; the focused rect is the well
  with the text inset (seen failing with the old inset field); *Linked only* lists the
  groups, is kept for the next opening and Enter still links; the inspector's link menu
  shows its three keys without overlap (seen failing before).
- `links_picker_big.rs` (new, 2 + 1 timing): only the rows in view are laid out and the
  arrows keep the highlight in view; a still frame allocates the same with 2 000 rows as
  with 12; and (release, `--ignored`) opening, a keystroke and a still frame each under
  16.7 ms with 2 000 selected. All three seen failing on the old picker (2 000 rows laid
  out; 4 040 allocations; 1 400 ms).
- `links_browser.rs` (new, 6): the tiles (groups first) and the filter; a tile dragged
  onto the arrangement places a copy that is in the group, holds its content, starts at
  the snapped time on the row dropped on, is selected, and one undo restores the project
  (seen failing with the snap removed); a clip of its own starts a group; an automation
  tile stays on its track; a drop off the arrangement changes nothing; Alt+F5, a
  version-6 keymap gets it, both languages; no text collides in the pane, 75 % and 200 %,
  English and Spanish.
- Shots: `shoot_links` adds `browser` and `browser-drag` (a tile held over the vocal
  track: the chip and the ghost).

### Painting with the chosen tile, and waveforms on audio tiles (2026-10-14, round 26)

The two items the browser left open, closed in the same round at the owner's word.

**Painting** — FL Studio's playlist paint tool (B) with the picker panel's selected
pattern. The arrangement had no tool modes of its own (the tool keys were the piano
roll's), so the paint tool is added the way the rest of the arrangement works:

- **The source** is the clip browser's chosen tile: a click on a tile chooses it (the
  tile wears the selection's ring; a click again lets it go); a double-click selects its
  copies, as a click did before. With no tile chosen, the clip being edited becomes the
  source when the tool comes out; with neither, the browser is shown and the status line
  says to choose one.
- **The tool**: **B** with the arrangement focused (B is the paint tool in FL's playlist and
  piano roll; with the piano roll focused it is still its paint tool, so the key needed
  no new binding and the keymap file did not move for it), and a toolbar key with the paint
  icon, lit while it is out. **Esc** (with the arrangement focused; see the second review)
  puts it away and lets the source go. The hint bar
  names the source over the lanes ("Paint “Beat” · right-drag erases").
- **Gestures**, each one undo step made when the stroke ends: a **click** places a linked
  copy at the snapped time on the row; a **drag** paints copies end to end along the row it
  was pressed on — from the snapped press point, one clip length apart, as far as the
  pointer goes, left or right, never before the start — skipping any place where a copy
  would overlap a copy of the source already there or another of the stroke (*corrected
  by the second review below: it first skipped only equal starts*); a **right-drag** (or a drag with **Alt**) erases the copies
  of the source the stroke passes over on that row. While a stroke lasts, the copies to be
  placed show as ghosts in the clip's colour and those to be erased are crossed out.
- **Erasing only the source's copies — decided.** FL's paint tool erases whatever clip it
  passes over. Cantus's erases only the clip it paints (its group, or the clip alone),
  because the stroke is about that clip: a sweep across a busy track removes the pattern
  being laid down and never a neighbour by accident (the eraser of everything is the
  arrangement's Delete). When the source itself is erased another copy of its group takes
  its place; with none left, the tool is put away.
- Commands: `cmd.paint_linked_copies` is one `Batch` of `Linker::copy`'s commands (one
  `Linker`, so a clip of its own and its copies form one group) and `InsertClip`s;
  `cmd.erase_linked_copies` is one `Batch` of `RemoveClip`s. Both are shapes the undo fuzz
  already covers (linked copies, several in a batch; removals), so no new arm. The tool is
  an overlay sensed above the timeline while it is out, so the arrangement's own gestures
  (selecting, moving, the right-click menu) are untouched when it is away.

**Waveforms**: an audio tile's miniature is its waveform, read from the peak store the
arrangement uses (`PeakStore::lookup` → `SourcePeaks::columns` over the clip's window,
reversed for a reversed clip) — nothing is decoded on the UI thread; the store reads files
on its own worker. The columns are kept per clip with the project revision and the column
count, so a still frame neither looks the peaks up nor makes anything; a change to the
clip (any revision) makes them again. Until the peaks arrive the tile shows its kind only.

**What the tests hold.**

- `links_browser.rs` (+4): a click with the paint tool places one linked copy at the
  snapped time, one undo step, undone exactly; a stroke from bar 4 to past bar 8 places
  five copies end to end (12, 16, 20, 24, 28 beats), in the group, one step, and painting
  the same stretch again changes nothing; a right-drag erases both copies of the riff and
  leaves the beat painted between them, one step, the source cleared when its group is
  gone, undone exactly; the hint bar names the source, Esc clears it and puts the tool away,
  a double-click selects the group's copies. Seen failing before the tool (no API), then
  two at run time (the hint cut short in the bar; a ghost outside the lanes).
- `links_browser_waves.rs` (new, 2): the take's waveform arrives from the store, loud half
  then quiet half, and comes back reversed after `SetClipAudio` reverses the clip; a still
  frame of the pane with a waveform allocates no more than with a note clip in its place
  (16 and 16). Both seen failing with the cache switched off and the reverse dropped.
- Shots: `shoot_links` adds `browser-paint` (the bass line chosen, the tool out, a stroke
  held along the bass row: its ghosts) and gives the vocal take peaks in the browser shots
  (its tile and its clips show a waveform).

### The second review of the paint tool and the browser (2026-10-14, round 26)

A review of the work above and of the merged tree confirmed eleven findings; each fixed
with a test seen failing first (on the old code, or with the fix switched off).

- **No stacked copies.** Painting skipped a start only when it *equalled* a copy's start,
  so a stroke from another grid point (or a click inside a copy) laid copies over copies —
  the notes sounding twice. Now a start whose span overlaps a copy of the source already
  on the track, or another copy of the same stroke, is skipped (`ui::links::paintable`,
  shared by the stroke and its ghosts, which show only what will be placed).
- **Leftward strokes reach the pointer**: going left, the copy whose span holds the
  pointer is placed, as going right.
- **Focus**: a stroke, and a click or double-click on a tile, give the arrangement the edit
  focus, so B and Delete then act on it.
- **Esc** no longer is read raw by the tool: it reaches it through `ClearSelection`, after
  the mixer's claim, a device window to close and the appearance picker, and only while
  the arrangement has the edit focus (`CantusApp::put_paint_away`). Esc meant for the piano
  roll leaves the tool and its source alone.
- **Automation sources** paint, show their ghosts and erase on the track they automate
  (where the copy lands), in the tool and in a drag from the browser.
- **The browser's scrollbar** scrolls (its offset was thrown away).
- **The picker keeps the highlighted row in view** when the filter or the switch changes
  the rows shown, and walking up to the first row under a band brings the band back.
- **Peaks queued by the browser ask for a frame** when they arrive (the context is passed
  to the store).
- **The filter keeps the keyboard**: the arrows no longer move egui's focus off the field,
  so the letter typed right after an arrow is not lost.
- **Font coverage (merged tree)**: the release builder's test treats a character literal as
  drawn text; the combining-mark range in `fold` is now written as numbers
  (`0x300..=0x36F` on `u32::from(c)`), which is what it is — nothing is drawn.

Tests: `links_browser.rs` +6 (no stacking on a one-beat grid, a click inside a copy; a
leftward stroke; focus; Esc only from the arrangement; automation ghosts and erase in both
gestures — seen failing with the row fix switched off; the browser's scrollbar),
`links_picker_big.rs` +2 (the highlighted row in view after a filter and the band brought
back; a letter after an arrow), `font_coverage` green on the merged tree.
