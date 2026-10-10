# ADR 0040 — *Save* off the interface's thread

**Status:** Accepted · **Date:** 2026-10-09 · **Round:** 24 (beta-rest) · **Extends:** ADR 0006 (project format: the atomic, verified write and the `.bak`)

> Numbering: written as 0040 on `r24/beta-rest`; the integrator renumbers it if another
> branch took it.

## Context

Since round 22 the autosave runs on its own thread (`autosave.rs`), after the soak found
0.7–1.8 s frames at every interval. *File ▸ Save*, *Save As* and Ctrl+S still wrote on the
interface's thread: the project serialised and hashed twice, written beside the file,
`fsync`ed, the old file kept as `.bak`, read back, parsed and verified. Measured in this
round (`save_async.rs::measure_the_copy_against_the_write`, the `test` profile): the demos
take 25–52 ms per save; a session of 64 tracks × 4 000 notes (54 MB of JSON) **3.8 s** —
3.8 s of a frozen window, audio meters and all.

## Decision

- **What the frame takes**: the audio collected into the project's folder (its commands
  change the project) and the hosted plug-ins' state as it is now (a plug-in answers only
  on its main thread) — both inside `PluginHost::project_for_save`, which returns an owned
  copy of the project. The copy costs 0.07–0.17 ms on the demos and 3.6 ms on the big
  session; serialising alone costs 3–19 ms and 2.3 s. So the **copy** is what is taken,
  not the serialised bytes.
- **What the thread does**: `cantus_project::save` as before — the same atomic write,
  `.bak` and verification. The next frames collect the result (`poll_save`, every frame;
  the repaint ticker wakes every 100 ms while one is in flight).
- **One save at a time.** A save asked for while one runs is **queued** (the last asked
  wins) and starts when the first ends; the same file after a failure is not retried.
  *Save* while one runs goes where that save goes (`save_target`): an untitled project is
  not asked for a name a second time.
- **For the project it was asked for.** The queued save carries the project's epoch: when
  the project is replaced while a save is written (the undo or redo of a replaced project,
  which does not wait), the queued save and the errand waiting on it are dropped, never
  written with the project that came in; a landed save of the project before only logs.
- **The crash journal** (round 22) lets go of a recording pass only when its takes are in
  the file written: the job carries the passes that were finished when the copy was taken,
  and only those leave the journal when it lands — a pass that ends while the file is
  written stays until the next save or autosave holds it. The autosave's job does the same
  (it had the same latent gap since round 22).
- **Dirty at the right revision.** `CommandBus::mark_saved_at(revision)` marks the revision
  the copy was taken at: an edit made while the file was written keeps the project dirty.
- **The status line** says *Saving “x.cantus”…*, then *Saved …* or the failure in words in
  the interface's language (`recovery.save_failed`, round 22); the project stays dirty and
  pointed at its old file when a save fails.
- **Waiting.** New, Open, a demo (`replace_project`, `open`), a DAWproject import
  (`import_dawproject`), a recovery (`restore_recovery`), the *Save changes?* question
  (`ask_or`) and the close (`close_session`) wait for a save in flight first — the save may
  make the question unnecessary, and the close's autosave and journal must see its result.
  The question's **Save** and the demo picker's do not wait: what they were asked for
  (`Then`) is carried out once the save has landed, never if it failed, and the question is
  asked again if the project was edited while it was written. The picker closes while that
  save is written (nothing is left in it to cancel).
- `save_to_path` keeps its old contract — it returns once the file is written — for the
  soak, scripts and tests; the interface calls `save`, `save_as` and `start_save`.

## Consequences

- A save never blocks the frame by itself, whatever the project's size; the autosave and
  the save no longer differ in how they write. What waits for a save in flight (New, Open,
  an import, a recovery, the *Save changes?* question, the close) does wait on the
  interface's thread, for what is left of it — up to the 3.8 s of the big session if one
  of them follows Ctrl+S at once. Those waits are what the rules above ask for.
- Collecting audio into the project's folder still copies files on the interface's thread
  before the save starts (only when a source lies outside the folder: a first save, a
  *Save As* elsewhere). Moving it needs the copy's commands applied after the thread ends;
  left for a later round.
- The stop of the process while a save is in flight is the same as before: the write is
  atomic (temporary file, rename), so the file on disk is the old one or the new one.

## What the tests hold

`crates/cantus-ui/tests/save_async.rs` holds a save's thread before it writes
(`Saving::hold`), so "in flight" is a state, not a race: a save returns at once, nothing is
on disk until it is released, the file then lands whole and verified and the project is
clean; an edit during the save keeps it dirty and the file holds the old name; a failed
save says so in English and Spanish and keeps the project dirty and unpointed; a second
save is queued and takes the later edits; New, Open and the close wait (the close leaves
no journal to recover); the question's *Save* opens the other file only after the save
landed, and not at all when it failed. Before the change (the old synchronous save behind
`start_save`) four of the six failed. After the review (round 24) four more: a queued save
is never written with a project that replaced the one it was asked for (an import, which
now waits; the undo of one, which the epoch rule holds); a second *Save* of an untitled
project goes where its first went; the demo picker's *Save first* closes the picker and
opens the demo once saved, and asks again after an edit made meanwhile; a recording pass
that ends during a save stays in the journal, on disk too, until the next save holds it.
Each failed with its fix taken out.
