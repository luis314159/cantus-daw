# Round 24 — linked clips (BACKLOG §6d.2)

Branch: **`r24/linked`** (worktree `daw-r24/linked`, base master cead440), left
uncommitted for the integrator. Decision record: **ADR 0037** (`docs/adr/0037-linked-clips.md`).
Schema **21** on this branch (another round-24 builder may take 21 too; renumber at the
merge — the migration writes nothing, so only the number and the fixture's name move).

## What was asked

*A copied instrument clip in the arrangement is the same clip: editing one changes every
copy. "Make unique" (context menu, toolbar, a key) detaches one copy; "Link to…" where it
makes sense. Audio clips too: copies share their settings until made unique. Shared clips
show it (a link mark, "×N", the selection highlights the siblings). Schema bump, old
projects keep loading with every clip unique. Paste's default is a setting; Alt-drag and
Duplicate follow it. The clipboard becomes `cantus-clip/2` with the link id (v2 reads v1,
refuses newer). Design first, then the ADR; undo exact, the fuzz covering linking,
unlinking and editing through a sibling; the engine rendering linked clips correctly and
cheaply.*

## The design, in one paragraph

A **link-group id on the clip** (`Clip::link: Option<LinkId>`), the content **replicated**
on every member, kept equal by **`Command::apply` itself**: every command that changes a
shared field (notes, audio settings, automation curve, name, colour) runs its edit on the
clip it names and then on every sibling; joining a group is refused unless the content
already matches. The alternative — a content pool keyed by id — was rejected because every
reader of `clip.notes` / `audio` / `automation` (about forty, in five crates, several hot)
would have to change, while with a link id no reader changes at all and undo stays exact by
construction (ADR 0037 §1 has the comparison). Shared: notes, audio region (file, gain,
fades and curves, reverse, slip), automation, name, colour. Each copy's own: start, length,
offset, loop brace, mute, lane.

## What was built

| Part | What |
|---|---|
| model | `LinkId`, `Clip::link` (serde default, skipped when none) |
| `cantus_core::links` (new) | `ClipContent` (the shared part), `same_content`, `adopt`, `members`, `count`, `siblings`, `sizes`, `first_member`, `check` (the invariant), `repair` (on load), `Linker` (linked copies; one group per clip per batch), `make_unique`, `compatible`, `link_to` |
| commands | **`SetClipLink`** (join/leave; `cmd.make_unique` / `cmd.link_clip`), **`SetClipContent`** (every shared field at once; `cmd.set_clip_content`); `edit_shared` carries `InsertNotes`, `RemoveNotes`, `SetNotes`, `SetClipAudio`, `SetClipAutomation`, `SetClipName`, `SetClipColor`, `SetClipContent` to every sibling; `check_joins` guards `InsertClip`, `InsertTrack` (whose own clips are also compared with each other), `SetClipLink`; `follow_window` grows the siblings that showed what a grown copy showed |
| structural edits | split keeps a note group, takes an audio clip with fades out of its group first; glue takes the kept clip out when the glue changes its content, brings in only the notes each later clip *plays*, lets the heard version of a note win over a hidden one, and gives a note both clips play a fresh id; a loop piece written out once leaves the group; `record_pass::piece` keeps it; a note paste, a piano-roll Duplicate or an overdub that grows a linked copy grows its untrimmed siblings in the same batch |
| clipboard | `cantus-clip/2` (`READS` = v1, v2); `paste_clips(…, linked, …)`: a linked paste joins the original's group (a new one for a unique original; a member of the group if the original is gone) only from this project and run with the content unchanged — else unique with fresh ids |
| snapshot | on each track, a note id played by more than one clip keeps its own id in the clip with the lowest id and gets `voice_tag(note, clip)` in the others (bits 30 and 63 set, distinct in the 31 bits a plug-in note id keeps), so overlapping copies — linked or made unique — never release each other's voices, and the tag never depends on the link: linking, unlinking or copying changes no sample of an untouched clip, on seeded instruments too. Tracks whose clips share no id compile as before (no golden moved, no `DSP_VERSION` bump) |
| project | `SCHEMA_VERSION` 22 (21 on the branch; renumbered at the merge), `migrate_21_to_22` (writes nothing), `links::repair` on load (`LoadReport::links_repaired`, carried in `Loaded` and said on the status line by *Open* and at start), `fixtures/v22.cantus` (the Riff linked three times across Synth and Organ, one copy trimmed and slipped; the vocal take linked twice) |
| setting | `Settings::link_copies`, **on by default**; paste, Duplicate and the Alt-drag copy follow it; a checked row *Copies are linked* in the arrangement's menu (and the empty timeline's right-click), saved to `settings.json` |
| actions & keys | `MakeUnique` **Alt+U**, `LinkClips` **Alt+L** (the selection becomes the edited clip), `SelectLinked` **Shift+Alt+L**; all three in the Clip menu of the menu bar; keymap file version **4** (`NEW_IN_V4`), so an existing `keymap.toml` gets them |
| arrangement | chain mark before the name and **×N** at the title bar's right; the selected clip's siblings outlined in dashes of the selection colour; the toolbar's link key (lit while the selection is linked, a press makes it unique); the clip menu's *Make unique*, *Select linked copies*, *Link selection to this clip* (a right-click on a selected clip makes it the edited clip, so the entry and Alt+L are one action with one target); the dashed outline drawn only where it is on screen; `cantus_ui::links` (`make_unique_clips`, `link_clips_to`, `select_linked`, `toggle_link_copies`, `LinkMarks`) |
| kit | `Glyph::Link`, `Icon::Link` |
| strings | `locales/areas/links.{en,es}.txt` (registered in `i18n.rs`) |
| shots | `examples/shoot_links.rs`: `links-arrangement`, `links-menu`, `links-timeline-menu` (`[es]` too) |

