# Round 32 — score: the score from the piano roll (ADR 0075)

Builder: **score** · branch `r32/score` from the contract 13986457 · schema **45** ·
keymap **13**.

## What was done

- **`crates/cantus-score`** (GPL-3.0-or-later, no egui): notes in → the logical score →
  an engraved **display list in staff spaces** that the screen, the PDF and the golden
  tests share.
  - Reading for the score only (`read.rs`): per quarter, straight 16ths/32nds, triplets,
    sextuplets, quintuplets weighed by onset error plus a price; quarter triplets merged
    over the half note; the track's *Read as* overrides.
  - Grand staff on *Auto* for two-hand parts; the split follows the hands (`hands.rs`) or
    a fixed note; two voices per staff (`voices.rs`).
  - Rhythm after Gould (`rhythm.rs`): ties that show the beat and the half bar, the centred
    syncopations, the Scotch snap; stricter rests; whole-bar rests.
  - Spelling from the song's key (`spell.rs`), accidentals per bar and line, courtesy
    accidentals in parentheses; clefs by range; 8va/8vb lines; beams by the metre with
    hooks; stems by the farthest note; tuplet brackets only where there is no beam;
    dynamics from velocity; tempo marks (♩ = 120, ♩. in compound time, *accel.*/*rit.*) and
    metre changes from the tempo map.
  - Layout (`layout/`): shared columns, √2 spacing per doubling, collision minima; system
    breaks by dynamic programming; skyline stacking; Behind-Bars beams (slope by interval,
    capped, flat when concave, 3 + ¾ per beam stems, middle-line rule, sit/straddle/hang
    quantisation); ties away from the stem; accidental columns (top, bottom, inward; a
    seventh shares); the brace from Bravura's outline stretched taller than wider.
    Continuous, Wrap and Pages (A4, Letter) formats.
  - **Bravura 1.482** (OFL, `assets/`, in `THIRD-PARTY-NOTICES.md`) with its engraving
    defaults and anchors in a generated table (`tools/score/gen_bravura.py`).
  - **MusicXML 4.0** out (`musicxml.rs`, velocity in the note's `dynamics`) and in
    (`musicxml_in.rs`: ties joined, transposition undone, concert key).
  - **PDF** (`pdf.rs`): vector pages, Bravura CFF and IBM Plex TrueType embedded as tagged
    subsets with ToUnicode; title and composer.
- **Model** (schema 45): `Track::score: ScoreSettings`; `Command::SetTrackScore`
  (`cmd.score.settings`); migration 44→45 (nothing to write); hand-edit repair on load;
  `fixtures/v45.cantus`; the freeze key leaves `score` out.
- **Interface** (`score.rs`, `views/score_{view,bar,paint}.rs`): the **Score** switch in
  the piano roll's toolbar (`Alt+S`; `Alt+Shift+S` the pane; keymap 13) — the clip's part under the roll,
  resizable, following the playhead, sharing the selection; the **Score pane** (*View ▸
  Score*) for the selected track; the settings toolbar (clef, staves, split, pitch, read
  as, dynamics, part name); zoom; **Files**: MusicXML (part or song), PDF A4/Letter,
  Print, Import MusicXML (each part a clip on a new track, one undo step). Bravura as the
  music font role in `design.rs` (`music_font`, `score_text_font`, `score_faces`).
- **Editing on the staff** (it fits): click selects (Shift adds; in the pane the note's
  clip with it), ↑/↓ a degree of the key, Ctrl an octave, Delete; note input (N, 3–7,
  `.`, A–G, click moves the cursor, Esc) — every edit a command (`cmd.score.*`).
- **Print**, per system: Windows' *print* verb on the PDF (else it opens); `lp` on Linux
  and macOS (else `xdg-open` / `open`).
- ADR 0075, the manual's section (EN, ES) and its shortcut row, strings in
  `locales/areas/score.{en,es}.txt`, shots in `docs/round32/score/`.

## How it was tested

- `cantus-score`: unit tests (reading, metre strengths, rhythm in 4/4, 6/8 and triplets,
  spelling, hands, voices, chord shapes, beams, skylines, brace, glyph table vs the font);
  `tests/engraving.rs` — **seven golden passages** (melody, two-hand piano, triplets,
  syncopation, long ties in 3/4, metre change 4/4 → 7/8 → 6/8 → 3/4 → 4/4 with a tempo
  change, a high line under 8va; one with dynamics, one transposed) wrapped and on A4,
  plus *nothing collides*, *every note drawn / stems 2.5–9 spaces*, *maps agree, nothing
  off the paper*; `tests/musicxml_roundtrip.rs` (every passage back exactly; a jittered
  performance back as read; measures fill); `tests/pdf.rs` (lopdf: A4 595.28 × 841.89,
  Letter 612 × 792, four tagged subset fonts, each under a quarter of its face, ToUnicode,
  title and composer found in the text, a 24-repeat part on several pages).
- MusicXML validated against the **MusicXML 4.0 XSD** for every passage with .NET's
  validator (`tools/score/validate_musicxml.ps1`; a deliberately broken file is refused).
- `cantus-project`: `schema_45_keeps_score_settings`; every older fixture still loads.
- `cantus-core`: the undo fuzz runs `SetTrackScore` from its own stream (applied and
  refused counted, undone and redone through every checkpoint).
- `cantus-ui/tests/score.rs`: split and pane draw (heads = notes); settings undo; staff
  keys (step, octave, input, delete); MusicXML/PDF export and MusicXML import through the
  app; keymap 13 migration (Alt+S, Alt+Shift+S); boot fonts hold Bravura; **200 bars × 8 notes: first frame
  (engraving) ≈ 15 ms, cached frames ≤ 0.8 ms** on this machine (asserted < 2 s and
  < 30 ms).
- Looked at: `cargo run -p cantus-score --example render_png` (tiny-skia from the fonts'
  outlines) and `cargo run -p cantus-ui --example shoot_score` (the app's painter, Paper and
  Slate, 100 % and 200 %), compared with Gould and with how Dorico/MuseScore set the same
  figures; fixed what fell short (flags and ledger lines in the spacing, quarter triplets,
  courtesy accidentals, the last system's balance, the brace).

## Numbers

- Windows, `RUSTFLAGS=-D warnings`: `cargo clippy --workspace --all-targets` clean;
  tests — cantus-core 524 passed (1 ignored), cantus-project 137 (1 ignored),
  cantus-score 39, cantus-ui + xtask 1 906 (30 ignored), 0 failed.
- Linux (WSL Ubuntu 24.04, `tools/wsl/check.sh r32/score`, own clone): clippy and the
  plug-ins without editor clean; the whole workspace's tests 4 770 passed, 0 failed, 58
  ignored (headless).
- 200 bars × 8 notes in the staff under the piano roll: first frame (engraving) ≈ 15 ms,
  cached frames ≤ 0.8 ms (`a_200_bar_part_draws_within_the_frame_budget`).
- PDF of the piano passage: four embedded subsets, each under a quarter of its face.
- Shots: `docs/round32/score/` — the piano roll with its staff and the Score pane (Paper,
  Slate; 100 %, 200 %), every passage wrapped and on A4.

## The review's fix round

Nine findings (integrator's review of `13986457..8614c98e`), each fixed with a test written
first and seen failing:

1. **PDF windings** — every filled shape now winds one way (one nonzero path; overlaps no
   longer cancel): `every_filled_shape_winds_the_same_way_and_overlaps_stay_black`
   (signed areas, and a tiny-skia raster black where a beam crosses a staff line).
2. **Print path** — the PDF is written as `cantus-score-<pid>-<safe name>.pdf` (only
   `[A-Za-z0-9 _.-]`), handed to PowerShell's `Start-Process -Verb Print` with
   `CREATE_NO_WINDOW`, else `explorer.exe`; `lp`/`xdg-open`/`open` elsewhere; on a thread of
   its own; never `cmd`. `cantus-ui` forbids `unsafe`, so no `ShellExecuteW`:
   `the_print_file_name_is_safe`.
3. **Voice 2 dropped a note** — same-onset second-voice chords become one chord as long as
   the shortest, the longer notes tied on; no chord is dropped:
   `three_ends_at_one_onset_keep_every_note`, the new `voices` golden passage.
4. **Staff keys** — taken with exactly their modifiers; Esc only in note input:
   `the_staff_takes_only_its_own_keys` (Shift+↑ transposes an octave, Alt+C toggles the
   count-in during input, Esc clears the selection).
5. **Undo** — each settings change its own step: `each_settings_change_is_one_undo_step`.
6. **Modal keys** — the smaller alteration first and per-mode degrees (F♮ in C Lydian, B♭
   in D Dorian); MusicXML writes and reads `<mode>` (dorian, lydian…):
   `modes_spell_their_chromatic_notes_by_the_signature`, `a_modal_key_keeps_its_mode`.
7. **Note input** — only inside the clip's window (it stops at the end); a transposed
   part's letters are written notes: `note_input_stays_in_the_window_and_reads_written_letters`.
8. **The last bar's last 32nd** — an onset there stays in the bar:
   `a_note_at_the_very_end_of_the_last_bar_is_drawn`.
9. **Import and paper** — MusicXML into a song with no notes brings its tempo, metres and
   key (each only where the song accepts it); a song with notes keeps its own. The default
   paper follows Windows' *Region* setting (`reg query`), `LC_PAPER`/`LANG` elsewhere:
   `an_import_into_an_empty_song_brings_its_tempo_metre_and_key`,
   `the_default_paper_follows_the_region`.

After the fixes, Windows `RUSTFLAGS=-D warnings`: clippy clean; cantus-score + cantus-ui
1 920 passed, 0 failed, 30 ignored. Linux (WSL): clippy and the plug-ins without editor
clean; the workspace's tests 4 782 passed, 0 failed, 58 ignored. The manual (EN, ES) and
ADR 0075 say what changed.

## After the fix rounds

- A last system at least half full is now justified like the others (Dorico's default;
  before, two thirds): a short piece's second system no longer stops halfway across the
  page. Three goldens re-blessed with it (`melody-wrap`, `melody-a4`, `piano-a4`), in the
  showcase branch's first commit.

## What needs the owner

- **MuseScore 4 on this machine**: running the MuseScore AppImage to open our MusicXML and
  render the passages side by side was refused by the session's permission rules; the files
  were checked against the XSD instead. Opening `cargo run -p cantus-score --example
  dump_xml -- <dir>`'s files in MuseScore 4 (or allowing the AppImage) is the last check.
- **Printing on real printers** (Windows' default PDF application, CUPS on Linux) and the
  look of the score by eye.
