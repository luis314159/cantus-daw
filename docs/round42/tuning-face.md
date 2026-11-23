# Round 42 — `tuning-face`: the song's tuning on screen, and MTS-ESP

Builder `tuning-face`, branch `r42/tuning-face` (from `r42/contract`). ADR 0155's surface,
ADR 0156; round 41's leftover (BACKLOG §6j, the picker's pictures).

## Built

**Project ▸ Tuning** (`crates/cantus-ui/src/views/tuning*.rs`): a kit window over a dimmed
room, as the export dialog (no stock egui widget; tokens only; the kit's text everywhere).
*Options ▸ Tuning…* and **Ctrl+Alt+T** (`Action::OpenTuning`, keymap file version 20 with
its migration, the palette's words "tuning temperament microtuning Scala MTS-ESP diapason").

- Left: the system (Equal · Temperament · Scala · MTS-ESP; the MTS-ESP segment veiled with
  the reason when the library is absent), the temperament (the contract's 15) and its key
  (‹ › a semitone), *Follow the song's key* (a field of the tuning: `SetProjectTuning`;
  greyed with the reason when the song has no key; the key row then shows the song's
  tonic), a scale's name, notes and period, the keyboard map's reference read-out; the
  diapason (‹ › 1 Hz, merged into one undo step while stepping) and its presets 415 … 443;
  **Import .scl / .kbm…** and **Export…**; MTS-ESP's status, **Be the MTS-ESP master**
  (refused with the reason while another master holds it, *Reinitialise MTS-ESP* only where
  the library shares state between processes; greyed for a track's window and for a song that
  follows MTS-ESP), the audition (I–IV–V–I, i–iv–V–i in a minor song, on the edited track,
  the selected one, or the first instrument track).
- Right: the **circle of fifths** (outer ring the fifths, inner ring the major thirds, each
  coloured by its beat rate at middle C on the meters' low → mid → clip scale; the tonic
  ringed; hover a note for its fifth's and third's beats and the fifth in cents from pure;
  the centre names the tonic and A4) with its legend, and the **degrees** (cents above C, from
  equal at the chosen A4, the fifth's and third's beats with their colour dots); a scale not
  12 to the octave lists its degrees as written, in cents and from its own equal division.
- Footer: the summary (or "the default"), *Equal, A440* / *Follow the project*, *Close*.
- Every control is one command with its exact inverse (`SetProjectTuning`,
  `SetTrackTuning`; the contract's undo-fuzz arms).

**The summary line** on the top bar's project area, after the song's length, only when the
tuning is not the default ("… · 2:03 · Werckmeister III in Eb · A415"), keys named with
`scale::ROOT_NAMES`; its cache key now includes the history's revision.

**The track's override** in the inspector (`r42:tuning-face` lines in `inspector_track.rs`
and `inspector_menus.rs`): a *Tuning* row — *Follows the project* / *Equal temperament* /
*Its own…* (which makes the track's own from what it plays and opens the window on it).

**Chorda's Edit ▸ Tuning**: under the stretch display, a line saying which tuning the piano
plays (the project's, equal, the track's own); a click opens the window. Chorda's stretch,
stiffness and unisons stay its own (`r42:tuning-face` lines in `views/inst/chorda.rs`).

**MTS-ESP** (ADR 0156): `crates/cantus-rt/src/mts.rs` — `LIBMTS` loaded at run time from its
standard place (Windows `%CommonProgramW6432%\MTS-ESP\LIBMTS.dll`, macOS `/Library/Application
Support/MTS-ESP/libMTS.dylib`, Linux `/usr/local/lib/libMTS.so`) with `windows-sys`
`LoadLibraryW` / `libc::dlopen` — no new dependency; the client (register, `HasMaster`, the
table read through `MTS_GetTuningTable`, unmapped keys, the scale's name) and the master
(`MasterState`, register with ODDSound's refusal and reinitialise rules, publish, filter,
name, clients); never unloaded. `crates/cantus-engine/src/mts.rs` — `MtsLink`, the
`ExternalTuning` implementation (Hz ↔ semitones from 12-TET at A440 through
`cantus_dsp::math`; unmapped keys `NAN` in, the lower neighbour's frequency out, as ODDSound
advises). The boot (`CantusApp::assemble`) loads it on the main thread
(`views::tuning::load_mts`); when Cantus is the master the project's table is published again
whenever the history's revision gives another table.

**The picker's pictures made ahead** (`views/picker_warm.rs`, `r42:tuning-face` lines in
`views/picker_thumbs.rs`): after the start-up (4 s) and while no hand moves (1.5 s), with the
picker closed, the pictures are made one editor at a time on a worker, after anything a tile
asks for —
the recent devices, the favourites, then the instruments and the effects in the catalogue's
order; the atlas's one page keeps the first 28, the rest go to the disk cache only (read back
in a frame when their tile asks). Done, it stops and asks for no frame. Again in a new look:
on demand, as before (the warm-up runs once a session).

**Chorda's own tuning, from inside Chorda** (the owner's ask of 2026-11-22: "change only
Chorda and keep the whole song as it is"; `views/tuning_chorda.rs`, `r42:tuning-face` lines in
`views/inst/chorda.rs`): a full-width *Temperament* card under Strings and Tuning — *Plays in*
(the project's tuning / its own: the track's `TrackTuning`), *Tuning* (24 presets in groups —
Equal ×3, Historical ×12 and Just ×3 rotated to the *Key* below, Character ×6 — and *Yours*, the
user's `.scl` files in the data folder's `Tuning`), *Key* (‹ ›), *Save as mine* / *Import* /
*Export*, the cadence, a small circle of fifths and the purest and roughest major thirds. A
preset is one undo step: a `Batch` of `SetTrackTuning` (its own) and the `SetInstrumentValue`s
of the stretch and unisons it needs. **The A4 rule**: while the track plays no tuning (its table
12-TET at A440 to the bit), Chorda's own A4 knob as always; following the project's tuning, the
project's A4 greyed ("the project's"); with its own, a field with ‹ › that edits its A4.

**Words**: `areas/tuning.{en,es}.txt` complete in both; `docs/manual/{en,es}/tuning.md`,
linked from both manual indexes. The design sheet: `shoot_tuning` (named in DEVELOPING §4).

## Tests

- `crates/cantus-rt/tests/mts.rs` (4) against the fake `LIBMTS` (built by the test with the
  toolchain's `rustc`, once per run): no master → nothing read; a master's table, scale name
  and unmapped key read by a client; Cantus does not follow itself; a second master refused,
  reinitialise offered only where the library shares state, and taken only when asked; a
  resigned master publishes nothing; the block's read and publish allocate nothing (a
  counting allocator); a library that is not `LIBMTS` refused by name. Unit tests (2): each
  system's place; a missing file is *not installed*.
- `crates/cantus-engine/tests/mts_external.rs` (4): Hz ↔ semitones within 1e-5 semitone both
  ways; unmapped keys published at the lower neighbour's frequency; without the library every
  call is nothing; Cantus follows a fake master (unmapped key `NAN`, the name) and leads
  clients, a second master refused, no allocation in 500 blocks.
- `crates/cantus-ui/tests/tuning_face_e2e.rs` (3), on `CantusApp::assemble`: *marvel12.scl*
  (MIT `tuning-library`) imported through the window's action → one `SetProjectTuning` with
  its 12 degrees as written; Chorda's measured pitch moves by `offsets()`'s table (0.1 cent);
  exported and read back to 1e-9 cent; undo/redo; three must-fail files refused with their
  reason, nothing changed. And the owner's ask: Chorda's card's action, *Werckmeister III* in
  E → only the Chorda track gets its own tuning, Chorda measured against its table, Calor on
  another track renders the same to the bit, the project's tuning stays the default, one undo.
- `crates/cantus-ui/tests/tuning_whole.rs` (3): the window in its five states, the top bar's
  tuned line, and Chorda's card (own, and the project's), in both languages, Slate Lapis Night
  and Paper at 100 %, Night at 75/150/200 %, the kit's clip-aware probe on — nothing cut; the
  window's controls overlap nothing.