Decided on the way: copies are **linked by default** (the owner's wording: a copy *is* the
same clip); the three ways of copying follow the one setting so they never disagree;
automation clips link too (same target only — a curve moved onto another target would
change lanes); *Link to* without a target uses the clip the piano roll is editing; a
linked paste never links across projects or to content that changed since the copy.

## The review (two adversarial reviewers, 13 findings)

Each was checked against the code; 12 were real and fixed with a test written first and
watched failing, one was a gap in the report.

| Finding | Verdict | What was done |
|---|---|---|
| Make unique leaves note ids shared; overlapping unique copies cut each other | real | fixed in the snapshot, not by re-iding (re-iding would change how a seeded instrument plays the clip made unique): ids shared by two clips on a track are told apart whatever the link (`tell_copies_apart`) |
| Joining/leaving a group changes how an untouched Pulsus clip sounds (two reports) | real, and wider: Calor, Gravis, Lamina, Modulus, Organum and Exemplar seed from the tag too | the tag no longer depends on the link; the lowest clip id keeps the raw ids, so a Duplicate leaves the original bit-identical and Make unique changes no sample (engine null test on Pulsus II and Calor) |
| Glue doubles notes (edited split halves; left-trimmed linked copy) | real (the first also a regression) | glue brings in only notes a later clip plays; the heard version wins over a hidden one (`SetNotes`) |
| `InsertTrack` guard misses the track's own clips | real | compared with each other too; unit test and a fuzz arm (the fuzz breaks the invariant without the guard) |
| Existing installs never get Alt+U/Alt+L/Shift+Alt+L | real | keymap `FILE_VERSION` 4, `NEW_IN_V4`; test loads a version-3 file |
| Overlapping copies on a CLAP/VST3 track: the 31-bit note id drops the salt | real | `voice_tag` sets bit 30 and mixes the clip id into every bit; core test checks the copies' tags are distinct in 31 bits (no hosted-plug-in render test, see below) |
| Edits that grow a pattern are silent in the other copies | real | `links::follow_window` (ADR 0037 §3): siblings that showed what the edited copy showed grow with it, in the batch; wired into note paste, piano-roll Duplicate and overdub; core and UI tests, and the fuzz's paste arm |
| Clip menu shows Alt+L but Alt+L targets another clip | real | the right-click makes the clip the edited clip (keeping the selection); the entry is now `Entry::Run(LinkClips)` |
| "1 clips made unique" | real | `links.status.made_unique_one` (en/es) |
| Repaired link group is silent | real | `Loaded::links_repaired`; *Open* and start say "N clips … were unlinked" (en/es, singular too); UI test |
| Dashed outline built along the whole clip | real | `dashed_rect` draws only the dashes on screen, anchored to the clip so they do not crawl; unit test (a 100 000 px edge, 1 200 px on screen: at most 173 dashes) |
| English timeline-menu shot missing | real (report) | rendered and looked at; all six shots regenerated |

## Tests

