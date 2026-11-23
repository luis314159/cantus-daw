# Round 42 — `strings-rec`: the recorded strings and the note's articulation (ADR 0158)

*The `strings-rec` builder's report. Branch `r42/strings-rec` from `r42/contract`.*

## What was built

### The note's articulation, end to end

- **Delivery** (`NoteEvent::art`, `cantus-core/src/snapshot.rs`): a note-on carries its
  `Note::art` (`None` on every note-off and on a note without one). The engine's `fire`
  (`processor.rs`, one marked line, the integrator's and `tuning`'s OK) hands it to the
  instrument just before `note_on_seq`, through `TrackSlot::note_art` (native units only: a
  hosted plug-in, Firmus and a kit hear nothing). `fire` is the one place an arrangement clip,
  a session clip's projection, an offline render, a freeze or a stem turns an event into a
  note-on. The note effects carry it on the notes they move (`notefx::Msg::art`; an arpeggio's
  own notes carry none; `processor_msg.rs` got one marked `art: None` for live input). The
  art is not in the events' sort key; equality includes it (a changed art is a changed
  snapshot).
- **The clip's look-ahead** (`note_art::tie_legato`, at compile, off the audio thread): a
  note marked *legato* that starts while no earlier note sounds, at most an eighth after the
  last one ended, ties that note to itself (its note-off one tick after the legato note).
  Lindemann's expired US 7,718,885 is the idea; nothing moves for a note without art.
- **Seek and chase**: Cantus does not chase notes (a note begun before the playhead is not
  started); every note a render or a seek starts carries its art (tested).
- **The piano roll's articulation lane** (`views/piano_roll_art.rs`): from the velocity row's
  **+** ▸ *Articulation*. Each note's marks at its start, drawn by hand as a score prints them
  (tenuto, slur, dot, wedge, a plucked ring, three tremolo strokes; ⊓ down-bow, V up-bow; a
  hollow ring for "the instrument decides"). A click on a note's mark — or a double-click on
  the note in the grid (FL's note properties gesture) — opens **the note's menu**:
  articulations and bows, the current checked; one `SetNoteArt` for the note or the selection.
  Shots: `docs/round42/strings-rec/`.

**Shared files touched** (each line marked `r42:strings-rec`):

- with the integrator's OK: `processor.rs` (`fire`, three lines) and `processor_msg.rs` (one
  `art: None`);
- engine: `graph/track.rs`, `notefx/{mod,arp,bake,play,tests}.rs`, `inst/sampler.rs`;
- core and project: `snapshot.rs`, `inst/sampler.rs`, `write_fixture.rs`, `format.rs`;
- UI: `sampler_sfz.rs`, `sampler_map.rs`, `piano_roll.rs`, `piano_roll_lanes.rs`;
- content: `cantus-content/src/{manifest,install,lib,models}.rs`, `manifest/content.json` (five
  packs appended, format 6);
- tests: `{rt_memory,licences,support,pack_sounds,samples_browser}.rs`;
- docs: `docs/{DEVELOPING,BACKLOG-excellence,beta-checklist}.md`, and an amendment to ADR 0158.

### Exemplar (schema 82)

- **Articulation and bow per zone** (`z.art`, `z.bow`; `exemplar_art.rs`): a note that names
  an articulation plays its zones (or the nearest the key has: staccato ↔ spiccato, legato →
  sustain, tremolo → sustain) whatever keyswitch is in force; a note with none follows the
  keyswitch, so a key-switched instrument is played both ways. The bow turns while the
  instrument sounds and starts down after a silence; a slurred note keeps its bow; a bow the
  key lacks plays the other, never both.
- **A continuous controller crossfade** (`z.xfcc`, `z.xfinlo` … `z.xfcurve`: SFZ's
  `xfin_loccN` … `xf_cccurve`): followed while the note sounds, folded into the layer's gain
  ramp (sample by sample) — never a switch at thresholds. Equal power by default. The tone
  that follows the dynamic is the existing modulation matrix (mod wheel → cutoff).
- **The scripted legato** (`legato`, `legato.xf`, `legato.glide`, `legato.skip`;
  `exemplar_legato.rs`): a sustained note started while another sustained note is held (≥ 20 ms
  before: a chord never slurs) crossfades out of it (`x(2 − x)` curves) and glides from its
  pitch, both lengthened by the interval (a twelfth per semitone, at most ×2), starting past its
  attack; the old note plays no release samples. A note marked *legato* slurs even with the
  switch off. Two ordinary samples, nothing stored or built after the note-on.
