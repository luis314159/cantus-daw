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
picker closed, the pictures are made one editor at a time after anything a tile asks for —
the recent devices, the favourites, then the instruments and the effects in the catalogue's
order; the atlas's one page keeps the first 28, the rest go to the disk cache only (read back
in a frame when their tile asks). Done, it stops and asks for no frame. Again in a new look:
on demand, as before (the warm-up runs once a session).

**Words**: `areas/tuning.{en,es}.txt` complete in both; `docs/manual/{en,es}/tuning.md`,
linked from both manual indexes. The design sheet: `shoot_tuning` (named in DEVELOPING §4).

## Tests

<!-- filled at the end -->

## Measured

<!-- filled at the end -->

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
- The e2e's pitch check compares the measured move with `Tuning::offsets()`; with `tuning`'s
  scales it measures marvel12's cents (Chorda's tuned design must be ready for the render —
  `chorda::ensure` of the tuned design if theirs needs it).

## Left (the owner's)

- The design sheet's answer (`docs/round42/tuning-face/`).
- The checklist's Ronda 42 MTS-ESP items with a real `LIBMTS`, and listening to the cadence.