New: `cantus-core/tests/links.rs` (17, + 1 ignored measurement), six arms in the undo fuzz
(`undo_invariants.rs`, with `links::check` after every step and every undo and a coverage
assertion for each arm), `cantus-project` `linked_clip_tests` (3), `cantus-engine/tests/linked_clips.rs`
(5), `cantus-ui/tests/links.rs` (14), a keymap test (a version-3 file gets the link keys),
an `arrangement_paint` test (only the dashes on screen), a clipboard codec test (v1 read, v2 keeps the link, v3
refused), `app_flows` (the Alt-drag copy linked with the setting on, independent off).
ADR 0037 lists what each holds.

Watched failing first: the core propagation tests with `edit_shared` switched off (6 of
11); the overlapping-copies render before the tag change (the first copy cut the second's
notes); the glue test without the glue fix (4 notes instead of 8). After the review: the
voice-tag core test and both new engine tests with the old link-dependent salt; the
`InsertTrack` test and the fuzz without the guard; both glue tests (doubled notes); the
follow-window core and UI tests without `follow_window`; the menu test without the
right-click change; the keymap test at version 3.

Results at the close, after the review (this worktree; the machine shared with two other
builders and a soak):

- The touched crates (`cargo test -p cantus-core -p cantus-engine -p cantus-project -p
  cantus-ui --no-fail-fast`): 1 271 passed, 1 failed, 21 ignored. The failure was
  `crash_recovery::killed_mid_recording_the_take_survives_and_comes_back` (a child process
  killed mid-recording; timing under load); alone it passes (2 passed). The three
  `audio_setup*` binaries Windows refuses by name (os error 740) passed under neutral names
  (4 + 6 + 4).
- `cargo test --workspace --no-fail-fast`: **1 939 passed, 0 failed, 29 ignored** in 193
  result lines; plus the four refused binaries run copied to neutral names from their
  crate folders (`audio_setup` 4, `audio_setup_flows` 6, `audio_setup_first` 4,
  `cantus_update` 10): **1 963 green** (1 951 before the review; round 23 closed at 1 921).
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- rustfmt: every file changed in the review pass checks clean (`rustfmt --check`); it was
  run only on those files and on the new ones, never `cargo fmt --all`.

## Measurements

Moving one note in a 2 000-note pattern (`links::measure_an_edit_through_a_sibling`):

| Group | Edit (release) | Edit (dev profile) | Snapshot compile (release) |
|---|---|---|---|
| 1 clip | 5.1 µs (3.3 µs after the review) | 9.8 µs | 0.01 ms |
| 64 clips | 284.6 µs (186.9 µs after the review) | 380.4 µs | 0.51 ms (1.07 ms after the review) |

The edit costs one pass per member — inside a frame even mid-drag. The snapshot compiles a
linked copy as it compiles an unlinked one; since the review it also tells apart note ids
that more than one clip plays on a track (one sort of the played (id, clip) pairs, on
tracks with two or more clips): for 63 copies of a 2 000-note pattern on one track
(126 000 note-ons) that is about 0.5 ms more per compile. A track with one clip pays
nothing. The numbers before and after were taken on different loads of a shared machine;
the edit did not get faster on purpose. A project
with no linked clip pays one scan per frame in the arrangement (`LinkMarks::of`) and one
`Option` check per edit.

## What is left

- An inspector row for a linked clip (the group's size, *Make unique*), and a *Link to…*
  picker by group name; a browser of groups to drag from (FL's picker panel).
- The project file stores each copy's notes; storing a group's content once would shrink
  files with many copies of big patterns (a format change, no model change).
- Seeded instruments: a later copy on the original's track draws different random choices
  from the original (as an unlinked duplicate always did); a copy on another track draws
  the same. Deleting the original hands its ids to the next copy, whose choices then
  change (ADR 0037, Consequences).
- Glue keeps the first clip's own hidden notes past its old end, which become audible when
  the window grows (behaviour from before this round; the reviewers asked only about the
  later clips' side).
- No render test with a hosted CLAP/VST3 instrument for overlapping copies: the 31-bit
  distinctness of the tags is checked at the snapshot (`voice_tags_…_survive_31_bits`).
- `ReplaceProject` (a recovery, a DAWproject import) does not check link groups; its
  projects come from `cantus_project::load` (repaired) or importers that write no links.
- The owner, by hand: Duplicate a pattern, edit a copy in the piano roll, watch every copy
  change; Make unique (Alt+U); Alt-drag with the menu's switch on and off; paste after a
  restart (a v2 text from another run pastes unique — `ClipOrigin` — as designed).
