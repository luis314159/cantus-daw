# Round 29 — warp: warping, time-stretch and pitch-shift of audio

Item 5 of round 29 (BACKLOG §3 *Audio editing*). Part A (the stretcher, the bench and the
listening set, the analysis, the model at schema 34, the editor and the inspector) and
part B (playback under the tempo map through `audio_time`, DAWproject `<Audio>`/`<Warps>`,
the tests). Decisions: ADR 0061. Branch `r29/warp`, merged with `r29/integration` at
`0933835` (tempo's 33, controllers' placeholder 35, isolation's 36 around my 34).

## Done

1. **The stretcher** (`cantus_dsp::stretch`, MPL-2.0, our own implementation from papers;
   licences reviewed in ADR 0061 §1 before any code — no GPL/LGPL code read for code, no
   crate added, `THIRD-PARTY-NOTICES.md` unchanged; ADR 0061 corrects
   `research/engine/dsp-quality.md`'s "Rubber Band is usable" note, which predates ADR 0047).
   One entry point, `stretch::render(channels, rate, &Map, &Params) -> Vec<Vec<f32>>`,
   offline and deterministic, exactly the requested length, and four modes:
   - **Complex**: a phase vocoder with Laroche–Dolson identity phase locking, the
     instantaneous frequency from two frames one synthesis hop apart, zero-phase frames;
     transposition by bin mapping with the formants kept by a true envelope
     (Röbel–Rodet) on a pooled 512-point grid; percussive energy split off by a
     Driedger HPSS soft mask (separation factor 2) and carried by short OLA grains;
     transients held at their own speed (`Map::hold`) with the phases reset on them;
     the right channel's phases locked to the left's image where the two agree within a
     quarter bin, so the stereo image holds.
   - **Rhythm**: a slice at every transient and marker, each played as the source from
     where the map puts its transient (resampled by the transposition); the gap after it,
     when slowed, filled by its tail — *Fade*, *Loop* or *Ping-pong*.
   - **Melodic**: TD-PSOLA on the pitch base's epochs (ADR 0053), the Complex render
     cross-faded in where the line is unvoiced or more than one voice; formants kept by
     the grains.
   - **Re-pitch**: tape — read at the map's speed through Exemplar's windowed-sinc kernel
     on the mip level that does not alias; its pitch is its speed.
   - **Identity**: a map that is the identity, untransposed, returns the source to the bit.
2. **The bench and the listening set**: `cantus-bench --full --only warp` (eleven
   measurements; excluded from `--quick`, it takes ~40 s) and `cantus-bench listen <dir>`
   writing the A/B set to `F:\projects\daw-r29\shots\warp\listen\` with `README.txt`
   (five materials × four modes × ×0.5/×0.8/×1.25/×2, transpositions, the brief's
   ×1.25 −3 st case, each source beside them).
3. **The analysis at import**: SuperFlux onsets to the sample; autocorrelation + comb
   tempo with an octave rule into 70–180 BPM; Ellis DP beats; the downbeat from low
   onsets; loop detection; the WAV `acid` chunk; on `WarpService` (the engine's worker,
   beside the pitch service, never on the interface thread), cached in memory and in
   `<data>/cache/warp/` (folder set in `CantusApp::assemble`, the boot path). **The
   import rule**: a loop is warped at once in the import's own undo step; long files and
   recordings play at natural speed; a paste keeps what it copied.
4. **The model, schema 34**: `AudioRegion::warp: Option<Box<Warp>>` (markers as source
   frame + content tick; mode, cents, formants, tail, follow, the detected tempo);
   `Command::SetClipWarp` with its exact inverse and `cmd.warp.*` in both languages;
   undo-fuzz arm 44 with its `warped` counter; `migrate_33_to_34` (writes nothing),
   the fixture block, `fixtures/v34.cantus`, `schema_34_keeps_the_warp`. Split, trim,
   slip, glue, loops, links, the clipboard and comping keep every marker valid; a warped
   window takes no comp seam.
5. **The interface**: the waveform drawn through the warp; transients along a band and
   markers as gold flags on the clip (double-click/drag the band, drag a marker — snapped
   through `views/bars.rs`, `Ctrl` free, `Alt` slides the audio under it — double-click,
   `Del` or the menu to remove); **Alt-drag of the right edge stretches the clip**; the
   clip editor's **Pitch | Warp** tabs (the warp editor with its bar); the inspector's
   **Warp** section; the clip menu's warp list (modes, *Follow the project tempo*,
   *Quantise audio…*, *Detect tempo*, *Warp from here*, *Set the project tempo from this
   clip*, *Edit warp*, *Bounce warped audio*, *Reset warp*, *Remove warp marker*); the
   *Quantise audio* dialog; *Bounce warped audio* on its own thread; `Glyph::Warp`;
   `shoot_warp` (PNGs in `F:\projects\daw-r29\shots\warp\`). On a warped clip the fade and
   gain squares sit below the marker band, so no flag is covered.
6. **Playback under the tempo map**: `audio_time::{frame_through, tick_through,
   clip_frame_at, clip_tick_at_frame, warped_length}` — one mapping for the engine and
   every interface site; each warped window rendered (`warp_render::Plan`) with frame 0
   on the window's start (the clip's `slip` = −offset seconds, so `Transport::clip_zero`
   places it unchanged); a key with the tempo map's `span_key`; the bridge (linear
   varispeed of the same map) while a render is on its way; three renders kept per clip
   within 512 MB; `compile_for_render` waits for pitch then warp renders. The identity
   warp under one tempo is dropped by the snapshot: such a clip plays the same bits as
   before this round.
7. **DAWproject**: audio clips written as `<Clip>` → `<Warps contentTimeUnit="seconds">`
   → `<Audio>` + `<File>` in the archive and `<Warp time contentTime>` points (algorithm
   complex/beats/tones/repitch, fades), XSD-valid; read back from any application (points
   on the natural line come back unwarped with their fine offset; the files written under
   the data folder); a transposition, pitch edits and a reversed clip reported.
8. **Finish**: ADR 0061; the manual's *Warping and stretching audio* (§5, after the pitch
   editor) in English and Spanish; this file.

## Measurements

`cargo run -p cantus-bench --release -- --full --only warp` (one core, stereo, 48 kHz):

| What | Target | Measured |
|---|---|---|
| Identity, every mode | the source to the bit | to the bit |
| Output length | exact | exact (every mode, x0.25 to x4, with transpositions) |
| A hit's place through markers | 12 samples or less | Rhythm 0 (the slice is the source); Complex 5 median |
| A marker's frame on its tick (engine; 44.1/48/96 kHz; under a ramp) | 1 sample | 1 sample or less |
| Attack 10-90 %, x0.5 to x2 | the source's + 1 ms | the source's (92.5 frames, as the source) |
| Pre-echo before a hit (Complex) | -30 dB or less | -36 to -39 dB median |
| Pitch of a shifted tone (-12 to +12 st, x0.8 to x1.25) | 3 cents | 0.004 cents or less |
| A held chord's added level wander | 1 dB | -0.14 dB (x0.8), 0.64 dB (x1.25) |
| A voice +5 st, formants kept (envelope distance) | Complex 4 dB, Melodic 3 dB | 3.68 dB, 0.92 dB (not kept: 10.0, 9.2) |
| A delayed stereo pair, x0.8 / x1.5, +4 st | 0.05 rad | within |
| Tempo within 1 BPM (generated clicks and drums, 60-200 BPM, swung too) | 97 % | 100 % inside 70-180 (69/69); the 18 answers outside it are the octave the rule picks by design |
| CPU per second of audio (x1.25, -3 st) | - | Complex 0.11 s, Melodic 0.12 s, Re-pitch 5 ms, Rhythm 1 ms |

No golden hash moved; `DSP_VERSION` unchanged (nothing plays differently unless a clip is
warped). The A/B listening set is in `F:\projects\daw-r29\shots\warp\listen\` (111 WAVs and
`README.txt`); the screenshots (arrangement, editor, inspector, quantise dialog, both
themes and languages, 75/100/200 %) in `F:\projects\daw-r29\shots\warp\`.

## Tests and what each holds

- **cantus-dsp** `stretch_tests.rs` (12): identity to the bit in every mode; exact length;
  a hit lands where its marker puts it; Rhythm's slices on their frame exactly; a tone's
  pitch and level through a stretch; Re-pitch moves pitch with speed; transposition by the
  semitones; a voice's formants kept; attacks sharp with no pre-echo; the stereo image
  kept; determinism; the map inverts and holds keep its breakpoints. `onset_tests.rs` (5):
  every click on its sample; a swung drum loop hit by hit; a vibrato is not a note;
  silence and one hit; determinism. `beat_tests.rs` (5): a click track's tempo and
  beats; a swung loop's tempo and downbeat; whole-bar loops told and made exact; silence;
  the corpus within 1 BPM.
- **cantus-core** `warp_tests.rs` (10): the map through the markers and its inverse; no
  markers = the source's tempo; identity told and nothing else; invalid warps refused;
  every edit keeps the warp valid; quantise by strength; a grid warps a loop bar by bar;
  one function for a clip's time; a clip never warped writes nothing new; a stretched
  warp plays its window longer. `audio_time` unit tests (one tempo = the old rule to the
  bit; natural speed from its start). `tests/warp.rs` (5): set and undone exactly;
  refusals; a marker drag is one undo step; the snapshot drops a warp that plays as the
  source; a warped window takes no comp seam. Undo fuzz arm 44.
- **cantus-project**: `schema_34_keeps_the_warp`, every fixture loads;
  `tests/dawproject_audio.rs` (2): written XSD-valid with the files carried; read back
  with markers on their frames, the mode, fades, natural-speed clips unwarped with their
  fine offset, nothing reported dropped.
- **cantus-engine** `tests/warp.rs` (9): the analysis finds a loop's tempo and hits and
  keeps them on disk; the `acid` chunk; an identity warp plays the source to the bit; a
  92 BPM loop plays in time at 128 BPM; a marker on its tick to the sample; offline =
  real time at block sizes 1/37/64/512 and mixed; while a render is on its way the last
  render plays; an export waits for the warp renders; the render and the interface read
  one mapping (to the bit, under a ramp).
- **cantus-ui** `tests/warp.rs` (7): a loop imported warps itself in the import's undo
  step; a file that is not a loop stays at natural speed; a marker added, moved and
  deleted by `Del` (not the clip); quantise pulls a loose take onto the grid; the
  arrangement draws markers to grab, with the fade and gain squares below the band;
  bounce leaves the clip on a file of its own; Alt-drag of the right edge stretches.
  `tests/warp_boot.rs`: the boot path keeps analyses in the data folder. `fx_lint.rs`,
  `kit_lint.rs`, `i18n.rs` cover the new views and strings.

## Files outside my area (additive)

- `cantus-core`: `audio.rs` (the `warp` field), `command.rs` (one `SetClipWarp` block),
  `snapshot.rs` (the warp, the source rate, the identity dropped), `comp.rs` (a warped
  window takes no seam), `lib.rs` (`mod warp`), `audio_time.rs` (tempo's file, extended
  for warped clips as the brief says: `frame_through`, `tick_through`, `clip_frame_at`,
  `clip_tick_at_frame`, `warped_length`), `tests/undo_invariants.rs` (arm 44, `warped`).
- `cantus-dsp/src/lib.rs` (`mod beat, onset, stretch`).
- `cantus-engine`: `lib.rs` (`mod warp`), `samples.rs` (the warp attach after the pitch
  attach, `loading()` counting both), `pitch_attach.rs` (`render_of`), `backend.rs`
  (`compile_for_render` waits for the warp renders after the pitch renders).
- `cantus-project`: `format.rs` (`migrate_33_to_34`, the test), `examples/write_fixture.rs`
  (the 34 block), `fixtures/v34.cantus` and the rewritten `v36.cantus`,
  `dawproject/{mod, read, read_timeline, write}.rs` (audio clips; `audio.rs` new),
  `report.rs` (the new keys, scanning `audio.rs`).
- `cantus-ui`: `app.rs` (the `warp` state, the service poll, the import hook, the
  pre-hooks), `lib.rs` (`mod warp…`), `views/mod.rs`, `views/arrangement.rs` (the warp
  handles routed in begin/update/reset/hint/click/menu), `views/arrangement_audio.rs`
  and `_edit.rs` (the waveform through the warp, the markers, the squares below the band,
  Alt-drag of the right edge), `views/inspector.rs` (the section, `Act::Warp`),
  `views/piano_roll.rs` (the Warp tab opens the warp editor), `views/pitch.rs` (the tabs
  line), `pitch.rs` (`ClipTime` through `audio_time` and the warp), `pitch_demo.rs` (a
  path), `audio_files.rs` and `audio_clips.rs` (the import rule, glue only with the same
  warp), `clips.rs` (the undo label), `kit/glyph.rs` (`Glyph::Warp`), `views/interop.rs`
  (`export_with`, the imported media placed), `locales/areas/interop.{en,es}.txt`
  (`d_audio_clips` no longer says audio clips are dropped), `tests/fx_lint.rs` (the new
  views scanned).
- `cantus-bench` (isolation's crate for its suite): `src/warp.rs` new, `lib.rs` (the
  `warp` group, kept out of `--quick`), `main.rs` (`listen`), `Cargo.toml` (`hound`,
  already in the workspace: no new notice); `Cargo.lock`.
- No keymap action was added (`Del` reuses `Action::Delete`; Alt-drag is a modifier), so
  nothing was appended to `NEW_IN_V9`.

## Left

Nothing of mine. Owner-only: **listen** to `F:\projects\daw-r29\shots\warp\listen\`
(`README.txt` says what each file is) and say which mode or ratio sounds worse than
élastique — the numbers above are met, the ear is the owner's.

## Notes for the integrator

- **Known-issue lines this work retires** (the integrator edits them; tempo retires the
  rest of the same line): the manual's §14 `docs/manual/en/README.md:1013` "**No
  time-stretch**: audio clips keep their speed when the tempo changes" (the *one tempo
  and one time signature* half is tempo's) and `:1051` "warping" in *Not in this beta*;
  Spanish `docs/manual/es/README.md:1097` and `:1138`; the release notes
  `docs/releases/v0.1.0-beta.1.md:145`, `:184`, `:358`, `:398`. Line numbers are on
  `r29/warp`.
- **Schema**: mine is 34 (`migrate_33_to_34` replaces the placeholder); controllers' 35 is
  still a placeholder on this branch; the fixture blocks are in order 33, 34, 36.
- **Merge hot spots**: `views/arrangement.rs` (the handle routing beside tempo's and
  controllers'), `app.rs` (pre-hooks), `samples.rs` / `backend.rs` (freeze touches the
  render waits too), `cantus-bench/src/lib.rs` (`GROUPS`), the undo fuzz counters.
- `research/engine/dsp-quality.md` §2 and `research/engine/README.md` said Rubber Band
  was "usable since Cantus is GPL" (written before ADR 0047); both now point at ADR 0061 §1.
- A minor thing seen and fixed at the end: on a warped clip the gain square sat on the
  marker band at the clip's centre; the fade and gain squares now sit below the band.