- `crates/cantus-ui/tests/picker_warm.rs` (1): the warm-up waits for the start-up and a quiet
  hand; then every device is ready or left to the picker as heavy; the picker opened and
  scrolled draws no editor; the per-pass budget holds (only the step that finds a device heavy
  may exceed it, once per such device); at rest a pass costs a plain pass.
- Unit tests: the words (summary, names, numbers), the circle (equal temperament's fifth
  0.889 and third 10.38 beats at middle C, a pure fifth does not beat, the fifths' order, the
  nearest-note naming), the interim Scala reader/writer, the top bar's summary and its width.
- Also run green: `keyboard_reach` (6), `cantus-ui --lib` (608), `cantus-rt` lib (32).

## Measured

- The window's tests and shots run on the dummy engine; pitch measured on the offline render.
- The picker's warm-up (Windows, two other builders compiling — the integrator measures
  quiet): drawn on the interface thread at first, a pass took up to 0.9 s (50 of 400 over a
  60 Hz frame); every editor's capture frame exceeded a 4 ms slice there. Now each capture
  runs whole on a worker of its own (`cantus-picker-ahead`, wearing the interface thread's
  faces, colourways, views and language; a panic there gives a card, never a crash) and the
  interface thread only uploads the finished picture: 55 pictures, 6 291 passes, **p50
  0.024 ms, p99 0.135 ms, max 2.93 ms** per pass against a plain pass of 0.003 ms; none over
  the 4 ms budget; at rest 0.004 ms vs 0.003 ms. The worker's pictures were looked at (Chorda's
  piano, Tympanum's kit, Comprimo's face): as the picker draws them.
