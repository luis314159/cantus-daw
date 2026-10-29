# Round 32 — integration: a face of its own for each unit, and the score from the piano roll

**The ask** (the owner, 2026-10-27, `docs/next-session.md` of round 31): BACKLOG §6h point 1 —
each of our effects and instruments with a face of its own (colour and material, a
characteristic display, its own control shapes and lettering; a style sheet per unit,
`kit::skin`; the exported CLAP/VST3 wearing the same face; done when every unit is shot in
both themes at 100 % and 200 %, the frame budget holds with three editors open and a test
fails for a unit left plain) — and the score from the piano roll (§0.2 *Notation*: an
engraved staff, MusicXML and PDF, printing, editing on the staff if it fits with quality,
golden engraving tests and a MusicXML round trip). During the round the owner added: the
README and the landing page with these features and screenshots; round 33 = Ableton's
core, round 34 = AI.

## How

- **Item 0 had nothing**: `macos.yml`'s last run (public b95a9dd9) was green with nothing
  retried; no issues on `cantus-daw`; no owner notes.
- Contract `13986457` on `r32/integration`: `kit/skin.rs` (a `Skin` is data; `Style.skin`,
  `Style::skinned`; the shell applies it; one sheet file per family), `tests/skins.rs` (the
  plain-look test waiting, ignored), empty locale areas `skin` and `score`,
  `docs/round32/BUILDING.md` (ADR 0074 skins; ADR 0075, schema 45, keymap 13 the score).
- Builders in `F:\projects\daw-r32\{skin,score,instruments,dynamics,modulation}`, never more
  than three compiling at first, four later with a free-RAM check. The families began with
  their design notes, colours and displays while the skin builder drew the kit, merged the
  kit when it landed (`f1ee9891`) and the kit's fix round (`f4d0538c`), then finished.
- Reviews, one agent each, read-only: the kit (7 findings), the score (9), instruments (5),
  modulation (3 + words), dynamics (4), the fix rounds (4), delivery 2 (6), the integrated
  tree (9, three overlapping), the last fix rounds. Every finding fixed with a test that
  fails before its fix (where a test could not fail before — a new API — the builder said
  so).
- Merges into `r32/integration`, in order: skin's kit, score, skin's fix round, instruments,
  dynamics, modulation (each after its own review and fix round), skin's delivery 2 (the
  plain-look test turned on: all 32 units have a face), the showcase, the last fix rounds.
- The owner's decisions during the round: keep Concentus's "Juno I/II" and "Dimension 1–4"
  (listed for the legal review with the cores' vehicle names); round 33 = Ableton's core,
  then AI.

## What each part brought

| Part | ADR | What |
|---|---|---|
| skin | 0074 | The kit draws every sheet: materials (procedural tiles), displays (phosphor, LCD, VFD, glass, paper, scope; a VU needle), knob and switch shapes with custom painters, lettering, plates, ornaments; the worn skin (`Style::current()` inside a unit's window); `display_style` for drawing inside a display; voices by the display's polarity; danger/ok floored; cue hues kept apart from the accent; the contrast floor after every tint and over every material; *Units wear their own faces* (Settings ▸ Appearance, boot path); the exported plug-ins always wear it; thumbnails keyed by a stable digest and pruned; text size in every title; the frame budget with three distinct rich faces (p95 ≤ 4 ms at 200 % and 2 px/pt); then the tone/space faces (Aequalis, Fervor, Spatium, Latitudo, Speculum, Tonus) |
| score | 0075 | `crates/cantus-score`: reading for the page (straight and tuplet grids per beat), voices, ties and rests after Gould, beams by metre, spelling by key and mode, clefs and 8va, the grand staff that follows the hands, dynamics from velocity, tempo and metre marks; engraving in Bravura (spacing by duration, systems broken over the whole piece, collision minima); MusicXML 4.0 out and in (XSD-valid; opened in MuseScore 4), PDF (fonts subset), printing (path as data, never a shell); the staff under the piano roll (`Alt+S`) and the Score pane (`Alt+Shift+S`); per-track score settings (schema 45, `SetTrackScore`); editing on the staff; keymap 13 |
| instruments | — | Unda, Calor, Modulus, Exemplar, Gravis, Lamina, Organum, Chorda (soundboard and hammers), Pulsus; Modulus's membrane key and Organum's stop tab |
| dynamics | — | Comprimo (a VU needle, one-pole 65 ms), Terminus, Multiplex, Sibilus, Porta, Culmen (LED-ring encoder), Tonsor, Impetus, Discrimen; the picker's capture canvas from the largest editor |
| modulation | — | Concentus (bucket-brigade beads), Flexus, Phasis, Tremor, Vibrans, Anulus (ring knob), Translatio, Fluctus |
| showcase | — | README (49 pictures: every effect and instrument in its face, a score section) and the landing page (a score section and a gallery of the 32 faces, EN/ES) |

## Causes found, not waits

- `freeze.rs` failed now and then while several builders tested at once: its sweep deleted
  every other process's renders in the shared temp folder. It now sweeps only folders
  untouched for an hour (test code).
- Under heavy load (four builds compiling) `pack_sounds.rs`'s 500 ms timings and
  `cantus-sandbox`'s `session::a_hang_never_blocks_the_interfaces_frames_and_is_ended` failed
  once each and passed alone every time; the final suites ran on a quiet machine.

## Results

On `f0e0a726` (0.1.0-beta.7): **Windows** `cargo test --workspace` 4 879 passed, 0 failed,
59 ignored; the engine fuzz (`CANTUS_WORKERS=3`) 895 passed; `cargo clippy --workspace
--all-targets` clean. **Linux** (WSL Ubuntu 24.04, `tools/wsl/check.sh`): clippy clean, with
and without the plug-ins' editor; 4 861 passed, 0 failed, 59 ignored. **macOS**: the runner's
`macos.yml` at publish.

## What only the owner can do

- Look at the units' faces and the score in beta.7 and say what does not please; print a
  score on a real printer; open the MusicXML in MuseScore 4 (installed on this machine with
  the owner's leave; the passages are in `F:\projects\daw-r32\musicxml` until the folder is
  cleaned).
- The legal review now also lists Concentus's "Juno I/II" and "Dimension 1–4" (kept by the
  owner's decision, 2026-10-28) beside the cores' vehicle model names, Chorda and the CLA.
- Everything rounds 30–31 left: the Mac session and the Apple account, a real Linux desktop,
  Flathub, compacting the WSL disk image, listening to round 31's features, the UMC in
  duplex, code signing, the RVR1960 permission, the trademark filing; REAPER and FL Studio
  installed for the comparisons.
