# Round 42 — the builders' briefs

Read `docs/round42/BUILDING.md` first (ownership, rules, markers), then `CLAUDE.md`, your ADR,
the research it cites and the BACKLOG item. Each **done when** is the owner's bar; what only
his ears or eyes settle is handed to him, never decided for him.

## `tuning` — the project's tuning, in every instrument (ADR 0155; BACKLOG §0.2)

**What:** the model, the formats and the engine of ADR 0155, in every pitched unit.

- **The model** (`cantus-core::tuning`, the contract's types): `Tuning::offsets()` for every
  system — `Equal` with any diapason; `Temperament { id, key }` from the tables of
  `microtuning-verified.md` §5 (Pythagorean, ¼- and ⅙-comma meantone, Werckmeister III and IV,
  Kirnberger II and III — the standard values, not the archive's variant spellings —
  Vallotti, Young (1799, the well-known one), Kellner, Neidhardt, Lehman's Bach; 5-limit just
  major and minor; 7-limit barbershop), each rotated to its key; `Scale` with or without a
  `.kbm` (the reference key and frequency, the formal octave, unmapped keys → `NAN`, non-octave
  periods such as Bohlen–Pierce). `External` reads `ExternalTuning` (the contract's seam;
  `tuning-face` implements MTS-ESP behind it).
- **The formats:** `.scl`/`.kbm` read and write (`tuning_scala.rs`): every rule and quirk in the
  verified note; MIDI Tuning Standard (`tuning_mts.rs`): bulk dump, single-note change,
  scale/octave 1- and 2-byte, read (three checksum conventions) and write (the spec's).
  **Outside bytes:** commit the MIT `tuning-library` test files (in
  `F:\projects\daw-r42\research\tuning-library-tests\`, with their notice and the must-fail
  set); `tools/test/scala-archive.sh` downloads version 94 from its fixed Wayback copy
  (`microtuning-verified.md` §2) into the data folder, checks SHA-256 `d2218f2f…b97b9a`, and an
  `#[ignore]` test (run by the tool, and by the integrator) parses all 5 401 files — each parses
  or is refused for a stated reason, round-trips to the same cents within 1e-9. The MMA's
  example table's nine correct rows as test bytes.
- **The engine:** the snapshot's resolved table per instrument track (`Arc<TuningTable>`, off
  the audio thread); `set_tuning` after configure in `apply_instrument` and in every offline
  copy (freeze, range renders, stems, the listen fork — `pitch-map.md` §6); MTS-ESP's block
  refresh through `ExternalTuning` (no allocation: `rt_memory`).
- **Every pitched instrument** at its insertion point (`pitch-map.md` §2–3): Firmus, Calor,
  Unda, Modulus, Gravis (Strings through `extra`, not `note_of`), Lamina, Organum (guarded),
  Exemplar (keytrack-scaled, glides, the mip reach), Chorda (into `Tuning.tune[i]`, part of the
  design key), Fides; Arcus through the same seam when it lands. **Pitched effects:** Tonus's
  targets, Saltus's harmony and melody, Nitor's tonic, Loquax's carrier. **Hosted:** a tuning
  expression at note-on only when the offset is not zero; MTS SysEx on a plain MIDI route when
  asked (a per-route switch, off by default).

**Done when:** per instrument, the default touches no sample (the null corpus unchanged, to the
bit) and a tuned key's measured pitch is its table's within 0.1 cent (`tunekit`), block sizes
and offline = real time with a tuning set; the whole archive parsed; the end-to-end test — a
project in Werckmeister III in E flat at A415 plays Chorda, Exemplar and the CLAP test synth
(`cantus-test-plugins/clap-synth` reading the tuning expression) in tune with each other through
`CantusApp::assemble`; `docs/round42/tuning.md`.

## `tuning-face` — the surface and MTS-ESP (ADR 0155's surface, ADR 0156)

**What:** where a professional sets the tuning, and the link to every other program.

- **Project ▸ Tuning** (a kit window, `Ctrl+Alt+T` if free): the system (Equal, a temperament
  and its key, a Scala scale with its keyboard map, External); the diapason (a field and presets
  415, 430, 432, 440, 442, 443); *follow the song's key* (rotates the temperament's key when
  the song's key changes — a command, undoable); **import / export** `.scl` + `.kbm`; a
  **circle of fifths** coloured by the beat rate of each fifth and major third (how temperaments
  are compared), a **table** of the 12 (or N) degrees in cents and as deviation from equal,
  and an **audition** that plays a cadence in the chosen key on the selected track's instrument;
  the one-line summary on the top bar's project area only when the tuning is not the default.
- **The track's override** in the inspector: *Follow the project* / *Equal temperament* /
  *Its own…* (the same window, for that track).
- **Chorda's Edit ▸ Tuning**: the project's tuning shown, its own stretch and unisons beside it
  (they stay Chorda's).
- **MTS-ESP** (`cantus-rt::mts`): the run-time loader of `LIBMTS` from its standard place on
  each system (main thread), the client read and the master publish behind the engine's
  `ExternalTuning` seam, *Be the MTS-ESP master* with the refusal when another master is
  registered, greyed with the reason when the library is absent. Tests against a fake library
  (a test `cdylib` exporting the same C symbols). A *Ronda 42* checklist item for the owner:
  Cantus with Surge XT, client and master.
- **Round 41's leftover:** the picker's pictures warmed during idle after start-up, so its
  first opening shows every tile drawn (BACKLOG §6j); measured inside the frame budget.
- A design sheet of the window to the owner first (both themes); the manual page (EN/ES).

**Done when:** every control a `Command` with its inverse; the window in both themes at 100 %
and 200 % (shoot), nothing overlapping, every label whole in both languages (the `r41_whole`
test's pattern); the end-to-end test — import a Scala file from outside bytes through the
window's action in `CantusApp::assemble`, hear Chorda play it (measured pitch), export it and
read the same cents back; the fake-library tests; `docs/round42/tuning-face.md`.

## `arcus` — the bowed string (ADR 0157; BACKLOG §3e)

**What:** Arcus, a modelled bowed string instrument on Fides's bow, string and body — the cello
and the double bass first, violin and viola only if they pass the same gate.

- Read `research/instruments-next/strings-verified.md` §3 and §6 (the measurements and the
  gate) and `docs/round41/fides.md` (what Fides's bow measured, why its tracker failed).
- Reuse Fides's DSP by its public functions; to expose something private ask the integrator
  (Fides's golden hashes must not move, except in the bowed-pitch commit below).
- The string tables (tension, mass, stiffness, losses per string, cited), the bodies (the
  cello's from Fides, the bass scaled and said so plainly; the violin fit to the CNSM admittance,
  CC BY 4.0, credited), friction (the hyperbolic curve by default; the thermal model of
  Galluzzo 2017 as an option, measured before choosing), bow-hair noise as its own measured layer.
- **Techniques:** arco sustain, legato at 0 ms (the string rings on, the finger moves),
  portamento on request, détaché with re-bow and the bow's direction (alternating by itself, or
  the note's `art.bow`, ADR 0158), pizzicato, tremolo, delayed vibrato whose partials' movement
  comes from the body. **Playing:** velocity → force and acceleration inside the Guettler
  region; CC1/CC11/aftertouch/breath/MPE pressure → the dynamic, continuously; CC2/CC21 → vibrato;
  MPE slide → contact point; never silent with a keyboard alone.
- **Pitch:** the correction table measured offline per pitch × force × β, applied to the loop
  length; and the same correction to **Fides's bow** as its next model version (round 41's
  leftover), in one commit with its goldens and the measured change.
- The kit shell (Play / Edit / Matrix), its face from a design sheet the owner approves, 24
  presets by ear (cello and bass solo: legato lines, pizzicato, tremolo, a sul tasto-like
  softness by contact point…), its manual page, its card, `--editor arcus`, its tuning through
  ADR 0155's seam.

**Done when:** the gate of ADR 0157 (Schelleng, attack, pitch ≤ 1 cent bowed and 0.1 plucked,
vibrato, body, CPU, the house invariants) measured and written in `docs/round42/arcus.md`;
renders of the cello and the bass (a solo note at three dynamics, a four-bar legato phrase with
a CC1 swell, pizzicato) for the owner's blind ABX against the Karoryfer cello and the Smolken
bass; then **`BowedString` in `InstrumentKind::ALL` and the picker** in one commit, with the null
corpus's new item and an end-to-end test that picks Arcus through `CantusApp::assemble`, plays a
legato pair and hears both notes in tune.

## `strings-rec` — the recorded strings and the note's articulation (ADR 0158; wave 2)

**What:** strings to play today, from CC0 recordings, and the articulation every note can carry.

- **The note's articulation:** the engine hands `Note::art` to the instrument before its note
  (`note_art`, the contract's hook) from clips, the session view and a render alike; the piano
  roll's articulation lane (marks per note, the note's menu, `SetNoteArt`), both themes.
- **Exemplar gains:** articulation by key-switch and by `art`; a **continuous** CC1 crossfade
  across dynamic layers with a tone filter that follows it (never stepped at thresholds);
  release envelopes; a scripted legato by overlap with the interval's crossfade and glide, and
  look-ahead from the clip's notes when known (no transition built after a note-on).
- **The packs** (ADR 0030, on demand, credited): VSCO 2 CE's strings (fix the files' octave),
  the Karoryfer Bigcat cello, the Smolken double bass (Meatbass, Sneakybass as extras); loops
  found offline by a tool in `tools/content/strings/` and stored as metadata; the card says
  honestly "a finished sketch", the content's bar.
- Presets by ear (sections and solos: sustain, spiccato, pizzicato, tremolo, legato lines) and
  renders for the owner; these are Arcus's ABX references too.

**Done when:** a note without art changes no sample; zones in tune ±1 cent after the octave fix;
loops click-free by an offline check; the CC1 crossfade monotone in level; the end-to-end test
installs a pack from a local file through `CantusApp::assemble`, writes a four-bar line with a
legato pair and a CC1 swell, and hears it; `docs/round42/strings-rec.md`.