- Chorda's pitch under a tuning: on the contract's `offsets()` (flat), the measured moves are
  0.000 c and the table 0.000 c — the comparison becomes the real one at the merge.

## Verified only in emulation

- MTS-ESP: against the fake `LIBMTS` (`crates/cantus-rt/tests/fake_mts/fake_mts.rs`, the same C
  symbols, built by the tests with `rustc`); no machine of this round had ODDSound's library.
  The checklist item *Ronda 42 — la afinación de la canción y MTS-ESP* holds what only a
  machine with it confirms (Cantus as Surge XT's client and as its master, two masters, the
  reinitialise after a crash, macOS's signed load).

## For the integrator (the merge with `tuning`)

- `views/tuning_io.rs`: the four Scala readers/writers and `latin1` are interim
  (`r42:tuning-face interim`): make their bodies calls to `cantus_core::tuning_scala::*`.
- `views::tuning::effective`: make its body `t.resolved_for_key(key.map(|k| k.root))`.
- `views::tuning::install_mts`: uncomment `app.engine.set_external_tuning(Arc::new(link.clone()))`.
- `views::tuning::design_preview` (the sheet's temperament table before `offsets()` filled
  them): remove it and its one use in `shoot_tuning.rs`.
- The e2e's pitch checks compare the measured move with `Tuning::offsets()`; with `tuning`'s
  tables they measure marvel12's and Werckmeister III's cents (Chorda's tuned design must be
  ready for the render — `chorda::ensure` of the tuned design if theirs needs it).
- `views::tuning::chorda::a4_of`: make it `cantus_core::inst::chorda::follows_tuning` /
  `effective_a4`; Chorda's *designing…* caption (`views/inst/chorda.rs::designing`) must ask
  `chorda::design(chorda::tuning_for(&values_of(spec), &track_table.0), fs)`.
- Merge check (the integrator's): in Werckmeister III rotated to E, *Purest major thirds* should
  rank B (on G, ~394 c) before D (on Bb, ~400 c) once `offsets()` feeds the card.
- Engine: `EngineHandle::set_external_tuning` in `views::tuning::install_mts` (commented there).

## Left (the owner's)

- The design sheet's answer (`docs/round42/tuning-face/`).
- The checklist's Ronda 42 MTS-ESP items with a real `LIBMTS`, and listening to the cadence.