- **Release envelopes** per articulation group (the packs' SFZ: long for sustains, short for
  short notes).
- Schema 82: a migration that writes nothing, `fixtures/v82.cantus` (a *Strings* Exemplar),
  `schema_82_keeps_exemplar_s_articulations_and_legato`; every older fixture loads.
- The SFZ importer reads `xfin_loccN`…, `xf_cccurve`, `cantus_articulation`, `cantus_bow`,
  `cantus_legato*`; the zone card's title shows a zone's articulation and bow.

### The packs (on demand, credited, never in git)

| Pack | Source | Instruments (+ presets) | Size | Measured |
|---|---|---|---|---|
| `vsco2-strings` | VSCO 2 CE, 740 raw files at commit `44030090` (CC0) | violins, violas, celli (sections), solo violin, solo bass, solo bass *senza vibrato* (+ tremolo, spiccato, pizzicato of each: 21) | 720 MB | octave fix +1 on 670 files, 0 on 74 (the solo violin's *Arco Vib* and *Pizz*, named in scientific pitch, as the research said); 271 of 272 held samples looped |
| `karoryfer-bigcat-cello` | release v1.001 zip (CC0) | Bigcat cello (+ staccato, pizzicato) | 131 MB | octave fix +1 on all 410 (named C1–C5, a cello's C2–C6); 136 of 136 sustains looped |
| `dsmolken-double-bass` | release v1.001 zip (CC0) | Smolken double bass (+ pizzicato) | 237 MB | no octave fix (288 files); 136 of 159 arco looped (the rest have too short a steady part and play to their end) |
| `karoryfer-meatbass` | release v1.001 zip (CC0) | Meatbass (+ pizzicato) | 231 MB | no fix; 170 of 170 arco looped |
| `karoryfer-sneakybass` | release v1.000 zip (CC0; its readme is the licence file) | Sneakybass (pizzicato) | 149 MB | octave fix −1 (named at written pitch; a bass sounds an octave lower) |

- **The build at install** (`stringsbuild.rs`, manifest **format 6**, validated): each zone's
  file, keys (split halfway between measured roots), root and tune, articulation, bow, round
  robin, its controller crossfade (sustains: the dynamic layers spread over CC1, equal power)
  or velocity layer (short notes), and its loop, written as plain SFZ with keyswitches
  (`sw_last`: an octave under the lowest zone, or above the highest for a bass) and the
  scripted legato on for the main instruments (90 ms crossfade, 60 ms glide, 90 ms past the
  attack). A preset is an instrument *like* another (`like`: its zones held once).
- **Measured offline** (`tools/content/strings/map.sh` → `cantus-content/examples/strings_map`):
  YIN pitch over each sample's steady part, searched 2.5× either side of its name; the octave
  fix is the majority per instrument and articulation (a file more than 50 cents off its key so
  fixed plays at its key untuned: 123 files, nearly all plucked notes and Sneakybass). The
  loop: a whole number of periods in the steady part, its seam searched within a period,
  crossfaded over a third (≤ 400 ms), **kept only if Exemplar plays it click-free** (the
  seam's energy above about 5 kHz, by the second difference, within 3 dB of the loop's own
  90th percentile); five places tried. 713 loops kept, none clicking.
- **Levels**: every instrument and preset levelled so its four-bar line plays at −16 LUFS
  (`render.sh` measures, `merge.py` sets `gain_db`, within ±24 dB).
- **The cards** say what the content is: *"A finished sketch, honestly: two dynamic layers, no
  recorded legato (Cantus scripts it by overlap), loops found by Cantus"* (and in Spanish), with
  the credits (Versilian Studios / Sam Gossner with Ivy Audio / Simon Dalzell; Karoryfer Lecolds
  and bigcat, played by Kamila Borowiak; D. Smolken; Karoryfer Samples).

## How it is tested

- **The null**: the engine suite (1 483 passed, 0 failed, 25 ignored), with every golden hash
  and the null corpus unchanged. In `strings_rec_art.rs`, a note with an articulation the
  instrument has no zones of plays every sample as a note without one, to the bit, and
  `SetNoteArt` undone gives the song back to the bit. DSP: 1 346 passed.
- **Delivery**: `strings_rec_art.rs` (4 tests). A note's art chooses Exemplar's zones from a
  clip (with the fallback); blocks of 64 and 333 give the same bits; a render started mid-clip
  is the whole render from there, every note's art included; a session clip's notes carry their
  art into the projection, its snapshot and the instrument. Core: the art rides on note-ons
  only and the events' order is unchanged; `tie_legato` ties only what it must (core and
  project: 1 172 passed).
- **Real time**: `rt_memory::articulated_notes_never_allocate` (a slurred, pizzicato line under
  a CC1 lane with three workers: no allocation).
- **Exemplar** (`exemplar_art_tests.rs`, 6 tests):
  - the keyswitch and the note's art both choose; the fallback;
  - the bow turns and starts down after a silence, and never doubles;
  - the CC crossfade's law (the squares sum to 1 at every value), the level monotone across a
    mod-wheel sweep, no step at a jump of the wheel (≤ 1/32 per sample);
  - the legato's glide (early, middle, arrived within 0.1 %), the old note gone, a chord never
    slurring, a note marked legato slurring with the switch off;
  - no release sample after a slur, and a pizzicato never slurs.
- **Schema 82**: the fixture keeps everything, v81 has none of it, and every older fixture
  loads. The undo fuzz (`SetNoteArt`'s arm, from the contract) is green in the core run.
- **End to end** (`cantus-ui/tests/strings_rec_e2e.rs`): a pack zip made of synthetic tones
  (its loops found by the analysis in those bytes) is installed **from a local file** through
  `CantusApp::assemble` (size and SHA-256 checked, its SFZ written at install) and put on the
  track as the picker puts it. A four-bar line then plays: no gap through the legato pair, G3
  within 5 cents after the glide, the held note's level rising beat by beat with the mod-wheel
  swell (×1.5), and the pizzicato decaying by its own articulation.
- **The packs against their sources** (`cantus-content/tests/strings_packs.rs`, ignored: the
  samples are not in git; `CANTUS_STRINGS_SRC=<dir> cargo test --release -p cantus-content
  --test strings_packs -- --ignored --nocapture`). Every zone is played by Exemplar at its root
  with its tuning and measured by **McLeod's NSDF** (not the YIN that set the tuning): of the 709 held zones the two estimators read as one note, **88.0 % are within a cent and 98.4 % within five**; of the 1 037 short notes, 50.0 % and 91.3 %; 238 zones gave no reading (the estimators read different notes, or none). The gate holds what was measured (held ≥ 85 % and ≥ 97 %);
  all 713 loops are click-free.
- Content: 118 passed (the manifest valid, the strings build written, its validation).

## Verified only in emulation

- **The ±1 cent bar**: a second estimator agrees within a cent for most held zones and within
  five cents for nearly all (figures above). The rest are where two estimators part on an
  ensemble's spread of players or a bow's vibrato. Zones that NSDF reads an octave or a fifth
  away from YIN (a weak fundamental, such as a bass's low C) are counted as *no reading*.
  Plucked notes drift in pitch as they ring, so their tune is YIN's mean over the first 0.6 s.
  Only an ear settles these.
- **Click-free** is a measurement (energy above about 5 kHz at the seam). A vibrato repeating
  in a 2–4 s loop is not measured: the ear decides.
- Every sound was checked by numbers and none was heard by a person. The pictures were looked
  at (the lane and the menu, Night and Paper, English and Spanish).

## Left (the owner's)

- **Listen** in `F:\projects\daw-r42\listen\strings-rec\` (48 kHz 24-bit, each at −1 dBFS
  peak, plus a `-lufs23` copy):
  - every instrument's and preset's `*-line.wav`: bar 1 the legato pair, bars 2–3 the mod-wheel
    swell on a held note, bar 4 détaché notes turning the bow;
  - **`abx-cello.wav`, `abx-bass.wav`**: the round's ABX phrase (`tools/arcus/abx_phrase.mid`)
    on the Bigcat cello and the Smolken bass, its 30 ms overlaps slurred, the bar after the
    marker plucked. These are Arcus's references.

  Say which presets are acceptable. The legato's crossfade, glide and start (90 / 60 / 90 ms)
  were set by measurement, not by ear.
- The lane's look (`docs/round42/strings-rec/`) and the checklist's *Ronda 42* item.
- A lawyer on US 7,915,514 / 8,404,958 and US 7,557,288 before release (the checklist).
